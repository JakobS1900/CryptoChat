use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::{Result, Context};
use std::fs;

/// Permission level for who can invite new members
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InvitePermission {
    AdminsOnly,
    AllMembers,
    Whitelist(Vec<String>), // List of fingerprints allowed to invite
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSettings {
    pub invite_permission: InvitePermission,
    pub max_members: Option<usize>,
    /// Disappearing message timer in seconds (None = disabled)
    pub disappearing_timer_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMember {
    pub fingerprint: String,
    pub username: String,
    pub public_key: String,
    pub address: String,
    pub joined_at: String, // ISO8601
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,           // UUID
    pub name: String,
    pub created_at: String,   // ISO8601
    pub creator_fingerprint: String,
    pub members: Vec<GroupMember>,
    pub admins: Vec<String>,  // Fingerprints of admins
    pub settings: GroupSettings,
    /// Shared symmetric key (AES-256) for this group
    pub symmetric_key: Vec<u8>,
}

/// Helper struct for serialization to encrypted storage
#[derive(Serialize, Deserialize)]
struct GroupListWrapper {
    groups: Vec<Group>,
}

fn get_groups_path() -> Result<PathBuf> {
    Ok(crate::request_store::get_data_dir()?.join("groups.enc"))
}

/// Load all groups from encrypted storage
pub fn load_groups(fingerprint: &str) -> Result<Vec<Group>> {
    let path = get_groups_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    // Reuse encrypted_storage module but we need to repurpose it for generic data
    // Since encrypted_storage handles Vec<StoredMessage>, we might need to extend it 
    // or just use the raw encrypt/decrypt functions generic over content.
    // 
    // For now, let's implement a generic load/save in encrypted_storage OR
    // manually call encrypt/decrypt from here using the same key derivation.
    // simpler to add generic support to encrypted_storage, but let's just use 
    // the raw generic helpers if we made them public.
    
    // Checks encrypted_storage.rs... it has specific struct.
    // We should refactor encrypted_storage to be generic or copy the logic.
    // For speed, I'll use the raw logic here importing dependencies.
    
    use crate::encrypted_storage::{derive_storage_key, EncryptedStore};
    use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
    
    let json = fs::read(&path)?;
    let store: EncryptedStore = serde_json::from_slice(&json)?;
    let key = derive_storage_key(fingerprint);
    
    let cipher = Aes256Gcm::new_from_slice(&key).context("Failed to create cipher")?;
    let nonce = Nonce::from_slice(&store.iv);
    
    let plaintext = cipher.decrypt(nonce, store.ciphertext.as_slice())
        .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;
        
    let groups: Vec<Group> = serde_json::from_slice(&plaintext)?;
    Ok(groups)
}

/// Save all groups to encrypted storage
pub fn save_groups(groups: &[Group], fingerprint: &str) -> Result<()> {
    use crate::encrypted_storage::{derive_storage_key, EncryptedStore};
    use aes_gcm::{aead::{Aead, KeyInit, OsRng}, Aes256Gcm, Nonce};
    use rand::RngCore;
    
    let key = derive_storage_key(fingerprint);
    let json = serde_json::to_vec(groups)?;
    
    let mut iv = [0u8; 12];
    OsRng.fill_bytes(&mut iv);
    
    let cipher = Aes256Gcm::new_from_slice(&key).context("Failed to create cipher")?;
    let nonce = Nonce::from_slice(&iv);
    
    let ciphertext = cipher.encrypt(nonce, json.as_slice())
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
        
    let store = EncryptedStore {
        iv: iv.to_vec(),
        ciphertext,
    };
    
    let path = get_groups_path()?;
    let file_content = serde_json::to_vec(&store)?;
    fs::write(&path, file_content)?;
    Ok(())
}

pub fn create_group(
    name: String, 
    creator: GroupMember, 
    fingerprint: &str
) -> Result<Group> {
    use rand::RngCore;
    
    // Generate random AES key
    let mut key = [0u8; 32];
    aes_gcm::aead::OsRng.fill_bytes(&mut key);
    
    let group = Group {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        created_at: chrono::Utc::now().to_rfc3339(),
        creator_fingerprint: creator.fingerprint.clone(),
        members: vec![creator.clone()],
        admins: vec![creator.fingerprint],
        settings: GroupSettings {
            invite_permission: InvitePermission::AdminsOnly, // Default
            max_members: None,
            disappearing_timer_secs: None,
        },
        symmetric_key: key.to_vec(),
    };
    
    // Load existing, add new, save
    let mut groups = load_groups(fingerprint).unwrap_or_default();
    groups.push(group.clone());
    save_groups(&groups, fingerprint)?;
    
    Ok(group)
}

pub fn delete_group(group_id: &str, fingerprint: &str) -> Result<()> {
    let mut groups = load_groups(fingerprint)?;
    groups.retain(|g| g.id != group_id);
    save_groups(&groups, fingerprint)?;
    Ok(())
}
