use std::collections::HashMap;

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub sender_name: String,
    pub content: String,
    pub is_mine: bool,
    pub timestamp: String,
    /// Optional image data for inline preview (stored in memory)
    pub image_data: Option<Vec<u8>>,
    /// Filename for images (used for save button)
    pub image_filename: Option<String>,
    /// Emoji reactions: (emoji, sender_name)
    pub reactions: Vec<(String, String)>,
    /// Custom emotes used in this message (name -> hash)
    pub emotes: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String, 
    pub name: String,
    pub messages: Vec<ChatMessage>,
    pub unread_count: usize,
    pub last_activity: u64,
    pub input_draft: String,
    #[serde(skip)]
    pub is_typing: bool,
    pub last_read: Option<String>,
    pub peer_address: Option<String>,
}

impl Conversation {
    pub fn new(id: String, name: String, peer_address: Option<String>) -> Self {
        Self {
            id,
            name,
            messages: Vec::new(),
            unread_count: 0,
            last_activity: 0,
            input_draft: String::new(),
            is_typing: false,
            last_read: None,
            peer_address,
        }
    }
}
