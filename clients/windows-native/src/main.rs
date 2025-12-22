//! CryptoChat Windows Native Client - Modern UI with Chat Bubbles

mod account_store;
mod app;
mod color_store;
mod encrypted_storage;
mod group_store;
mod keystore;
mod network;
mod qr_exchange;
mod request_store;
mod theme;
mod emote_manager;
mod conversation;
mod conversation_store;

use conversation::{ChatMessage, Conversation};

use iced::widget::{button, column, container, row, text, text_input, scrollable, Space, mouse_area};
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
    
    // UI State
    scroll_id: scrollable::Id,
    message_input: String,
    // chat_messages: Vec<ChatMessage>, 
    conversations: std::collections::HashMap<String, Conversation>,
    active_conversation_id: Option<String>,
    
    status: String,
    generating_keys: bool,
    listening_port: Option<u16>,
    /// Saved contacts
    contacts: Vec<request_store::SimpleContact>,
    /// Unread message count for visual notification
    unread_count: usize,
    /// Whether peer is currently typing
    peer_is_typing: bool,
    /// Animation phase for typing dots (0, 1, 2 for ".", "..", "...")
    typing_dots_phase: u8,
    /// Last read timestamp from peer (for ✓✓)
    peer_last_read: Option<String>,
    /// Show emoji picker panel
    show_emoji_picker: bool,
    /// Emoji suggestions for :emoji: autocomplete
    emoji_suggestions: Vec<(&'static str, &'static str)>,
    /// Dark mode enabled (false = light mode)
    dark_mode: bool,
    /// Which message index has reaction picker open (None = closed)
    reaction_picker_for_msg: Option<usize>,
    /// Pending connection requests awaiting user approval
    pending_requests: Vec<PendingRequest>,
    /// List of groups the user is in
    groups: Vec<group_store::Group>,
    /// Group pending deletion (for confirmation dialog)
    pending_group_delete: Option<String>,
    /// Group invite input for joining groups
    group_invite_input: String,
    /// Currently selected group for messaging (None = direct chat)
    selected_group_id: Option<String>,
    /// Password input for login/create account
    password_input: String,
    /// Confirm password input for account creation
    confirm_password_input: String,
    /// Login error message
    login_error: Option<String>,
    
    // Color settings
    /// Show settings modal
    show_settings: bool,
    /// Settings tab (0=Solid, 1=Gradient, 2=Rainbow)
    settings_tab: u8,
    /// Color preferences
    color_prefs: color_store::ColorPreferences,
    /// Rainbow animation offset (0.0 - 1.0)
    rainbow_offset: f32,
    /// Gradient color 1 (for editing)
    gradient_color1: String,
    /// Gradient color 2 (for editing)
    gradient_color2: String,
    
    // Custom Emotes
    emote_manager: emote_manager::EmoteManager,
}

#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Login,
    Onboarding,
    Chat,
}



