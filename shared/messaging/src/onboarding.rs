//! Onboarding protocol for initial key exchange and trust establishment.

use crate::{DeviceId, MessagingError, Result};
use cryptochat_crypto_core::pgp::PgpKeyPair;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request to exchange public keys during onboarding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyExchangeRequest {
    pub request_id: Uuid,
    pub device_id: DeviceId,
    pub user_id: String,
    pub public_key_armored: String,
    pub timestamp_ms: i64,
}

impl KeyExchangeRequest {
    pub fn new(user_id: String, device_id: DeviceId, keypair: &PgpKeyPair) -> Result<Self> {
        let public_key_armored = keypair
            .export_public_key()
            .map_err(|e| MessagingError::Crypto(format!("failed to export public key: {}", e)))?;

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Ok(Self {
            request_id: Uuid::new_v4(),
            device_id,
            user_id,
            public_key_armored,
            timestamp_ms,
        })
    }
}

/// Response to key exchange containing the peer's public key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyExchangeResponse {
    pub request_id: Uuid,
    pub device_id: DeviceId,
    pub user_id: String,
    pub public_key_armored: String,
    pub fingerprint: String,
    pub timestamp_ms: i64,
}

impl KeyExchangeResponse {
    pub fn new(
        request_id: Uuid,
        user_id: String,
        device_id: DeviceId,
        keypair: &PgpKeyPair,
    ) -> Result<Self> {
        let public_key_armored = keypair
            .export_public_key()
            .map_err(|e| MessagingError::Crypto(format!("failed to export public key: {}", e)))?;

        let fingerprint = keypair.fingerprint();

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Ok(Self {
            request_id,
            device_id,
            user_id,
            public_key_armored,
            fingerprint,
            timestamp_ms,
        })
    }
}

/// Bundle containing all key material and metadata needed to establish trust.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBundle {
    pub user_id: String,
    pub device_id: DeviceId,
    pub public_key_armored: String,
    pub fingerprint: String,
    pub created_at_ms: i64,
}

impl KeyBundle {
    pub fn new(user_id: String, device_id: DeviceId, keypair: &PgpKeyPair) -> Result<Self> {
        let public_key_armored = keypair
            .export_public_key()
            .map_err(|e| MessagingError::Crypto(format!("failed to export public key: {}", e)))?;

        let fingerprint = keypair.fingerprint();

        let created_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        Ok(Self {
            user_id,
            device_id,
            public_key_armored,
            fingerprint,
            created_at_ms,
        })
    }

    /// Import the public key from this bundle.
    pub fn import_public_key(&self) -> Result<PgpKeyPair> {
        PgpKeyPair::from_public_key(&self.public_key_armored)
            .map_err(|e| MessagingError::Crypto(format!("failed to import public key: {}", e)))
    }

    /// Verify that the fingerprint matches the public key.
    pub fn verify_fingerprint(&self) -> Result<bool> {
        let keypair = self.import_public_key()?;
        Ok(keypair.fingerprint() == self.fingerprint)
    }
}

/// Trust status for a peer's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustStatus {
    /// Not yet verified by the user.
    Unverified,
    /// Verified through out-of-band mechanism (QR code, SAS, etc).
    Verified,
    /// Previously verified but key has changed.
    Changed,
}

/// Trust record associating a device with trust status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustRecord {
    pub device_id: DeviceId,
    pub fingerprint: String,
    pub status: TrustStatus,
    pub verified_at_ms: Option<i64>,
}

impl TrustRecord {
    pub fn new_unverified(device_id: DeviceId, fingerprint: String) -> Self {
        Self {
            device_id,
            fingerprint,
            status: TrustStatus::Unverified,
            verified_at_ms: None,
        }
    }

    pub fn mark_verified(&mut self) {
        self.status = TrustStatus::Verified;
        self.verified_at_ms = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        );
    }

    pub fn mark_changed(&mut self) {
        self.status = TrustStatus::Changed;
    }

    pub fn is_verified(&self) -> bool {
        self.status == TrustStatus::Verified
    }
}

/// Short Authentication String (SAS) for out-of-band verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SasVerification {
    pub local_fingerprint: String,
    pub remote_fingerprint: String,
    pub sas_words: Vec<String>,
}

impl SasVerification {
    /// Generate SAS words from two fingerprints for manual verification.
    pub fn generate(local_fingerprint: &str, remote_fingerprint: &str) -> Self {
        // Simple deterministic word generation from fingerprints
        // In production, use proper SAS algorithm (e.g., emoji or word list)
        let combined = format!("{}{}", local_fingerprint, remote_fingerprint);
        let hash = cryptochat_crypto_core::Fingerprint::from_public_key(combined.as_bytes());

        // Generate 6 simple words for verification
        let words = hash
            .as_str()
            .chars()
            .take(6)
            .map(|c| format!("word_{}", c))
            .collect();

        Self {
            local_fingerprint: local_fingerprint.to_string(),
            remote_fingerprint: remote_fingerprint.to_string(),
            sas_words: words,
        }
    }

    /// Check if the provided SAS matches.
    pub fn verify(&self, other_sas: &[String]) -> bool {
        self.sas_words == other_sas
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_exchange_request_creation() {
        let keypair = PgpKeyPair::generate("alice@example.com").unwrap();
        let device_id = DeviceId::new();
        let request =
            KeyExchangeRequest::new("alice@example.com".to_string(), device_id, &keypair).unwrap();

        assert!(!request.public_key_armored.is_empty());
        assert!(request.public_key_armored.contains("BEGIN PGP PUBLIC KEY BLOCK"));
    }

    #[test]
    fn test_key_bundle_creation_and_verification() {
        let keypair = PgpKeyPair::generate("bob@example.com").unwrap();
        let device_id = DeviceId::new();
        let bundle = KeyBundle::new("bob@example.com".to_string(), device_id, &keypair).unwrap();

        assert_eq!(bundle.fingerprint, keypair.fingerprint());
        assert!(bundle.verify_fingerprint().unwrap());
    }

    #[test]
    fn test_key_bundle_import() {
        let keypair = PgpKeyPair::generate("charlie@example.com").unwrap();
        let device_id = DeviceId::new();
        let bundle =
            KeyBundle::new("charlie@example.com".to_string(), device_id, &keypair).unwrap();

        let imported = bundle.import_public_key().unwrap();
        assert_eq!(imported.fingerprint(), keypair.fingerprint());
    }

    #[test]
    fn test_trust_record_lifecycle() {
        let device_id = DeviceId::new();
        let mut record = TrustRecord::new_unverified(device_id, "ABC123".to_string());

        assert_eq!(record.status, TrustStatus::Unverified);
        assert!(!record.is_verified());

        record.mark_verified();
        assert_eq!(record.status, TrustStatus::Verified);
        assert!(record.is_verified());
        assert!(record.verified_at_ms.is_some());

        record.mark_changed();
        assert_eq!(record.status, TrustStatus::Changed);
        assert!(!record.is_verified());
    }

    #[test]
    fn test_sas_generation_is_deterministic() {
        let fp1 = "ABCD1234";
        let fp2 = "EFGH5678";

        let sas1 = SasVerification::generate(fp1, fp2);
        let sas2 = SasVerification::generate(fp1, fp2);

        assert_eq!(sas1.sas_words, sas2.sas_words);
    }

    #[test]
    fn test_sas_verification() {
        let fp1 = "ABCD1234";
        let fp2 = "EFGH5678";

        let sas = SasVerification::generate(fp1, fp2);
        assert!(sas.verify(&sas.sas_words));
        assert!(!sas.verify(&vec!["wrong".to_string()]));
    }
}
