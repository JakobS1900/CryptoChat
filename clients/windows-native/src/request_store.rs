//! Local storage for message requests and contacts using JSON files.
//!
//! This module handles persistence of message requests and the contact list
//! to the local filesystem. In the future, this could be migrated to SQLite.

use anyhow::{Context, Result};
use cryptochat_messaging::requests::{MessageRequest, RequestStatus};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

// Re-export types for use in other modules
pub use cryptochat_messaging::requests::Contact;

/// Storage structure for all message requests
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RequestStore {
    /// Map of request_id -> MessageRequest
    requests: HashMap<String, MessageRequest>,
}

/// Storage structure for contacts
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContactStore {
    /// Map of fingerprint -> Contact
    contacts: HashMap<String, Contact>,
}

/// Get the data directory for CryptoChat
fn get_data_dir() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .context("Could not find home directory")?;

    // Include instance suffix for multi-instance testing
    let dir_name = match crate::get_instance_id() {
        Some(id) => format!(".cryptochat_{}", id),
        None => ".cryptochat".to_string(),
    };
    let data_dir = PathBuf::from(home).join(dir_name);

    // Create directory if it doesn't exist
    if !data_dir.exists() {
        fs::create_dir_all(&data_dir)
            .context("Failed to create data directory")?;
    }

    Ok(data_dir)
}

/// Get path to requests.json
fn get_requests_path() -> Result<PathBuf> {
    Ok(get_data_dir()?.join("requests.json"))
}

/// Get path to contacts.json
fn get_contacts_path() -> Result<PathBuf> {
    Ok(get_data_dir()?.join("contacts.json"))
}

/// Load all message requests from disk
pub fn load_requests() -> Result<Vec<MessageRequest>> {
    let path = get_requests_path()?;

    if !path.exists() {
        return Ok(Vec::new());
    }

    let json = fs::read_to_string(&path)
        .context("Failed to read requests file")?;

    let store: RequestStore = serde_json::from_str(&json)
        .context("Failed to parse requests JSON")?;

    Ok(store.requests.into_values().collect())
}

/// Save a message request to disk
pub fn save_request(request: &MessageRequest) -> Result<()> {
    let path = get_requests_path()?;

    // Load existing requests
    let mut requests = load_requests()?
        .into_iter()
        .map(|r| (r.request_id.to_string(), r))
        .collect::<HashMap<_, _>>();

    // Update or insert the request
    requests.insert(request.request_id.to_string(), request.clone());

    let store = RequestStore { requests };

    // Write to disk
    let json = serde_json::to_string_pretty(&store)
        .context("Failed to serialize requests")?;

    fs::write(&path, json)
        .context("Failed to write requests file")?;

    Ok(())
}

/// Load all pending message requests
pub fn load_pending_requests() -> Result<Vec<MessageRequest>> {
    Ok(load_requests()?
        .into_iter()
        .filter(|r| r.is_pending())
        .collect())
}

/// Delete a message request (used when rejecting)
pub fn delete_request(request_id: &str) -> Result<()> {
    let path = get_requests_path()?;

    if !path.exists() {
        return Ok(());
    }

    // Load existing requests
    let mut requests = load_requests()?
        .into_iter()
        .map(|r| (r.request_id.to_string(), r))
        .collect::<HashMap<_, _>>();

    // Remove the request
    requests.remove(request_id);

    let store = RequestStore { requests };

    // Write to disk
    let json = serde_json::to_string_pretty(&store)
        .context("Failed to serialize requests")?;

    fs::write(&path, json)
        .context("Failed to write requests file")?;

    Ok(())
}

/// Load all contacts from disk
pub fn load_contacts() -> Result<Vec<Contact>> {
    let path = get_contacts_path()?;

    if !path.exists() {
        return Ok(Vec::new());
    }

    let json = fs::read_to_string(&path)
        .context("Failed to read contacts file")?;

    let store: ContactStore = serde_json::from_str(&json)
        .context("Failed to parse contacts JSON")?;

    Ok(store.contacts.into_values().collect())
}