/// Pending connection request waiting for user approval
#[derive(Debug, Clone)]
pub struct PendingRequest {
    pub sender_fingerprint: String,
    pub sender_public_key: String,
    pub sender_address: String,
    pub sender_name: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmotePayload {
    pub content: String,
    pub emotes: std::collections::HashMap<String, String>,
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
    
    // Emote Upload
    UploadEmote,
    EmoteFileSelected(Option<std::path::PathBuf>),
    
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
    /// Accept a pending connection request (index in pending_requests)
    AcceptRequest(usize),
    /// Decline a pending connection request (index in pending_requests)
    DeclineRequest(usize),
    /// Add current peer to saved contacts
    AddToContacts,
    /// Create a new group chat
    CreateGroup,
    /// Select a group from the list (group_id)
    SelectGroup(String),
    /// Select a conversation (fingerprint)
    SelectConversation(String),
    /// Copy group invite key to clipboard
    CopyGroupKey(String),
    /// Request to delete a group (shows confirmation)
    RequestDeleteGroup(String),
    /// Confirm group deletion
    ConfirmDeleteGroup(String),
    /// Cancel group deletion
    CancelDeleteGroup,
    /// Group invite input changed
    GroupInviteInputChanged(String),
    /// Join a group from invite JSON
    JoinGroup,
    /// Password input changed
    PasswordInputChanged(String),
    /// Confirm password input changed
    ConfirmPasswordChanged(String),
    /// Login attempt
    Login,
    /// Create new account
    CreateAccount,
    /// Login result
    LoginResult(Result<(), String>),
    
    // Settings/Color customization
    /// Toggle settings modal
    ToggleSettings,
    /// Switch settings tab (0=Solid, 1=Gradient, 2=Rainbow)
    SetSettingsTab(u8),
    /// Hue slider changed (0-360)
    SetHue(f32),
    /// Saturation slider changed (0-1)
    SetSaturation(f32),
    /// Set gradient color 1
    SetGradientColor1(String),
    /// Set gradient color 2
    SetGradientColor2(String),
    /// Set rainbow speed
    SetRainbowSpeed(f32),
    /// Set "their" bubble color
    SetTheirBubbleColor(String),
    /// Save color preferences
    SaveColorPrefs,
    /// Tick for rainbow animation
    RainbowTick,
    /// Tick for typing dots animation
    TypingDotsTick,
    
    // Reactions
    /// Show reaction picker for a message (message index)
    ShowReactionPicker(usize),
    /// Add a reaction to a message (message index, emoji)
    AddReaction(usize, String),
    /// Hide reaction picker
    HideReactionPicker,
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
        
        // Check if account exists (needs login)
        let has_account = account_store::account_exists();
        
        // Load keys if account exists (will decrypt after login) or from legacy keystore
        let has_keys = if has_account {
            false // Will load after login
        } else if let Ok(Some(stored_key)) = keystore::load_keypair() {
            // Legacy keys exist - load them but still show account creation to secure them
            if let Ok(keypair) = cryptochat_crypto_core::pgp::PgpKeyPair::from_secret_key(&stored_key.secret_key_armored) {
                if keypair.fingerprint() == stored_key.fingerprint {
                    app_state.set_keypair(keypair);
                    true
                } else { false }
            } else { false }
        } else { false };
        
        // Determine initial view:
        // - has_account → Login (enter password)
        // - has_keys but no account → Login (to create password for existing keys)
        // - no keys → Onboarding (generate new keys)
        let view = if has_account || has_keys { View::Login } else { View::Onboarding };
        let saved_username = request_store::load_username().ok().flatten();
        let default_username = saved_username.unwrap_or_else(|| format!("User{}", get_instance_id().unwrap_or(1)));
        
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
                scroll_id: scrollable::Id::unique(),
                message_input: String::new(),
                conversations: if let Ok(Some(key)) = keystore::load_keypair() {
                     conversation_store::load_conversations(&key.fingerprint).unwrap_or_default()
                } else {
                     std::collections::HashMap::new()
                },
                active_conversation_id: None,
                status: if has_keys { "Set username, then share your key".to_string() } else { "Generate keys".to_string() },
                generating_keys: false,
                listening_port: None,
                contacts: request_store::load_simple_contacts().unwrap_or_default(),
                unread_count: 0,
                peer_is_typing: false,
                typing_dots_phase: 0,
                peer_last_read: None,
                show_emoji_picker: false,
                emoji_suggestions: Vec::new(),
                dark_mode: true,  // Default to dark mode
                reaction_picker_for_msg: None,
                pending_requests: Vec::new(),
                groups: Vec::new(), // Will be loaded when fingerprint available
                pending_group_delete: None,
                group_invite_input: String::new(),
                selected_group_id: None,
                password_input: String::new(),
                confirm_password_input: String::new(),
                login_error: None,
                
                // Color settings - load from disk and apply to theme
                show_settings: false,
                settings_tab: 0,
                color_prefs: {
                    let prefs = color_store::load_preferences();
                    // Set initial bubble color in theme
                    if let color_store::BubbleStyle::Solid { ref color } = prefs.bubble_style {
                        if let Some((r, g, b)) = color_store::hex_to_rgb(color) {
                            theme::set_bubble_color(r, g, b);
                        }
                    } else if let color_store::BubbleStyle::Gradient { ref color1, ref color2 } = prefs.bubble_style {
                        if let Some((r, g, b)) = color_store::hex_to_rgb(color1) {
                            theme::set_bubble_color(r, g, b);
                        }
                        if let Some((r, g, b)) = color_store::hex_to_rgb(color2) {
                            theme::set_gradient_color2(r, g, b);
                        }
                    }
                    // Load "their" bubble color
                    if let Some((r, g, b)) = color_store::hex_to_rgb(&prefs.their_bubble_color) {
                        theme::set_their_bubble_color(r, g, b);
                    }
                    prefs
                },
                rainbow_offset: 0.0,
                gradient_color1: "#ff0000".to_string(),
                gradient_color2: "#0000ff".to_string(),
                
                emote_manager: emote_manager::EmoteManager::new(),
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
                self.my_username = name.clone();
                // Save username to disk for persistence
                let _ = request_store::save_username(&name);
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
                        
                        // Check if already in contacts (avoid duplicate connection)
                        let already_saved = self.contacts.iter().any(|c| c.fingerprint == res.fingerprint);
                        
                        if !already_saved {
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
                        }
                        
                        self.status = format!("Connected to {}!", peer_name);
                        
                        // Send OUR public key to the peer so they can encrypt messages to us
                        // Skip if we're already connected (prevents race conditions)
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
                            let my_fp = self.app_state.get_fingerprint().unwrap_or_default();
                            let port = self.listening_port.unwrap_or(network::DEFAULT_PORT);
                            let envelope = network::MessageEnvelope::TypingIndicator { 
                                is_typing,
                                sender_fingerprint: my_fp,
                                sender_listening_port: port,
                            };
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
                if self.message_input.trim().is_empty() {
                    return Command::none();
                }
                self.unread_count = 0; // Clear unread when user is active
                let content = self.message_input.clone();
                self.message_input.clear();
                
                // Emote parsing
                let mut emotes = std::collections::HashMap::new();
                if let Ok(lib) = self.emote_manager.library.read() {
                   for (name, emote) in lib.iter() {
                       let pattern = format!(":{}:", name);
                       if content.contains(&pattern) {
                           emotes.insert(name.clone(), emote.hash.clone());
                       }
                   }
                }
                
                let network_payload = if !emotes.is_empty() {
                    let payload = EmotePayload {
                        content: content.clone(),
                        emotes: emotes.clone(),
                    };
                    serde_json::to_string(&payload).unwrap_or(content.clone())
                } else {
                    content.clone()
                };

                let new_msg = ChatMessage {
                    sender_name: self.my_username.clone(),
                    content: content.clone(),
                    is_mine: true,
                    timestamp: chrono_time(),
                    image_data: None,
                    image_filename: None,
                    reactions: Vec::new(),
                    emotes: emotes,
                };
                // save_message_to_history(&new_msg); // TODO: Refactor persistence
                
                // Route to group or direct peer
                // Route to group or direct peer
                let group_id_opt = self.selected_group_id.clone();
                if let Some(ref group_id) = group_id_opt {
                    // Add to group conversation
                    self.add_message(group_id.clone(), "Group".to_string(), new_msg.clone(), None);

                    // Group message sending...
                    if let Some(group) = self.groups.iter().find(|g| &g.id == group_id) {
                        let member_addresses: Vec<String> = group.members.iter()
                            .filter(|m| m.fingerprint != self.app_state.get_fingerprint().unwrap_or_default())
                            .map(|m| m.address.clone())
                            .collect();
                        
                        if member_addresses.is_empty() {
                            self.status = "No other members in group yet".to_string();
                            return Command::none();
                        }
                        
                        let username = self.my_username.clone();
                        let group_id_clone = group_id.clone();
                        let fingerprint = self.app_state.get_fingerprint().unwrap_or_default();
                        let envelope = network::MessageEnvelope::GroupMessage {
                            group_id: group_id_clone,
                            sender_fingerprint: fingerprint,
                            sender_name: username,
                            encrypted_content: network_payload, 
                            timestamp: chrono_time(),
                            expires_at: None,
                        };
                        
                        let (sent, failures) = network::NetworkHandle::send_to_group(&member_addresses, envelope);
                        if failures.is_empty() {
                            self.status = format!("Sent to {} members", sent);
                        } else {
                            self.status = format!("Sent to {}/{} members", sent, member_addresses.len());
                        }

                        return self.snap_to_bottom(); // Snap after sending to group
                    } else {
                        self.status = "Group not found".to_string();
                        Command::none()
                    }
                } else if self.recipient_key_imported {
                    // Direct peer message
                    let app_state = self.app_state.clone();
                    let peer_addr = self.peer_address.clone().unwrap();
                    let username = self.my_username.clone();
                    // Get fingerprint for adding to local convo
                    if let Some(fp) = self.app_state.get_recipient_fingerprint() {
                        self.add_message(fp, self.peer_username.clone().unwrap_or("Peer".to_string()), new_msg.clone(), Some(peer_addr.clone()));
                    }
                    
                    let my_fp = self.app_state.get_fingerprint().unwrap_or_default();
                    // Launch async task to send
                    let app_state = self.app_state.clone();
                    let port = self.listening_port.unwrap_or(network::DEFAULT_PORT);
                    
                    return Command::batch(vec![
                        Command::perform(
                            async move {
                                 send_message_async(app_state, peer_addr, network_payload, username, my_fp, port).await
                            },
                            Message::MessageSent,
                        ),
                        self.snap_to_bottom()
                    ]);
                } else {
                    self.status = "Import a key or select a group first".to_string();
                    Command::none()
                }
            }
            Message::MessageSent(result) => {
                if let Err(e) = result {
                    self.status = format!("Send failed: {}", e);
                }
                Command::none()
            }
            Message::NetworkEvent(event) => {
                match event {
                    network::NetworkEvent::MessageReceived { encrypted_payload, sender_name, sender_fingerprint, sender_address } => {
                        match self.app_state.decrypt_message(&encrypted_payload) {
                            Ok(plaintext) => {
                                let name = sender_name.unwrap_or_else(|| 
                                    // Try to find name in contacts if sender_name is missing
                                    self.contacts.iter()
                                        .find(|c| c.public_key.contains(&sender_fingerprint) || c.address.contains(&sender_fingerprint)) // Fuzzy fallback
                                        .map(|c| c.name.clone())
                                        .unwrap_or_else(|| {
                                             if Some(sender_fingerprint.clone()) == self.app_state.get_recipient_fingerprint() {
                                                 self.peer_username.clone().unwrap_or("Peer".to_string())
                                             } else {
                                                 "Unknown".to_string()
                                             }
                                        })
                                );
                                let new_msg = ChatMessage {
                                    sender_name: name.clone(),
                                    content: plaintext.clone(),
                                    is_mine: false,
                                    timestamp: chrono_time(),
                                    image_data: None,
                                    image_filename: None,
                                    reactions: Vec::new(),
                                    emotes: std::collections::HashMap::new(),
                                };
                                // save_message_to_history(&new_msg); // TODO: Refactor persistence
                                self.add_message(sender_fingerprint.clone(), name.clone(), new_msg, Some(sender_address.clone()));
                                
                                // Show notification and play sound
                                show_notification(&format!("Message from {}", name), &plaintext);
                                play_notification_sound();
                                
                                // Reset typing indicator for this user
                                if let Some(conv) = self.conversations.get_mut(&sender_fingerprint) {
                                    conv.is_typing = false;
                                }
                                
                                self.peer_is_typing = false; // Legacy global reset
                                
                                // Send read receipt
                                if let Some(addr) = &self.peer_address {
                                    if Some(sender_fingerprint.clone()) == self.app_state.get_recipient_fingerprint() {
                                        // Only send RR if we are currently looking at this person?
                                        // Or always? Usually only if active.
                                        let ts = chrono_time();
                                        let my_fp = self.app_state.get_fingerprint().unwrap_or_default();
                                        let port = self.listening_port.unwrap_or(network::DEFAULT_PORT);
                                        let envelope = network::MessageEnvelope::ReadReceipt { 
                                            last_read_timestamp: ts,
                                            sender_fingerprint: my_fp,
                                            sender_listening_port: port,
                                        };
                                        let addr = addr.clone();
                                        let _ = std::thread::spawn(move || {
                                            let _ = network::NetworkHandle::send_message(&addr, envelope);
                                        });
                                    }
                                }
                                
                                // Sync username if changed
                                self.peer_username = Some(name.clone());
                                if let Ok(Some(r)) = self.app_state.get_recipient_keypair() {
                                    let fp = r.fingerprint();
                                    // Update in-memory contacts
                                    for c in self.contacts.iter_mut() {
                                        if c.fingerprint == fp && c.name != name {
                                            c.name = name.clone();
                                        }
                                    }
                                    // Update on disk
                                    let _ = request_store::update_contact_name(&fp, &name);
                                }
                                
                                // Sync peer_address for active conversation (enables bidirectional replies)
                                if Some(&sender_fingerprint) == self.active_conversation_id.as_ref() {
                                    self.peer_address = Some(sender_address.clone());
                                    self.app_state.set_peer_address(sender_address.clone());
                                    return self.snap_to_bottom();
                                }
                                Command::none()
                            },
                            Err(e) => {
                                self.status = format!("Decrypt error: {}", e);
                                Command::none()
                            }
                        }
                    }
                    network::NetworkEvent::RequestReceived { sender_fingerprint, sender_public_key, sender_address, sender_name } => {
                        // Add to pending requests instead of auto-connecting
                        let name = sender_name.clone().unwrap_or_else(|| sender_fingerprint[..8].to_string());
                        
                        // Check if we already have this request pending
                        let already_pending = self.pending_requests.iter().any(|r| r.sender_fingerprint == sender_fingerprint);
                        if !already_pending {
                            let pending = PendingRequest {
                                sender_fingerprint: sender_fingerprint.clone(),
                                sender_public_key,
                                sender_address,
                                sender_name,
                                timestamp: chrono_time(),
                            };
                            self.pending_requests.push(pending);
                            show_notification("Connection Request", &format!("{} wants to chat", name));
                            play_notification_sound();
                            self.status = format!("Request from: {} (Accept/Decline)", name);
                        }
                        Command::none()
                    }
                    network::NetworkEvent::TypingUpdate { is_typing, sender_fingerprint, sender_address } => {
                        if let Some(conv) = self.conversations.get_mut(&sender_fingerprint) {
                            conv.is_typing = is_typing;
                            conv.peer_address = Some(sender_address);
                        }
                        Command::none()
                    }
                    network::NetworkEvent::ReadReceiptReceived { last_read_timestamp, sender_fingerprint, sender_address } => {
                        if let Some(conv) = self.conversations.get_mut(&sender_fingerprint) {
                            conv.last_read = Some(last_read_timestamp);
                            conv.peer_address = Some(sender_address);
                        }
                        Command::none()
                    }
                    network::NetworkEvent::FileReceived { filename, encrypted_data, sender_name, sender_fingerprint, sender_address } => {
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
                                            reactions: Vec::new(),
                                            emotes: std::collections::HashMap::new(),
                                        };
                                        
                                        // Don't save to history if it's an image (too large)
                                        // if !is_image { save_message_to_history(&new_msg); } // TODO: Refactor persistence
                                        self.add_message(sender_fingerprint.clone(), name.clone(), new_msg, Some(sender_address.clone()));
                                        
                                        show_notification(&format!("File from {}", name), &format!("Received: {}", filename));
                                        play_notification_sound();
                                        // unread handled by add_message
                                        self.status = format!("Received: {}", filename);
                                    }
                                }
                        }
                    }
                        Command::none()
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
                        Command::none()
                    }
                    network::NetworkEvent::GroupInviteReceived { group_id, group_name, .. } => {
                        // TODO: Implement pending group invites
                        self.status = format!("Received invite to group: {}", group_name);
                        show_notification("New Group Invite", &format!("Invited to {}", group_name));
                        // Later: Add to pending_groups list
                        Command::none()
                    }
                    network::NetworkEvent::GroupMessageReceived { group_id, sender_fingerprint, sender_name, encrypted_content, timestamp, .. } => {
                        // Add received group message to chat
                        // Decrypt group message (TODO: Implement Group Encryption)
                        let new_msg = ChatMessage {
                            sender_name: sender_name.clone(),
                            content: encrypted_content, 
                            is_mine: false,
                            timestamp,
                            image_data: None,
                            image_filename: None,
                            reactions: Vec::new(),
                            emotes: std::collections::HashMap::new(),
                        };
                        // self.chat_messages.push(new_msg);
                        self.add_message(group_id.clone(), "Group".to_string(), new_msg, None);
                        
                        show_notification(&format!("{} ({})", sender_name, "Group"), "New group message");
                        play_notification_sound();
                         // Unread handled in add_message
                        Command::none()
                    }
                    
                    network::NetworkEvent::GroupJoinReceived { group_id, new_member, sender_address } => {
                        // A new member joined - add them to our local group
                        if let Some(group) = self.groups.iter_mut().find(|g| g.id == group_id) {
                            // Check if member already exists
                            if !group.members.iter().any(|m| m.fingerprint == new_member.fingerprint) {
                                let new_name = new_member.username.clone();
                                group.members.push(new_member);
                                
                                // Capture data before releasing mutable borrow
                                let member_count = group.members.len();
                                let members_clone = group.members.clone();
                                let gid = group_id.clone();
                                
                                // Save updated group and send sync response
                                if let Ok(Some(stored_key)) = keystore::load_keypair() {
                                    let all_groups: Vec<_> = self.groups.iter().cloned().collect();
                                    let _ = group_store::save_groups(&all_groups, &stored_key.fingerprint);
                                    
                                    // Send our full member list back to the new joiner
                                    let sync_response = network::MessageEnvelope::GroupMemberSync {
                                        group_id: gid,
                                        members: members_clone,
                                    };
                                    let _ = network::NetworkHandle::send_message(&sender_address, sync_response);
                                    
                                    self.status = format!("{} joined the group ({} members)", new_name, member_count);
                                }
                            }
                        }
                        Command::none()
                    }
                    
                    network::NetworkEvent::GroupMemberSyncReceived { group_id, members } => {
                        // Received member list from another member - merge it with ours
                        if let Some(group) = self.groups.iter_mut().find(|g| g.id == group_id) {
                            let mut updated = false;
                            for member in members {
                                if !group.members.iter().any(|m| m.fingerprint == member.fingerprint) {
                                    group.members.push(member);
                                    updated = true;
                                }
                            }
                            
                            let member_count = group.members.len();
                            if updated {
                                // Save updated group
                                if let Ok(Some(stored_key)) = keystore::load_keypair() {
                                    let all_groups: Vec<_> = self.groups.iter().cloned().collect();
                                    let _ = group_store::save_groups(&all_groups, &stored_key.fingerprint);
                                    self.status = format!("Synced with group - now {} members", member_count);
                                }
                            }
                        }
                        Command::none()
                    }
                    
                    network::NetworkEvent::ReactionReceived { msg_timestamp, emoji, sender_name, sender_fingerprint, sender_address } => {
                        // Find message by timestamp and update reaction (toggle)
                        // Try to find conversation (DM for now - active refactor limitation)
                        if let Some(conv) = self.conversations.get_mut(&sender_fingerprint) {
                            conv.peer_address = Some(sender_address);
                            if let Some(msg) = conv.messages.iter_mut().find(|m| m.timestamp == msg_timestamp) {
                                // Check if sender already reacted with this emoji (toggle off)
                                if let Some(pos) = msg.reactions.iter().position(|(e, s)| e == &emoji && s == &sender_name) {
                                    msg.reactions.remove(pos);
                                } else {
                                    msg.reactions.push((emoji, sender_name));
                                }
                            }
                        }
                        Command::none()
                    }
                    
                    network::NetworkEvent::EmoteRequestReceived { hash, sender_addr_raw } => {
                        use base64::Engine;
                        if let Some(path) = self.emote_manager.get_emote_path(&hash) {
                            if let Ok(data) = std::fs::read(&path) {
                                let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
                                let envelope = network::MessageEnvelope::EmoteData { hash, data: encoded };
                                let _ = std::thread::spawn(move || {
                                    let _ = network::NetworkHandle::send_message(&sender_addr_raw, envelope);
                                });
                            }
                        }
                        Command::none()
                    }
                    
                    network::NetworkEvent::EmoteDataReceived { hash, data } => {
                        use base64::Engine;
                         if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&data) {
                             let _ = self.emote_manager.save_to_cache(&hash, &bytes);
                         }
                        Command::none()
                    }
                    
                    network::NetworkEvent::Error(e) => {
                        self.status = format!("Network: {}", e);
                        Command::none()
                    },
                }
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
                if let Some(conv) = self.get_active_conversation_mut() {
                    conv.messages.clear();
                    // request_store::clear_history(); // TODO: Clear per-user history
                    self.status = "History cleared".to_string();
                } else {
                     self.status = "No active chat to clear".to_string();
                }
                Command::none()
            }
            Message::SelectContact(index) => {
                if let Some(contact) = self.contacts.get(index) {
                     let fp = contact.fingerprint.clone();
                     let name = contact.name.clone();
                     let address = contact.address.clone();
                     
                     // Create conversation if it doesn't exist (using add_message to initialize)
                     if !self.conversations.contains_key(&fp) {
                        // We need a dummy message to init? Or just create entry?
                        // add_message appends message. 
                        // Let's manually create entry to avoid dummy message if possible, or use system message.
                        // Actually add_message is fine.
                        // But wait, add_message signature wants ChatMessage.
                        // I'll manually insert.
                        let conv = Conversation::new(fp.clone(), name, Some(address));
                        self.conversations.insert(fp.clone(), conv);
                     }
                     
                     // Trigger SelectConversation
                     // recursive update call or duplication? Duplication is safer for borrow checker.
                     // Logic of SelectConversation:
                     self.active_conversation_id = Some(fp.clone());
                     
                     // Load contact details
                        if let Ok(keypair) = cryptochat_crypto_core::pgp::PgpKeyPair::from_public_key(&contact.public_key) {
                            self.app_state.set_recipient_keypair(keypair);
                        }
                        self.app_state.set_peer_address(contact.address.clone());
                        self.peer_address = Some(contact.address.clone());
                        self.peer_username = Some(contact.name.clone());
                        self.recipient_key_imported = true;
                        self.status = format!("Chatting with {}", contact.name);
                        self.selected_group_id = None; 
                        
                        return self.snap_to_bottom();
                }
                Command::none()
            }
            Message::SelectConversation(id) => {
                if let Some(conv) = self.conversations.get(&id) {
                     self.active_conversation_id = Some(id.clone());
                     self.peer_username = Some(conv.name.clone());
                     self.peer_address = conv.peer_address.clone();
                     self.selected_group_id = None; 
                     
                     // Try to match with contact to load key
                     if let Some(contact) = self.contacts.iter().find(|c| c.fingerprint == id) {
                         if let Ok(keypair) = cryptochat_crypto_core::pgp::PgpKeyPair::from_public_key(&contact.public_key) {
                             self.app_state.set_recipient_keypair(keypair);
                             self.recipient_key_imported = true;
                         }
                         if let Some(ref addr) = conv.peer_address {
                             self.app_state.set_peer_address(addr.clone());
                         }
                     } else {
                         // Maybe it's a group?
                         if let Some(group) = self.groups.iter().find(|g| g.id == id) {
                             self.selected_group_id = Some(id.clone());
                             // Group logic...
                         } else {
                             // Unknown peer (stranger). Key should be in AppState if imported manually?
                             // But switching away and back might lose it if we rely on AppState only?
                             // Needed: Store keys in Conversation? Or KeyStore? 
                             // Phase 3 stuff. For now, assume if not in contacts, we might lose key on switch if not persisted.
                             // But we load from keystore usually.
                         }
                     }
                     
                     self.status = format!("Chatting with {}", conv.name);
                     
                     return self.snap_to_bottom();
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
                let my_fp = self.app_state.get_fingerprint().unwrap_or_default();
                let port = self.listening_port.unwrap_or(network::DEFAULT_PORT);
                return Command::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            pick_and_send_file(app_state, peer_addr, sender_name, my_fp, port)
                        }).await.map_err(|e| e.to_string())?
                    },
                    |r| Message::FileSent(r),
                );
            }
            Message::UploadEmote => {
                 self.status = "Opening file picker...".to_string();
                 return Command::perform(
                    async {
                        tokio::task::spawn_blocking(|| {
                            pick_emote_file()
                        }).await.map_err(|e| e.to_string())?
                    },
                    |res| match res {
                        Ok(Some(path)) => Message::EmoteFileSelected(Some(path)),
                        _ => Message::EmoteFileSelected(None),
                    }
                );
            }
            Message::EmoteFileSelected(opt_path) => {
                if let Some(path) = opt_path {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        let name = stem.to_string();
                        // Import into manager
                        match self.emote_manager.import_emote(&path, name.clone()) {
                            Ok(_) => self.status = format!("Emote imported: :{} :", name),
                            Err(e) => self.status = format!("Import failed: {}", e),
                        }
                    }
                }
                Command::none()
            }
            Message::FileSent(result) => {
                if let Err(e) = result {
                    self.status = format!("File send failed: {}", e);
                } else if let Ok((filename, raw_data)) = result {
                     if let Some(fp) = self.app_state.get_recipient_fingerprint() {
                        let is_image = filename.to_lowercase().ends_with(".png") || filename.to_lowercase().ends_with(".jpg");
                         
                        let new_msg = ChatMessage {
                            sender_name: self.my_username.clone(),
                            content: if is_image { format!("[Image: {}]", filename) } else { format!("[File: {}]", filename) },
                            is_mine: true,
                            timestamp: chrono_time(),
                            image_data: if is_image { Some(raw_data.clone()) } else { None },
                            image_filename: Some(filename),
                            reactions: Vec::new(),
                            emotes: std::collections::HashMap::new(),
                        };
                        // self.chat_messages.push(new_msg);
                        self.add_message(fp, self.peer_username.clone().unwrap(), new_msg, None);
                     }
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
                if let Some(msg) = self.get_active_messages().get(index) {
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
            Message::AcceptRequest(idx) => {
                if idx < self.pending_requests.len() {
                    let req = self.pending_requests.remove(idx);
                    let name = req.sender_name.clone().unwrap_or_else(|| req.sender_fingerprint[..8].to_string());
                    
                    if let Ok(keypair) = cryptochat_crypto_core::pgp::PgpKeyPair::from_public_key(&req.sender_public_key) {
                        self.app_state.set_recipient_keypair(keypair);
                        self.app_state.set_peer_address(req.sender_address.clone());
                        let peer_addr = req.sender_address.clone();
                        self.peer_address = Some(req.sender_address);
                        self.peer_username = req.sender_name;
                        self.recipient_key_imported = true;
                        self.status = format!("Connected: {}", name);
                        self.view = View::Chat;
                        
                        // Send AcceptedResponse back to requester so they establish connection too
                        if let (Ok(Some(our_key)), Some(port)) = (keystore::load_keypair(), self.listening_port) {
                            let envelope = network::MessageEnvelope::AcceptedResponse {
                                sender_fingerprint: our_key.fingerprint.clone(),
                                sender_public_key: our_key.public_key_armored.clone(),
                                sender_listening_port: port,
                                sender_name: Some(self.my_username.clone()),
                            };
                            return Command::perform(
                                async move {
                                    network::NetworkHandle::send_message(&peer_addr, envelope)
                                        .map_err(|e| e.to_string())
                                },
                                |result| Message::MessageSent(result),
                            );
                        }
                    } else {
                        self.status = format!("Failed to import key from {}", name);
                    }
                }
                Command::none()
            }
            Message::DeclineRequest(idx) => {
                if idx < self.pending_requests.len() {
                    let req = self.pending_requests.remove(idx);
                    let name = req.sender_name.unwrap_or_else(|| req.sender_fingerprint[..8].to_string());
                    self.status = format!("Declined request from {}", name);
                }
                Command::none()
            }
            Message::AddToContacts => {
                // Add current peer to contacts
                if self.recipient_key_imported {
                    if let Ok(Some(recipient)) = self.app_state.get_recipient_keypair() {
                        let fingerprint = recipient.fingerprint();
                        let name = self.peer_username.clone().unwrap_or_else(|| fingerprint[..8].to_string());
                        let address = self.peer_address.clone().unwrap_or_default();
                        
                        // Check if already in contacts
                        let already_saved = self.contacts.iter().any(|c| c.fingerprint == fingerprint);
                        if already_saved {
                            self.status = format!("{} is already in contacts", name);
                        } else {
                            let contact = request_store::SimpleContact {
                                name: name.clone(),
                                fingerprint: fingerprint.clone(),
                                public_key: recipient.export_public_key().unwrap_or_default(),
                                address,
                            };
                            if let Ok(()) = request_store::upsert_simple_contact(&contact) {
                                self.contacts.push(contact);
                                self.status = format!("Added {} to contacts!", name);
                            }
                        }
                    }
                }
                Command::none()
            }
            Message::CreateGroup => {
                // TODO: Show group creation modal
                // For now, create a test group with self as only member
                if let Ok(Some(stored_key)) = keystore::load_keypair() {
                    let me = group_store::GroupMember {
                        fingerprint: stored_key.fingerprint.clone(),
                        username: self.my_username.clone(),
                        public_key: stored_key.public_key_armored.clone(),
                        address: format!("127.0.0.1:{}", self.listening_port.unwrap_or(62780)),
                        joined_at: chrono::Utc::now().to_rfc3339(),
                    };
                    
                    match group_store::create_group(
                        format!("Group {}", self.groups.len() + 1),
                        me,
                        &stored_key.fingerprint,
                    ) {
                        Ok(group) => {
                            self.status = format!("Created group: {}", group.name);
                            // Auto-select the new group so creator can chat immediately
                            self.selected_group_id = Some(group.id.clone());
                            self.groups.push(group);
                        }
                        Err(e) => {
                            self.status = format!("Failed to create group: {}", e);
                        }
                    }
                }
                Command::none()
            }
            Message::SelectGroup(group_id) => {
                // Switch to group chat mode
                if let Some(group) = self.groups.iter().find(|g| g.id == group_id) {
                    self.selected_group_id = Some(group_id);
                    self.status = format!("Chatting in: {}", group.name);
                } else {
                    self.status = "Group not found".to_string();
                }
                Command::none()
            }
            Message::CopyGroupKey(group_id) => {
                // Find the group and create a shareable invite with FULL member list
                if let Some(group) = self.groups.iter().find(|g| g.id == group_id) {
                    // Include full member list so new joiners know everyone
                    let members_data: Vec<serde_json::Value> = group.members.iter().map(|m| {
                        serde_json::json!({
                            "fingerprint": m.fingerprint,
                            "username": m.username,
                            "public_key": m.public_key,
                            "address": m.address,
                        })
                    }).collect();
                    
                    let invite = serde_json::json!({
                        "type": "group_invite",
                        "group_id": group.id,
                        "group_name": group.name,
                        "creator": self.my_username,
                        "members": members_data,
                    });
                    if let Ok(invite_str) = serde_json::to_string(&invite) {
                        let _ = copy_to_clipboard(&invite_str);
                        self.status = format!("Copied invite for '{}' ({} members)", group.name, group.members.len());
                    }
                }
                Command::none()
            }
            Message::RequestDeleteGroup(group_id) => {
                // Set pending deletion - UI will show confirmation
                self.pending_group_delete = Some(group_id);
                Command::none()
            }
            Message::ConfirmDeleteGroup(group_id) => {
                // Actually delete the group
                if let Ok(Some(stored_key)) = keystore::load_keypair() {
                    if let Err(e) = group_store::delete_group(&group_id, &stored_key.fingerprint) {
                        self.status = format!("Delete failed: {}", e);
                    } else {
                        // Remove from memory
                        if let Some(group) = self.groups.iter().find(|g| g.id == group_id) {
                            let name = group.name.clone();
                            self.groups.retain(|g| g.id != group_id);
                            self.status = format!("Deleted group: {}", name);
                        }
                    }
                }
                self.pending_group_delete = None;
                Command::none()
            }
            Message::CancelDeleteGroup => {
                self.pending_group_delete = None;
                Command::none()
            }
            Message::GroupInviteInputChanged(input) => {
                self.group_invite_input = input;
                Command::none()
            }
            Message::JoinGroup => {
                // Parse the invite and add the group locally with ALL members
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&self.group_invite_input) {
                    if json_val.get("type").and_then(|t| t.as_str()) == Some("group_invite") {
                        let group_id = json_val.get("group_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let group_name = json_val.get("group_name").and_then(|v| v.as_str()).unwrap_or("Unknown Group").to_string();
                        let creator = json_val.get("creator").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
                        
                        // Check if already in this group
                        if self.groups.iter().any(|g| g.id == group_id) {
                            self.status = format!("Already in group '{}'", group_name);
                        } else if let Ok(Some(stored_key)) = keystore::load_keypair() {
                            // Parse existing members from invite
                            let mut members: Vec<group_store::GroupMember> = Vec::new();
                            if let Some(members_arr) = json_val.get("members").and_then(|v| v.as_array()) {
                                for m in members_arr {
                                    let member = group_store::GroupMember {
                                        fingerprint: m.get("fingerprint").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                        username: m.get("username").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
                                        public_key: m.get("public_key").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                        address: m.get("address").and_then(|v| v.as_str()).unwrap_or("127.0.0.1:62780").to_string(),
                                        joined_at: chrono::Utc::now().to_rfc3339(),
                                    };
                                    members.push(member);
                                }
                            }
                            
                            // Add self if not already in members
                            let my_fp = stored_key.fingerprint.clone();
                            if !members.iter().any(|m| m.fingerprint == my_fp) {
                                let me = group_store::GroupMember {
                                    fingerprint: my_fp,
                                    username: self.my_username.clone(),
                                    public_key: stored_key.public_key_armored.clone(),
                                    address: format!("127.0.0.1:{}", self.listening_port.unwrap_or(62780)),
                                    joined_at: chrono::Utc::now().to_rfc3339(),
                                };
                                members.push(me);
                            }
                            
                            let member_count = members.len();
                            let group = group_store::Group {
                                id: group_id,
                                name: group_name.clone(),
                                created_at: chrono::Utc::now().to_rfc3339(),
                                creator_fingerprint: creator.clone(),
                                members,
                                admins: vec![creator],
                                settings: group_store::GroupSettings {
                                    invite_permission: group_store::InvitePermission::AdminsOnly,
                                    max_members: None,
                                    disappearing_timer_secs: None,
                                },
                                symmetric_key: vec![0u8; 32], // Placeholder - real key comes from network
                            };
                            
                            // Save to storage
                            let mut groups = group_store::load_groups(&stored_key.fingerprint).unwrap_or_default();
                            groups.push(group.clone());
                            if let Err(e) = group_store::save_groups(&groups, &stored_key.fingerprint) {
                                self.status = format!("Failed to save group: {}", e);
                            } else {
                                // Broadcast join announcement to all OTHER members
                                let my_member_info = group.members.iter()
                                    .find(|m| m.fingerprint == stored_key.fingerprint)
                                    .cloned();
                                
                                if let Some(me) = my_member_info {
                                    let other_members: Vec<String> = group.members.iter()
                                        .filter(|m| m.fingerprint != stored_key.fingerprint)
                                        .map(|m| m.address.clone())
                                        .collect();
                                    
                                    if !other_members.is_empty() {
                                        let announcement = network::MessageEnvelope::GroupJoinAnnouncement {
                                            group_id: group.id.clone(),
                                            new_member: me,
                                        };
                                        let (sent, _) = network::NetworkHandle::send_to_group(&other_members, announcement);
                                        self.status = format!("Joined '{}' - syncing with {} members", group_name, sent);
                                    } else {
                                        self.status = format!("Joined '{}'", group_name);
                                    }
                                }
                                
                                // Auto-select the group so chat is immediately enabled
                                self.selected_group_id = Some(group.id.clone());
                                self.groups.push(group);
                                self.group_invite_input.clear();
                            }
                        }
                    } else {
                        self.status = "Invalid invite: not a group invite".to_string();
                    }
                } else {
                    self.status = "Invalid JSON in invite".to_string();
                }
                Command::none()
            }
            Message::PasswordInputChanged(password) => {
                self.password_input = password;
                self.login_error = None; // Clear error on input
                Command::none()
            }
            Message::ConfirmPasswordChanged(password) => {
                self.confirm_password_input = password;
                self.login_error = None;
                Command::none()
            }
            Message::Login => {
                let password = self.password_input.clone();
                match account_store::login(&password) {
                    Ok((account, secret_key)) => {
                        // Load keypair from decrypted secret key
                        match cryptochat_crypto_core::pgp::PgpKeyPair::from_secret_key(&secret_key) {
                            Ok(keypair) => {
                                self.app_state.set_keypair(keypair);
                                self.my_username = account.username.clone();
                                self.password_input.clear();
                                self.view = View::Chat;
                                self.status = format!("Welcome back, {}!", account.username);
                                // Load groups
                                self.groups = group_store::load_groups(&account.fingerprint).unwrap_or_default();
                                return Command::perform(async { start_network_async().await }, Message::NetworkStarted);
                            }
                            Err(e) => {
                                self.login_error = Some(format!("Key error: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        self.login_error = Some(format!("{}", e));
                    }
                }
                Command::none()
            }
            Message::CreateAccount => {
                if self.password_input != self.confirm_password_input {
                    self.login_error = Some("Passwords don't match".to_string());
                    return Command::none();
                }
                if self.password_input.len() < 4 {
                    self.login_error = Some("Password must be at least 4 characters".to_string());
                    return Command::none();
                }
                
                self.generating_keys = true;
                self.status = "Creating account...".to_string();
                let password = self.password_input.clone();
                let username = self.my_username.clone();
                
                // Check if keypair is already loaded (legacy migration)
                let existing_keypair = self.app_state.get_keypair_for_export();
                
                Command::perform(
                    async move {
                        let (secret_key, public_key, fingerprint) = if let Some((sk, pk, fp)) = existing_keypair {
                            // Use existing keys from legacy keystore
                            (sk, pk, fp)
                        } else {
                            // Generate new keypair
                            let keypair = cryptochat_crypto_core::pgp::PgpKeyPair::generate("CryptoChat User")
                                .map_err(|e| format!("{}", e))?;
                            let sk = keypair.export_secret_key().map_err(|e| format!("{}", e))?;
                            let pk = keypair.export_public_key().map_err(|e| format!("{}", e))?;
                            let fp = keypair.fingerprint();
                            (sk, pk, fp)
                        };
                        
                        // Create account with encrypted key
                        account_store::create_account(&username, &password, &secret_key, &public_key, &fingerprint)
                            .map_err(|e| format!("{}", e))?;
                        
                        // Also save to keystore for backward compat
                        let stored = keystore::StoredKey::new(secret_key, public_key, fingerprint);
                        keystore::save_keypair(&stored).map_err(|e| format!("{}", e))?;
                        
                        Ok(())
                    },
                    Message::LoginResult,
                )
            }
            Message::LoginResult(result) => {
                self.generating_keys = false;
                match result {
                    Ok(()) => {
                        // Account created, now login
                        self.login_error = None;
                        self.confirm_password_input.clear();
                        self.status = "Account created! Logging in...".to_string();
                        // Trigger login
                        return self.update(Message::Login);
                    }
                    Err(e) => {
                        self.login_error = Some(e);
                    }
                }
                Command::none()
            }
            
            // Color settings handlers
            Message::ToggleSettings => {
                self.show_settings = !self.show_settings;
                Command::none()
            }
            Message::SetSettingsTab(tab) => {
                self.settings_tab = tab;
                // Update bubble style based on tab
                match tab {
                    0 => {
                        // Solid - update from hue/saturation
                        let color = color_store::hsl_to_hex(self.color_prefs.hue, self.color_prefs.saturation, 0.5);
                        self.color_prefs.bubble_style = color_store::BubbleStyle::Solid { color };
                    }
                    1 => {
                        // Gradient
                        self.color_prefs.bubble_style = color_store::BubbleStyle::Gradient {
                            color1: self.gradient_color1.clone(),
                            color2: self.gradient_color2.clone(),
                        };
                    }
                    2 => {
                        // Rainbow
                        self.color_prefs.bubble_style = color_store::BubbleStyle::Rainbow { speed: 1.0 };
                    }
                    _ => {}
                }
                Command::none()
            }
            Message::SetHue(hue) => {
                self.color_prefs.hue = hue;
                let color = color_store::hsl_to_hex(hue, self.color_prefs.saturation, 0.5);
                self.color_prefs.bubble_style = color_store::BubbleStyle::Solid { color };
                Command::none()
            }
            Message::SetSaturation(sat) => {
                self.color_prefs.saturation = sat;
                let color = color_store::hsl_to_hex(self.color_prefs.hue, sat, 0.5);
                self.color_prefs.bubble_style = color_store::BubbleStyle::Solid { color };
                Command::none()
            }
            Message::SetGradientColor1(color) => {
                self.gradient_color1 = color.clone();
                self.color_prefs.bubble_style = color_store::BubbleStyle::Gradient {
                    color1: color,
                    color2: self.gradient_color2.clone(),
                };
                Command::none()
            }
            Message::SetGradientColor2(color) => {
                self.gradient_color2 = color.clone();
                self.color_prefs.bubble_style = color_store::BubbleStyle::Gradient {
                    color1: self.gradient_color1.clone(),
                    color2: color,
                };
                Command::none()
            }
            Message::SetRainbowSpeed(speed) => {
                self.color_prefs.bubble_style = color_store::BubbleStyle::Rainbow { speed };
                Command::none()
            }
            Message::SetTheirBubbleColor(color) => {
                self.color_prefs.their_bubble_color = color;
                Command::none()
            }
            Message::SaveColorPrefs => {
                // Apply the color to theme immediately
                // Apply "their" color
                if let Some((r, g, b)) = color_store::hex_to_rgb(&self.color_prefs.their_bubble_color) {
                    theme::set_their_bubble_color(r, g, b);
                }
                
                match &self.color_prefs.bubble_style {
                    color_store::BubbleStyle::Solid { color } => {
                        if let Some((r, g, b)) = color_store::hex_to_rgb(color) {
                            theme::set_bubble_color(r, g, b);
                        }
                    }
                    color_store::BubbleStyle::Gradient { color1, color2 } => {
                        if let Some((r, g, b)) = color_store::hex_to_rgb(color1) {
                            theme::set_bubble_color(r, g, b);
                        }
                        if let Some((r, g, b)) = color_store::hex_to_rgb(color2) {
                            theme::set_gradient_color2(r, g, b);
                        }
                    }
                    color_store::BubbleStyle::Rainbow { .. } => {
                        // Rainbow updates dynamically
                    }
                }
                let _ = color_store::save_preferences(&self.color_prefs);
                self.show_settings = false;
                self.status = "Color preferences saved!".to_string();
                Command::none()
            }
            Message::RainbowTick => {
                // Update rainbow offset for animation
                if let color_store::BubbleStyle::Rainbow { speed } = &self.color_prefs.bubble_style {
                    self.rainbow_offset = (self.rainbow_offset + 0.005 * speed) % 1.0;
                    // Update bubble color with rainbow hue
                    let hue = self.rainbow_offset * 360.0;
                    let color = color_store::hsl_to_hex(hue, 0.8, 0.5);
                    if let Some((r, g, b)) = color_store::hex_to_rgb(&color) {
                        theme::set_bubble_color(r, g, b);
                    }
                }
                Command::none()
            }
            Message::TypingDotsTick => {
                // Cycle typing dots animation phase: 0 -> 1 -> 2 -> 0
                self.typing_dots_phase = (self.typing_dots_phase + 1) % 3;
                Command::none()
            }
            Message::ShowReactionPicker(msg_idx) => {
                self.reaction_picker_for_msg = Some(msg_idx);
                Command::none()
            }
            Message::HideReactionPicker => {
                self.reaction_picker_for_msg = None;
                Command::none()
            }
            Message::AddReaction(msg_idx, emoji) => {
                let my_username_clone = self.my_username.clone();
                if let Some(conv) = self.get_active_conversation_mut() {
                   if let Some(msg) = conv.messages.get_mut(msg_idx) {
                    let msg_timestamp = msg.timestamp.clone();
                    
                    // Check if user already reacted with this emoji (toggle off)
                    if let Some(pos) = msg.reactions.iter().position(|(e, sender)| e == &emoji && sender == &my_username_clone) {
                        msg.reactions.remove(pos);
                    } else {
                        msg.reactions.push((emoji.clone(), my_username_clone.clone()));
                    }
                    
                    // Send reaction to peer
                    if let Some(ref addr) = self.peer_address {
                        let my_fp = self.app_state.get_fingerprint().unwrap_or_default();
                        let port = self.listening_port.unwrap_or(network::DEFAULT_PORT);
                        let envelope = network::MessageEnvelope::Reaction {
                            msg_timestamp,
                            emoji,
                            sender_name: my_username_clone,
                            sender_fingerprint: my_fp,
                            sender_listening_port: port,
                        };
                        let addr = addr.clone();
                        let _ = std::thread::spawn(move || {
                            let _ = network::NetworkHandle::send_message(&addr, envelope);
                        });
                    }
                    }
                }

                self.reaction_picker_for_msg = None;
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let content: Element<Message> = match self.view {
            View::Login => self.view_login(),
            View::Onboarding => self.view_onboarding(),
            View::Chat => {
                row![
                    container(self.view_sidebar()).width(Length::Fixed(250.0)),
                    container(self.view_chat()).width(Length::Fill)
                ].into()
            },
        };
        container(content).width(Length::Fill).height(Length::Fill).into()
    }

    fn subscription(&self) -> Subscription<Message> {
        // Network polling
        let network_sub = iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::PollNetwork);
        
        // Typing dots animation (when peer is typing)
        let typing_sub = if self.peer_is_typing {
            Some(iced::time::every(std::time::Duration::from_millis(400)).map(|_| Message::TypingDotsTick))
        } else {
            None
        };
        
        // Rainbow animation tick (when rainbow mode is active)
        let rainbow_sub = if let color_store::BubbleStyle::Rainbow { speed } = &self.color_prefs.bubble_style {
            // Faster ticks with smaller increments for smoother animation
            let interval = (50.0 / speed) as u64;
            Some(iced::time::every(std::time::Duration::from_millis(interval)).map(|_| Message::RainbowTick))
        } else {
            None
        };
        
        // Combine all active subscriptions
        match (typing_sub, rainbow_sub) {
            (Some(t), Some(r)) => Subscription::batch([network_sub, t, r]),
            (Some(t), None) => Subscription::batch([network_sub, t]),
            (None, Some(r)) => Subscription::batch([network_sub, r]),
            (None, None) => network_sub,
        }
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
    sender_fingerprint: String,
    listening_port: u16,
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
        sender_fingerprint,
        sender_listening_port: listening_port,
    };
    
    network::NetworkHandle::send_message(&peer_addr, envelope)
        .map_err(|e| format!("Send failed: {}", e))?;
    
    // Return filename and raw data for sender's display
    Ok((filename, file_data))
}

fn pick_emote_file() -> Result<Option<std::path::PathBuf>, String> {
    let ps_cmd = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = 'Select Emote Image'
$dialog.Filter = 'Images (*.png;*.jpg;*.jpeg;*.gif)|*.png;*.jpg;*.jpeg;*.gif|All Files (*.*)|*.*'
if ($dialog.ShowDialog() -eq 'OK') { $dialog.FileName } else { '' }
"#;
    
    let output = std::process::Command::new("powershell")
        .args(["-WindowStyle", "Hidden", "-Sta", "-Command", ps_cmd])
        .output()
        .map_err(|e| format!("File picker failed: {}", e))?;
    
    let file_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if file_path.is_empty() {
        return Ok(None);
    }
    Ok(Some(std::path::PathBuf::from(file_path)))
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

// Chat history helpers - uses encrypted storage when fingerprint available
fn load_chat_history_sync() -> Vec<ChatMessage> {
    // Try to get fingerprint for encrypted storage
    let fingerprint = keystore::load_keypair()
        .ok()
        .flatten()
        .map(|k| k.fingerprint.clone());
    
    let stored_messages = if let Some(ref fp) = fingerprint {
        request_store::load_chat_history_encrypted(fp).unwrap_or_default()
    } else {
        // Fallback to old unencrypted format if no keys yet
        request_store::load_chat_history().unwrap_or_default()
    };
    
    stored_messages.into_iter()
        .map(|m| ChatMessage {
            sender_name: m.sender_name,
            content: m.content,
            is_mine: m.is_mine,
            timestamp: m.timestamp,
            image_data: None,  // Images not stored in history
            image_filename: None,
            reactions: Vec::new(),
            emotes: m.emotes,
        })
        .collect()
}

fn save_message_to_history(msg: &ChatMessage) {
    let stored = request_store::StoredMessage {
        sender_name: msg.sender_name.clone(),
        content: msg.content.clone(),
        is_mine: msg.is_mine,
        timestamp: msg.timestamp.clone(),
        expires_at: None, // TODO: Add disappearing timer support
        emotes: msg.emotes.clone(),
    };
    
    // Try to get fingerprint for encrypted storage
    if let Ok(Some(key)) = keystore::load_keypair() {
        let _ = request_store::append_message_encrypted(&stored, &key.fingerprint);
    } else {
        let _ = request_store::append_message(&stored);
    }
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
        // First, check if this is a group invite (has "type": "group_invite")
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&input) {
            if json_val.get("type").and_then(|t| t.as_str()) == Some("group_invite") {
                // This is a group invite - return helpful error
                let group_name = json_val.get("group_name").and_then(|n| n.as_str()).unwrap_or("Unknown");
                return Err(format!("This is a group invite for '{}'. Group invites will be sent over the network once connected. For now, share personal keys first, then receive invites.", group_name));
            }
        }
        
        // Standard key share import
        let key_share: network::KeyShareData = serde_json::from_str(&input).map_err(|e| format!("Invalid JSON: {}", e))?;
        let keypair = cryptochat_crypto_core::pgp::PgpKeyPair::from_public_key(&key_share.public_key).map_err(|e| format!("Invalid key: {}", e))?;
        let fingerprint = keypair.fingerprint();
        app_state.set_recipient_keypair(keypair);
        app_state.set_peer_address(key_share.address.clone());
        Ok(ImportResult { fingerprint, address: key_share.address, username: key_share.username })
    }).await.map_err(|e| format!("{}", e))?
}

async fn send_message_async(app_state: Arc<app::AppState>, peer_address: String, content: String, username: String, sender_fingerprint: String, sender_listening_port: u16) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let encrypted = app_state.encrypt_message(&content).map_err(|e| format!("{}", e))?;
        let envelope = network::MessageEnvelope::RegularMessage { 
            encrypted_payload: encrypted, 
            sender_name: Some(username), 
            sender_fingerprint,
            sender_listening_port,
        };
        network::NetworkHandle::send_message(&peer_address, envelope).map_err(|e| format!("{}", e))
    }).await.map_err(|e| format!("{}", e))?
}

