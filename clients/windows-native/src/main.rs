//! CryptoChat Windows Native Client - Modern UI with Chat Bubbles

mod app;
mod keystore;
mod network;
mod qr_exchange;
mod request_store;
mod theme;

use iced::widget::{button, column, container, row, text, text_input, scrollable, Space};
use iced::{Application, Command, Element, Font, Length, Settings, Subscription, Theme, Background, Border, Color};
use std::sync::{Arc, OnceLock, Mutex};
use tokio::sync::mpsc;

static INSTANCE_ID: OnceLock<Option<u32>> = OnceLock::new();
static NETWORK_RECEIVER: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<network::NetworkEvent>>>> = OnceLock::new();

/// Font for emoji rendering (Segoe UI Emoji loaded in Settings)
const EMOJI_FONT: Font = Font::with_name("Segoe UI Emoji");

/// Emoji list for :emoji: autocomplete (name, emoji)
const EMOJI_LIST: &[(&str, &str)] = &[
    ("smile", "😀"), ("grin", "😁"), ("joy", "😂"), ("wink", "😉"),
    ("heart_eyes", "😍"), ("kiss", "😘"), ("thinking", "🤔"), ("neutral", "😐"),
    ("sad", "😢"), ("cry", "😭"), ("angry", "😠"), ("cool", "😎"),
    ("thumbsup", "👍"), ("thumbsdown", "👎"), ("clap", "👏"), ("wave", "👋"),
    ("fire", "🔥"), ("heart", "❤️"), ("star", "⭐"), ("party", "🎉"),
    ("check", "✅"), ("x", "❌"), ("100", "💯"), ("pray", "🙏"),
];

pub fn get_instance_id() -> Option<u32> {
    INSTANCE_ID.get().copied().flatten()
}

pub fn get_instance_suffix() -> String {
    get_instance_id().map(|id| format!(" #{}", id)).unwrap_or_default()
}

/// Main application state
pub struct CryptoChat {
    app_state: Arc<app::AppState>,
    view: View,
    /// Our username
    my_username: String,
    /// Peer's username
    peer_username: Option<String>,
    /// Username input field
    username_input: String,
    /// Key share input
    key_share_input: String,
    recipient_key_imported: bool,
    peer_address: Option<String>,
    message_input: String,
    chat_messages: Vec<ChatMessage>,
    status: String,
    generating_keys: bool,
    listening_port: Option<u16>,
    /// Saved contacts
    contacts: Vec<request_store::SimpleContact>,
    /// Unread message count for visual notification
    unread_count: usize,
    /// Whether peer is currently typing
    peer_is_typing: bool,
    /// Last read timestamp from peer (for ✓✓)
    peer_last_read: Option<String>,
    /// Show emoji picker panel
    show_emoji_picker: bool,
    /// Emoji suggestions for :emoji: autocomplete
    emoji_suggestions: Vec<(&'static str, &'static str)>,
    /// Dark mode enabled (false = light mode)
    dark_mode: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Onboarding,
    Chat,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub sender_name: String,
    pub content: String,
    pub is_mine: bool,
    pub timestamp: String,
    /// Optional image data for inline preview (stored in memory)
    pub image_data: Option<Vec<u8>>,
    /// Filename for images (used for save button)
    pub image_filename: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    GenerateKeys,
    KeysGenerated(Result<KeyGenResult, String>),
    UsernameChanged(String),
    CopyKeyShare,
    ShowQR,
    CopyQR,
    ScanQR,
    ScanQRResult(Result<ImportResult, String>),
    KeyShareInputChanged(String),
    ImportKeyShare,
    KeyShareImported(Result<ImportResult, String>),
    MessageInputChanged(String),
    SendMessage,
    MessageSent(Result<(), String>),
    NetworkStarted(Result<u16, String>),
    NetworkEvent(network::NetworkEvent),
    PollNetwork,
    ClearHistory,
    SelectContact(usize),
    PickFile,
    /// Result contains (filename, raw_file_data) for successful sends
    FileSent(Result<(String, Vec<u8>), String>),
    ToggleEmojiPicker,
    InsertEmoji(String),
    /// Select emoji from :emoji: autocomplete (name, emoji)
    SelectEmojiSuggestion(String, String),
    /// Remove a contact (synced to peer)
    RemoveContact(usize),
    /// Save image from inline preview to disk (index in chat_messages)
    SaveImage(usize),
    /// Toggle between light and dark mode
    ToggleTheme,
}

#[derive(Debug, Clone)]
pub struct KeyGenResult {
    pub fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct ImportResult {
    pub fingerprint: String,
    pub address: String,
    pub username: Option<String>,
}

impl Application for CryptoChat {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let app_state = Arc::new(app::AppState::new());
        
        let has_keys = if let Ok(Some(stored_key)) = keystore::load_keypair() {
            if let Ok(keypair) = cryptochat_crypto_core::pgp::PgpKeyPair::from_secret_key(&stored_key.secret_key_armored) {
                if keypair.fingerprint() == stored_key.fingerprint {
                    app_state.set_keypair(keypair);
                    true
                } else { false }
            } else { false }
        } else { false };
        
        let view = if has_keys { View::Chat } else { View::Onboarding };
        let default_username = format!("User{}", get_instance_id().unwrap_or(1));
        
        let init_command = if has_keys {
            Command::perform(async { start_network_async().await }, Message::NetworkStarted)
        } else {
            Command::none()
        };
        
        (
            Self {
                app_state,
                view,
                my_username: default_username,
                peer_username: None,
                username_input: String::new(),
                key_share_input: String::new(),
                recipient_key_imported: false,
                peer_address: None,
                message_input: String::new(),
                chat_messages: load_chat_history_sync(),
                status: if has_keys { "Set username, then share your key".to_string() } else { "Generate keys".to_string() },
                generating_keys: false,
                listening_port: None,
                contacts: request_store::load_simple_contacts().unwrap_or_default(),
                unread_count: 0,
                peer_is_typing: false,
                peer_last_read: None,
                show_emoji_picker: false,
                emoji_suggestions: Vec::new(),
                dark_mode: true,  // Default to dark mode
            },
            init_command,
        )
    }

