use super::transport::{
    EnvelopeRequest, EnvelopeResponse, NodeBehaviour, NodeEvent, OverlayCommand,
};
use super::{DiscoveryService, OverlayError, OverlayResult, ReplicationService};
use crate::storage::{NodeStorage, PendingEnvelope};
use futures::StreamExt;
use libp2p::kad::QueryId;
use libp2p::request_response::{
    Event as RequestResponseEvent, Message as RequestResponseMessage, OutboundRequestId,
};
use libp2p::swarm::{Swarm, SwarmEvent};
use libp2p::PeerId;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{interval, MissedTickBehavior};
use tracing::{debug, warn};

pub struct OverlayRuntime {
    swarm: Swarm<NodeBehaviour>,
    command_rx: mpsc::Receiver<OverlayCommand>,
    discovery: DiscoveryService,
    replication: ReplicationService,
    storage: NodeStorage,
    replication_factor: usize,
    retry_interval: Duration,
    pending_replications: HashMap<OutboundRequestId, (String, PeerId)>,
    bootstrap_query: Option<QueryId>,
}

impl OverlayRuntime {
    pub fn new(
        swarm: Swarm<NodeBehaviour>,
        command_rx: mpsc::Receiver<OverlayCommand>,
        replication_factor: usize,
        retry_interval: Duration,
        discovery: DiscoveryService,
        replication: ReplicationService,
        storage: NodeStorage,
    ) -> Self {
        Self {
            swarm,
            command_rx,
            discovery,
            replication,
            storage,
            replication_factor,
            retry_interval,
            pending_replications: HashMap::new(),
            bootstrap_query: None,
        }
    }

