//! Message queue for pending outbound messages.

use super::{MessageReceipt, ReceiptStatus, TransportEnvelope};
use cryptochat_messaging::DeviceId;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// In-memory message queue with persistence hooks.
#[derive(Clone)]
pub struct MessageQueue {
    queue: Arc<RwLock<VecDeque<QueuedMessage>>>,
}

#[derive(Debug, Clone)]
pub struct QueuedMessage {
    pub envelope: TransportEnvelope,
    pub attempts: usize,
    pub queued_at_ms: i64,
}

impl MessageQueue {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Add a message to the queue.
    pub async fn enqueue(&self, envelope: TransportEnvelope) {
        let queued_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let queued = QueuedMessage {
            envelope,
            attempts: 0,
            queued_at_ms,
        };

        self.queue.write().await.push_back(queued);
    }

    /// Get the next message from the queue without removing it.
    pub async fn peek(&self) -> Option<QueuedMessage> {
        self.queue.read().await.front().cloned()
    }

    /// Remove a specific message from the queue by message_id.
    pub async fn remove(&self, message_id: &Uuid) -> Option<QueuedMessage> {
        let mut queue = self.queue.write().await;
        if let Some(pos) = queue
            .iter()
            .position(|msg| msg.envelope.message_id == *message_id)
        {
            queue.remove(pos)
        } else {
            None
        }
    }

    /// Mark a message as attempted and re-queue if needed.
    pub async fn mark_attempted(&self, message_id: &Uuid) {
        let mut queue = self.queue.write().await;
        if let Some(msg) = queue
            .iter_mut()
            .find(|msg| msg.envelope.message_id == *message_id)
        {
            msg.attempts += 1;
        }
    }

    /// Get current queue length.
    pub async fn len(&self) -> usize {
        self.queue.read().await.len()
    }

    /// Check if queue is empty.
    pub async fn is_empty(&self) -> bool {
        self.queue.read().await.is_empty()
    }

    /// Clear all messages from the queue.
    pub async fn clear(&self) {
        self.queue.write().await.clear();
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cryptochat_messaging::{ConversationId, PlaintextMessage};
    use cryptochat_messaging::pgp_envelope::PgpEnvelope;
    use cryptochat_crypto_core::pgp::PgpKeyPair;

    #[tokio::test]
    async fn test_queue_enqueue_and_peek() {
        let queue = MessageQueue::new();

        let alice = PgpKeyPair::generate("alice@example.com").unwrap();
        let bob = PgpKeyPair::generate("bob@example.com").unwrap();

        let message = PlaintextMessage::new(
            ConversationId::new(),
            DeviceId::new(),
            b"test".to_vec(),
        );

        let pgp_envelope = PgpEnvelope::from_plaintext(message, &alice, bob.cert()).unwrap();
        let transport = TransportEnvelope::new(DeviceId::new(), pgp_envelope);

        queue.enqueue(transport.clone()).await;

        let peeked = queue.peek().await.unwrap();
        assert_eq!(peeked.envelope.message_id, transport.message_id);
        assert_eq!(peeked.attempts, 0);
    }

    #[tokio::test]
    async fn test_queue_remove() {
        let queue = MessageQueue::new();

        let alice = PgpKeyPair::generate("alice@example.com").unwrap();
        let bob = PgpKeyPair::generate("bob@example.com").unwrap();

        let message = PlaintextMessage::new(
            ConversationId::new(),
            DeviceId::new(),
            b"test".to_vec(),
        );

        let pgp_envelope = PgpEnvelope::from_plaintext(message, &alice, bob.cert()).unwrap();
        let transport = TransportEnvelope::new(DeviceId::new(), pgp_envelope);
        let msg_id = transport.message_id;

        queue.enqueue(transport).await;
        assert_eq!(queue.len().await, 1);

        let removed = queue.remove(&msg_id).await;
        assert!(removed.is_some());
        assert_eq!(queue.len().await, 0);
    }
}
