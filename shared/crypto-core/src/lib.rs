//! Core cryptographic helpers for CryptoChat.
//!
//! The production system will integrate OpenPGP libraries. Until then, this
//! crate provides deterministic, test-friendly primitives that mimic the
//! required behaviors (key generation, signing, verification, and envelope
//! encryption) so higher layers can be developed in parallel.

pub mod pgp;
use base64::{engine::general_purpose, Engine as _};
use rand::{rngs::OsRng, RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};
use std::fmt::{self, Display};
use uuid::Uuid;

/// Result type exposed by crypto-core APIs.
pub type Result<T> = std::result::Result<T, CryptoError>;

/// Errors returned by the crypto core.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("verification failed")]
    VerificationFailed,
    #[error("invalid ciphertext length")]
    InvalidCiphertext,
    #[error("internal error: {0}")]
    Internal(String),
}

/// Represents a PGP-style key fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Fingerprint(String);

impl Fingerprint {
    /// Creates a fingerprint from a public key byte slice.
    pub fn from_public_key(public_key: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(public_key);
        let digest = hasher.finalize();
        let encoded = general_purpose::STANDARD_NO_PAD.encode(digest);
        Self(encoded)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Represents a deterministic key pair for signing and envelope encryption.
#[derive(Debug, Clone)]
pub struct KeyPair {
    fingerprint: Fingerprint,
    public_key: Vec<u8>,
    private_key: Vec<u8>,
}

impl KeyPair {
    /// Generates a key pair backed by OS randomness.
    pub fn generate() -> Result<Self> {
        let mut seed = [0u8; 32];
        OsRng.fill_bytes(&mut seed);
        Self::from_seed(&seed)
    }

    /// Generates a deterministic key pair from an arbitrary seed.
    pub fn from_seed(seed: &[u8]) -> Result<Self> {
        let mut seed_bytes = [0u8; 32];
        if seed.len() >= 32 {
            seed_bytes.copy_from_slice(&seed[..32]);
        } else {
            let mut hasher = Sha256::new();
            hasher.update(seed);
            let digest = hasher.finalize();
            seed_bytes.copy_from_slice(&digest[..32]);
        }

        let mut rng = ChaCha20Rng::from_seed(seed_bytes);
        let mut private_key = vec![0u8; 64];
        rng.fill_bytes(&mut private_key);

        // For placeholder public key derivation, hash the private key bytes.
        let mut hasher = Sha256::new();
        hasher.update(&private_key);
        let public_key = hasher.finalize().to_vec();

        let fingerprint = Fingerprint::from_public_key(&public_key);
        Ok(Self {
            fingerprint,
            public_key,
            private_key,
        })
    }

    pub fn fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }

    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    pub fn private_key(&self) -> &[u8] {
        &self.private_key
    }
}

/// Represents a detached signature.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Signature(String);

impl Signature {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Produce a deterministic signature over the provided message body.
pub fn sign_message(key_pair: &KeyPair, message: &[u8]) -> Result<Signature> {
    let mut hasher = Sha256::new();
    hasher.update(key_pair.private_key());
    hasher.update(message);
    let digest = hasher.finalize();
    Ok(Signature(general_purpose::STANDARD_NO_PAD.encode(digest)))
}

/// Verify a deterministic signature created by [`sign_message`].
pub fn verify_signature(key_pair: &KeyPair, message: &[u8], signature: &Signature) -> Result<()> {
    let expected = sign_message(key_pair, message)?;
    if expected == *signature {
        Ok(())
    } else {
        Err(CryptoError::VerificationFailed)
    }
}

/// Symmetric envelope encrypted payload.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EncryptedPayload {
    pub nonce: String,
    pub ciphertext: String,
}

impl EncryptedPayload {
    pub fn new(nonce: &[u8], ciphertext: &[u8]) -> Self {
        Self {
            nonce: general_purpose::STANDARD_NO_PAD.encode(nonce),
            ciphertext: general_purpose::STANDARD_NO_PAD.encode(ciphertext),
        }
    }

    pub fn decode(&self) -> Result<(Vec<u8>, Vec<u8>)> {
        let nonce = general_purpose::STANDARD_NO_PAD
            .decode(&self.nonce)
            .map_err(|e| CryptoError::Internal(format!("failed to decode nonce: {e}")))?;
        let ciphertext = general_purpose::STANDARD_NO_PAD
            .decode(&self.ciphertext)
            .map_err(|e| CryptoError::Internal(format!("failed to decode ciphertext: {e}")))?;
        Ok((nonce, ciphertext))
    }
}

/// Encrypts a message deterministically for prototyping purposes.
///
/// **Important:** replace this with real OpenPGP session key handling before launch.
pub fn encrypt_message(key_pair: &KeyPair, plaintext: &[u8]) -> Result<EncryptedPayload> {
    let mut hasher = Sha256::new();
    hasher.update(key_pair.fingerprint().as_str().as_bytes());
    hasher.update(plaintext.len().to_le_bytes());
    let seed: [u8; 32] = hasher.finalize().into();

    let mut nonce = [0u8; 24];
    nonce.copy_from_slice(&seed[..24]);

    let mut keystream = ChaCha20Rng::from_seed(seed);
    let mut ciphertext = plaintext.to_vec();
    for byte in &mut ciphertext {
        *byte ^= (keystream.next_u32() & 0xFF) as u8;
    }

    Ok(EncryptedPayload::new(&nonce, &ciphertext))
}

/// Decrypts an [`EncryptedPayload`] created by [`encrypt_message`].
pub fn decrypt_message(key_pair: &KeyPair, payload: &EncryptedPayload) -> Result<Vec<u8>> {
    let (nonce, mut ciphertext) = payload.decode()?;
    if nonce.len() != 24 {
        return Err(CryptoError::InvalidCiphertext);
    }

    let mut hasher = Sha256::new();
    hasher.update(key_pair.fingerprint().as_str().as_bytes());
    hasher.update(ciphertext.len().to_le_bytes());
    let seed: [u8; 32] = hasher.finalize().into();

    let mut keystream = ChaCha20Rng::from_seed(seed);
    for byte in &mut ciphertext {
        *byte ^= (keystream.next_u32() & 0xFF) as u8;
    }

    Ok(ciphertext)
}

/// Generates an opaque identifier for device registrations.
pub fn generate_device_id() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_is_deterministic_from_seed() {
        let seed = b"deterministic-seed";
        let first = KeyPair::from_seed(seed).unwrap();
        let second = KeyPair::from_seed(seed).unwrap();
        assert_eq!(first.public_key(), second.public_key());
        assert_eq!(first.private_key(), second.private_key());
        assert_eq!(first.fingerprint(), second.fingerprint());
    }

    #[test]
    fn signatures_roundtrip() {
        let keypair = KeyPair::from_seed(b"signatures").unwrap();
        let message = b"hello secure world";
        let signature = sign_message(&keypair, message).unwrap();
        verify_signature(&keypair, message, &signature).unwrap();
    }

    #[test]
    fn encryption_roundtrip() {
        let keypair = KeyPair::from_seed(b"encryption").unwrap();
        let payload = encrypt_message(&keypair, b"secret message").unwrap();
        let decrypted = decrypt_message(&keypair, &payload).unwrap();
        assert_eq!(decrypted, b"secret message");
    }

    #[test]
    fn generate_device_id_is_uuid() {
        let id = generate_device_id();
        assert!(Uuid::parse_str(&id).is_ok());
    }
}