    fn title(&self) -> String {
        let suffix = get_instance_suffix();
        let unread = if self.unread_count > 0 {
            format!("({}) ", self.unread_count)
        } else {
            String::new()
        };
        if let Some(port) = self.listening_port {
            format!("{}CryptoChat{} - {} - Port {}", unread, suffix, self.my_username, port)
        } else {
            format!("{}CryptoChat{}", unread, suffix)
        }
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::GenerateKeys => {
                if self.generating_keys { return Command::none(); }
                self.generating_keys = true;
                self.status = "Generating keys...".to_string();
                Command::perform(async { generate_keys_async().await }, Message::KeysGenerated)
            }
            Message::KeysGenerated(result) => {
                self.generating_keys = false;
                match result {
                    Ok(res) => {
                        if let Ok(Some(stored_key)) = keystore::load_keypair() {
                            if let Ok(keypair) = cryptochat_crypto_core::pgp::PgpKeyPair::from_secret_key(&stored_key.secret_key_armored) {
                                self.app_state.set_keypair(keypair);
                                self.status = format!("Keys ready!");
                                self.view = View::Chat;
                                return Command::perform(async { start_network_async().await }, Message::NetworkStarted);
                            }
                        }
                    }
                    Err(e) => self.status = format!("Error: {}", e),
                }
                Command::none()
            }
            Message::NetworkStarted(result) => {
                match result {
                    Ok(port) => {
                        self.listening_port = Some(port);
                        self.status = "Ready - Copy & share your key!".to_string();
                    }
                    Err(e) => self.status = format!("Network error: {}", e),
                }
                Command::none()
            }
            Message::UsernameChanged(name) => {
                self.my_username = name;
                Command::none()
            }
            Message::CopyKeyShare => {
                if let (Ok(Some(stored_key)), Some(port)) = (keystore::load_keypair(), self.listening_port) {
                    let key_share = network::KeyShareData {
                        public_key: stored_key.public_key_armored.clone(),
                        address: format!("127.0.0.1:{}", port),
                        username: Some(self.my_username.clone()),
                    };
                    if let Ok(json) = serde_json::to_string(&key_share) {
                        if copy_to_clipboard(&json).is_ok() {
                            self.status = "Key share copied!".to_string();
                        }
                    }
                }
                Command::none()
            }
            Message::KeyShareInputChanged(value) => {
                self.key_share_input = value;
                Command::none()
            }
            Message::ImportKeyShare => {
                if self.key_share_input.trim().is_empty() {
                    self.status = "Paste peer's key share".to_string();
                    return Command::none();
                }
                let input = self.key_share_input.clone();
                let app_state = self.app_state.clone();
                Command::perform(
                    async move { import_key_share_async(app_state, input).await },
                    Message::KeyShareImported,
                )
            }
            Message::KeyShareImported(result) => {
                match result {
                    Ok(res) => {
                        self.recipient_key_imported = true;
                        self.peer_address = Some(res.address.clone());
                        self.peer_username = res.username.clone();
                        self.app_state.set_peer_address(res.address.clone());
                        self.key_share_input.clear();
                        let peer_name = res.username.clone().unwrap_or_else(|| "Peer".to_string());
                        self.status = format!("Connected to {}! Sending our key...", peer_name);
                        
                        // Save contact for future reconnection
                        let contact = request_store::SimpleContact {
                            name: peer_name.clone(),
                            fingerprint: res.fingerprint.clone(),
                            public_key: self.app_state.recipient_keypair.read().unwrap()
                                .as_ref().map(|k| k.export_public_key().unwrap_or_default()).unwrap_or_default(),
                            address: res.address.clone(),
                        };
                        let _ = request_store::upsert_simple_contact(&contact);
                        self.contacts = request_store::load_simple_contacts().unwrap_or_default();
                        
                        // Send OUR public key to the peer so they can encrypt messages to us
                        if let (Ok(Some(our_key)), Some(port)) = (keystore::load_keypair(), self.listening_port) {
                            let envelope = network::MessageEnvelope::AcceptedResponse {
                                sender_fingerprint: our_key.fingerprint.clone(),
                                sender_public_key: our_key.public_key_armored.clone(),
                                sender_listening_port: port,
                                sender_name: Some(self.my_username.clone()),
                            };
                            let peer_addr = res.address.clone();
                            return Command::perform(
                                async move {
                                    network::NetworkHandle::send_message(&peer_addr, envelope)
                                        .map_err(|e| e.to_string())
                                },
                                |result| Message::MessageSent(result),
                            );
                        }
                    }
                    Err(e) => self.status = format!("Import failed: {}", e),
                }
                Command::none()
            }
            Message::MessageInputChanged(value) => {
                let was_empty = self.message_input.is_empty();
                self.message_input = value.clone();
                let is_empty = self.message_input.is_empty();
                
                // Discord-style :emoji: autocomplete
                self.emoji_suggestions.clear();
                if let Some(colon_pos) = value.rfind(':') {
                    let after_colon = &value[colon_pos + 1..];
                    // Only show suggestions if we have at least 2 chars after : and no space
                    if after_colon.len() >= 2 && !after_colon.contains(' ') {
                        let query = after_colon.to_lowercase();
                        self.emoji_suggestions = EMOJI_LIST.iter()
                            .filter(|(name, _)| name.contains(&query.as_str()))
                            .take(8)
                            .copied()
                            .collect();
                    }
                }
                
                // Send typing indicator when user starts/stops typing
                if self.recipient_key_imported {
                    if let Some(addr) = &self.peer_address {
                        let is_typing = !is_empty;
                        if was_empty != is_empty || is_typing {
                            let envelope = network::MessageEnvelope::TypingIndicator { is_typing };
                            let addr = addr.clone();
                            let _ = std::thread::spawn(move || {
                                let _ = network::NetworkHandle::send_message(&addr, envelope);
                            });
                        }
                    }
                }
                Command::none()
            }
            Message::SendMessage => {
                if self.message_input.trim().is_empty() || !self.recipient_key_imported {
                    return Command::none();
                }
                self.unread_count = 0; // Clear unread when user is active
                let content = self.message_input.clone();
                self.message_input.clear();
                let new_msg = ChatMessage {
                    sender_name: self.my_username.clone(),
                    content: content.clone(),
                    is_mine: true,
                    timestamp: chrono_time(),
                    image_data: None,
                    image_filename: None,
                };
                save_message_to_history(&new_msg);
                self.chat_messages.push(new_msg);
                let app_state = self.app_state.clone();
                let peer_addr = self.peer_address.clone().unwrap();
                let username = self.my_username.clone();
                Command::perform(
                    async move { send_message_async(app_state, peer_addr, content, username).await },
                    Message::MessageSent,
                )
            }
            Message::MessageSent(result) => {
                if let Err(e) = result {
                    self.status = format!("Send failed: {}", e);
                }
                Command::none()
            }
            Message::NetworkEvent(event) => {
                match event {
                    network::NetworkEvent::MessageReceived { encrypted_payload, sender_name } => {
                        match self.app_state.decrypt_message(&encrypted_payload) {
                            Ok(plaintext) => {
                                let name = sender_name.unwrap_or_else(|| 
                                    self.peer_username.clone().unwrap_or_else(|| "Peer".to_string())
                                );
                                let new_msg = ChatMessage {
                                    sender_name: name.clone(),
                                    content: plaintext.clone(),
                                    is_mine: false,
                                    timestamp: chrono_time(),
                                    image_data: None,
                                    image_filename: None,
                                };
                                save_message_to_history(&new_msg);
                                self.chat_messages.push(new_msg);
                                
                                // Show notification and play sound
                                show_notification(&format!("Message from {}", name), &plaintext);
                                play_notification_sound();
                                self.unread_count += 1;
                                self.peer_is_typing = false; // They sent, so not typing
                                
                                // Send read receipt
                                if let Some(addr) = &self.peer_address {
                                    let ts = chrono_time();
                                    let envelope = network::MessageEnvelope::ReadReceipt { last_read_timestamp: ts };
                                    let addr = addr.clone();
                                    let _ = std::thread::spawn(move || {
                                        let _ = network::NetworkHandle::send_message(&addr, envelope);
                                    });
                                }
                            }
                            Err(e) => self.status = format!("Decrypt error: {}", e),
                        }
                    }
                    network::NetworkEvent::RequestReceived { sender_fingerprint, sender_public_key, sender_address, sender_name } => {
                        if !self.recipient_key_imported {
                            if let Ok(keypair) = cryptochat_crypto_core::pgp::PgpKeyPair::from_public_key(&sender_public_key) {
                                self.app_state.set_recipient_keypair(keypair);
                                self.app_state.set_peer_address(sender_address.clone());
                                self.peer_address = Some(sender_address);
                                self.peer_username = sender_name.clone();
                                self.recipient_key_imported = true;
                                let name = sender_name.unwrap_or_else(|| sender_fingerprint[..8].to_string());
                                self.status = format!("Auto-connected: {}", name);
                            }
                        }
                    }
                    network::NetworkEvent::TypingUpdate { is_typing } => {
                        self.peer_is_typing = is_typing;
                    }
                    network::NetworkEvent::ReadReceiptReceived { last_read_timestamp } => {
                        self.peer_last_read = Some(last_read_timestamp);
                    }
                    network::NetworkEvent::FileReceived { filename, encrypted_data, sender_name } => {
                        // Decrypt and save file
                        use base64::Engine;
                        if let Ok(Some(stored_key)) = keystore::load_keypair() {
                            if let Ok(keypair) = cryptochat_crypto_core::pgp::PgpKeyPair::from_secret_key(&stored_key.secret_key_armored) {
                                if let Ok(data_bytes) = base64::engine::general_purpose::STANDARD.decode(&encrypted_data) {
                                    if let Ok(decrypted) = keypair.decrypt(&data_bytes) {
                                        let name = sender_name.unwrap_or_else(|| self.peer_username.clone().unwrap_or_else(|| "Peer".to_string()));
                                        
                                        // Check if this is an image file
                                        let is_image = filename.to_lowercase().ends_with(".png") 
                                            || filename.to_lowercase().ends_with(".jpg")
                                            || filename.to_lowercase().ends_with(".jpeg")
                                            || filename.to_lowercase().ends_with(".gif")
                                            || filename.to_lowercase().ends_with(".bmp");
                                        
                                        let new_msg = ChatMessage {
                                            sender_name: name.clone(),
                                            content: if is_image { format!("[Image: {}]", filename) } else { format!("[File: {}]", filename) },
                                            is_mine: false,
                                            timestamp: chrono_time(),
                                            image_data: if is_image { Some(decrypted.clone()) } else { None },
                                            image_filename: Some(filename.clone()),
                                        };
                                        
                                        // Don't save to history if it's an image (too large)
                                        if !is_image {
                                            save_message_to_history(&new_msg);
                                        }
                                        self.chat_messages.push(new_msg);
                                        show_notification(&format!("File from {}", name), &format!("Received: {}", filename));
                                        play_notification_sound();
                                        self.unread_count += 1;
                                        self.peer_is_typing = false;
                                        self.status = format!("Received: {}", filename);
                                    }
                                }
                            }
                        }
                    }
                    network::NetworkEvent::ContactRemovalReceived { fingerprint } => {
                        // Peer removed us as a contact, remove them too
                        if let Some(idx) = self.contacts.iter().position(|c| c.fingerprint == fingerprint) {
                            let contact = self.contacts.remove(idx);
                            let _ = request_store::save_simple_contacts(&self.contacts);
                            self.status = format!("{} removed you as contact", contact.name);
                            
                            // Disconnect if this was current peer
                            if self.peer_address.as_ref() == Some(&contact.address) {
                                self.recipient_key_imported = false;
                                self.peer_address = None;
                                self.peer_username = None;
                            }
                        }
                    }
                    network::NetworkEvent::Error(e) => self.status = format!("Network: {}", e),
                }
                Command::none()
            }
            Message::PollNetwork => {
                if let Some(receiver_mutex) = NETWORK_RECEIVER.get() {
                    if let Ok(mut guard) = receiver_mutex.lock() {
                        if let Some(ref mut receiver) = *guard {
                            if let Ok(event) = receiver.try_recv() {
                                return Command::perform(async move { event }, Message::NetworkEvent);
                            }
                        }
                    }
                }
                Command::none()
            }
            Message::ShowQR => {
                // Generate and show QR code (save to temp file and open)
                if let Some(keypair) = self.app_state.get_keypair() {
                    if let Ok(payload) = qr_exchange::QrPayload::create_and_sign(&keypair) {
                        if let Ok(img) = qr_exchange::generate_qr_image(&payload) {
                            let path = format!("{}/.cryptochat{}/qr_code.png", 
                                std::env::var("USERPROFILE").unwrap_or_default(),
                                get_instance_id().map(|i| format!("_{}", i)).unwrap_or_default());
                            if qr_exchange::save_qr_to_file(&img, &path).is_ok() {
                                let _ = std::process::Command::new("cmd").args(["/C", "start", &path]).spawn();
                                self.status = "QR code opened!".to_string();
                            }
                        }
                    }
                }
                Command::none()
            }
            Message::CopyQR => {
                // Generate QR and copy to clipboard
                if let Some(keypair) = self.app_state.get_keypair() {
                    if let Ok(payload) = qr_exchange::QrPayload::create_and_sign(&keypair) {
                        if let Ok(img) = qr_exchange::generate_qr_image(&payload) {
                            if copy_image_to_clipboard(&img).is_ok() {
                                self.status = "QR copied to clipboard! Paste in other instance".to_string();
                            } else {
                                self.status = "Failed to copy QR".to_string();
                            }
                        }
                    }
                }
                Command::none()
            }
            Message::ScanQR => {
                // Scan QR from clipboard image
                let app_state = self.app_state.clone();
                Command::perform(
                    async move { scan_qr_from_clipboard_async(app_state).await },
                    Message::ScanQRResult,
                )
            }
            Message::ScanQRResult(result) => {
                match result {
                    Ok(res) => {
                        self.recipient_key_imported = true;
                        self.peer_address = Some(res.address.clone());
                        self.peer_username = res.username.clone();
                        self.app_state.set_peer_address(res.address.clone());
                        let name = res.username.as_deref().unwrap_or("Peer");
                        self.status = format!("Imported from QR: {}! Sending our key...", name);
                        
                        // Send OUR public key to the peer
                        if let (Ok(Some(our_key)), Some(port)) = (keystore::load_keypair(), self.listening_port) {
                            let envelope = network::MessageEnvelope::AcceptedResponse {
                                sender_fingerprint: our_key.fingerprint.clone(),
                                sender_public_key: our_key.public_key_armored.clone(),
                                sender_listening_port: port,
                                sender_name: Some(self.my_username.clone()),
                            };
                            let peer_addr = res.address.clone();
                            return Command::perform(
                                async move {
                                    network::NetworkHandle::send_message(&peer_addr, envelope)
                                        .map_err(|e| e.to_string())
                                },
                                |result| Message::MessageSent(result),
                            );
                        }
                    }
                    Err(e) => self.status = format!("QR scan failed: {}", e),
                }
                Command::none()
            }
            Message::ClearHistory => {
                self.chat_messages.clear();
                let _ = request_store::save_chat_history(&[]);
                self.status = "History cleared".to_string();
                Command::none()
            }
            Message::SelectContact(index) => {
                // Guard: Don't allow if already connected
                if self.recipient_key_imported {
                    self.status = "Already connected. Disconnect first.".to_string();
                    return Command::none();
                }
                if let Some(contact) = self.contacts.get(index) {
                    // Set up connection to this contact
                    if let Ok(keypair) = cryptochat_crypto_core::pgp::PgpKeyPair::from_public_key(&contact.public_key) {
                        self.app_state.set_recipient_keypair(keypair);
                        self.app_state.set_peer_address(contact.address.clone());
                        self.peer_address = Some(contact.address.clone());
                        self.peer_username = Some(contact.name.clone());
                        self.recipient_key_imported = true;
                        self.status = format!("Reconnected to {}!", contact.name);
                        
                        // Send our key so they can respond
                        if let (Ok(Some(our_key)), Some(port)) = (keystore::load_keypair(), self.listening_port) {
                            let envelope = network::MessageEnvelope::AcceptedResponse {
                                sender_fingerprint: our_key.fingerprint.clone(),
                                sender_public_key: our_key.public_key_armored.clone(),
                                sender_listening_port: port,
                                sender_name: Some(self.my_username.clone()),
                            };
                            let peer_addr = contact.address.clone();
                            return Command::perform(
                                async move {
                                    network::NetworkHandle::send_message(&peer_addr, envelope)
                                        .map_err(|e| e.to_string())
                                },
                                |r| Message::MessageSent(r),
                            );
                        }
                    }
                }
                Command::none()
            }
            Message::PickFile => {
                if !self.recipient_key_imported {
                    self.status = "Connect to a peer first".to_string();
                    return Command::none();
                }
                // Use PowerShell to open file picker
                let app_state = self.app_state.clone();
                let peer_addr = self.peer_address.clone();
                let sender_name = self.my_username.clone();
                return Command::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            pick_and_send_file(app_state, peer_addr, sender_name)
                        }).await.map_err(|e| e.to_string())?
                    },
                    |r| Message::FileSent(r),
                );
            }
            Message::FileSent(result) => {
                match result {
                    Ok((filename, file_data)) => {
                        self.status = format!("Sent: {}", filename);
                        
                        // Check if this is an image and add to sender's chat
                        let is_image = filename.to_lowercase().ends_with(".png") 
                            || filename.to_lowercase().ends_with(".jpg")
                            || filename.to_lowercase().ends_with(".jpeg")
                            || filename.to_lowercase().ends_with(".gif")
                            || filename.to_lowercase().ends_with(".bmp");
                        
                        let new_msg = ChatMessage {
                            sender_name: self.my_username.clone(),
                            content: if is_image { format!("[Image: {}]", filename) } else { format!("[File: {}]", filename) },
                            is_mine: true,
                            timestamp: chrono_time(),
                            image_data: if is_image { Some(file_data) } else { None },
                            image_filename: Some(filename),
                        };
                        self.chat_messages.push(new_msg);
                    }
                    Err(e) => self.status = format!("Send failed: {}", e),
                }
                Command::none()
            }
            Message::ToggleEmojiPicker => {
                self.show_emoji_picker = !self.show_emoji_picker;
                Command::none()
            }
            Message::InsertEmoji(emoji) => {
                self.message_input.push_str(&emoji);
                self.show_emoji_picker = false;
                Command::none()
            }
            Message::SelectEmojiSuggestion(_name, emoji) => {
                // Replace :text with emoji
                if let Some(colon_pos) = self.message_input.rfind(':') {
                    self.message_input.truncate(colon_pos);
                    self.message_input.push_str(&emoji);
                }
                self.emoji_suggestions.clear();
                Command::none()
            }
            Message::RemoveContact(index) => {
                if let Some(contact) = self.contacts.get(index).cloned() {
                    // Send removal notification to peer
                    let envelope = network::MessageEnvelope::ContactRemoved {
                        fingerprint: contact.fingerprint.clone(),
                    };
                    let peer_addr = contact.address.clone();
                    let _ = std::thread::spawn(move || {
                        let _ = network::NetworkHandle::send_message(&peer_addr, envelope);
                    });
                    
                    // Remove locally
                    self.contacts.remove(index);
                    let _ = request_store::save_simple_contacts(&self.contacts);
                    self.status = format!("Removed {} (synced)", contact.name);
                    
                    // If this was the current peer, disconnect
                    if self.peer_address.as_ref() == Some(&contact.address) {
                        self.recipient_key_imported = false;
                        self.peer_address = None;
                        self.peer_username = None;
                    }
                }
                Command::none()
            }
            Message::SaveImage(index) => {
                // Save image from inline preview to disk
                if let Some(msg) = self.chat_messages.get(index) {
                    if let (Some(data), Some(filename)) = (&msg.image_data, &msg.image_filename) {
                        let downloads_dir = format!("{}\\Downloads", std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Public".to_string()));
                        let _ = std::fs::create_dir_all(&downloads_dir);
                        let save_path = format!("{}\\{}", downloads_dir, filename);
                        match std::fs::write(&save_path, data) {
                            Ok(_) => {
                                self.status = format!("SAVED: {}", filename);
                                show_notification("Image Saved!", &format!("Saved to: {}", save_path));
                            }
                            Err(e) => self.status = format!("Save failed: {}", e),
                        }
                    }
                }
                Command::none()
            }
            Message::ToggleTheme => {
                self.dark_mode = !self.dark_mode;
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let content: Element<Message> = match self.view {
            View::Onboarding => self.view_onboarding(),
            View::Chat => self.view_chat(),
        };
        container(content).width(Length::Fill).height(Length::Fill).into()
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::PollNetwork)
    }

    fn theme(&self) -> Theme {
        if self.dark_mode {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
}

fn chrono_time() -> String {
    // Simple time format
    let now = std::time::SystemTime::now();
    let secs = now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    format!("{:02}:{:02}", hours, mins)
}

fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::Memory::*;
    use windows::Win32::Foundation::*;
    use std::ptr;
    unsafe {
        if OpenClipboard(HWND(ptr::null_mut())).is_err() { return Err("Clipboard error".into()); }
        let _ = EmptyClipboard();
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let hmem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2).map_err(|_| "Alloc")?;
        let ptr = GlobalLock(hmem);
        if !ptr.is_null() {
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
            GlobalUnlock(hmem);
        }
        SetClipboardData(13, HANDLE(hmem.0));
        let _ = CloseClipboard();
    }
    Ok(())
}

