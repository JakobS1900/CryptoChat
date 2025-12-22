//! Encrypted storage for chat history using AES-256-GCM
//! 
//! Security properties:
//! - Storage key derived from user's PGP fingerprint using PBKDF2
//! - Each save uses a random IV for forward secrecy
//! - Authenticated encryption prevents tampering

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use sha2::Sha256;
use std::fs;
use std::path::PathBuf;

/// Number of PBKDF2 iterations for key derivation
const PBKDF2_ITERATIONS: u32 = 100_000;

/// Salt for key derivation (device-specific in production, fixed for now)
const STORAGE_SALT: &[u8] = b"CryptoChat_Storage_Salt_v1";

/// Encrypted chat store format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedStore {
    /// 12-byte IV for AES-GCM
    pub iv: Vec<u8>,
    /// Encrypted data (JSON serialized messages)
    pub ciphertext: Vec<u8>,
}

/// A message with optional expiration for disappearing messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub sender_name: String,
    pub content: String,
    pub timestamp: String,
    pub is_mine: bool,
    /// ISO8601 timestamp when message should be deleted (None = never)
    pub expires_at: Option<String>,
    /// Image data if this is an image message (base64 encoded)
    pub image_data: Option<String>,
    pub image_filename: Option<String>,
    #[serde(default)]
    pub emotes: std::collections::HashMap<String, String>,
}

/// Derive a 256-bit storage key from the user's PGP fingerprint
/// 
/// This ties the encryption to the user's identity - only someone with
/// access to this fingerprint can derive the key.
pub fn derive_storage_key(fingerprint: &str) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(
        fingerprint.as_bytes(),
        STORAGE_SALT,
        PBKDF2_ITERATIONS,
        &mut key,
    );
    key
}

/// Encrypt generic data for storage on disk
pub fn encrypt_data<T: Serialize + ?Sized>(data: &T, storage_key: &[u8; 32]) -> Result<EncryptedStore> {
    // Serialize data to JSON
    let json = serde_json::to_vec(data)
        .context("Failed to serialize data")?;
    
    // Generate random 12-byte IV
    let mut iv_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut iv_bytes);
    
    // Encrypt with AES-256-GCM
    let cipher = Aes256Gcm::new_from_slice(storage_key)
        .context("Failed to create cipher")?;
    let nonce = Nonce::from_slice(&iv_bytes);
    
    let ciphertext = cipher.encrypt(nonce, json.as_slice())
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
    
    Ok(EncryptedStore {
        iv: iv_bytes.to_vec(),
        ciphertext,
    })
}

/// Decrypt generic data from encrypted storage
pub fn decrypt_data<T: DeserializeOwned>(store: &EncryptedStore, storage_key: &[u8; 32]) -> Result<T> {
    // Decrypt with AES-256-GCM
    let cipher = Aes256Gcm::new_from_slice(storage_key)
        .context("Failed to create cipher")?;
    
    let nonce = Nonce::from_slice(&store.iv);
    
    let plaintext = cipher.decrypt(nonce, store.ciphertext.as_slice())
        .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;
    
    // Deserialize JSON
    let data: T = serde_json::from_slice(&plaintext)
        .context("Failed to deserialize data")?;
    
    Ok(data)
}

/// Get the path to the encrypted chat history file
fn get_encrypted_history_path() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .context("Could not find home directory")?;
    
    let dir_name = match crate::get_instance_id() {
        Some(id) => format!(".cryptochat_{}", id),
        None => ".cryptochat".to_string(),
    };
    
    let data_dir = PathBuf::from(home).join(dir_name);
    if !data_dir.exists() {
        fs::create_dir_all(&data_dir)?;
    }
    
    Ok(data_dir.join("chat_history.enc"))
}

/// Save messages to encrypted file
pub fn save_encrypted_history(messages: &[StoredMessage], fingerprint: &str) -> Result<()> {
    let storage_key = derive_storage_key(fingerprint);
    let encrypted = encrypt_data(messages, &storage_key)?;
    
    let path = get_encrypted_history_path()?;
    let json = serde_json::to_vec(&encrypted)?;
    fs::write(&path, json)?;
    
    Ok(())
}

/// Load messages from encrypted file
pub fn load_encrypted_history(fingerprint: &str) -> Result<Vec<StoredMessage>> {
    let path = get_encrypted_history_path()?;
    
    if !path.exists() {
        return Ok(Vec::new());
    }
    
    let json = fs::read(&path)?;
    let encrypted: EncryptedStore = serde_json::from_slice(&json)?;
    
    let storage_key = derive_storage_key(fingerprint);
    decrypt_data(&encrypted, &storage_key)
}

/// Remove expired messages (for disappearing message feature)
pub fn cleanup_expired_messages(messages: &mut Vec<StoredMessage>) -> usize {
    let now = chrono::Utc::now();
    let initial_count = messages.len();
    
    messages.retain(|msg| {
        if let Some(ref expires_at) = msg.expires_at {
            // Parse ISO8601 timestamp and check if expired
            match chrono::DateTime::parse_from_rfc3339(expires_at) {
                Ok(expiry) => expiry > now,
                Err(_) => true, // Keep if we can't parse
            }
        } else {
            true // Keep messages without expiration
        }
    });
    
    initial_count - messages.len()
}

/// Check if there's an existing unencrypted history file to migrate
pub fn has_unencrypted_history() -> bool {
    if let Ok(path) = get_encrypted_history_path() {
        // Check for old .json file
        let old_path = path.with_extension("json");
        old_path.exists()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let messages = vec![
            StoredMessage {
                sender_name: "Alice".to_string(),
                content: "Hello, world!".to_string(),
                timestamp: "2024-01-01T12:00:00Z".to_string(),
                is_mine: true,
                expires_at: None,
                image_data: None,
                image_filename: None,
                emotes: std::collections::HashMap::new(),
            },
        ];
        
        let key = derive_storage_key("TEST_FINGERPRINT");
        let encrypted = encrypt_messages(&messages, &key).unwrap();
        let decrypted = decrypt_messages(&encrypted, &key).unwrap();
        
        assert_eq!(decrypted.len(), 1);
        assert_eq!(decrypted[0].content, "Hello, world!");
    }
    
    #[test]
    fn test_wrong_key_fails() {
        let messages = vec![
            StoredMessage {
                sender_name: "Alice".to_string(),
                content: "Secret message".to_string(),
                timestamp: "2024-01-01T12:00:00Z".to_string(),
                is_mine: true,
                expires_at: None,
                image_data: None,
                image_filename: None,
                emotes: std::collections::HashMap::new(),
            },
        ];
        
        let key1 = derive_storage_key("FINGERPRINT_1");
        let key2 = derive_storage_key("FINGERPRINT_2");
        
        let encrypted = encrypt_messages(&messages, &key1).unwrap();
        let result = decrypt_messages(&encrypted, &key2);
        
        assert!(result.is_err()); // Wrong key should fail
    }
}