impl CryptoChat {
    fn get_active_messages(&self) -> &[ChatMessage] {
        if let Some(id) = &self.active_conversation_id {
            if let Some(conv) = self.conversations.get(id) {
                return &conv.messages;
            }
        }
        &[]
    }
    
    fn get_active_conversation_mut(&mut self) -> Option<&mut Conversation> {
        if let Some(id) = self.active_conversation_id.as_ref() {
            return self.conversations.get_mut(id);
        }
        None
    }
    
    fn snap_to_bottom(&self) -> Command<Message> {
        scrollable::snap_to(self.scroll_id.clone(), scrollable::RelativeOffset::END)
    }

    fn save_conversations(&self) {
        if let Some(fp) = self.app_state.get_fingerprint() {
            if let Err(e) = conversation_store::save_conversations(&self.conversations, &fp) {
                eprintln!("Failed to save conversations: {}", e);
            }
        }
    }

    fn add_message(&mut self, fingerprint: String, name: String, msg: ChatMessage, peer_address: Option<String>) {
        let active_id = self.active_conversation_id.clone();
        let conv = self.conversations.entry(fingerprint.clone()).or_insert_with(|| {
             Conversation::new(fingerprint.clone(), name, peer_address.clone())
        });
        
        // Update peer address if provided
        if let Some(addr) = peer_address {
            conv.peer_address = Some(addr);
        }
        
        conv.messages.push(msg);
        
        // Update activity timestamp
        conv.last_activity = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        
        // Update unread if not active
        if Some(&fingerprint) != active_id.as_ref() {
            conv.unread_count += 1;
        }
        
        self.save_conversations();
    }

