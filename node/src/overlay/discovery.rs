use super::{OverlayConfig, OverlayError, OverlayResult, TransportHandle};
use libp2p::kad::{store::MemoryStore, Event as KademliaEvent, Mode, QueryId};
use libp2p::{kad, Multiaddr, PeerId};
use std::collections::HashSet;
use std::{str::FromStr, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    PeerAdded(PeerId),
    PeerRemoved(PeerId),
}

#[derive(Clone)]
pub struct DiscoveryService {
    config: OverlayConfig,
    transport: TransportHandle,
    peers: Arc<Mutex<HashSet<PeerId>>>,
    event_tx: mpsc::Sender<DiscoveryEvent>,
}

impl DiscoveryService {
    pub fn new(config: OverlayConfig, transport: TransportHandle) -> Self {
        let (event_tx, _event_rx) = mpsc::channel(64);
        Self {
            config,
            transport,
            peers: Arc::new(Mutex::new(HashSet::new())),
            event_tx,
        }
    }

    pub fn event_sender(&self) -> mpsc::Sender<DiscoveryEvent> {
        self.event_tx.clone()
    }

    pub async fn bootstrap(&self) -> OverlayResult<()> {
        let bootstrap_addrs = parse_bootstrap(&self.config)?;
        for (peer, addr) in bootstrap_addrs {
            info!(%peer, %addr, "adding bootstrap peer");
            self.transport.dial(addr.clone()).await?;
            self.insert_peer(peer).await;
        }
        Ok(())
    }

    pub async fn handle_kad_event(&self, event: &KademliaEvent) -> OverlayResult<()> {
        match event {
            KademliaEvent::RoutingUpdated { peer, .. } => {
                self.insert_peer(*peer).await;
            }
            KademliaEvent::UnroutablePeer { peer } => {
                self.remove_peer(*peer).await;
            }
            KademliaEvent::ModeChanged { new_mode, .. } => {
                debug!(?new_mode, "kademlia mode changed");
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn peers(&self) -> Vec<PeerId> {
        self.peers.lock().await.iter().cloned().collect()
    }

    pub fn start_bootstrap_queries(
        &self,
        behaviour: &mut kad::Behaviour<MemoryStore>,
        local_peer_id: PeerId,
    ) -> QueryId {
        let query_id = behaviour.get_closest_peers(local_peer_id);
        behaviour.set_mode(Some(Mode::Client));
        query_id
    }

    async fn insert_peer(&self, peer: PeerId) {
        let mut peers = self.peers.lock().await;
        if peers.insert(peer) {
            let _ = self.event_tx.send(DiscoveryEvent::PeerAdded(peer)).await;
        }
    }

    async fn remove_peer(&self, peer: PeerId) {
        let mut peers = self.peers.lock().await;
        if peers.remove(&peer) {
            let _ = self.event_tx.send(DiscoveryEvent::PeerRemoved(peer)).await;
        }
    }
}

fn parse_bootstrap(config: &OverlayConfig) -> OverlayResult<Vec<(PeerId, Multiaddr)>> {
    let mut peers = Vec::new();
    for addr in &config.bootstrap_peers {
        let mut parts = addr.split_whitespace();
        let addr_part = parts
            .next()
            .ok_or_else(|| OverlayError::InvalidAddress(format!("missing multiaddr in {addr}")))?;
        let peer_part = parts
            .next()
            .ok_or_else(|| OverlayError::InvalidAddress(format!("missing peer id in {addr}")))?;

        let multiaddr = Multiaddr::from_str(addr_part)
            .map_err(|e| OverlayError::InvalidAddress(format!("{addr_part}: {e}")))?;
        let peer = PeerId::from_str(peer_part)
            .map_err(|e| OverlayError::InvalidAddress(format!("{peer_part}: {e}")))?;
        peers.push((peer, multiaddr));
    }
    Ok(peers)
}
