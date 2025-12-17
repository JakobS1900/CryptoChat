use super::{OverlayConfig, OverlayError, OverlayResult};
use async_trait::async_trait;
use bincode;
use cryptochat_messaging::EncryptedEnvelope;
use futures::prelude::*;
use libp2p::core::{muxing::StreamMuxerBox, transport::Transport as CoreTransport};
use libp2p::{
    identify, identity,
    kad::{
        store::MemoryStore, Behaviour as KademliaBehaviour, Config as KademliaConfig,
        Event as KademliaEvent,
    },
    ping, quic,
    request_response::{
        self, Behaviour as RequestResponse, Config as RequestResponseConfig,
        Event as RequestResponseEvent, ProtocolSupport,
    },
    swarm::{Config as SwarmConfig, NetworkBehaviour, Swarm},
    Multiaddr, PeerId, StreamProtocol,
};
use serde::{Deserialize, Serialize};
use std::{io, sync::Arc, time::Duration};
use tokio::sync::{mpsc, oneshot};

const IDENTIFY_PROTOCOL: &str = "/cryptochat/overlay/1.0.0";
const AGENT_VERSION: &str = concat!("cryptochat-node/", env!("CARGO_PKG_VERSION"));
const KAD_PROTOCOL: &str = "/cryptochat/kad/1.0.0";
const ENVELOPE_PROTOCOL: &str = "/cryptochat/envelope/1.0.0";

