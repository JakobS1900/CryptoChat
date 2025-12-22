use crate::conversation::Conversation;
use crate::encrypted_storage::{derive_storage_key, encrypt_data, decrypt_data, EncryptedStore};
use crate::request_store::get_data_dir;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

fn get_conversations_path(fingerprint: &str) -> Result<PathBuf> {
    // Encrypted file extension .enc, specific to this user fingerprint
    Ok(get_data_dir()?.join(format!("conversations_{}.enc", fingerprint)))
}

/// Save all conversations to encrypted disk storage
pub fn save_conversations(conversations: &HashMap<String, Conversation>, fingerprint: &str) -> Result<()> {
    let key = derive_storage_key(fingerprint);
    let encrypted = encrypt_data(conversations, &key)?;
    
    let path = get_conversations_path(fingerprint)?;
    let json = serde_json::to_vec(&encrypted)?;
    fs::write(&path, json).context("Failed to write conversations file")?;
    
    Ok(())
}

/// Load conversations from encrypted disk storage
pub fn load_conversations(fingerprint: &str) -> Result<HashMap<String, Conversation>> {
    let path = get_conversations_path(fingerprint)?;
    
    if !path.exists() {
        return Ok(HashMap::new());
    }
    
    let json = fs::read(&path).context("Failed to read conversations file")?;
    let encrypted: EncryptedStore = serde_json::from_slice(&json).context("Failed to parse encrypted store")?;
    
    let key = derive_storage_key(fingerprint);
    let conversations: HashMap<String, Conversation> = decrypt_data(&encrypted, &key)?;
    
    Ok(conversations)
}
