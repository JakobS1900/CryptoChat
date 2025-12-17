//! Message pipeline integrating PGP encryption with overlay networking.

mod pipeline;
mod queue;

pub use pipeline::{MessagePipeline, PipelineConfig, SendMessageRequest, SendMessageResponse};
pub use queue::{MessageQueue, QueuedMessage};

use cryptochat_messaging::pgp_envelope::PgpEnvelope;
use cryptochat_messaging::{ConversationId, DeviceId, PlaintextMessage};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Errors that can occur in the messaging pipeline.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("overlay error: {0}")]
    Overlay(String),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("invalid envelope: {0}")]
    InvalidEnvelope(String),
}

pub type Result<T> = std::result::Result<T, PipelineError>;

/// Envelope format used for overlay transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportEnvelope {
    pub message_id: Uuid,
    pub conversation_id: ConversationId,
    pub sender_device: DeviceId,
    pub recipient_device: DeviceId,
    pub created_ms: i64,
    pub pgp_envelope: PgpEnvelope,
}

impl TransportEnvelope {
    pub fn new(
        recipient_device: DeviceId,
        pgp_envelope: PgpEnvelope,
    ) -> Self {
        Self {
            message_id: pgp_envelope.message_id,
            conversation_id: pgp_envelope.conversation_id.clone(),
            sender_device: pgp_envelope.sender_device.clone(),
            recipient_device,
            created_ms: pgp_envelope.created_ms,
            pgp_envelope,
        }
    }
}

/// Receipt acknowledging message delivery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReceipt {
    pub message_id: Uuid,
    pub delivered_to: DeviceId,
    pub delivered_at_ms: i64,
    pub status: ReceiptStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiptStatus {
    Queued,
    Sent,
    Delivered,
    Failed,
}

impl MessageReceipt {
    pub fn new(message_id: Uuid, delivered_to: DeviceId, status: ReceiptStatus) -> Self {
        let delivered_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Self {
            message_id,
            delivered_to,
            delivered_at_ms,
            status,
        }
    }
}