    pub async fn run(mut self) {
        if self.bootstrap_query.is_none() {
            let local_peer = *self.swarm.local_peer_id();
            let behaviour = self.swarm.behaviour_mut();
            let query_id = self
                .discovery
                .start_bootstrap_queries(&mut behaviour.kademlia, local_peer);
            self.bootstrap_query = Some(query_id);
        }

        if let Err(err) = self.replay_pending().await {
            warn!(?err, "failed to replay pending envelopes");
        }

        let mut retry_timer = interval(self.retry_interval);
        retry_timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
        // Skip the immediate first tick since we just replayed pending items.
        retry_timer.tick().await;

        loop {
            tokio::select! {
                cmd = self.command_rx.recv() => {
                    match cmd {
                        Some(OverlayCommand::Dial(addr)) => {
                            if let Err(err) = self.dial_addr(addr.clone()) {
                                warn!(%err, %addr, "dial failure");
                            }
                        }
                        Some(OverlayCommand::Publish { envelope, responder }) => {
                            let message_id = envelope.message_id.to_string();
                            let peers = self.discovery.peers().await;
                            if peers.is_empty() {
                                let _ = responder
                                    .send(Err(OverlayError::Replication("no peers available".into())));
                                self.replication
                                    .notify_failure(&message_id, "no peers available".into())
                                    .await;
                                continue;
                            }

                            let max_targets = self.replication_factor.max(1);
                            let target_peers: Vec<PeerId> = peers.into_iter().take(max_targets).collect();

                            if let Err(err) = self
                                .storage
                                .insert_outbound(&message_id, &envelope, &target_peers)
                            {
                                let reason = format!("failed to persist envelope: {err}");
                                let _ = responder
                                    .send(Err(OverlayError::Replication(reason.clone())));
                                self.replication
                                    .notify_failure(&message_id, reason)
                                    .await;
                                continue;
                            }

                            let mut sent_any = false;
                            for peer in target_peers.iter() {
                                if self.in_flight(&message_id, peer) {
                                    continue;
                                }

                                let request_id = self
                                    .swarm
                                    .behaviour_mut()
                                    .request_response
                                    .send_request(
                                        peer,
                                        EnvelopeRequest { envelope: envelope.clone() },
                                    );
                                self.pending_replications
                                    .insert(request_id, (message_id.clone(), peer.clone()));
                                sent_any = true;
                            }

                            if sent_any {
                                self.replication.notify_enqueued(&message_id).await;
                            }
                            let _ = responder.send(Ok(()));
                        }
                        Some(OverlayCommand::Shutdown(done_tx)) => {
                            let _ = done_tx.send(());
                            break;
                        }
                        None => break,
                    }
                }
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await;
                }
                _ = retry_timer.tick() => {
                    if let Err(err) = self.retry_pending().await {
                        warn!(?err, "failed to retry pending envelopes");
                    }
                }
            }
        }
    }

    fn dial_addr(&mut self, addr: libp2p::Multiaddr) -> OverlayResult<()> {
        let dial_opts = libp2p::swarm::dial_opts::DialOpts::unknown_peer_id()
            .address(addr.clone())
            .build();
        self.swarm
            .dial(dial_opts)
            .map_err(|e| OverlayError::Transport(format!("dial error: {e}")))?;
        Ok(())
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<NodeEvent>) {
        match event {
            SwarmEvent::Behaviour(NodeEvent::Kademlia(ev)) => {
                if let Err(err) = self.discovery.handle_kad_event(&ev).await {
                    warn!(%err, "discovery event error");
                }
            }
            SwarmEvent::Behaviour(NodeEvent::RequestResponse(event)) => {
                self.handle_request_response(event).await;
            }
            SwarmEvent::Behaviour(other) => {
                debug!(?other, "overlay behaviour event");
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                debug!(%address, "listening address announced");
            }
            _ => {}
        }
    }

    async fn replay_pending(&mut self) -> OverlayResult<()> {
        let records = self.storage.load_pending().map_err(|e| {
            OverlayError::Replication(format!("failed to load pending envelopes: {e}"))
        })?;

        self.resend_pending(records).await
    }

    async fn retry_pending(&mut self) -> OverlayResult<()> {
        let records = self.storage.load_pending().map_err(|e| {
            OverlayError::Replication(format!("failed to load pending envelopes: {e}"))
        })?;
        self.resend_pending(records).await
    }

    async fn resend_pending(&mut self, records: Vec<PendingEnvelope>) -> OverlayResult<()> {
        for record in records {
            if record.pending_peers.is_empty() {
                continue;
            }

            for peer in record.pending_peers.iter() {
                if self.in_flight(&record.message_id, peer) {
                    continue;
                }

                self.replication
                    .notify_retry(&record.message_id, peer)
                    .await;

                let request_id = self.swarm.behaviour_mut().request_response.send_request(
                    peer,
                    EnvelopeRequest {
                        envelope: record.envelope.clone(),
                    },
                );

                self.pending_replications
                    .insert(request_id, (record.message_id.clone(), peer.clone()));
            }
        }

        Ok(())
    }

    fn in_flight(&self, message_id: &str, peer: &PeerId) -> bool {
        self.pending_replications
            .values()
            .any(|(pending_id, pending_peer)| pending_id == message_id && pending_peer == peer)
    }

    async fn handle_request_response(
        &mut self,
        event: RequestResponseEvent<EnvelopeRequest, EnvelopeResponse>,
    ) {
        match event {
            RequestResponseEvent::Message { peer, message } => match message {
                RequestResponseMessage::Request {
                    request, channel, ..
                } => match self.storage.store_inbound(&request.envelope) {
                    Ok(_) => {
                        if let Err(err) = self
                            .swarm
                            .behaviour_mut()
                            .request_response
                            .send_response(channel, EnvelopeResponse { accepted: true })
                        {
                            warn!(?err, %peer, "failed to send replication response");
                        }
                    }
                    Err(err) => {
                        warn!(?err, %peer, "failed to persist inbound envelope");
                        let _ = self
                            .swarm
                            .behaviour_mut()
                            .request_response
                            .send_response(channel, EnvelopeResponse { accepted: false });
                    }
                },
                RequestResponseMessage::Response {
                    request_id,
                    response,
                } => {
                    if let Some((message_id, expected_peer)) =
                        self.pending_replications.remove(&request_id)
                    {
                        if response.accepted {
                            match self.storage.mark_peer_success(&message_id, &expected_peer) {
                                Ok(_) => {
                                    self.replication
                                        .notify_ack(&message_id, &expected_peer)
                                        .await;
                                }
                                Err(err) => {
                                    warn!(?err, %expected_peer, "failed to update storage after ack");
                                }
                            }
                        } else {
                            let reason = "replication rejected".to_string();
                            self.replication.notify_failure(&message_id, reason).await;
                        }
                    } else {
                        debug!(%peer, ?request_id, "replication response for unknown request");
                    }
                }
            },
            RequestResponseEvent::OutboundFailure {
                peer,
                error,
                request_id,
            } => {
                if let Some((message_id, _expected_peer)) =
                    self.pending_replications.remove(&request_id)
                {
                    let reason = format!("outbound failure: {error:?}");
                    self.replication.notify_failure(&message_id, reason).await;
                } else {
                    debug!(%peer, ?request_id, ?error, "outbound failure for unknown request");
                }
            }
            RequestResponseEvent::InboundFailure {
                peer,
                error,
                request_id,
            } => {
                debug!(%peer, ?request_id, ?error, "replication inbound failure");
            }
            RequestResponseEvent::ResponseSent { peer, .. } => {
                debug!(%peer, "replication response sent");
            }
        }
    }
}
