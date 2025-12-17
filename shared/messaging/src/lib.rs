//! Messaging protocol models shared across CryptoChat clients and services.

pub mod onboarding;
pub mod pgp_envelope;
pub mod requests;
use cryptochat_crypto_core::{
    decrypt_message, encrypt_message, sign_message, verify_signature, EncryptedPayload, KeyPair,
    Signature,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Unique identifier assigned to a logical conversation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConversationId(pub Uuid);

impl ConversationId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Unique identifier for a device instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub Uuid);

impl DeviceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Represents the plaintext body of a message before encryption.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaintextMessage {
    pub message_id: Uuid,
    pub conversation_id: ConversationId,
    pub sender_device: DeviceId,
    pub created_ms: i64,
    pub body: Vec<u8>,
}

impl PlaintextMessage {
    pub fn new(conversation_id: ConversationId, sender_device: DeviceId, body: Vec<u8>) -> Self {
        let created_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        Self {
            message_id: Uuid::new_v4(),
            conversation_id,
            sender_device,
            created_ms,
            body,
        }
    }
}

/// Represents a minimal encrypted payload envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    pub message_id: Uuid,
    pub conversation_id: ConversationId,
    pub sender_fingerprint: String,
    pub sender_device: DeviceId,
    pub created_ms: i64,
    pub payload: EncryptedPayload,
    pub signature: Signature,
}

impl EncryptedEnvelope {
    /// Encrypt and sign a plaintext message using the provided key pair.
    pub fn from_plaintext(message: PlaintextMessage, key_pair: &KeyPair) -> crate::Result<Self> {
        let payload = encrypt_message(key_pair, &message.body)
            .map_err(|e| MessagingError::Crypto(format!("{e:?}")))?;
        let signature = sign_message(key_pair, &message.body)
            .map_err(|e| MessagingError::Crypto(format!("{e:?}")))?;

        Ok(Self {
            message_id: message.message_id,
            conversation_id: message.conversation_id,
            sender_fingerprint: key_pair.fingerprint().as_str().to_owned(),
            sender_device: message.sender_device,
            created_ms: message.created_ms,
            payload,
            signature,
        })
    }

    /// Decrypts the payload and verifies the signature using the provided key pair.
    pub fn into_plaintext(self, key_pair: &KeyPair) -> crate::Result<PlaintextMessage> {
        let ciphertext = decrypt_message(key_pair, &self.payload)
            .map_err(|e| MessagingError::Crypto(format!("{e:?}")))?;

        verify_signature(key_pair, &ciphertext, &self.signature)
            .map_err(|e| MessagingError::Crypto(format!("{e:?}")))?;

        Ok(PlaintextMessage {
            message_id: self.message_id,
            conversation_id: self.conversation_id,
            sender_device: self.sender_device,
            created_ms: self.created_ms,
            body: ciphertext,
        })
    }
}

/// Basic delivery receipt model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryReceipt {
    pub message_id: Uuid,
    pub delivered_to: Uuid,
    pub timestamp_ms: i64,
}

impl DeliveryReceipt {
    pub fn new(message_id: Uuid, delivered_to: Uuid) -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        Self {
            message_id,
            delivered_to,
            timestamp_ms,
        }
    }
}

/// Messaging-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum MessagingError {
    #[error("cryptographic failure: {0}")]
    Crypto(String),
}

pub type Result<T> = std::result::Result<T, MessagingError>;

#[cfg(test)]
mod tests {
    use super::*;
    use cryptochat_crypto_core::KeyPair;

    #[test]
    fn envelope_roundtrip() {
        let keypair = KeyPair::from_seed(b"test-envelope").unwrap();
        let conversation = ConversationId::new();
        let device = DeviceId::new();

        let message = PlaintextMessage::new(
            conversation.clone(),
            device.clone(),
            b"hello world".to_vec(),
        );
        let envelope = EncryptedEnvelope::from_plaintext(message.clone(), &keypair).unwrap();
        let decrypted = envelope.into_plaintext(&keypair).unwrap();

        assert_eq!(message.body, decrypted.body);
        assert_eq!(message.conversation_id, decrypted.conversation_id);
        assert_eq!(message.sender_device, decrypted.sender_device);
    }
}