/// Commands sent to the overlay runtime.
#[derive(Debug)]
pub enum OverlayCommand {
    Dial(Multiaddr),
    Publish {
        envelope: EncryptedEnvelope,
        responder: oneshot::Sender<OverlayResult<()>>,
    },
    Shutdown(oneshot::Sender<()>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeRequest {
    pub envelope: EncryptedEnvelope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeResponse {
    pub accepted: bool,
}

#[derive(Clone, Default)]
pub(crate) struct EnvelopeCodec;

#[async_trait]
impl request_response::Codec for EnvelopeCodec {
    type Protocol = StreamProtocol;
    type Request = EnvelopeRequest;
    type Response = EnvelopeResponse;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        io.read_to_end(&mut buf).await?;
        bincode::deserialize(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        io.read_to_end(&mut buf).await?;
        bincode::deserialize(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        request: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let bytes = bincode::serialize(&request)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        io.write_all(&bytes).await?;
        io.flush().await?;
        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        response: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let bytes = bincode::serialize(&response)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        io.write_all(&bytes).await?;
        io.flush().await?;
        Ok(())
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "NodeEvent", event_process = false)]
pub(crate) struct NodeBehaviour {
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub kademlia: KademliaBehaviour<MemoryStore>,
    pub(crate) request_response: RequestResponse<EnvelopeCodec>,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant, dead_code)]
pub enum NodeEvent {
    Identify(identify::Event),
    Ping(ping::Event),
    Kademlia(KademliaEvent),
    RequestResponse(RequestResponseEvent<EnvelopeRequest, EnvelopeResponse>),
}

impl From<identify::Event> for NodeEvent {
    fn from(event: identify::Event) -> Self {
        Self::Identify(event)
    }
}

impl From<ping::Event> for NodeEvent {
    fn from(event: ping::Event) -> Self {
        Self::Ping(event)
    }
}

impl From<KademliaEvent> for NodeEvent {
    fn from(event: KademliaEvent) -> Self {
        Self::Kademlia(event)
    }
}

impl From<RequestResponseEvent<EnvelopeRequest, EnvelopeResponse>> for NodeEvent {
    fn from(event: RequestResponseEvent<EnvelopeRequest, EnvelopeResponse>) -> Self {
        Self::RequestResponse(event)
    }
}

struct TransportState {
    peer_id: PeerId,
    command_tx: mpsc::Sender<OverlayCommand>,
}

/// Handle to the underlying libp2p swarm and transport.
#[derive(Clone)]
pub struct TransportHandle {
    inner: Arc<TransportState>,
}

#[derive(Clone)]
pub struct OverlayNetwork;

pub(crate) struct RuntimeComponents {
    pub(crate) swarm: Swarm<NodeBehaviour>,
    pub(crate) command_rx: mpsc::Receiver<OverlayCommand>,
    pub(crate) replication_factor: usize,
}

impl OverlayNetwork {
    pub(crate) async fn initialize(
        config: &OverlayConfig,
    ) -> OverlayResult<(TransportHandle, RuntimeComponents)> {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        let identify_cfg = identify::Config::new(IDENTIFY_PROTOCOL.into(), local_key.public())
            .with_agent_version(AGENT_VERSION.into());
        let ping_cfg = ping::Config::new();

        let kad_protocol = StreamProtocol::new(KAD_PROTOCOL);
        let kad_cfg = KademliaConfig::new(kad_protocol);
        let store = MemoryStore::new(local_peer_id);
        let kademlia = KademliaBehaviour::with_config(local_peer_id, store, kad_cfg);

        let rr_config =
            RequestResponseConfig::default().with_request_timeout(Duration::from_secs(20));
        let request_response = RequestResponse::with_codec(
            EnvelopeCodec::default(),
            std::iter::once((
                StreamProtocol::new(ENVELOPE_PROTOCOL),
                ProtocolSupport::Full,
            )),
            rr_config,
        );

        let behaviour = NodeBehaviour {
            identify: identify::Behaviour::new(identify_cfg),
            ping: ping::Behaviour::new(ping_cfg),
            kademlia,
            request_response,
        };

        let transport = quic::tokio::Transport::new(quic::Config::new(&local_key))
            .map(|(peer, connection), _| (peer, StreamMuxerBox::new(connection)))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            .boxed();
        let swarm_config = SwarmConfig::with_tokio_executor();
        let mut swarm = Swarm::new(transport, behaviour, local_peer_id, swarm_config);

        // Listen on QUIC sockets.
        let listen_addrs = ["/ip4/0.0.0.0/udp/0/quic-v1", "/ip6/::/udp/0/quic-v1"];
        for addr in listen_addrs {
            let multiaddr: Multiaddr = addr.parse().map_err(|e| {
                OverlayError::Transport(format!("invalid listen address {addr}: {e}"))
            })?;
            Swarm::listen_on(&mut swarm, multiaddr)
                .map_err(|e| OverlayError::Transport(format!("failed to listen: {e}")))?;
        }

        let (command_tx, command_rx) = mpsc::channel(64);

        let handle = TransportHandle {
            inner: Arc::new(TransportState {
                peer_id: local_peer_id,
                command_tx,
            }),
        };

        Ok((
            handle,
            RuntimeComponents {
                swarm,
                command_rx,
                replication_factor: config.replication_factor.max(1),
            },
        ))
    }
}

impl TransportHandle {
    pub fn peer_id(&self) -> PeerId {
        self.inner.peer_id
    }

    pub async fn dial(&self, addr: Multiaddr) -> OverlayResult<()> {
        self.inner
            .command_tx
            .send(OverlayCommand::Dial(addr))
            .await
            .map_err(|e| OverlayError::Transport(format!("failed to send dial command: {e}")))
    }

    pub async fn publish(
        &self,
        envelope: EncryptedEnvelope,
        responder: oneshot::Sender<OverlayResult<()>>,
    ) -> OverlayResult<()> {
        self.inner
            .command_tx
            .send(OverlayCommand::Publish {
                envelope,
                responder,
            })
            .await
            .map_err(|e| OverlayError::Replication(format!("failed to send publish command: {e}")))
    }

    pub async fn request_shutdown(&self) -> OverlayResult<oneshot::Receiver<()>> {
        let (tx, rx) = oneshot::channel();
        self.inner
            .command_tx
            .send(OverlayCommand::Shutdown(tx))
            .await
            .map_err(|e| OverlayError::Transport(format!("failed to send shutdown: {e}")))?;
        Ok(rx)
    }
}