/// Pick and send an encrypted file
fn pick_and_send_file(
    app_state: Arc<app::AppState>,
    peer_addr: Option<String>,
    sender_name: String,
) -> Result<(String, Vec<u8>), String> {
    let peer_addr = peer_addr.ok_or("No peer connected")?;
    
    // Use PowerShell to open file picker
    let ps_cmd = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = 'Select file to send'
$dialog.Filter = 'All Files (*.*)|*.*'
if ($dialog.ShowDialog() -eq 'OK') { $dialog.FileName } else { '' }
"#;
    
    let output = std::process::Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-Command", ps_cmd])
        .output()
        .map_err(|e| format!("File picker failed: {}", e))?;
    
    let file_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if file_path.is_empty() {
        return Err("No file selected".into());
    }
    
    // Read file
    let file_data = std::fs::read(&file_path)
        .map_err(|e| format!("Read failed: {}", e))?;
    
    let filename = std::path::Path::new(&file_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    
    // Encrypt with recipient's public key
    let recipient_key = app_state.recipient_keypair.read().unwrap();
    let recipient = recipient_key.as_ref().ok_or("No recipient key")?;
    let encrypted = cryptochat_crypto_core::pgp::PgpKeyPair::encrypt(recipient.cert(), &file_data)
        .map_err(|e| format!("Encrypt failed: {}", e))?;
    
    // Encode as base64
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&encrypted);
    
    // Send file message
    let envelope = network::MessageEnvelope::FileMessage {
        filename: filename.clone(),
        encrypted_data: encoded,
        sender_name: Some(sender_name),
    };
    
    network::NetworkHandle::send_message(&peer_addr, envelope)
        .map_err(|e| format!("Send failed: {}", e))?;
    
    // Return filename and raw data for sender's display
    Ok((filename, file_data))
}

