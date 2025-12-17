//! CryptoChat Windows Native Client - Modern UI with Chat Bubbles

mod app;
mod keystore;
mod network;
mod qr_exchange;
mod request_store;
mod theme;

use iced::widget::{button, column, container, row, text, text_input, scrollable, Space};
use iced::{Application, Command, Element, Length, Settings, Subscription, Theme, Background, Border, Color};
use std::sync::{Arc, OnceLock, Mutex};
use tokio::sync::mpsc;

static INSTANCE_ID: OnceLock<Option<u32>> = OnceLock::new();
static NETWORK_RECEIVER: OnceLock<Mutex<Option<mpsc::UnboundedReceiver<network::NetworkEvent>>>> = OnceLock::new();

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
}

#[derive(Debug, Clone)]
pub enum Message {
    GenerateKeys,
    KeysGenerated(Result<KeyGenResult, String>),
    UsernameChanged(String),
    CopyKeyShare,
    KeyShareInputChanged(String),
    ImportKeyShare,
    KeyShareImported(Result<ImportResult, String>),
    MessageInputChanged(String),
    SendMessage,
    MessageSent(Result<(), String>),
    NetworkStarted(Result<u16, String>),
    NetworkEvent(network::NetworkEvent),
    PollNetwork,
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
                chat_messages: Vec::new(),
                status: if has_keys { "Set username, then share your key".to_string() } else { "Generate keys".to_string() },
                generating_keys: false,
                listening_port: None,
            },
            init_command,
        )
    }

    fn title(&self) -> String {
        let suffix = get_instance_suffix();
        if let Some(port) = self.listening_port {
            format!("CryptoChat{} - {} - Port {}", suffix, self.my_username, port)
        } else {
            format!("CryptoChat{}", suffix)
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
                        self.app_state.set_peer_address(res.address);
                        self.key_share_input.clear();
                        let peer_name = res.username.as_deref().unwrap_or("Peer");
                        self.status = format!("Connected to {}!", peer_name);
                    }
                    Err(e) => self.status = format!("Import failed: {}", e),
                }
                Command::none()
            }
            Message::MessageInputChanged(value) => {
                self.message_input = value;
                Command::none()
            }
            Message::SendMessage => {
                if self.message_input.trim().is_empty() || !self.recipient_key_imported {
                    return Command::none();
                }
                let content = self.message_input.clone();
                self.message_input.clear();
                self.chat_messages.push(ChatMessage {
                    sender_name: self.my_username.clone(),
                    content: content.clone(),
                    is_mine: true,
                    timestamp: chrono_time(),
                });
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
                                self.chat_messages.push(ChatMessage {
                                    sender_name: name,
                                    content: plaintext,
                                    is_mine: false,
                                    timestamp: chrono_time(),
                                });
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
        Theme::Dark
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
        
        let copy_btn = button(text("Copy Key Share").size(12)).padding([6, 12]).on_press(Message::CopyKeyShare);
        
        let import_section = if self.recipient_key_imported {
            let peer_name = self.peer_username.as_deref().unwrap_or("Connected");
            column![text(format!("Connected to: {}", peer_name)).size(11)]
        } else {
            column![
                text("Paste peer's key:").size(11),
                text_input("{...}", &self.key_share_input).on_input(Message::KeyShareInputChanged).padding(6).size(10),
                button(text("Import").size(11)).padding([4, 10]).on_press(Message::ImportKeyShare),
            ].spacing(4)
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
                copy_btn,
                Space::with_height(12),
                import_section,
            ].padding(10).spacing(2)
        ).width(220).height(Length::Fill);
        
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
            let bubbles: Vec<Element<Message>> = self.chat_messages.iter().map(|msg| {
                self.render_bubble(msg)
            }).collect();
            scrollable(column(bubbles).spacing(8).padding(16)).width(Length::Fill).height(Length::Fill).into()
        };
        
        // Input
        let can_send = self.recipient_key_imported;
        let input_row = if can_send {
            row![
                text_input("Type a message...", &self.message_input)
                    .on_input(Message::MessageInputChanged)
                    .on_submit(Message::SendMessage)
                    .padding(10).size(14),
                button(text("Send")).padding([10, 16]).on_press(Message::SendMessage),
            ].spacing(8).padding(12)
        } else {
            row![
                text_input("Connect first...", "").padding(10).size(14),
                button(text("Send")).padding([10, 16]),
            ].spacing(8).padding(12)
        };
        
        let chat_area = column![
            container(row![text("Chat").size(18), Space::with_width(Length::Fill), text(&self.status).size(10)].padding(10)),
            messages_view,
            input_row,
        ].width(Length::Fill).height(Length::Fill);
        
        row![sidebar, chat_area].width(Length::Fill).height(Length::Fill).into()
    }
    
    fn render_bubble(&self, msg: &ChatMessage) -> Element<Message> {
        let name_label = if msg.is_mine {
            format!("{} (You)", msg.sender_name)
        } else {
            msg.sender_name.clone()
        };
        
        let bubble_content = column![
            text(&name_label).size(11),
            text(&msg.content).size(14),
            text(&msg.timestamp).size(9),
        ].spacing(3);
        
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
    
    CryptoChat::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(900.0, 650.0),
            min_size: Some(iced::Size::new(700.0, 450.0)),
            ..Default::default()
        },
        ..Default::default()
    })
}
