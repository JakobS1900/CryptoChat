//! Overlay coordination layer backed by libp2p.

mod config;
mod discovery;
mod replication;
mod runtime;
mod subscriptions;
mod transport;

pub use config::OverlayConfig;
pub use discovery::{DiscoveryEvent, DiscoveryService};
pub use replication::{ReplicationEvent, ReplicationService};
pub use subscriptions::{OverlayNotification, SubscriptionManager};
pub use transport::{OverlayNetwork, TransportHandle};

use crate::storage::NodeStorage;
use runtime::OverlayRuntime;
use tokio::sync::broadcast;
pub type OverlayResult<T> = Result<T, OverlayError>;

#[derive(Debug, thiserror::Error)]
pub enum OverlayError {
    #[error("transport layer not initialized")]
    TransportNotInitialized,
    #[error("transport error: {0}")]
    Transport(String),
    #[error("invalid multiaddr: {0}")]
    InvalidAddress(String),
    #[error("discovery layer failed: {0}")]
    Discovery(String),
    #[error("replication layer failed: {0}")]
    Replication(String),
    #[error("subscription error: {0}")]
    Subscription(String),
    #[error("not implemented")]
    NotImplemented,
}

pub struct OverlayHandle {
    transport: TransportHandle,
    discovery: DiscoveryService,
    replication: ReplicationService,
    subscriptions: SubscriptionManager,
    _storage: NodeStorage,
    runtime_task: tokio::task::JoinHandle<()>,
}

impl OverlayHandle {
    pub async fn start(config: OverlayConfig) -> OverlayResult<Self> {
        let storage = NodeStorage::open(&config.storage_path)
            .map_err(|e| OverlayError::Replication(format!("failed to initialize storage: {e}")))?;

        let (transport, runtime_components) = OverlayNetwork::initialize(&config).await?;
        let discovery = DiscoveryService::new(config.clone(), transport.clone());
        let replication = ReplicationService::new(config.clone(), transport.clone());
        let subscriptions = SubscriptionManager::new();

        let runtime = OverlayRuntime::new(
            runtime_components.swarm,
            runtime_components.command_rx,
            runtime_components.replication_factor,
            config.retry_interval,
            discovery.clone(),
            replication.clone(),
            storage.clone(),
        );
        let runtime_handle = tokio::spawn(async move { runtime.run().await });

        discovery.bootstrap().await?;

        Ok(Self {
            transport,
            discovery,
            replication,
            subscriptions,
            _storage: storage,
            runtime_task: runtime_handle,
        })
    }

    pub async fn shutdown(self) -> OverlayResult<()> {
        let OverlayHandle {
            transport,
            discovery: _,
            replication,
            subscriptions: _,
            _storage: _,
            runtime_task,
        } = self;

        let shutdown_rx = transport.request_shutdown().await?;
        shutdown_rx
            .await
            .map_err(|e| OverlayError::Transport(format!("shutdown channel error: {e}")))?;

        runtime_task
            .await
            .map_err(|e| OverlayError::Transport(format!("runtime join error: {e}")))?;

        drop(replication);
        Ok(())
    }
    pub fn discovery(&self) -> &DiscoveryService {
        &self.discovery
    }

    pub fn subscribe_replication(&self) -> broadcast::Receiver<ReplicationEvent> {
        self.replication.subscribe()
    }

    pub fn replication(&self) -> &ReplicationService {
        &self.replication
    }

    pub fn subscriptions(&self) -> &SubscriptionManager {
        &self.subscriptions
    }

    pub fn transport(&self) -> &TransportHandle {
        &self.transport
    }
}