/// Show Windows toast notification
fn show_notification(title: &str, message: &str) {
    let ps_cmd = format!(
        r#"
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
[Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null
$template = @"
<toast>
    <visual>
        <binding template="ToastText02">
            <text id="1">{}</text>
            <text id="2">{}</text>
        </binding>
    </visual>
    <audio src="ms-winsoundevent:Notification.IM"/>
</toast>
"@
$xml = New-Object Windows.Data.Xml.Dom.XmlDocument
$xml.LoadXml($template)
$toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier("CryptoChat").Show($toast)
"#,
        title.replace("\"", "'"),
        message.replace("\"", "'").replace("\n", " ")
    );
    
    let _ = std::process::Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-Command", &ps_cmd])
        .spawn();
}

/// Play notification sound using Windows IM notification
fn play_notification_sound() {
    // Use Windows Media Player to play system notification sound
    let _ = std::process::Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-Command", 
            r#"(New-Object Media.SoundPlayer "C:\Windows\Media\Windows Notify Email.wav").PlaySync()"#])
        .spawn();
}

fn copy_image_to_clipboard(img: &image::ImageBuffer<image::Luma<u8>, Vec<u8>>) -> Result<(), String> {
    // Save to temp file and use Windows to copy (simplest cross-platform approach)
    let path = format!("{}/.cryptochat_qr_temp.png", std::env::var("USERPROFILE").unwrap_or_default());
    img.save(&path).map_err(|e| format!("Save failed: {}", e))?;
    
    // Use PowerShell to copy image to clipboard
    let result = std::process::Command::new("powershell")
        .args(["-Command", &format!(
            "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.Clipboard]::SetImage([System.Drawing.Image]::FromFile('{}'))",
            path.replace("/", "\\")
        )])
        .output();
    
    match result {
        Ok(output) if output.status.success() => Ok(()),
        _ => Err("Clipboard copy failed".into()),
    }
}

async fn scan_qr_from_clipboard_async(app_state: Arc<app::AppState>) -> Result<ImportResult, String> {
    tokio::task::spawn_blocking(move || {
        // Save clipboard image to temp file using PowerShell
        let temp_path = format!("{}/.cryptochat_qr_scan.png", std::env::var("USERPROFILE").unwrap_or_default());
        let ps_cmd = format!(
            "Add-Type -AssemblyName System.Windows.Forms; $img = [System.Windows.Forms.Clipboard]::GetImage(); if ($img) {{ $img.Save('{}') }} else {{ exit 1 }}",
            temp_path.replace("/", "\\")
        );
        
        let result = std::process::Command::new("powershell")
            .args(["-Command", &ps_cmd])
            .output()
            .map_err(|e| format!("PowerShell failed: {}", e))?;
        
        if !result.status.success() {
            return Err("No image in clipboard".to_string());
        }
        
        // Scan QR from file
        let payload = qr_exchange::scan_qr_from_file(&temp_path)
            .map_err(|e| format!("QR scan failed: {}", e))?;
        
        // Import the key
        let keypair = cryptochat_crypto_core::pgp::PgpKeyPair::from_public_key(payload.public_key())
            .map_err(|e| format!("Invalid key: {}", e))?;
        let fingerprint = keypair.fingerprint();
        app_state.set_recipient_keypair(keypair);
        
        // Clean up
        let _ = std::fs::remove_file(&temp_path);
        
        // For QR, address is embedded in the port from our listening port
        // The QR payload doesn't include address, so we need to get it from clipboard text or manual entry
        // For now, use localhost with a placeholder
        Ok(ImportResult {
            fingerprint,
            address: "127.0.0.1:62780".to_string(), // Default, will be replaced
            username: None,
        })
    }).await.map_err(|e| format!("{}", e))?
}

// Chat history helpers
fn load_chat_history_sync() -> Vec<ChatMessage> {
    request_store::load_chat_history()
        .unwrap_or_default()
        .into_iter()
        .map(|m| ChatMessage {
            sender_name: m.sender_name,
            content: m.content,
            is_mine: m.is_mine,
            timestamp: m.timestamp,
            image_data: None,  // Images not stored in history
            image_filename: None,
        })
        .collect()
}

fn save_message_to_history(msg: &ChatMessage) {
    let stored = request_store::StoredMessage {
        sender_name: msg.sender_name.clone(),
        content: msg.content.clone(),
        is_mine: msg.is_mine,
        timestamp: msg.timestamp.clone(),
    };
    let _ = request_store::append_message(&stored);
}

async fn start_network_async() -> Result<u16, String> {
    let (sender, receiver) = mpsc::unbounded_channel();
    let _ = NETWORK_RECEIVER.set(Mutex::new(Some(receiver)));
    let handle = network::NetworkHandle::start_with_sender(sender).map_err(|e| format!("{}", e))?;
    Ok(handle.port())
}

async fn generate_keys_async() -> Result<KeyGenResult, String> {
    tokio::task::spawn_blocking(|| {
        let keypair = cryptochat_crypto_core::pgp::PgpKeyPair::generate("CryptoChat User").map_err(|e| format!("{}", e))?;
        let fingerprint = keypair.fingerprint();
        let public_key = keypair.export_public_key().map_err(|e| format!("{}", e))?;
        let secret_key = keypair.export_secret_key().map_err(|e| format!("{}", e))?;
        let stored = keystore::StoredKey { fingerprint: fingerprint.clone(), public_key_armored: public_key, secret_key_armored: secret_key };
        keystore::save_keypair(&stored).map_err(|e| format!("{}", e))?;
        Ok(KeyGenResult { fingerprint })
    }).await.map_err(|e| format!("{}", e))?
}

async fn import_key_share_async(app_state: Arc<app::AppState>, input: String) -> Result<ImportResult, String> {
    tokio::task::spawn_blocking(move || {
        let key_share: network::KeyShareData = serde_json::from_str(&input).map_err(|e| format!("Invalid JSON: {}", e))?;
        let keypair = cryptochat_crypto_core::pgp::PgpKeyPair::from_public_key(&key_share.public_key).map_err(|e| format!("Invalid key: {}", e))?;
        let fingerprint = keypair.fingerprint();
        app_state.set_recipient_keypair(keypair);
        app_state.set_peer_address(key_share.address.clone());
        Ok(ImportResult { fingerprint, address: key_share.address, username: key_share.username })
    }).await.map_err(|e| format!("{}", e))?
}

async fn send_message_async(app_state: Arc<app::AppState>, peer_address: String, content: String, username: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let encrypted = app_state.encrypt_message(&content).map_err(|e| format!("{}", e))?;
        let envelope = network::MessageEnvelope::RegularMessage { encrypted_payload: encrypted, sender_name: Some(username) };
        network::NetworkHandle::send_message(&peer_address, envelope).map_err(|e| format!("{}", e))
    }).await.map_err(|e| format!("{}", e))?
}

