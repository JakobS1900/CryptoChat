//! Message pipeline coordinating encryption, overlay publishing, and receipts.

use super::{MessageQueue, MessageReceipt, PipelineError, ReceiptStatus, Result, TransportEnvelope};
use cryptochat_crypto_core::pgp::PgpKeyPair;
use cryptochat_messaging::pgp_envelope::PgpEnvelope;
use cryptochat_messaging::{ConversationId, DeviceId, PlaintextMessage};
use sequoia_openpgp::Cert;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

/// Configuration for the message pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub max_retries: usize,
    pub retry_delay_ms: u64,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 5000,
        }
    }
}

/// Request to send a message through the pipeline.
#[derive(Debug)]
pub struct SendMessageRequest {
    pub conversation_id: ConversationId,
    pub recipient_device: DeviceId,
    pub recipient_cert: Cert,
    pub body: Vec<u8>,
}

/// Response after queuing a message.
#[derive(Debug)]
pub struct SendMessageResponse {
    pub message_id: Uuid,
    pub queued_at_ms: i64,
}

/// Message pipeline handling encryption, queueing, and overlay publishing.
pub struct MessagePipeline {
    config: PipelineConfig,
    local_keypair: Arc<RwLock<Option<PgpKeyPair>>>,
    local_device: DeviceId,
    queue: MessageQueue,
    receipts: Arc<RwLock<HashMap<Uuid, Vec<MessageReceipt>>>>,
    receipt_tx: mpsc::UnboundedSender<MessageReceipt>,
}

impl MessagePipeline {
    pub fn new(
        config: PipelineConfig,
        local_device: DeviceId,
    ) -> (Self, mpsc::UnboundedReceiver<MessageReceipt>) {
        let (receipt_tx, receipt_rx) = mpsc::unbounded_channel();

        let pipeline = Self {
            config,
            local_keypair: Arc::new(RwLock::new(None)),
            local_device,
            queue: MessageQueue::new(),
            receipts: Arc::new(RwLock::new(HashMap::new())),
            receipt_tx,
        };

        (pipeline, receipt_rx)
    }

    /// Set the local keypair for signing and encryption.
    pub async fn set_keypair(&self, keypair: PgpKeyPair) {
        *self.local_keypair.write().await = Some(keypair);
    }

    /// Send a message through the pipeline.
    pub async fn send_message(&self, request: SendMessageRequest) -> Result<SendMessageResponse> {
        let keypair = self
            .local_keypair
            .read()
            .await
            .clone()
            .ok_or_else(|| PipelineError::Crypto("keypair not initialized".to_string()))?;

        // Create plaintext message
        let plaintext = PlaintextMessage::new(
            request.conversation_id,
            self.local_device.clone(),
            request.body,
        );

        let message_id = plaintext.message_id;

        // Encrypt and sign with PGP
        let pgp_envelope = PgpEnvelope::from_plaintext(plaintext, &keypair, &request.recipient_cert)
            .map_err(|e| PipelineError::Crypto(format!("encryption failed: {}", e)))?;

        // Wrap in transport envelope
        let transport = TransportEnvelope::new(request.recipient_device, pgp_envelope);

        let queued_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        // Enqueue for overlay delivery
        self.queue.enqueue(transport).await;

        // Create queued receipt
        let receipt = MessageReceipt::new(message_id, self.local_device.clone(), ReceiptStatus::Queued);
        self.add_receipt(receipt.clone()).await;
        let _ = self.receipt_tx.send(receipt);

        Ok(SendMessageResponse {
            message_id,
            queued_at_ms,
        })
    }