    fn view_login(&self) -> Element<Message> {
        let has_account = account_store::account_exists();
        
        let title = text(if has_account { "Welcome Back" } else { "Create Account" }).size(24);
        let subtitle = text(if has_account { "Enter your password to unlock" } else { "Set a password to protect your keys" }).size(12);
        
        let username_input = column![
            text("Username:").size(11),
            text_input("Your name", &self.my_username)
                .on_input(Message::UsernameChanged)
                .padding(8).size(14),
        ].spacing(4);
        
        let password_input = column![
            text("Password:").size(11),
            text_input("Password", &self.password_input)
                .on_input(Message::PasswordInputChanged)
                .padding(8).size(14),
        ].spacing(4);
        
        let confirm_input: Element<Message> = if !has_account {
            column![
                text("Confirm Password:").size(11),
                text_input("Confirm password", &self.confirm_password_input)
                    .on_input(Message::ConfirmPasswordChanged)
                    .padding(8).size(14),
            ].spacing(4).into()
        } else {
            Space::with_height(0).into()
        };
        
        let error_text: Element<Message> = if let Some(ref error) = self.login_error {
            text(error).size(11).style(iced::theme::Text::Color(Color::from_rgb(1.0, 0.3, 0.3))).into()
        } else {
            Space::with_height(0).into()
        };
        
        let action_btn = if has_account {
            button(text("Login").size(14)).padding([10, 30]).on_press(Message::Login)
        } else {
            if self.generating_keys {
                button(text("Creating...").size(14)).padding([10, 30])
            } else {
                button(text("Create Account").size(14)).padding([10, 30]).on_press(Message::CreateAccount)
            }
        };
        
        let content = column![
            Space::with_height(60),
            text("🔐").size(48).font(EMOJI_FONT),
            title.font(EMOJI_FONT),
            subtitle.font(EMOJI_FONT),
            Space::with_height(20),
            username_input,
            password_input,
            confirm_input,
            error_text,
            Space::with_height(10),
            action_btn,
            Space::with_height(20),
            text(&self.status).size(10),
        ]
        .width(300)
        .spacing(8)
        .align_items(iced::Alignment::Center);
        
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    fn view_onboarding(&self) -> Element<Message> {
        let generate_btn = if self.generating_keys {
            button(text("Generating...").size(18)).padding([12, 24])
        } else {
            button(text("Generate Keys").size(18)).padding([12, 24]).on_press(Message::GenerateKeys)
        };
        column![
            Space::with_height(Length::FillPortion(1)),
            text("CryptoChat").size(48).font(EMOJI_FONT),
            text("Secure P2P Messaging").size(20).font(EMOJI_FONT),
            Space::with_height(40),
            generate_btn,
            Space::with_height(20),
            text(&self.status).size(16),
            Space::with_height(Length::FillPortion(2)),
        ].align_items(iced::Alignment::Center).width(Length::Fill).into()
    }
    
    fn view_sidebar(&self) -> Element<Message> {
        // --- 1. Identity & Config ---
        let fingerprint = self.app_state.get_fingerprint().map(|f| f[..12].to_string()).unwrap_or_default();
        let port = self.listening_port.map(|p| p.to_string()).unwrap_or_else(|| "...".into());

        let username_section = column![
            text("Your Name:").size(11),
            text_input("Username", &self.my_username)
                .on_input(Message::UsernameChanged)
                .padding(6).size(12),
        ].spacing(2);

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

        let theme_label = if self.dark_mode { "Light Mode" } else { "Dark Mode" };
        let theme_btn = button(text(theme_label).size(10)).padding([4, 8]).on_press(Message::ToggleTheme);
        let settings_btn = button(text("⚙ Colors").size(10)).padding([4, 8]).on_press(Message::ToggleSettings);
        let clear_btn = button(text("Clear History").size(10)).padding([4, 8]).on_press(Message::ClearHistory);

        // --- 2. Conversations (Active Chats) ---
        let mut convs: Vec<&Conversation> = self.conversations.values().collect();
        convs.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
        
        let chats_list: Element<Message> = if convs.is_empty() {
            container(text("No active chats").size(12).style(iced::theme::Text::Color(iced::Color::from_rgb(0.7, 0.7, 0.7)))).padding(10).into()
        } else {
            column(
                convs.iter().map(|c| {
                    let is_active = self.active_conversation_id.as_ref() == Some(&c.id);
                    let btn_text = if c.unread_count > 0 {
                        format!("{} ({})", c.name, c.unread_count) 
                    } else {
                        c.name.clone()
                    };
                    
                    let label = if is_active { format!("> {}", btn_text) } else { btn_text };
                    
                    button(text(label).size(12))
                        .width(Length::Fill)
                        .padding(8)
                        .on_press(Message::SelectConversation(c.id.clone()))
                        .into()
                }).collect::<Vec<_>>()
            ).spacing(2).into()
        };

        // --- 3. Requests ---
        let pending_section: Element<Message> = if self.pending_requests.is_empty() {
            Space::with_height(0).into()
        } else {
            let pending_rows: Vec<Element<Message>> = self.pending_requests.iter().enumerate().map(|(i, req)| {
                let name = req.sender_name.clone().unwrap_or_else(|| req.sender_fingerprint[..8].to_string());
                column![
                    text(format!("{} wants to chat", name)).size(10),
                    row![
                        button(text("Accept").size(9)).padding([3, 6]).on_press(Message::AcceptRequest(i)),
                        button(text("Decline").size(9)).padding([3, 6]).on_press(Message::DeclineRequest(i)),
                    ].spacing(4),
                ].spacing(2).into()
            }).collect();
            column![
                text("Pending Requests:").size(10),
                column(pending_rows).spacing(4),
            ].spacing(4).into()
        };

        // --- 4. Contacts ---
        let contacts_section: Element<Message> = if self.contacts.is_empty() {
            text("No saved contacts").size(9).style(iced::theme::Text::Color(iced::Color::from_rgb(0.6,0.6,0.6))).into()
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

        // --- 5. Groups ---
        let groups_section: Element<Message> = {
            let groups_list: Element<Message> = if let Some(ref pending_id) = self.pending_group_delete {
                // Confirmation dialog
                let group_name = self.groups.iter().find(|g| &g.id == pending_id).map(|g| g.name.as_str()).unwrap_or("?");
                column![
                     text(format!("Delete '{}'?", group_name)).size(10),
                     row![
                         button(text("Yes").size(9)).padding([3, 8]).on_press(Message::ConfirmDeleteGroup(pending_id.clone())),
                         button(text("No").size(9)).padding([3, 8]).on_press(Message::CancelDeleteGroup),
                     ].spacing(4),
                ].spacing(4).into()
            } else if self.groups.is_empty() {
                text("No groups yet").size(9).style(iced::theme::Text::Color(iced::Color::from_rgb(0.6,0.6,0.6))).into()
            } else {
                 column(
                    self.groups.iter().map(|g| {
                        row![
                            button(text(&g.name).size(10)).padding([4, 8]).on_press(Message::SelectGroup(g.id.clone())),
                            button(text("📋").size(9)).padding([3, 5]).on_press(Message::CopyGroupKey(g.id.clone())),
                            button(text("X").size(9)).padding([3, 5]).on_press(Message::RequestDeleteGroup(g.id.clone())),
                        ].spacing(2).into()
                    }).collect::<Vec<_>>()
                ).spacing(2).into()
            };
            
            // Group Join Section
            let join_section: Element<Message> = if self.group_invite_input.is_empty() {
                column![
                    text("Join Group:").size(9),
                    text_input("Paste invite...", &self.group_invite_input)
                        .on_input(Message::GroupInviteInputChanged)
                        .padding(4).size(9),
                ].spacing(2).into()
            } else {
                // Parse preview
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&self.group_invite_input) {
                    if json_val.get("type").and_then(|t| t.as_str()) == Some("group_invite") {
                        let name = json_val.get("group_name").and_then(|v| v.as_str()).unwrap_or("?");
                        let creator = json_val.get("creator").and_then(|v| v.as_str()).unwrap_or("?");
                        let members = json_val.get("members").and_then(|v| v.as_i64()).unwrap_or(1);
                        column![
                            text(format!("📌 {}", name)).size(10),
                            text(format!("By: {} • {} members", creator, members)).size(8),
                            row![
                                button(text("Join").size(9)).padding([3, 8]).on_press(Message::JoinGroup),
                                button(text("✕").size(9)).padding([3, 6]).on_press(Message::GroupInviteInputChanged(String::new())),
                            ].spacing(4),
                        ].spacing(3).into()
                    } else {
                        column![
                            text("❌ Invalid").size(9),
                            button(text("Clear").size(8)).padding([2, 6]).on_press(Message::GroupInviteInputChanged(String::new())),
                        ].spacing(2).into()
                    }
                } else {
                    column![
                        text_input("Paste invite...", &self.group_invite_input)
                            .on_input(Message::GroupInviteInputChanged)
                            .padding(4).size(9),
                    ].into()
                }
            };

            column![
                 row![
                     text("Groups:").size(10),
                     button(text("+").size(10)).padding([2, 6]).on_press(Message::CreateGroup),
                 ].spacing(4).align_items(iced::Alignment::Center),
                 groups_list,
                 join_section
            ].spacing(4).into()
        };

        // --- Combine Sidebar ---
        let content = column![
             text("CryptoChat").size(16).font(EMOJI_FONT),
             text(format!("ID: {}...", fingerprint)).size(10),
             text(format!("Port: {}", port)).size(10),
             Space::with_height(10),
             
             username_section,
             Space::with_height(10),
             
             text("Share:").size(10),
             row![copy_btn, copy_qr_btn].spacing(4),
             row![qr_btn].spacing(4),
             Space::with_height(10),
             
             import_section,
             Space::with_height(20),
             
             text("Chats").size(14).font(EMOJI_FONT),
             chats_list,
             Space::with_height(10),
             
             pending_section,
             Space::with_height(10),

             text("Contacts").size(12),
             contacts_section,
             Space::with_height(10),

             groups_section,
             
             Space::with_height(20),
             row![theme_btn, settings_btn, clear_btn].spacing(2),
        ]
        .spacing(4)
        .padding(10);

        scrollable(content).into()
    }

    fn view_chat(&self) -> Element<Message> {
        // Chat bubbles
        let messages_view: Element<Message> = if self.get_active_messages().is_empty() {
            container(
                column![
                    text("How to connect:").size(14),
                    text("1. Set your username in the sidebar").size(12),
                    text("2. Share your key with a peer").size(12),
                    text("3. Import their key").size(12),
                    text("4. Select the chat to start messaging!").size(12),
                ].spacing(4).align_items(iced::Alignment::Center)
            ).width(Length::Fill).height(Length::Fill).center_x().center_y().into()
        } else {
            let bubbles: Vec<Element<Message>> = self.get_active_messages().iter().enumerate().map(|(idx, msg)| {
                self.render_bubble(msg, idx)
            }).collect();
            scrollable(iced::widget::Column::with_children(bubbles).spacing(8).padding(16))
                .id(self.scroll_id.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        };
        
        // Typing indicator with animated dots
        let typing_indicator: Element<Message> = if self.peer_is_typing {
            let name = self.peer_username.as_deref().unwrap_or("Peer");
            let dots = match self.typing_dots_phase {
                0 => "●  ",
                1 => "● ●",
                _ => "●●●",
            };
            let typing_text = format!("  {} {}  {}", dots, name, "is typing");
            container(text(typing_text).size(11).style(iced::theme::Text::Color(Color::from_rgb(0.6, 0.6, 0.65))).font(EMOJI_FONT))
                .padding([4, 8])
                .into()
        } else {
            Space::with_height(0).into()
        };
        
        // Input area
        let can_send = self.recipient_key_imported || self.selected_group_id.is_some();
        let input_area: Element<Message> = if can_send {
            let action_bar: iced::widget::Row<'_, Message> = row![
                button(text("📎").font(EMOJI_FONT)).padding(10).on_press(Message::PickFile),
                button(text("✨").font(EMOJI_FONT)).padding(10).on_press(Message::UploadEmote),
                button(text("[:] Emoji")).padding([6, 10]).on_press(Message::ToggleEmojiPicker),
            ].spacing(8);
            
            let message_row: iced::widget::Row<'_, Message> = row![
                text_input("Type a message...", &self.message_input)
                    .on_input(Message::MessageInputChanged)
                    .on_submit(Message::SendMessage)
                    .padding(10).size(14),
                button(text("Send")).padding([10, 16]).on_press(Message::SendMessage),
            ].spacing(8);
            
            column![action_bar, message_row].spacing(6).padding(12).into()
        } else {
            Space::with_height(0).into()
        };
        
        // Emoji picker
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
        
        // Suggestions
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
        
        // Header
        let peer_name = self.peer_username.clone().unwrap_or_else(|| "Unknown".to_string());
        let peer_in_contacts = self.contacts.iter().any(|c| c.name == peer_name);
        
        let add_contact_btn: Element<Message> = if self.recipient_key_imported && !peer_in_contacts {
            button(text(format!("+ Add {}", peer_name)).size(10)).padding([4, 8]).on_press(Message::AddToContacts).into()
        } else {
            Space::with_width(0).into()
        };
        
        let header_content = row![
            text("Chat").size(18), 
            Space::with_width(8),
            add_contact_btn,
            Space::with_width(Length::Fill), 
            text(&self.status).size(10)
        ].padding(10);
        
        let chat_view = column![
            container(header_content),
            messages_view,
            typing_indicator,
            emoji_picker,
            emoji_suggestions_panel,
            input_area,
        ].width(Length::Fill).height(Length::Fill);
        
        // Settings modal
        if self.show_settings {
             let tab_buttons = row![
                button(text("Solid").size(12)).padding([6, 12]).on_press(Message::SetSettingsTab(0)),
                button(text("Gradient").size(12)).padding([6, 12]).on_press(Message::SetSettingsTab(1)),
                button(text("Rainbow").size(12)).padding([6, 12]).on_press(Message::SetSettingsTab(2)),
            ].spacing(4);
            
            let preview_text = match &self.color_prefs.bubble_style {
                color_store::BubbleStyle::Solid { color } => format!("Preview: {}", color),
                color_store::BubbleStyle::Gradient { color1, color2 } => format!("Gradient: {} → {}", color1, color2),
                color_store::BubbleStyle::Rainbow { speed } => format!("🌈 Rainbow (speed: {:.1}x)", speed),
            };
            
            let tab_content: Element<Message> = match self.settings_tab {
                0 => {
                    column![
                        text("Hue").size(11),
                        text(format!("Current: {:.0}°", self.color_prefs.hue)).size(10),
                        text("Select a preset:").size(10),
                        row![
                            button(text("🔵 Blue").size(12).font(EMOJI_FONT)).padding(4).on_press(Message::SetHue(210.0)),
                            button(text("🟢 Green").size(12).font(EMOJI_FONT)).padding(4).on_press(Message::SetHue(120.0)),
                            button(text("🟣 Purple").size(12).font(EMOJI_FONT)).padding(4).on_press(Message::SetHue(280.0)),
                        ].spacing(4),
                        row![
                            button(text("🟠 Orange").size(12).font(EMOJI_FONT)).padding(4).on_press(Message::SetHue(30.0)),
                            button(text("🔴 Red").size(12).font(EMOJI_FONT)).padding(4).on_press(Message::SetHue(0.0)),
                            button(text("🩷 Pink").size(12).font(EMOJI_FONT)).padding(4).on_press(Message::SetHue(330.0)),
                        ].spacing(4),
                    ].spacing(8).into()
                }
                1 => {
                    column![
                        text("Color 1 (start):").size(11),
                        row![
                            button(text("🔴").font(EMOJI_FONT)).padding(4).on_press(Message::SetGradientColor1("#ff0000".to_string())),
                            button(text("🟠").font(EMOJI_FONT)).padding(4).on_press(Message::SetGradientColor1("#ff9500".to_string())),
                            button(text("🟢").font(EMOJI_FONT)).padding(4).on_press(Message::SetGradientColor1("#32b432".to_string())),
                            button(text("🔵").font(EMOJI_FONT)).padding(4).on_press(Message::SetGradientColor1("#0a84ff".to_string())),
                            button(text("🟣").font(EMOJI_FONT)).padding(4).on_press(Message::SetGradientColor1("#9b59b6".to_string())),
                        ].spacing(4),
                        text("Color 2 (end):").size(11),
                        row![
                            button(text("🔴").font(EMOJI_FONT)).padding(4).on_press(Message::SetGradientColor2("#ff0000".to_string())),
                            button(text("🟠").font(EMOJI_FONT)).padding(4).on_press(Message::SetGradientColor2("#ff9500".to_string())),
                            button(text("🟢").font(EMOJI_FONT)).padding(4).on_press(Message::SetGradientColor2("#32b432".to_string())),
                            button(text("🔵").font(EMOJI_FONT)).padding(4).on_press(Message::SetGradientColor2("#0a84ff".to_string())),
                            button(text("🟣").font(EMOJI_FONT)).padding(4).on_press(Message::SetGradientColor2("#9b59b6".to_string())),
                        ].spacing(4),
                    ].spacing(8).into()
                }
                _ => {
                    column![
                        text("🌈 Rainbow Mode").size(14).font(EMOJI_FONT),
                        text("Bubbles cycle through colors!").size(11),
                        row![
                            button(text("Slow")).padding([4, 8]).on_press(Message::SetRainbowSpeed(0.5)),
                            button(text("Medium")).padding([4, 8]).on_press(Message::SetRainbowSpeed(1.0)),
                            button(text("Fast")).padding([4, 8]).on_press(Message::SetRainbowSpeed(2.0)),
                        ].spacing(4),
                    ].spacing(8).into()
                }
            };
            
            let modal = column![
                row![
                    text("Color Settings").size(16),
                    Space::with_width(Length::Fill),
                    button(text("✕").size(14)).padding(4).on_press(Message::ToggleSettings),
                ],
                tab_buttons,
                text(&preview_text).size(12),
                tab_content,
                text("Incoming Bubble Color:").size(12),
                row![
                    button(text("⚪ Default")).padding([4, 8]).on_press(Message::SetTheirBubbleColor("#2a2a2e".to_string())),
                    button(text("🔴").font(EMOJI_FONT)).padding(4).on_press(Message::SetTheirBubbleColor("#d11a1e".to_string())),
                    button(text("🔵").font(EMOJI_FONT)).padding(4).on_press(Message::SetTheirBubbleColor("#2c7be5".to_string())),
                    button(text("🟢").font(EMOJI_FONT)).padding(4).on_press(Message::SetTheirBubbleColor("#32b432".to_string())),
                    button(text("🟣").font(EMOJI_FONT)).padding(4).on_press(Message::SetTheirBubbleColor("#9b59b6".to_string())),
                    button(text("⚫").font(EMOJI_FONT)).padding(4).on_press(Message::SetTheirBubbleColor("#000000".to_string())),
                ].spacing(8),
                row![
                    button(text("Cancel")).padding([6, 16]).on_press(Message::ToggleSettings),
                    Space::with_width(Length::Fill),
                    button(text("Save")).padding([6, 16]).on_press(Message::SaveColorPrefs),
                ],
            ].spacing(12).padding(16);
            
            container(modal).width(Length::Fill).height(Length::Fill).center_x().center_y().into()
        } else {
            chat_view.into()
        }
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
            let mut content_col = column![
                text(&name_label).size(11),
                text(&msg.content).size(14).font(EMOJI_FONT),
            ];
            
            // Render custom emotes below
            if !msg.emotes.is_empty() {
                let mut ordered_emotes = Vec::new();
                for (name, hash) in &msg.emotes {
                    let pattern = format!(":{}:", name);
                    if let Some(idx) = msg.content.find(&pattern) {
                         ordered_emotes.push((idx, hash));
                    }
                }
                ordered_emotes.sort_by_key(|(idx, _)| *idx);
                
                let mut emotes_row = row![].spacing(6);
                for (_, hash) in ordered_emotes {
                    if let Some(path) = self.emote_manager.get_emote_path(hash) {
                        emotes_row = emotes_row.push(
                             iced::widget::Image::new(path)
                                 .width(Length::Fixed(32.0))
                                 .height(Length::Fixed(32.0))
                        );
                    }
                }
                content_col = content_col.push(emotes_row);
            }
            
            content_col.push(
                row![
                    text(&msg.timestamp).size(9),
                    text(status_indicator).size(9),
                ].spacing(4)
            ).spacing(3).into()
        };
        
