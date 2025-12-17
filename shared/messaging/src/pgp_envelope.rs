//! PGP-based encrypted message envelopes for end-to-end encryption.

use crate::{ConversationId, DeviceId, MessagingError, PlaintextMessage, Result};
use cryptochat_crypto_core::pgp::PgpKeyPair;
use serde::{Deserialize, Serialize};
use sequoia_openpgp::Cert;
use uuid::Uuid;

/// Encrypted envelope using OpenPGP for end-to-end encryption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgpEnvelope {
    pub message_id: Uuid,
    pub conversation_id: ConversationId,
    pub sender_fingerprint: String,
    pub sender_device: DeviceId,
    pub created_ms: i64,
    /// OpenPGP encrypted and signed payload (base64 encoded).
    pub encrypted_payload: String,
}

impl PgpEnvelope {
    /// Encrypt and sign a plaintext message using PGP.
    pub fn from_plaintext(
        message: PlaintextMessage,
        sender_keypair: &PgpKeyPair,
        recipient_cert: &Cert,
    ) -> Result<Self> {
        let encrypted_payload = sender_keypair
            .encrypt_and_sign(recipient_cert, &message.body)
            .map_err(|e| MessagingError::Crypto(format!("encrypt_and_sign failed: {}", e)))?;

        let encrypted_payload_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &encrypted_payload,
        );

        Ok(Self {
            message_id: message.message_id,
            conversation_id: message.conversation_id,
            sender_fingerprint: sender_keypair.fingerprint(),
            sender_device: message.sender_device,
            created_ms: message.created_ms,
            encrypted_payload: encrypted_payload_b64,
        })
    }

    /// Decrypt and verify the envelope using PGP.
    pub fn into_plaintext(
        self,
        recipient_keypair: &PgpKeyPair,
        sender_cert: &Cert,
    ) -> Result<PlaintextMessage> {
        let encrypted_payload = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &self.encrypted_payload,
        )
        .map_err(|e| MessagingError::Crypto(format!("base64 decode failed: {}", e)))?;

        let body = recipient_keypair
            .decrypt_and_verify(sender_cert, &encrypted_payload)
            .map_err(|e| MessagingError::Crypto(format!("decrypt_and_verify failed: {}", e)))?;

        Ok(PlaintextMessage {
            message_id: self.message_id,
            conversation_id: self.conversation_id,
            sender_device: self.sender_device,
            created_ms: self.created_ms,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pgp_envelope_roundtrip() {
        let alice = PgpKeyPair::generate("alice@example.com").unwrap();
        let bob = PgpKeyPair::generate("bob@example.com").unwrap();

        let conversation = ConversationId::new();
        let device = DeviceId::new();

        let message = PlaintextMessage::new(
            conversation.clone(),
            device.clone(),
            b"Hello from Alice to Bob!".to_vec(),
        );

        // Alice encrypts for Bob
        let envelope =
            PgpEnvelope::from_plaintext(message.clone(), &alice, bob.cert()).unwrap();

        // Bob decrypts from Alice
        let decrypted = envelope
            .into_plaintext(&bob, alice.cert())
            .unwrap();

        assert_eq!(message.body, decrypted.body);
        assert_eq!(message.message_id, decrypted.message_id);
        assert_eq!(message.conversation_id, decrypted.conversation_id);
    }

    #[test]
    fn test_pgp_envelope_signature_verification() {
        let alice = PgpKeyPair::generate("alice@example.com").unwrap();
        let bob = PgpKeyPair::generate("bob@example.com").unwrap();
        let eve = PgpKeyPair::generate("eve@example.com").unwrap();

        let conversation = ConversationId::new();
        let device = DeviceId::new();

        let message = PlaintextMessage::new(
            conversation.clone(),
            device.clone(),
            b"Signed message".to_vec(),
        );

        // Alice encrypts for Bob
        let envelope = PgpEnvelope::from_plaintext(message, &alice, bob.cert()).unwrap();

        // Bob tries to decrypt using Eve's cert as sender - should fail
        let result = envelope.into_plaintext(&bob, eve.cert());
        assert!(result.is_err());
    }
}