    /// Process an incoming transport envelope.
    pub async fn receive_envelope(
        &self,
        envelope: TransportEnvelope,
        sender_cert: &Cert,
    ) -> Result<PlaintextMessage> {
        let keypair = self
            .local_keypair
            .read()
            .await
            .clone()
            .ok_or_else(|| PipelineError::Crypto("keypair not initialized".to_string()))?;

        // Decrypt and verify
        let plaintext = envelope
            .pgp_envelope
            .into_plaintext(&keypair, sender_cert)
            .map_err(|e| PipelineError::Crypto(format!("decryption failed: {}", e)))?;

        // Create delivery receipt
        let receipt = MessageReceipt::new(
            plaintext.message_id,
            self.local_device.clone(),
            ReceiptStatus::Delivered,
        );
        self.add_receipt(receipt.clone()).await;
        let _ = self.receipt_tx.send(receipt);

        Ok(plaintext)
    }

    /// Mark a message as sent to overlay.
    pub async fn mark_sent(&self, message_id: Uuid) {
        let receipt = MessageReceipt::new(message_id, self.local_device.clone(), ReceiptStatus::Sent);
        self.add_receipt(receipt.clone()).await;
        let _ = self.receipt_tx.send(receipt);
    }

    /// Mark a message as failed.
    pub async fn mark_failed(&self, message_id: Uuid) {
        let receipt = MessageReceipt::new(message_id, self.local_device.clone(), ReceiptStatus::Failed);
        self.add_receipt(receipt.clone()).await;
        let _ = self.receipt_tx.send(receipt);
    }

    /// Get receipts for a specific message.
    pub async fn get_receipts(&self, message_id: &Uuid) -> Vec<MessageReceipt> {
        self.receipts
            .read()
            .await
            .get(message_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get the message queue (for overlay integration).
    pub fn queue(&self) -> &MessageQueue {
        &self.queue
    }

    async fn add_receipt(&self, receipt: MessageReceipt) {
        self.receipts
            .write()
            .await
            .entry(receipt.message_id)
            .or_insert_with(Vec::new)
            .push(receipt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pipeline_send_message() {
        let (pipeline, _rx) = MessagePipeline::new(
            PipelineConfig::default(),
            DeviceId::new(),
        );

        let alice = PgpKeyPair::generate("alice@example.com").unwrap();
        let bob = PgpKeyPair::generate("bob@example.com").unwrap();

        pipeline.set_keypair(alice).await;

        let request = SendMessageRequest {
            conversation_id: ConversationId::new(),
            recipient_device: DeviceId::new(),
            recipient_cert: bob.cert().clone(),
            body: b"Hello Bob!".to_vec(),
        };

        let response = pipeline.send_message(request).await.unwrap();
        assert_eq!(pipeline.queue().len().await, 1);

        let receipts = pipeline.get_receipts(&response.message_id).await;
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].status, ReceiptStatus::Queued);
    }

    #[tokio::test]
    async fn test_pipeline_receive_envelope() {
        let (alice_pipeline, _rx1) = MessagePipeline::new(
            PipelineConfig::default(),
            DeviceId::new(),
        );

        let (bob_pipeline, _rx2) = MessagePipeline::new(
            PipelineConfig::default(),
            DeviceId::new(),
        );

        let alice = PgpKeyPair::generate("alice@example.com").unwrap();
        let bob = PgpKeyPair::generate("bob@example.com").unwrap();

        alice_pipeline.set_keypair(alice.clone()).await;
        bob_pipeline.set_keypair(bob.clone()).await;

        // Alice sends to Bob
        let request = SendMessageRequest {
            conversation_id: ConversationId::new(),
            recipient_device: DeviceId::new(),
            recipient_cert: bob.cert().clone(),
            body: b"Hello Bob!".to_vec(),
        };

        alice_pipeline.send_message(request).await.unwrap();

        // Get the envelope from Alice's queue
        let queued = alice_pipeline.queue().peek().await.unwrap();
        let envelope = queued.envelope;

        // Bob receives it
        let plaintext = bob_pipeline
            .receive_envelope(envelope, alice.cert())
            .await
            .unwrap();

        assert_eq!(plaintext.body, b"Hello Bob!");

        // Check Bob has delivery receipt
        let receipts = bob_pipeline.get_receipts(&plaintext.message_id).await;
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].status, ReceiptStatus::Delivered);
    }
}