        // Build bubble appearance
        // Uses custom color from static storage (set on load and when saving preferences)
        // For gradient mode: alternate between color1 (even) and color2 (odd)
        let is_gradient = matches!(&self.color_prefs.bubble_style, color_store::BubbleStyle::Gradient { .. });
        let bubble_style: fn(&Theme) -> container::Appearance = if msg.is_mine {
            if is_gradient && msg_index % 2 == 1 {
                |_| theme::my_bubble_gradient2()
            } else {
                |_| theme::my_bubble_custom()
            }
        } else {
            |_| theme::their_bubble()
        };
        
        let bubble = container(bubble_content)
            .padding([10, 16])
            .max_width(500) // Max width for responsive layout
            .style(bubble_style);
        
        // Build reactions display row (if any reactions exist)
        // Discord-style: group same emojis and show count as pills
        let reactions_display: Element<Message> = if !msg.reactions.is_empty() {
            // Group reactions by emoji and count
            let mut emoji_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
            for (emoji, _) in &msg.reactions {
                *emoji_counts.entry(emoji.as_str()).or_insert(0) += 1;
            }
            
            // Create pill buttons for each emoji+count
            let pills: Vec<Element<Message>> = emoji_counts.iter().map(|(emoji, count)| {
                // Always show count like Discord (e.g. "❤️ 1")
                let label = format!("{} {}", emoji, count);
                
                container(text(label).size(12).font(EMOJI_FONT))
                    .padding([4, 8]) // Slightly more padding
                    .style(theme::reaction_pill)
                    .into()
            }).collect();
            
            row(pills).spacing(4).into()
        } else {
            Space::with_height(0).into()
        };
        