/// Save a contact to disk
pub fn save_contact(contact: &Contact) -> Result<()> {
    let path = get_contacts_path()?;

    // Load existing contacts
    let mut contacts = load_contacts()?
        .into_iter()
        .map(|c| (c.fingerprint.clone(), c))
        .collect::<HashMap<_, _>>();

    // Update or insert the contact
    contacts.insert(contact.fingerprint.clone(), contact.clone());

    let store = ContactStore { contacts };

    // Write to disk
    let json = serde_json::to_string_pretty(&store)
        .context("Failed to serialize contacts")?;

    fs::write(&path, json)
        .context("Failed to write contacts file")?;

    Ok(())
}

/// Check if a fingerprint is in the contact list
pub fn is_contact(fingerprint: &str) -> Result<bool> {
    Ok(load_contacts()?
        .iter()
        .any(|c| c.fingerprint == fingerprint))
}

/// Get a contact by fingerprint
pub fn get_contact(fingerprint: &str) -> Result<Option<Contact>> {
    Ok(load_contacts()?
        .into_iter()
        .find(|c| c.fingerprint == fingerprint))
}

// ============ Chat History ============

/// Stored chat message for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub sender_name: String,
    pub content: String,
    pub is_mine: bool,
    pub timestamp: String,
}

/// Get path to chat_history.json
fn get_history_path() -> Result<PathBuf> {
    Ok(get_data_dir()?.join("chat_history.json"))
}

/// Load chat history from disk
pub fn load_chat_history() -> Result<Vec<StoredMessage>> {
    let path = get_history_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let json = fs::read_to_string(&path).context("Failed to read chat history")?;
    let messages: Vec<StoredMessage> = serde_json::from_str(&json).unwrap_or_default();
    Ok(messages)
}

/// Save chat history to disk
pub fn save_chat_history(messages: &[StoredMessage]) -> Result<()> {
    let path = get_history_path()?;
    let json = serde_json::to_string_pretty(messages)?;
    fs::write(&path, json).context("Failed to save chat history")?;
    Ok(())
}

/// Append a message to chat history
pub fn append_message(msg: &StoredMessage) -> Result<()> {
    let mut history = load_chat_history().unwrap_or_default();
    history.push(msg.clone());
    save_chat_history(&history)
}

// ============ Simple Contacts ============

/// Simple contact for quick reconnection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleContact {
    pub name: String,
    pub fingerprint: String,
    pub public_key: String,
    pub address: String,
}

fn get_simple_contacts_path() -> Result<PathBuf> {
    Ok(get_data_dir()?.join("simple_contacts.json"))
}

/// Load simple contacts
pub fn load_simple_contacts() -> Result<Vec<SimpleContact>> {
    let path = get_simple_contacts_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let json = fs::read_to_string(&path)?;
    let contacts: Vec<SimpleContact> = serde_json::from_str(&json).unwrap_or_default();
    Ok(contacts)
}

/// Save all simple contacts
pub fn save_simple_contacts(contacts: &[SimpleContact]) -> Result<()> {
    let path = get_simple_contacts_path()?;
    let json = serde_json::to_string_pretty(contacts)?;
    fs::write(&path, json)?;
    Ok(())
}

/// Add or update a simple contact
pub fn upsert_simple_contact(contact: &SimpleContact) -> Result<()> {
    let mut contacts = load_simple_contacts().unwrap_or_default();
    // Remove existing with same fingerprint
    contacts.retain(|c| c.fingerprint != contact.fingerprint);
    contacts.push(contact.clone());
    save_simple_contacts(&contacts)
}

/// Update contact name by fingerprint (returns true if updated)
pub fn update_contact_name(fingerprint: &str, new_name: &str) -> Result<bool> {
    let mut contacts = load_simple_contacts().unwrap_or_default();
    let mut updated = false;
    for contact in contacts.iter_mut() {
        if contact.fingerprint == fingerprint && contact.name != new_name {
            contact.name = new_name.to_string();
            updated = true;
            break;
        }
    }
    if updated {
        save_simple_contacts(&contacts)?;
    }
    Ok(updated)
}
