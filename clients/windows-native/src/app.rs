//! Application state management
use base64::Engine;

use cryptochat_crypto_core::pgp::PgpKeyPair;
use cryptochat_messaging::{DeviceId, onboarding::TrustRecord};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct AppState {
    pub keypair: Arc<RwLock<Option<PgpKeyPair>>>,
    pub recipient_keypair: Arc<RwLock<Option<PgpKeyPair>>>,
    pub device_id: DeviceId,
    pub trust_records: Arc<RwLock<HashMap<String, TrustRecord>>>,
    pub peer_address: Arc<RwLock<Option<String>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            keypair: Arc::new(RwLock::new(None)),
            recipient_keypair: Arc::new(RwLock::new(None)),
            device_id: DeviceId::new(),
            trust_records: Arc::new(RwLock::new(HashMap::new())),
            peer_address: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_keypair(&self, keypair: PgpKeyPair) {
        *self.keypair.write().unwrap() = Some(keypair);
    }

    pub fn set_recipient_keypair(&self, keypair: PgpKeyPair) {
        *self.recipient_keypair.write().unwrap() = Some(keypair);
    }

    pub fn get_fingerprint(&self) -> Option<String> {
        self.keypair
            .read()
            .unwrap()
            .as_ref()
            .map(|kp| kp.fingerprint())
    }

    pub fn get_recipient_fingerprint(&self) -> Option<String> {
        self.recipient_keypair
            .read()
            .unwrap()
            .as_ref()
            .map(|kp| kp.fingerprint())
    }

    pub fn get_keypair(&self) -> Option<PgpKeyPair> {
        self.keypair.read().unwrap().clone()
    }

    /// Get a clone of the recipient's keypair
    pub fn get_recipient_keypair(&self) -> anyhow::Result<Option<PgpKeyPair>> {
        Ok(self.recipient_keypair.read().unwrap().clone())
    }

    pub fn encrypt_message(&self, plaintext: &str) -> anyhow::Result<String> {
        let my_keypair = self.keypair.read().unwrap();
        let recipient_keypair = self.recipient_keypair.read().unwrap();

        match (my_keypair.as_ref(), recipient_keypair.as_ref()) {
            (Some(my_key), Some(recipient_key)) => {
                let encrypted_bytes = my_key.encrypt_and_sign(recipient_key.cert(), plaintext.as_bytes())?;
                Ok(base64::engine::general_purpose::STANDARD.encode(&encrypted_bytes))
            }
            _ => anyhow::bail!("Keys not initialized"),
        }
    }

    pub fn decrypt_message(&self, encrypted_base64: &str) -> anyhow::Result<String> {
        let my_keypair = self.keypair.read().unwrap();
        let recipient_keypair = self.recipient_keypair.read().unwrap();

        match (my_keypair.as_ref(), recipient_keypair.as_ref()) {
            (Some(my_key), Some(recipient_key)) => {
                let encrypted_bytes = base64::engine::general_purpose::STANDARD.decode(encrypted_base64)?;
                let decrypted = my_key.decrypt_and_verify(recipient_key.cert(), &encrypted_bytes)?;
                Ok(String::from_utf8(decrypted)?)
            }
            _ => anyhow::bail!("Keys not initialized"),
        }
    }

    /// Decrypt a message using the sender's public key directly (for multi-chat support)
    /// This allows decrypting messages from any contact without requiring them to be "active"
    pub fn decrypt_message_with_sender_key(&self, encrypted_base64: &str, sender_public_key: &str) -> anyhow::Result<String> {
        let my_keypair = self.keypair.read().unwrap();
        
        match my_keypair.as_ref() {
            Some(my_key) => {
                // Parse the sender's public key to get their cert for signature verification
                let sender_keypair = PgpKeyPair::from_public_key(sender_public_key)?;
                let encrypted_bytes = base64::engine::general_purpose::STANDARD.decode(encrypted_base64)?;
                let decrypted = my_key.decrypt_and_verify(sender_keypair.cert(), &encrypted_bytes)?;
                Ok(String::from_utf8(decrypted)?)
            }
            None => anyhow::bail!("Own keypair not initialized"),
        }
    }

    /// Export keypair data for account migration (secret_key, public_key, fingerprint)
    pub fn get_keypair_for_export(&self) -> Option<(String, String, String)> {
        let keypair_guard = self.keypair.read().unwrap();
        if let Some(ref kp) = *keypair_guard {
            let secret_key = kp.export_secret_key().ok()?;
            let public_key = kp.export_public_key().ok()?;
            let fingerprint = kp.fingerprint();
            Some((secret_key, public_key, fingerprint))
        } else {
            None
        }
    }

    pub fn set_peer_address(&self, address: String) {
        *self.peer_address.write().unwrap() = Some(address);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