impl CryptoChat {
    fn view_onboarding(&self) -> Element<Message> {
        let generate_btn = if self.generating_keys {
            button(text("Generating...").size(18)).padding([12, 24])
        } else {
            button(text("Generate Keys").size(18)).padding([12, 24]).on_press(Message::GenerateKeys)
        };
        column![
            Space::with_height(Length::FillPortion(1)),
            text("CryptoChat").size(48),
            text("Secure P2P Messaging").size(20),
            Space::with_height(40),
            generate_btn,
            Space::with_height(20),
            text(&self.status).size(16),
            Space::with_height(Length::FillPortion(2)),
        ].align_items(iced::Alignment::Center).width(Length::Fill).into()
    }
    
    fn view_chat(&self) -> Element<Message> {
        // Sidebar
        let fingerprint = self.app_state.get_fingerprint().map(|f| f[..12].to_string()).unwrap_or_default();
        let port = self.listening_port.map(|p| p.to_string()).unwrap_or_else(|| "...".into());
        
        let username_section = column![
            text("Your Name:").size(11),
            text_input("Username", &self.my_username)
                .on_input(Message::UsernameChanged)
                .padding(6).size(12),
        ];
        
        let copy_btn = button(text("Copy Key").size(11)).padding([5, 8]).on_press(Message::CopyKeyShare);
        let qr_btn = button(text("Show QR").size(11)).padding([5, 8]).on_press(Message::ShowQR);
        let copy_qr_btn = button(text("Copy QR").size(11)).padding([5, 8]).on_press(Message::CopyQR);
        let scan_qr_btn = button(text("Scan QR").size(11)).padding([5, 8]).on_press(Message::ScanQR);
        
        let import_section = if self.recipient_key_imported {
            let peer_name = self.peer_username.as_deref().unwrap_or("Connected");
            column![text(format!("Connected to: {}", peer_name)).size(11)]
        } else {
            column![
                text("Paste peer's key:").size(11),
                text_input("{...}", &self.key_share_input).on_input(Message::KeyShareInputChanged).padding(6).size(10),
                row![
                    button(text("Import JSON").size(10)).padding([4, 8]).on_press(Message::ImportKeyShare),
                    scan_qr_btn,
                ].spacing(4),
            ].spacing(4)
        };
        
        let clear_btn = button(text("Clear History").size(10)).padding([4, 8]).on_press(Message::ClearHistory);
        let theme_label = if self.dark_mode { "Light Mode" } else { "Dark Mode" };
        let theme_btn = button(text(theme_label).size(10)).padding([4, 8]).on_press(Message::ToggleTheme);
        
        // Build contacts list with delete buttons
        let contacts_section: Element<Message> = if self.contacts.is_empty() {
            text("No saved contacts").size(9).into()
        } else {
            let contact_rows: Vec<Element<Message>> = self.contacts.iter().enumerate().map(|(i, c)| {
                row![
                    button(text(&c.name).size(10))
                        .padding([4, 8])
                        .on_press(Message::SelectContact(i)),
                    button(text("X").size(9))
                        .padding([4, 6])
                        .on_press(Message::RemoveContact(i)),
                ].spacing(4).into()
            }).collect();
            column(contact_rows).spacing(2).into()
        };
        
        let sidebar = container(
            column![
                text("CryptoChat").size(16),
                Space::with_height(8),
                text(format!("ID: {}...", fingerprint)).size(10),
                text(format!("Port: {}", port)).size(10),
                Space::with_height(10),
                username_section,
                Space::with_height(10),
                text("Share your key:").size(10),
                row![copy_btn, copy_qr_btn].spacing(4),
                row![qr_btn].spacing(4),
                Space::with_height(12),
                import_section,
                Space::with_height(12),
                text("Contacts:").size(10),
                contacts_section,
                Space::with_height(Length::Fill),
                row![theme_btn, clear_btn].spacing(4),
            ].padding(10).spacing(2)
        ).width(260).height(Length::Fill);
        
        // Chat bubbles
        let messages_view: Element<Message> = if self.chat_messages.is_empty() {
            container(
                column![
                    text("How to connect:").size(14),
                    text("1. Set your username above").size(12),
                    text("2. Copy Key Share").size(12),
                    text("3. Send to peer").size(12),
                    text("4. Import peer's key").size(12),
                ].spacing(4).align_items(iced::Alignment::Center)
            ).width(Length::Fill).height(Length::Fill).center_x().center_y().into()
        } else {
            let bubbles: Vec<Element<Message>> = self.chat_messages.iter().enumerate().map(|(idx, msg)| {
                self.render_bubble(msg, idx)
            }).collect();
            scrollable(column(bubbles).spacing(8).padding(16)).width(Length::Fill).height(Length::Fill).into()
        };
        
        // Typing indicator
        let typing_indicator: Element<Message> = if self.peer_is_typing {
            let name = self.peer_username.as_deref().unwrap_or("Peer");
            container(text(format!("{} is typing...", name)).size(12))
                .padding([4, 16])
                .into()
        } else {
            Space::with_height(0).into()
        };
        
        // Input area with action bar
        let can_send = self.recipient_key_imported;
        let input_area = if can_send {
            // Action bar with buttons
            let action_bar = row![
                button(text("[+] File")).padding([6, 10]).on_press(Message::PickFile),
                button(text("[:] Emoji")).padding([6, 10]).on_press(Message::ToggleEmojiPicker),
            ].spacing(8);
            
            // Message input row
            let message_row = row![
                text_input("Type a message...", &self.message_input)
                    .on_input(Message::MessageInputChanged)
                    .on_submit(Message::SendMessage)
                    .padding(10).size(14),
                button(text("Send")).padding([10, 16]).on_press(Message::SendMessage),
            ].spacing(8);
            
            column![action_bar, message_row].spacing(6).padding(12)
        } else {
            let message_row = row![
                text_input("Connect first...", "").padding(10).size(14),
                button(text("Send")).padding([10, 16]),
            ].spacing(8);
            column![message_row].padding(12)
        };
        
        // Emoji picker panel - using EMOJI_FONT for rendering
        let emoji_picker: Element<Message> = if self.show_emoji_picker {
            let emojis = ["😀", "😂", "😢", "😎", "🤔", "❤️", "👍", "👎", 
                          "🔥", "⭐", "🎉", "👋", "✅", "❌", "💯", "🙏"];
            let emoji_buttons: Vec<Element<Message>> = emojis.iter().map(|e| {
                button(text(*e).size(20).font(EMOJI_FONT))
                    .padding([6, 10])
                    .on_press(Message::InsertEmoji(e.to_string()))
                    .into()
            }).collect();
            container(
                row(emoji_buttons).spacing(4).padding(8)
            ).into()
        } else {
            Space::with_height(0).into()
        };
        
        // Discord-style :emoji: suggestions dropdown
        let emoji_suggestions_panel: Element<Message> = if !self.emoji_suggestions.is_empty() {
            let suggestion_items: Vec<Element<Message>> = self.emoji_suggestions.iter().map(|(name, emoji)| {
                button(
                    row![
                        text(*emoji).size(16).font(EMOJI_FONT),
                        text(format!(":{name}:")).size(12),
                    ].spacing(8)
                )
                .padding([6, 12])
                .on_press(Message::SelectEmojiSuggestion(name.to_string(), emoji.to_string()))
                .into()
            }).collect();
            container(
                column(suggestion_items).spacing(2).padding(8)
            ).into()
        } else {
            Space::with_height(0).into()
        };
        
        // Simple chat header with status
        let header_content = row![
            text("Chat").size(18), 
            Space::with_width(Length::Fill), 
            text(&self.status).size(10)
        ].padding(10);
        
        let chat_area = column![
            container(header_content),
            messages_view,
            typing_indicator,
            emoji_picker,
            emoji_suggestions_panel,
            input_area,
        ].width(Length::Fill).height(Length::Fill);
        
        row![sidebar, chat_area].width(Length::Fill).height(Length::Fill).into()
    }
    
