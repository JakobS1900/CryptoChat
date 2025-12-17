use std::path::PathBuf;
use std::time::Duration;

/// Configuration for the overlay network.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OverlayConfig {
    /// Multiaddresses for bootstrap peers.
    pub bootstrap_peers: Vec<String>,
    /// Number of replicas to maintain for offline delivery.
    pub replication_factor: usize,
    /// Time-to-live for cached envelopes.
    pub envelope_ttl: Duration,
    /// Maximum simultaneous libp2p connections.
    pub max_connections: usize,
    /// Filesystem path for persisted overlay data.
    pub storage_path: PathBuf,
    /// How often to retry pending envelopes.
    pub retry_interval: Duration,
}

impl OverlayConfig {
    pub fn with_storage_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.storage_path = path.into();
        self
    }

    pub fn with_retry_interval(mut self, interval: Duration) -> Self {
        self.retry_interval = interval;
        self
    }
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            bootstrap_peers: Vec::new(),
            replication_factor: 3,
            envelope_ttl: Duration::from_secs(60 * 60 * 24),
            max_connections: 128,
            storage_path: PathBuf::from("data/node"),
            retry_interval: Duration::from_secs(30),
        }
    }
}
