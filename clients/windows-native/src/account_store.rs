//! Account storage with password-protected PGP key encryption
//!
//! Security: Password hashed with Argon2, PGP key encrypted with password-derived AES key

use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit, Nonce};
use anyhow::{Context, Result, bail};
use argon2::{Argon2, PasswordHasher, PasswordVerifier, password_hash::{SaltString, PasswordHash}};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Account data stored on disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    /// Argon2 password hash
    pub password_hash: String,
    /// AES-GCM encrypted PGP secret key (base64)
    pub encrypted_secret_key: String,
    /// Public key (not encrypted, needed for key exchange)
    pub public_key: String,
    /// Key fingerprint for verification
    pub fingerprint: String,
    /// Nonce used for encryption (base64)
    pub encryption_nonce: String,
    /// Salt used for key derivation (separate from password hash salt)
    pub key_derivation_salt: String,
}

/// Get path to account.json
fn get_account_path() -> Result<PathBuf> {
    Ok(crate::request_store::get_data_dir()?.join("account.json"))
}

/// Check if an account exists
pub fn account_exists() -> bool {
    get_account_path().map(|p| p.exists()).unwrap_or(false)
}

/// Load account from disk
pub fn load_account() -> Result<Option<Account>> {
    let path = get_account_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let json = fs::read_to_string(&path).context("Failed to read account file")?;
    let account: Account = serde_json::from_str(&json).context("Failed to parse account")?;
    Ok(Some(account))
}

/// Save account to disk
pub fn save_account(account: &Account) -> Result<()> {
    let path = get_account_path()?;
    let json = serde_json::to_string_pretty(account)?;
    fs::write(&path, json).context("Failed to save account")?;
    Ok(())
}

/// Hash a password with Argon2
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("Hash failed: {}", e))?;
    Ok(hash.to_string())
}

/// Verify a password against stored hash
pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

/// Derive an AES-256 key from password using Argon2
fn derive_encryption_key(password: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| anyhow::anyhow!("Key derivation failed: {}", e))?;
    Ok(key)
}

/// Encrypt the secret key with password-derived key
pub fn encrypt_secret_key(secret_key: &str, password: &str) -> Result<(String, String, String)> {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    
    // Generate salt for key derivation
    let mut salt = [0u8; 16];
    rand::RngCore::fill_bytes(&mut OsRng, &mut salt);
    
    // Derive encryption key
    let key = derive_encryption_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
    
    // Generate nonce
    let mut nonce_bytes = [0u8; 12];
    rand::RngCore::fill_bytes(&mut OsRng, &mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, secret_key.as_bytes())
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
    
    Ok((
        STANDARD.encode(&ciphertext),
        STANDARD.encode(&nonce_bytes),
        STANDARD.encode(&salt),
    ))
}

/// Decrypt the secret key with password
pub fn decrypt_secret_key(encrypted: &str, nonce_b64: &str, salt_b64: &str, password: &str) -> Result<String> {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    
    let ciphertext = STANDARD.decode(encrypted).context("Invalid encrypted data")?;
    let nonce_bytes = STANDARD.decode(nonce_b64).context("Invalid nonce")?;
    let salt = STANDARD.decode(salt_b64).context("Invalid salt")?;
    
    // Derive encryption key
    let key = derive_encryption_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| anyhow::anyhow!("Wrong password or corrupted data"))?;
    
    String::from_utf8(plaintext).context("Invalid UTF-8 in decrypted key")
}

/// Create a new account with username, password, and existing PGP keypair
pub fn create_account(
    username: &str,
    password: &str,
    secret_key: &str,
    public_key: &str,
    fingerprint: &str,
) -> Result<Account> {
    if password.len() < 4 {
        bail!("Password must be at least 4 characters");
    }
    
    let password_hash = hash_password(password)?;
    let (encrypted_secret_key, encryption_nonce, key_derivation_salt) = 
        encrypt_secret_key(secret_key, password)?;
    
    let account = Account {
        username: username.to_string(),
        password_hash,
        encrypted_secret_key,
        public_key: public_key.to_string(),
        fingerprint: fingerprint.to_string(),
        encryption_nonce,
        key_derivation_salt,
    };
    
    save_account(&account)?;
    Ok(account)
}

/// Login with password and get decrypted secret key
pub fn login(password: &str) -> Result<(Account, String)> {
    let account = load_account()?.ok_or_else(|| anyhow::anyhow!("No account found"))?;
    
    // Verify password
    if !verify_password(password, &account.password_hash) {
        bail!("Wrong password");
    }
    
    // Decrypt secret key
    let secret_key = decrypt_secret_key(
        &account.encrypted_secret_key,
        &account.encryption_nonce,
        &account.key_derivation_salt,
        password,
    )?;
    
    Ok((account, secret_key))
}
