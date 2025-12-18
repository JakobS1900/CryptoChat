//! P2P networking with usernames and channel-based message delivery

use anyhow::Result;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use serde::{Serialize, Deserialize};
use tokio::sync::mpsc;

pub const DEFAULT_PORT: u16 = 62780;

/// Events sent from network to UI
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    MessageReceived { 
        encrypted_payload: String,
        sender_name: Option<String>,
    },
    RequestReceived {
        sender_fingerprint: String,
        sender_public_key: String,
        sender_address: String,
        sender_name: Option<String>,
    },
    TypingUpdate {
        is_typing: bool,
    },
    ReadReceiptReceived {
        last_read_timestamp: String,
    },
    FileReceived {
        filename: String,
        encrypted_data: String,
        sender_name: Option<String>,
    },
    ContactRemovalReceived {
        fingerprint: String,
    },
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageEnvelope {
    Request {
        sender_fingerprint: String,
        sender_public_key: String,
        sender_device_id: String,
        sender_listening_port: u16,
        first_message: String,
        sender_name: Option<String>,
    },
    AcceptedResponse {
        sender_fingerprint: String,
        sender_public_key: String,
        sender_listening_port: u16,
        sender_name: Option<String>,
    },
    RegularMessage {
        encrypted_payload: String,
        sender_name: Option<String>,
    },
    /// Typing indicator (true = started typing, false = stopped)
    TypingIndicator {
        is_typing: bool,
    },
    /// Read receipt for message acknowledgment  
    ReadReceipt {
        /// Timestamp of the last read message
        last_read_timestamp: String,
    },
    /// Encrypted file transfer
    FileMessage {
        filename: String,
        /// Base64-encoded encrypted file data
        encrypted_data: String,
        sender_name: Option<String>,
    },
    /// Contact removal notification
    ContactRemoved {
        /// Fingerprint of the contact being removed
        fingerprint: String,
    },
}

/// Key share data with username
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyShareData {
    pub public_key: String,
    pub address: String,
    pub username: Option<String>,
}

pub struct NetworkHandle {
    listener_port: u16,
    running: Arc<AtomicBool>,
}

impl NetworkHandle {
    pub fn start_with_sender(sender: mpsc::UnboundedSender<NetworkEvent>) -> Result<Self> {
        let listener = match TcpListener::bind(format!("127.0.0.1:{}", DEFAULT_PORT)) {
            Ok(l) => l,
            Err(_) => TcpListener::bind("127.0.0.1:0")?,
        };
        let listener_port = listener.local_addr()?.port();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        std::thread::spawn(move || {
            while running_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, addr)) => {
                        let sender = sender.clone();
                        let peer_addr = addr.to_string();
                        std::thread::spawn(move || {
                            if let Err(e) = handle_connection(&mut stream, &sender, &peer_addr) {
                                let _ = sender.send(NetworkEvent::Error(format!("{}: {}", addr, e)));
                            }
                        });
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self { listener_port, running })
    }

    pub fn start() -> Result<Self> {
        let (s, _) = mpsc::unbounded_channel();
        Self::start_with_sender(s)
    }

    pub fn port(&self) -> u16 { self.listener_port }

    pub fn send_message(peer_address: &str, envelope: MessageEnvelope) -> Result<()> {
        let mut stream = TcpStream::connect(peer_address)?;
        let json = serde_json::to_vec(&envelope)?;
        stream.write_all(&(json.len() as u32).to_be_bytes())?;
        stream.write_all(&json)?;
        stream.flush()?;
        Ok(())
    }

    pub fn stop(&self) { self.running.store(false, Ordering::Relaxed); }
}

fn handle_connection(stream: &mut TcpStream, sender: &mpsc::UnboundedSender<NetworkEvent>, peer_addr: &str) -> Result<()> {
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes)?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    let mut buffer = vec![0u8; len];
    stream.read_exact(&mut buffer)?;
    let envelope: MessageEnvelope = serde_json::from_slice(&buffer)?;

    match envelope {
        MessageEnvelope::Request { sender_fingerprint, sender_public_key, sender_listening_port, sender_name, .. } => {
            let ip = peer_addr.split(':').next().unwrap_or("127.0.0.1");
            let _ = sender.send(NetworkEvent::RequestReceived {
                sender_fingerprint,
                sender_public_key,
                sender_address: format!("{}:{}", ip, sender_listening_port),
                sender_name,
            });
        }
        MessageEnvelope::AcceptedResponse { sender_fingerprint, sender_public_key, sender_listening_port, sender_name } => {
            let ip = peer_addr.split(':').next().unwrap_or("127.0.0.1");
            let _ = sender.send(NetworkEvent::RequestReceived {
                sender_fingerprint,
                sender_public_key,
                sender_address: format!("{}:{}", ip, sender_listening_port),
                sender_name,
            });
        }
        MessageEnvelope::RegularMessage { encrypted_payload, sender_name } => {
            let _ = sender.send(NetworkEvent::MessageReceived { encrypted_payload, sender_name });
        }
        MessageEnvelope::TypingIndicator { is_typing } => {
            let _ = sender.send(NetworkEvent::TypingUpdate { is_typing });
        }
        MessageEnvelope::ReadReceipt { last_read_timestamp } => {
            let _ = sender.send(NetworkEvent::ReadReceiptReceived { last_read_timestamp });
        }
        MessageEnvelope::FileMessage { filename, encrypted_data, sender_name } => {
            let _ = sender.send(NetworkEvent::FileReceived { filename, encrypted_data, sender_name });
        }
        MessageEnvelope::ContactRemoved { fingerprint } => {
            let _ = sender.send(NetworkEvent::ContactRemovalReceived { fingerprint });
        }
    }
    Ok(())
}
