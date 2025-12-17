//! Simple P2P networking for direct message exchange
//!
//! This is a simplified implementation that will be upgraded to use
//! the full libp2p overlay network in a future iteration.

use anyhow::Result;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
use serde::{Serialize, Deserialize};

const WM_MESSAGE_RECEIVED: u32 = 0x0400 + 3; // WM_USER + 3
const WM_REQUEST_RECEIVED: u32 = 0x0400 + 5; // WM_USER + 5
const WM_ACCEPT_RECEIVED: u32 = 0x0400 + 7; // WM_USER + 7

/// Default port for CryptoChat - all users listen on this port by default
pub const DEFAULT_PORT: u16 = 62780;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageEnvelope {
    /// Initial message request from sender to recipient
    Request {
        sender_fingerprint: String,
        sender_public_key: String,
        sender_device_id: String,
        sender_listening_port: u16, // Port where sender is listening for replies
        first_message: String, // encrypted base64
    },
    /// Response when recipient accepts a request - sends recipient's key back to sender
    AcceptedResponse {
        sender_fingerprint: String,
        sender_public_key: String,
        sender_listening_port: u16,
    },
    /// Regular encrypted message after connection established
    RegularMessage {
        encrypted_payload: String,
    },
}

pub struct NetworkHandle {
    listener_port: u16,
    running: Arc<AtomicBool>,
}

impl NetworkHandle {
    /// Start listening for incoming messages on the default port (62780)
    /// Falls back to a random port if the default is already in use
    pub fn start(hwnd: HWND) -> Result<Self> {
        // Try default port first
        let listener = match TcpListener::bind(format!("127.0.0.1:{}", DEFAULT_PORT)) {
            Ok(listener) => listener,
            Err(_) => {
                // Default port taken, fall back to random port
                eprintln!("Default port {} in use, trying random port", DEFAULT_PORT);
                TcpListener::bind("127.0.0.1:0")?
            }
        };
        let listener_port = listener.local_addr()?.port();
        let running = Arc::new(AtomicBool::new(true));

        let running_clone = running.clone();
        let hwnd_raw = hwnd.0 as isize;

        // Spawn listener thread
        std::thread::spawn(move || {
            let hwnd = HWND(hwnd_raw as *mut _);

            while running_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, addr)) => {
                        println!("Accepted connection from: {}", addr);
                        // Spawn a new thread for each connection to avoid blocking
                        let hwnd_raw2 = hwnd.0 as isize;
                        std::thread::spawn(move || {
                            if let Err(e) = handle_incoming_connection(hwnd_raw2, &mut stream) {
                                eprintln!("Error handling incoming message from {}: {}", addr, e);
                            } else {
                                println!("Successfully handled message from {}", addr);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Error accepting connection: {}", e);
                        break;
                    }
                }
            }
            println!("Listener thread shutting down");
        });

        Ok(Self {
            listener_port,
            running,
        })
    }

    /// Get the port this client is listening on
    pub fn port(&self) -> u16 {
        self.listener_port
    }

    /// Send a message envelope to a peer
    pub fn send_message(peer_address: &str, envelope: MessageEnvelope) -> Result<()> {
        println!("Connecting to peer at {}...", peer_address);
        let mut stream = TcpStream::connect(peer_address)?;
        println!("Connected successfully");

        // Serialize envelope to JSON
        let json_bytes = serde_json::to_vec(&envelope)?;
        let len = json_bytes.len() as u32;
        println!("Sending {} bytes (envelope type: {})",
            len,
            match envelope {
                MessageEnvelope::Request { .. } => "Request",
                MessageEnvelope::AcceptedResponse { .. } => "AcceptedResponse",
                MessageEnvelope::RegularMessage { .. } => "RegularMessage",
            }
        );

        stream.write_all(&len.to_be_bytes())?;
        stream.write_all(&json_bytes)?;
        stream.flush()?;
        println!("Message sent and flushed successfully");

        Ok(())
    }

    /// Stop the network listener
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

fn handle_incoming_connection(hwnd_raw: isize, stream: &mut TcpStream) -> Result<()> {
    println!("Reading message length prefix...");

    // Read length prefix
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes)?;
    let len = u32::from_be_bytes(len_bytes) as usize;
    println!("Expecting message of {} bytes", len);

    // Read message
    let mut buffer = vec![0u8; len];
    stream.read_exact(&mut buffer)?;
    println!("Read {} bytes of message data", buffer.len());

    // Deserialize the envelope
    let envelope: MessageEnvelope = serde_json::from_slice(&buffer)?;
    let hwnd = HWND(hwnd_raw as *mut _);

    unsafe {
        match envelope {
            MessageEnvelope::Request {
                sender_fingerprint,
                sender_public_key,
                sender_device_id,
                sender_listening_port,
                first_message
            } => {
                println!("Received Request from fingerprint: {} (listening on port: {})",
                    &sender_fingerprint[..16], sender_listening_port);
                // Box the request data and send to UI thread
                let request_data = (sender_fingerprint, sender_public_key, sender_device_id, sender_listening_port, first_message);
                let request_box = Box::new(request_data);
                let request_ptr = Box::into_raw(request_box);
                PostMessageW(hwnd, WM_REQUEST_RECEIVED, WPARAM(0), LPARAM(request_ptr as isize)).ok();
                println!("Posted WM_REQUEST_RECEIVED to UI thread");
            }
            MessageEnvelope::AcceptedResponse {
                sender_fingerprint,
                sender_public_key,
                sender_listening_port,
            } => {
                println!("Received AcceptedResponse from fingerprint: {} (port: {})",
                    &sender_fingerprint[..16.min(sender_fingerprint.len())], sender_listening_port);
                // Box the response data and send to UI thread
                let response_data = (sender_fingerprint, sender_public_key, sender_listening_port);
                let response_box = Box::new(response_data);
                let response_ptr = Box::into_raw(response_box);
                PostMessageW(hwnd, WM_ACCEPT_RECEIVED, WPARAM(0), LPARAM(response_ptr as isize)).ok();
                println!("Posted WM_ACCEPT_RECEIVED to UI thread");
            }
            MessageEnvelope::RegularMessage { encrypted_payload } => {
                println!("Received RegularMessage ({} bytes encrypted)", encrypted_payload.len());
                let message_box = Box::new(encrypted_payload);
                let message_ptr = Box::into_raw(message_box);
                PostMessageW(hwnd, WM_MESSAGE_RECEIVED, WPARAM(0), LPARAM(message_ptr as isize)).ok();
                println!("Posted WM_MESSAGE_RECEIVED to UI thread");
            }
        }
    }

    Ok(())
}