        // Reaction picker (if open for this message)
        let picker: Element<Message> = if self.reaction_picker_for_msg == Some(msg_index) {
            let emojis = ["❤️", "👍", "😂", "😮", "😢", "🔥"];
            let buttons: Vec<Element<Message>> = emojis.iter().map(|e| {
                button(text(*e).font(EMOJI_FONT).size(18))
                    .padding([4, 8])
                    .on_press(Message::AddReaction(msg_index, e.to_string()))
                    .into()
            }).collect();
            row(buttons).spacing(4).into()
        } else {
            Space::with_height(0).into()
        };
        
        // Combine bubble + reactions + picker
        let bubble_with_reactions = column![
            bubble,
            reactions_display,
            picker,
        ].spacing(2);
        
        // Wrap with mouse_area for right-click reaction picker
        let bubble_interactive = mouse_area(bubble_with_reactions)
            .on_right_press(Message::ShowReactionPicker(msg_index));
        
        // Use row with spacers for better visual balance
        // Mine: small space | bubble | no space (right side)
        // Theirs: no space | bubble | small space (left side)
        if msg.is_mine {
            row![
                Space::with_width(Length::FillPortion(1)), // Left spacer (takes remaining space)
                bubble_interactive,
            ]
            .width(Length::Fill)
            .into()
        } else {
            row![
                bubble_interactive,
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
