use super::{OverlayConfig, OverlayError, OverlayResult, TransportHandle};
use cryptochat_messaging::EncryptedEnvelope;
use libp2p::PeerId;
use std::sync::Arc;
use tokio::sync::{broadcast, oneshot};
use tracing::debug;

#[derive(Debug, Clone)]
pub enum ReplicationEvent {
    PublishQueued { message_id: String },
    PublishAck { message_id: String, peer: PeerId },
    PublishFailed { message_id: String, reason: String },
    PublishRetry { message_id: String, peer: PeerId },
}

struct ReplicationInner {
    event_tx: broadcast::Sender<ReplicationEvent>,
    transport: TransportHandle,
    _config: OverlayConfig,
}

#[derive(Clone)]
pub struct ReplicationService {
    inner: Arc<ReplicationInner>,
}

impl ReplicationService {
    pub fn new(config: OverlayConfig, transport: TransportHandle) -> Self {
        let (event_tx, _rx) = broadcast::channel(128);
        Self {
            inner: Arc::new(ReplicationInner {
                event_tx,
                transport,
                _config: config,
            }),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ReplicationEvent> {
        self.inner.event_tx.subscribe()
    }

    pub async fn publish(&self, envelope: EncryptedEnvelope) -> OverlayResult<()> {
        let message_id = envelope.message_id.to_string();
        let (tx, rx) = oneshot::channel();
        self.inner
            .transport
            .publish(envelope, tx)
            .await
            .map_err(|e| OverlayError::Replication(format!("send command failed: {e}")))?;

        match rx.await {
            Ok(result) => {
                debug!(?result, message_id, "publish request sent");
                result
            }
            Err(e) => {
                self.notify_failure(&message_id, format!("publish channel error: {e}"))
                    .await;
                Err(OverlayError::Replication(format!(
                    "publish channel error: {e}"
                )))
            }
        }
    }

    pub async fn notify_enqueued(&self, message_id: &str) {
        debug!(message_id, "replication queued");
        let _ = self.inner.event_tx.send(ReplicationEvent::PublishQueued {
            message_id: message_id.to_string(),
        });
    }

    pub async fn notify_ack(&self, message_id: &str, peer: &PeerId) {
        debug!(message_id, ?peer, "replication ack");
        let _ = self.inner.event_tx.send(ReplicationEvent::PublishAck {
            message_id: message_id.to_string(),
            peer: *peer,
        });
    }

    pub async fn notify_failure(&self, message_id: &str, reason: String) {
        debug!(message_id, reason = reason.as_str(), "replication failure");
        let _ = self.inner.event_tx.send(ReplicationEvent::PublishFailed {
            message_id: message_id.to_string(),
            reason,
        });
    }

    pub async fn notify_retry(&self, message_id: &str, peer: &PeerId) {
        debug!(message_id, ?peer, "replication retry");
        let _ = self.inner.event_tx.send(ReplicationEvent::PublishRetry {
            message_id: message_id.to_string(),
            peer: *peer,
        });
    }
}