    fn render_bubble(&self, msg: &ChatMessage, msg_index: usize) -> Element<Message> {
        let name_label = if msg.is_mine {
            format!("{} (You)", msg.sender_name)
        } else {
            msg.sender_name.clone()
        };
        
        // Add read receipt indicators for sent messages
        let status_indicator = if msg.is_mine {
            // Compare timestamps to determine if message was read
            let is_read = self.peer_last_read.as_ref()
                .map(|lr| lr >= &msg.timestamp)
                .unwrap_or(false);
            if is_read {
                " [read]" // Double check = read
            } else {
                " [sent]"  // Single check = sent
            }
        } else {
            ""
        };
        
        // Build content based on whether this is an image message
        let bubble_content: Element<Message> = if let Some(ref image_bytes) = msg.image_data {
            // Render inline image with Save button
            let img_handle = iced::widget::image::Handle::from_memory(image_bytes.clone());
            let img_widget = iced::widget::Image::new(img_handle)
                .width(Length::Fixed(200.0))
                .height(Length::Shrink);
            
            column![
                text(&name_label).size(11),
                img_widget,
                row![
                    button(text("Save")).padding([4, 8]).on_press(Message::SaveImage(msg_index)),
                    Space::with_width(8),
                    text(&msg.timestamp).size(9),
                    text(status_indicator).size(9),
                ].spacing(4),
            ].spacing(3).into()
        } else {
            // Regular text message - use EMOJI_FONT for emoji support
            column![
                text(&name_label).size(11),
                text(&msg.content).size(14).font(EMOJI_FONT),
                row![
                    text(&msg.timestamp).size(9),
                    text(status_indicator).size(9),
                ].spacing(4),
            ].spacing(3).into()
        };
        
        let bubble_style: fn(&Theme) -> container::Appearance = if msg.is_mine {
            |_| theme::my_bubble()
        } else {
            |_| theme::their_bubble()
        };
        
        let bubble = container(bubble_content)
            .padding([10, 16])
            .style(bubble_style);
        
        // Use row with spacers for better visual balance
        // Mine: small space | bubble | no space (right side)
        // Theirs: no space | bubble | small space (left side)
        if msg.is_mine {
            row![
                Space::with_width(Length::FillPortion(1)), // Left spacer (takes remaining space)
                bubble,
            ]
            .width(Length::Fill)
            .into()
        } else {
            row![
                bubble,
                Space::with_width(Length::FillPortion(1)), // Right spacer
            ]
            .width(Length::Fill)
            .into()
        }
    }
}

fn main() -> iced::Result {
    let args: Vec<String> = std::env::args().collect();
    let mut instance_id: Option<u32> = None;
    for i in 0..args.len() {
        if args[i] == "--instance" && i + 1 < args.len() {
            if let Ok(id) = args[i + 1].parse::<u32>() { instance_id = Some(id); }
        }
    }
    INSTANCE_ID.set(instance_id).ok();
    
    // Load Segoe UI Emoji font for emoji support
    let emoji_font_bytes: &'static [u8] = include_bytes!("C:/Windows/Fonts/seguiemj.ttf");
    
    CryptoChat::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(900.0, 650.0),
            min_size: Some(iced::Size::new(700.0, 450.0)),
            ..Default::default()
        },
        fonts: vec![std::borrow::Cow::Borrowed(emoji_font_bytes)],
        ..Default::default()
    })
}
