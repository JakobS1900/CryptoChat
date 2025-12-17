//! UI module for Windows native controls

pub mod onboarding;
pub mod chat;
pub mod requests;

use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::Graphics::Gdi::*,
    Win32::System::DataExchange::*,
    Win32::System::Memory::*,
};
use cryptochat_crypto_core::pgp::PgpKeyPair;
use std::thread;
use std::collections::HashMap;
use std::cell::RefCell;
use crate::network::DEFAULT_PORT;

// Thread-local storage for sender listening ports (fingerprint → port)
thread_local! {
    static SENDER_PORTS: RefCell<HashMap<String, u16>> = RefCell::new(HashMap::new());
}

// Custom message for key generation completion
const WM_KEYGEN_COMPLETE: u32 = WM_USER + 1;
const WM_KEYGEN_ERROR: u32 = WM_USER + 2;
const WM_MESSAGE_RECEIVED: u32 = WM_USER + 3;
const WM_START_NETWORK_WITH_EXISTING_KEYS: u32 = WM_USER + 4;
const WM_REQUEST_RECEIVED: u32 = WM_USER + 5;
const WM_CONTACT_ACCEPTED: u32 = WM_USER + 6;
const WM_ACCEPT_RECEIVED: u32 = WM_USER + 7; // Response when peer accepts our request

// Helper function to set default GUI font on a control
pub unsafe fn set_default_font(control: HWND) {
    let font = GetStockObject(DEFAULT_GUI_FONT);
    SendMessageW(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
}

/// Main window procedure
pub extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                onboarding::create_onboarding_controls(hwnd);

                // Check if keys already exist and start network
                if let Some(app_state) = crate::get_app_state() {
                    if app_state.keypair.read().unwrap().is_some() {
                        // Post message to start network after window is fully created
                        PostMessageW(hwnd, WM_START_NETWORK_WITH_EXISTING_KEYS, WPARAM(0), LPARAM(0)).ok();
                    }
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_COMMAND => {
                let control_id = (wparam.0 & 0xFFFF) as isize;
                handle_command(hwnd, control_id);
                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let _ = BeginPaint(hwnd, &mut ps);
                // Paint is handled by child controls
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_SIZE => {
                // Force full window redraw on resize
                let _ = InvalidateRect(hwnd, None, true);
                LRESULT(0)
            }
            WM_GETMINMAXINFO => {
                // Set minimum window size
                if lparam.0 != 0 {
                    let minmax = lparam.0 as *mut MINMAXINFO;
                    (*minmax).ptMinTrackSize.x = 900;
                    (*minmax).ptMinTrackSize.y = 680;
                }
                LRESULT(0)
            }
            WM_KEYGEN_COMPLETE => {
                handle_keygen_complete(hwnd, wparam, lparam);
                LRESULT(0)
            }
            WM_KEYGEN_ERROR => {
                handle_keygen_error(hwnd, lparam);
                LRESULT(0)
            }
            WM_MESSAGE_RECEIVED => {
                handle_message_received(hwnd, lparam);
                LRESULT(0)
            }
            WM_START_NETWORK_WITH_EXISTING_KEYS => {
                handle_start_network_with_existing_keys(hwnd);
                LRESULT(0)
            }
            WM_REQUEST_RECEIVED => {
                handle_request_received(hwnd, lparam);
                LRESULT(0)
            }
            WM_CONTACT_ACCEPTED => {
                handle_contact_accepted(hwnd, lparam);
                LRESULT(0)
            }
            WM_ACCEPT_RECEIVED => {
                handle_accept_received(hwnd, lparam);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn handle_command(hwnd: HWND, control_id: isize) {
    match control_id {
        onboarding::ID_BUTTON_GENERATE => {
            // Update button to show progress
            if let Ok(btn) = GetDlgItem(hwnd, onboarding::ID_BUTTON_GENERATE as i32) {
                SetWindowTextW(btn, w!("Generating keys... (30-60 sec)")).ok();
            }

            // Spawn background thread for key generation (keeps UI responsive)
            let hwnd_raw = hwnd.0 as isize; // Extract raw pointer value
            thread::spawn(move || {
                let hwnd = HWND(hwnd_raw as *mut _); // Reconstruct HWND in thread
                match PgpKeyPair::generate("CryptoChat User") {
                    Ok(keypair) => {
                        // Box the keypair and send pointer via message
                        let keypair_box = Box::new(keypair);
                        let keypair_ptr = Box::into_raw(keypair_box);
                        PostMessageW(hwnd, WM_KEYGEN_COMPLETE, WPARAM(0), LPARAM(keypair_ptr as isize)).ok();
                    }
                    Err(e) => {
                        // Box error string and send pointer
                        let error_msg = format!("Failed to generate keys: {}", e);
                        let error_box = Box::new(error_msg);
                        let error_ptr = Box::into_raw(error_box);
                        PostMessageW(hwnd, WM_KEYGEN_ERROR, WPARAM(0), LPARAM(error_ptr as isize)).ok();
                    }
                }
            });
        }
        onboarding::ID_BUTTON_IMPORT_KEY => {
            // Import and verify recipient's public key
            if let Ok(hwnd_recipkey) = GetDlgItem(hwnd, onboarding::ID_EDIT_RECIPIENT_KEY as i32) {
                let text_len = GetWindowTextLengthW(hwnd_recipkey);
                if text_len > 0 {
                    let mut buffer = vec![0u16; (text_len + 1) as usize];
                    GetWindowTextW(hwnd_recipkey, &mut buffer);

                    // Convert UTF-16 to String
                    let key_text = String::from_utf16_lossy(&buffer[..text_len as usize]);

                    // Validate the key using crypto-core
                    match PgpKeyPair::from_public_key(&key_text) {
                        Ok(recipient_keypair) => {
                            let fingerprint = recipient_keypair.fingerprint();

                            // Store recipient key in app state
                            if let Some(app_state) = crate::get_app_state() {
                                app_state.set_recipient_keypair(recipient_keypair);
                            }

                            let success_msg = format!("✓ Valid PGP key imported!\n\nFingerprint:\n{}", fingerprint);
                            let success_wide: Vec<u16> = success_msg.encode_utf16().chain(std::iter::once(0)).collect();

                            MessageBoxW(
                                hwnd,
                                PCWSTR(success_wide.as_ptr()),
                                w!("Key Imported"),
                                MB_OK | MB_ICONINFORMATION,
                            );

                            // Show peer address input and "Start Chat" button
                            if let Ok(label) = GetDlgItem(hwnd, onboarding::ID_LABEL_PEER_ADDRESS as i32) {
                                let _ = ShowWindow(label, SW_SHOW);
                            }
                            if let Ok(input) = GetDlgItem(hwnd, onboarding::ID_EDIT_PEER_ADDRESS as i32) {
                                let _ = ShowWindow(input, SW_SHOW);
                            }
                            if let Ok(start_btn) = GetDlgItem(hwnd, onboarding::ID_BUTTON_START_CHAT as i32) {
                                let _ = ShowWindow(start_btn, SW_SHOW);
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("Invalid PGP key!\n\nError: {}\n\nPlease paste a valid PGP PUBLIC KEY BLOCK.", e);
                            let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                            MessageBoxW(
                                hwnd,
                                PCWSTR(error_wide.as_ptr()),
                                w!("Import Failed"),
                                MB_OK | MB_ICONERROR,
                            );
                        }
                    }
                } else {
                    MessageBoxW(
                        hwnd,
                        w!("Please paste a recipient's public key first!"),
                        w!("Info"),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
            }
        }
        onboarding::ID_BUTTON_START_CHAT => {
            println!("=== START CHAT BUTTON CLICKED ===");
            // Read peer address
            if let Ok(peer_input) = GetDlgItem(hwnd, onboarding::ID_EDIT_PEER_ADDRESS as i32) {
                let text_len = GetWindowTextLengthW(peer_input);
                let mut buffer = vec![0u16; (text_len + 1) as usize];
                GetWindowTextW(peer_input, &mut buffer);
                let peer_address = String::from_utf16_lossy(&buffer[..text_len as usize]);

                // Validate peer address format (must be IP:PORT)
                if !peer_address.contains(':') || peer_address.ends_with(':') {
                    MessageBoxW(
                        hwnd,
                        w!("Please enter a valid peer address with port (e.g., 127.0.0.1:5000)"),
                        w!("Invalid Address"),
                        MB_OK | MB_ICONWARNING,
                    );
                    return;
                }

                // Validate that the port part is a number
                if let Some(port_str) = peer_address.split(':').last() {
                    if port_str.parse::<u16>().is_err() {
                        MessageBoxW(
                            hwnd,
                            w!("Please enter a valid port number (1-65535)"),
                            w!("Invalid Port"),
                            MB_OK | MB_ICONWARNING,
                        );
                        return;
                    }
                }

                // Test connection to peer
                match peer_address.parse::<std::net::SocketAddr>() {
                    Ok(socket_addr) => {
                        match std::net::TcpStream::connect_timeout(&socket_addr, std::time::Duration::from_secs(2)) {
                            Ok(_) => {
                                // Connection successful
                            }
                            Err(_) => {
                                let warning = format!(
                                    "Warning: Could not connect to {}\n\n\
                                    Possible reasons:\n\
                                    • The peer is not running CryptoChat\n\
                                    • Wrong port number (check their 'Listening on port' field)\n\
                                    • Firewall is blocking the connection\n\n\
                                    Continue anyway?",
                                    peer_address
                                );
                                let warning_wide: Vec<u16> = warning.encode_utf16().chain(std::iter::once(0)).collect();
                                if MessageBoxW(
                                    hwnd,
                                    PCWSTR(warning_wide.as_ptr()),
                                    w!("Connection Test Failed"),
                                    MB_YESNO | MB_ICONWARNING,
                                ).0 != IDYES.0 {
                                    return;
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Invalid socket address format, but we already validated IP:PORT above
                        // This shouldn't happen, but continue anyway
                    }
                }

                // Set peer address and start chat
                if let Some(app_state) = crate::get_app_state() {
                    app_state.set_peer_address(peer_address.clone());

                    // Send message request to peer
                    if let (Some(my_keypair), Some(_recipient_keypair)) = (
                        app_state.keypair.read().unwrap().as_ref(),
                        app_state.recipient_keypair.read().unwrap().as_ref(),
                    ) {
                        println!("Sending message request to peer...");
                        // Create initial greeting message
                        let initial_message = "Hello! I'd like to chat with you.";

                        // Encrypt the message
                        match app_state.encrypt_message(initial_message) {
                            Ok(encrypted) => {
                                println!("Initial greeting encrypted successfully");

                                // Get our listening port
                                let my_port = app_state.network.read().unwrap()
                                    .as_ref()
                                    .map(|n| n.port())
                                    .unwrap_or(DEFAULT_PORT);

                                println!("Including our listening port in request: {}", my_port);

                                // Create request envelope
                                let envelope = crate::network::MessageEnvelope::Request {
                                    sender_fingerprint: my_keypair.fingerprint(),
                                    sender_public_key: my_keypair.export_public_key().unwrap_or_default(),
                                    sender_device_id: format!("{:?}", app_state.device_id),
                                    sender_listening_port: my_port,
                                    first_message: encrypted,
                                };

                                println!("Sending Request envelope to {}", peer_address);
                                // Send request
                                if let Err(e) = crate::network::NetworkHandle::send_message(&peer_address, envelope) {
                                    let error_msg = format!("Failed to send request: {}\n\nContinuing to chat anyway...", e);
                                    let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                    MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()), w!("Warning"), MB_OK | MB_ICONWARNING);
                                }
                            }
                            Err(e) => {
                                let error_msg = format!("Failed to encrypt request: {}\n\nContinuing to chat anyway...", e);
                                let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()), w!("Warning"), MB_OK | MB_ICONWARNING);
                            }
                        }
                    }

                    // Hide all onboarding controls
                    hide_onboarding_controls(hwnd);

                    // Create chat window controls
                    chat::create_chat_controls(hwnd);

                    // Get listening port from network handle
                    let port = app_state.network.read().unwrap()
                        .as_ref()
                        .map(|n| n.port())
                        .unwrap_or(0);

                    // Add welcome message with connection info
                    if let Ok(history) = GetDlgItem(hwnd, chat::ID_EDIT_MESSAGE_HISTORY as i32) {
                        let welcome = format!(
                            "=== CryptoChat Session Started ===\r\n\r\n\
                            All messages are end-to-end encrypted with PGP.\r\n\
                            Only you and your recipient can read these messages.\r\n\r\n\
                            Listening on port: {}\r\n\
                            Connected to peer: {}\r\n\r\n",
                            port, peer_address
                        );
                        let welcome_wide: Vec<u16> = welcome.encode_utf16().chain(std::iter::once(0)).collect();
                        SetWindowTextW(history, PCWSTR(welcome_wide.as_ptr())).ok();
                    }
                }
            }
        }
        chat::ID_BUTTON_SEND => {
            // Get message input
            if let Ok(input_ctrl) = GetDlgItem(hwnd, chat::ID_EDIT_MESSAGE_INPUT as i32) {
                let text_len = GetWindowTextLengthW(input_ctrl);
                if text_len > 0 {
                    let mut buffer = vec![0u16; (text_len + 1) as usize];
                    GetWindowTextW(input_ctrl, &mut buffer);
                    let message_text = String::from_utf16_lossy(&buffer[..text_len as usize]);

                    println!("UI: Send button clicked - message: {}", message_text);

                    // Encrypt message with recipient's key
                    let encrypted_result = if let Some(app_state) = crate::get_app_state() {
                        println!("UI: Encrypting message...");
                        app_state.encrypt_message(&message_text)
                    } else {
                        Err(anyhow::anyhow!("App state not available"))
                    };

                    match encrypted_result {
                        Ok(encrypted_message) => {
                            // Add to local history (showing plaintext for sender)
                            if let Ok(history) = GetDlgItem(hwnd, chat::ID_EDIT_MESSAGE_HISTORY as i32) {
                                let hist_len = GetWindowTextLengthW(history);
                                let mut hist_buffer = vec![0u16; (hist_len + 1) as usize];
                                GetWindowTextW(history, &mut hist_buffer);
                                let current_history = String::from_utf16_lossy(&hist_buffer[..hist_len as usize]);

                                let new_message = format!("{}[You]: {}\r\n\r\n",
                                    current_history, message_text);
                                let new_wide: Vec<u16> = new_message.encode_utf16().chain(std::iter::once(0)).collect();
                                SetWindowTextW(history, PCWSTR(new_wide.as_ptr())).ok();

                                // Scroll to bottom (EM_SETSEL = 0x00B1, EM_SCROLLCARET = 0x00B7)
                                SendMessageW(history, 0x00B1, WPARAM(new_message.len()), LPARAM(new_message.len() as isize));
                                SendMessageW(history, 0x00B7, WPARAM(0), LPARAM(0));
                            }

                            // Clear input
                            SetWindowTextW(input_ctrl, w!("")).ok();

                            // Send encrypted message to peer via network
                            if let Some(app_state) = crate::get_app_state() {
                                if let Err(e) = app_state.send_encrypted_message(encrypted_message) {
                                    let error_msg = format!("Failed to send message: {}", e);
                                    let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                    MessageBoxW(
                                        hwnd,
                                        PCWSTR(error_wide.as_ptr()),
                                        w!("Network Error"),
                                        MB_OK | MB_ICONERROR,
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("Encryption failed: {}", e);
                            let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                            MessageBoxW(
                                hwnd,
                                PCWSTR(error_wide.as_ptr()),
                                w!("Error"),
                                MB_OK | MB_ICONERROR,
                            );
                        }
                    }
                } else {
                    MessageBoxW(
                        hwnd,
                        w!("Please type a message first!"),
                        w!("Info"),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
            }
        }
        onboarding::ID_BUTTON_COPY_KEY => {
            // Copy public key to clipboard
            if let Ok(hwnd_pubkey) = GetDlgItem(hwnd, onboarding::ID_EDIT_PUBLIC_KEY as i32) {
                let text_len = GetWindowTextLengthW(hwnd_pubkey);
                if text_len > 0 {
                    let mut buffer = vec![0u16; (text_len + 1) as usize];
                    GetWindowTextW(hwnd_pubkey, &mut buffer);

                    // Copy to clipboard
                    if copy_to_clipboard(hwnd, &buffer) {
                        MessageBoxW(
                            hwnd,
                            w!("Public key copied to clipboard!"),
                            w!("Success"),
                            MB_OK | MB_ICONINFORMATION,
                        );
                    } else {
                        MessageBoxW(
                            hwnd,
                            w!("Failed to copy to clipboard"),
                            w!("Error"),
                            MB_OK | MB_ICONERROR,
                        );
                    }
                } else {
                    MessageBoxW(
                        hwnd,
                        w!("Please generate keys first!"),
                        w!("Info"),
                        MB_OK | MB_ICONINFORMATION,
                    );
                }
            }
        }
        onboarding::ID_BUTTON_GENERATE_QR => {
            // Generate QR code from user's public key with cryptographic signature
            if let Some(app_state) = crate::get_app_state() {
                let keypair = app_state.keypair.read().unwrap();
                if let Some(kp) = keypair.as_ref() {
                    // Create and sign QR payload
                    match crate::qr_exchange::QrPayload::create_and_sign(kp) {
                        Ok(payload) => {
                            let fingerprint = kp.fingerprint();
                            
                            // Generate and save QR code
                            match crate::qr_exchange::generate_qr_image(&payload) {
                                Ok(img) => {
                                    match crate::qr_exchange::save_qr_to_file(&img, "cryptochat_qr.png") {
                                        Ok(_) => {
                                            let msg = format!(
                                                "✓ QR code saved to cryptochat_qr.png!

Share this with your contact.

Fingerprint for verification:
{}",
                                                crate::qr_exchange::format_fingerprint(&fingerprint)
                                            );
                                            let msg_wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                                            MessageBoxW(hwnd, PCWSTR(msg_wide.as_ptr()),
                                                       w!("QR Code Generated"), MB_OK | MB_ICONINFORMATION);
                                        }
                                        Err(e) => {
                                            let error_msg = format!("Failed to save QR code: {}", e);
                                            let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                            MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()),
                                                       w!("Error"), MB_OK | MB_ICONERROR);
                                        }
                                    }
                                }
                                Err(e) => {
                                    let error_msg = format!("Failed to generate QR image: {}", e);
                                    let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                    MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()),
                                               w!("Error"), MB_OK | MB_ICONERROR);
                                }
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to create QR payload: {}", e);
                            let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                            MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()),
                                       w!("Error"), MB_OK | MB_ICONERROR);
                        }
                    }
                } else {
                    MessageBoxW(hwnd, w!("Please generate encryption keys first!"),
                               w!("Info"), MB_OK | MB_ICONINFORMATION);
                }
            }
        }
        onboarding::ID_BUTTON_SCAN_QR => {
            // Check if user already has keys
            let has_own_keys = if let Some(app_state) = crate::get_app_state() {
                app_state.keypair.read().unwrap().is_some()
            } else {
                false
            };

            // If no keys, auto-generate them silently (requester mode)
            if !has_own_keys {
                let confirm_msg = "You don't have encryption keys yet.\n\nGenerate keys now to send a message request?\n\n(This will take 30-60 seconds)";
                let confirm_wide: Vec<u16> = confirm_msg.encode_utf16().chain(std::iter::once(0)).collect();

                if MessageBoxW(
                    hwnd,
                    PCWSTR(confirm_wide.as_ptr()),
                    w!("Generate Keys?"),
                    MB_YESNO | MB_ICONQUESTION
                ).0 == IDYES.0 {
                    // Show progress message
                    let progress_msg = "Generating your encryption keys...\n\nThis will take 30-60 seconds.\nPlease wait...";
                    let progress_wide: Vec<u16> = progress_msg.encode_utf16().chain(std::iter::once(0)).collect();
                    MessageBoxW(hwnd, PCWSTR(progress_wide.as_ptr()),
                               w!("Please Wait"), MB_OK | MB_ICONINFORMATION);

                    // Generate keys in background
                    let hwnd_raw = hwnd.0 as isize;
                    thread::spawn(move || {
                        let hwnd = HWND(hwnd_raw as *mut _);
                        match PgpKeyPair::generate("CryptoChat User") {
                            Ok(keypair) => {
                                let keypair_box = Box::new(keypair);
                                let keypair_ptr = Box::into_raw(keypair_box);
                                PostMessageW(hwnd, WM_KEYGEN_COMPLETE, WPARAM(1), LPARAM(keypair_ptr as isize)).ok();
                            }
                            Err(e) => {
                                let error_msg = format!("Failed to generate keys: {}", e);
                                let error_box = Box::new(error_msg);
                                let error_ptr = Box::into_raw(error_box);
                                PostMessageW(hwnd, WM_KEYGEN_ERROR, WPARAM(0), LPARAM(error_ptr as isize)).ok();
                            }
                        }
                    });
                    return; // Exit handler, will continue after key generation
                }
            }

            // Open file picker dialog for QR code image
            use windows::Win32::UI::Controls::Dialogs::*;

            let filter = "PNG Images\0*.png\0JPEG Images\0*.jpg;*.jpeg\0All Files\0*.*\0\0";
            let filter_wide: Vec<u16> = filter.encode_utf16().collect();
            let mut file_path_buffer = [0u16; 260];

            let mut ofn = OPENFILENAMEW {
                lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
                hwndOwner: hwnd,
                lpstrFilter: PCWSTR(filter_wide.as_ptr()),
                lpstrFile: PWSTR(file_path_buffer.as_mut_ptr()),
                nMaxFile: file_path_buffer.len() as u32,
                lpstrTitle: w!("Select QR Code Image"),
                Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
                ..Default::default()
            };

            if GetOpenFileNameW(&mut ofn).as_bool() {
                let len = file_path_buffer.iter().position(|&c| c == 0).unwrap_or(file_path_buffer.len());
                let file_path = String::from_utf16_lossy(&file_path_buffer[..len]);

                match crate::qr_exchange::scan_qr_from_file(&file_path) {
                    Ok(payload) => {
                        match PgpKeyPair::from_public_key(payload.public_key()) {
                            Ok(recipient_keypair) => {
                                let fingerprint = recipient_keypair.fingerprint();

                                if let Some(app_state) = crate::get_app_state() {
                                    app_state.set_recipient_keypair(recipient_keypair);
                                }

                                let success_msg = format!(
                                    "✓ Contact added!\n\nFingerprint:\n{}\n\nYou can now send them a message request.",
                                    crate::qr_exchange::format_fingerprint(&fingerprint)
                                );
                                let success_wide: Vec<u16> = success_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                MessageBoxW(hwnd, PCWSTR(success_wide.as_ptr()),
                                           w!("Success"), MB_OK | MB_ICONINFORMATION);

                                // Show peer address input
                                if let Ok(label) = GetDlgItem(hwnd, onboarding::ID_LABEL_PEER_ADDRESS as i32) {
                                    let _ = ShowWindow(label, SW_SHOW);
                                }
                                if let Ok(input) = GetDlgItem(hwnd, onboarding::ID_EDIT_PEER_ADDRESS as i32) {
                                    let _ = ShowWindow(input, SW_SHOW);
                                }
                                if let Ok(btn) = GetDlgItem(hwnd, onboarding::ID_BUTTON_START_CHAT as i32) {
                                    let _ = ShowWindow(btn, SW_SHOW);
                                }
                            }
                            Err(e) => {
                                let error_msg = format!("Failed to parse public key: {}", e);
                                let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()),
                                           w!("Error"), MB_OK | MB_ICONERROR);
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!(
                            "Failed to scan QR code:\n\n{}\n\nMake sure the image contains a valid CryptoChat QR code.",
                            e
                        );
                        let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                        MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()),
                                   w!("QR Scan Failed"), MB_OK | MB_ICONERROR);
                    }
                }
            }
        }
        onboarding::ID_BUTTON_VIEW_REQUESTS => {
            // Open the requests window
            open_requests_window();
        }
        onboarding::ID_BUTTON_CONTINUE => {
            // User is ready to receive messages without sending first
            // Just inform them they can now receive requests and wait
            let msg = "✓ You're now ready to receive messages!\n\n\
                      Your listening port is displayed above.\n\
                      Share your fingerprint and public key with contacts.\n\n\
                      When someone sends you a message request, it will appear in 'View Requests'.";
            let msg_wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
            MessageBoxW(hwnd, PCWSTR(msg_wide.as_ptr()), w!("Ready!"), MB_OK | MB_ICONINFORMATION);
        }

        _ => {}
    }
}

unsafe fn handle_keygen_complete(hwnd: HWND, wparam: WPARAM, lparam: LPARAM) {
    // Reconstruct keypair from pointer
    let keypair_ptr = lparam.0 as *mut PgpKeyPair;
    let keypair = *Box::from_raw(keypair_ptr);

    // Check if this was auto-generation (wparam=1) or manual (wparam=0)
    let is_auto = wparam.0 == 1;

    // Reset button text (only for manual generation)
    if !is_auto {
        if let Ok(btn) = GetDlgItem(hwnd, onboarding::ID_BUTTON_GENERATE as i32) {
            SetWindowTextW(btn, w!("Generate Encryption Keys")).ok();
        }
    }

    // Get fingerprint
    let fingerprint = keypair.fingerprint();
    let fingerprint_wide: Vec<u16> = fingerprint.encode_utf16().chain(std::iter::once(0)).collect();

    // Get public key
    let public_key = keypair.export_public_key().unwrap_or_default();
    let public_key_wide: Vec<u16> = public_key.encode_utf16().chain(std::iter::once(0)).collect();

    // Update fingerprint display (only if not auto-generated)
    if !is_auto {
        if let Ok(hwnd_fingerprint) = GetDlgItem(hwnd, onboarding::ID_EDIT_FINGERPRINT as i32) {
            SetWindowTextW(hwnd_fingerprint, PCWSTR(fingerprint_wide.as_ptr())).ok();
        }

        // Update public key display
        if let Ok(hwnd_pubkey) = GetDlgItem(hwnd, onboarding::ID_EDIT_PUBLIC_KEY as i32) {
            SetWindowTextW(hwnd_pubkey, PCWSTR(public_key_wide.as_ptr())).ok();
        }
    }

    // Store keypair in app state
    if let Some(app_state) = crate::get_app_state() {
        app_state.set_keypair(keypair.clone());

        // Start network listener immediately and display port
        match app_state.start_network(hwnd) {
            Ok(port) => {
                let port_text = if port == crate::network::DEFAULT_PORT {
                    format!("Listening on port {} (default)", port)
                } else {
                    format!("Listening on port {} (fallback - {} was taken)", port, crate::network::DEFAULT_PORT)
                };
                let port_wide: Vec<u16> = port_text.encode_utf16().chain(std::iter::once(0)).collect();
                if let Ok(port_ctrl) = GetDlgItem(hwnd, onboarding::ID_EDIT_MY_PORT as i32) {
                    SetWindowTextW(port_ctrl, PCWSTR(port_wide.as_ptr())).ok();
                }
            }
            Err(e) => {
                let error_msg = format!("Warning: Failed to start network: {}", e);
                let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                if let Ok(port_ctrl) = GetDlgItem(hwnd, onboarding::ID_EDIT_MY_PORT as i32) {
                    SetWindowTextW(port_ctrl, PCWSTR(error_wide.as_ptr())).ok();
                }
            }
        }
    }

    // Save keypair to Windows Credential Manager for persistence
    let secret_key = keypair.export_secret_key().unwrap_or_default();
    let public_key = keypair.export_public_key().unwrap_or_default();
    let fingerprint = keypair.fingerprint();

    let stored_key = crate::keystore::StoredKey {
        secret_key_armored: secret_key,
        public_key_armored: public_key,
        fingerprint: fingerprint.clone(),
    };

    match crate::keystore::save_keypair(&stored_key) {
        Ok(_) => {
            if is_auto {
                // Auto-generated: Show brief success and prompt to scan QR
                let msg = "✓ Keys generated!\n\nNow scan your contact's QR code to continue.";
                let msg_wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                MessageBoxW(hwnd, PCWSTR(msg_wide.as_ptr()), w!("Success"), MB_OK | MB_ICONINFORMATION);

                // Trigger QR scan button click
                PostMessageW(hwnd, WM_COMMAND, WPARAM(onboarding::ID_BUTTON_SCAN_QR as usize), LPARAM(0)).ok();
            } else {
                // Manual generation: Show full details
                let msg = format!(
                    "✓ Encryption keys generated and securely saved!\n\n\
                    Your keys are encrypted with Windows DPAPI and stored in Credential Manager.\n\n\
                    Fingerprint: {}",
                    crate::qr_exchange::format_fingerprint(&fingerprint)
                );
                let msg_wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                MessageBoxW(hwnd, PCWSTR(msg_wide.as_ptr()), w!("Success"), MB_OK | MB_ICONINFORMATION);

                // Show the Continue button so users can proceed without importing a contact's key
                if let Ok(btn) = GetDlgItem(hwnd, onboarding::ID_BUTTON_CONTINUE as i32) {
                    let _ = ShowWindow(btn, SW_SHOW);
                }
            }
        }
        Err(e) => {
            let msg = format!(
                "⚠ Keys generated but failed to save to secure storage: {}\n\n\
                You'll need to regenerate keys next time you start the app.",
                e
            );
            let msg_wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
            MessageBoxW(hwnd, PCWSTR(msg_wide.as_ptr()), w!("Warning"), MB_OK | MB_ICONWARNING);
        }
    }
}

unsafe fn handle_keygen_error(hwnd: HWND, lparam: LPARAM) {
    // Reconstruct error string from pointer
    let error_ptr = lparam.0 as *mut String;
    let error_msg = *Box::from_raw(error_ptr);

    // Reset button on error
    if let Ok(btn) = GetDlgItem(hwnd, onboarding::ID_BUTTON_GENERATE as i32) {
        SetWindowTextW(btn, w!("Generate Encryption Keys")).ok();
    }

    let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
    MessageBoxW(
        hwnd,
        PCWSTR(error_wide.as_ptr()),
        w!("Error"),
        MB_OK | MB_ICONERROR,
    );
}

unsafe fn hide_onboarding_controls(hwnd: HWND) {
    // Hide all onboarding control IDs
    let ids = [
        onboarding::ID_BUTTON_GENERATE,
        onboarding::ID_BUTTON_COPY_KEY,
        onboarding::ID_BUTTON_GENERATE_QR,
        onboarding::ID_BUTTON_SCAN_QR,
        onboarding::ID_BUTTON_VIEW_REQUESTS,
        onboarding::ID_EDIT_FINGERPRINT,
        onboarding::ID_EDIT_PUBLIC_KEY,
        onboarding::ID_EDIT_RECIPIENT_KEY,
        onboarding::ID_BUTTON_IMPORT_KEY,
        onboarding::ID_BUTTON_START_CHAT,
        onboarding::ID_EDIT_PEER_ADDRESS,
        onboarding::ID_LABEL_PEER_ADDRESS,
        onboarding::ID_LABEL_MY_PORT,
        onboarding::ID_EDIT_MY_PORT,
    ];

    for id in ids {
        if let Ok(ctrl) = GetDlgItem(hwnd, id as i32) {
            let _ = ShowWindow(ctrl, SW_HIDE);
        }
    }

    // Also hide static text labels (they don't have IDs but we can enumerate child windows)
    // For now, this is good enough - the important controls are hidden
}

unsafe fn copy_to_clipboard(hwnd: HWND, text: &[u16]) -> bool {
    // Open clipboard
    if OpenClipboard(hwnd).is_err() {
        return false;
    }

    // Empty clipboard
    if EmptyClipboard().is_err() {
        let _ = CloseClipboard();
        return false;
    }

    // Calculate size (excluding null terminator, will be added)
    let len = text.iter().position(|&c| c == 0).unwrap_or(text.len());
    let size = (len + 1) * 2; // +1 for null terminator, *2 for UTF-16

    // Allocate global memory
    let hglob = match GlobalAlloc(GMEM_MOVEABLE, size) {
        Ok(h) => h,
        Err(_) => {
            let _ = CloseClipboard();
            return false;
        }
    };

    // Lock memory and copy text
    let ptr = GlobalLock(hglob);
    if ptr.is_null() {
        let _ = GlobalFree(hglob);
        let _ = CloseClipboard();
        return false;
    }

    let dest = std::slice::from_raw_parts_mut(ptr as *mut u16, len + 1);
    dest[..len].copy_from_slice(&text[..len]);
    dest[len] = 0; // Null terminator
    GlobalUnlock(hglob);

    // Set clipboard data (CF_UNICODETEXT = 13)
    if SetClipboardData(13, HANDLE(hglob.0)).is_err() {
        let _ = GlobalFree(hglob);
        let _ = CloseClipboard();
        return false;
    }

    // Close clipboard
    let _ = CloseClipboard();
    true
}

unsafe fn handle_start_network_with_existing_keys(hwnd: HWND) {
    if let Some(app_state) = crate::get_app_state() {
        // Start network and display port
        match app_state.start_network(hwnd) {
            Ok(port) => {
                // Display fingerprint
                if let Some(keypair) = app_state.keypair.read().unwrap().as_ref() {
                    let fingerprint = keypair.fingerprint();
                    let fingerprint_wide: Vec<u16> = fingerprint.encode_utf16().chain(std::iter::once(0)).collect();
                    if let Ok(fp_ctrl) = GetDlgItem(hwnd, onboarding::ID_EDIT_FINGERPRINT as i32) {
                        SetWindowTextW(fp_ctrl, PCWSTR(fingerprint_wide.as_ptr())).ok();
                    }

                    // Display public key
                    let public_key = keypair.export_public_key().unwrap_or_default();
                    let pubkey_wide: Vec<u16> = public_key.encode_utf16().chain(std::iter::once(0)).collect();
                    if let Ok(pk_ctrl) = GetDlgItem(hwnd, onboarding::ID_EDIT_PUBLIC_KEY as i32) {
                        SetWindowTextW(pk_ctrl, PCWSTR(pubkey_wide.as_ptr())).ok();
                    }
                }

                // Display port
                let port_text = format!("Listening on port {}", port);
                let port_wide: Vec<u16> = port_text.encode_utf16().chain(std::iter::once(0)).collect();
                if let Ok(port_ctrl) = GetDlgItem(hwnd, onboarding::ID_EDIT_MY_PORT as i32) {
                    SetWindowTextW(port_ctrl, PCWSTR(port_wide.as_ptr())).ok();
                }
            }
            Err(e) => {
                let error_msg = format!("Warning: Failed to start network: {}", e);
                let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                if let Ok(port_ctrl) = GetDlgItem(hwnd, onboarding::ID_EDIT_MY_PORT as i32) {
                    SetWindowTextW(port_ctrl, PCWSTR(error_wide.as_ptr())).ok();
                }
            }
        }
    }
}

unsafe fn handle_message_received(hwnd: HWND, lparam: LPARAM) {
    // Reconstruct encrypted message string from pointer
    let message_ptr = lparam.0 as *mut String;
    let encrypted_message = *Box::from_raw(message_ptr);

    println!("UI: handle_message_received called ({} bytes encrypted)", encrypted_message.len());

    // Decrypt the message
    let decrypted_result = if let Some(app_state) = crate::get_app_state() {
        println!("UI: Attempting to decrypt message...");
        let result = app_state.decrypt_message(&encrypted_message);
        if let Ok(ref plaintext) = result {
            println!("UI: Successfully decrypted: {}", plaintext);
        } else if let Err(ref e) = result {
            println!("UI: Decryption failed: {}", e);
        }
        result
    } else {
        Err(anyhow::anyhow!("App state not available"))
    };

    match decrypted_result {
        Ok(plaintext) => {
            // Add to chat history
            if let Ok(history) = GetDlgItem(hwnd, chat::ID_EDIT_MESSAGE_HISTORY as i32) {
                let hist_len = GetWindowTextLengthW(history);
                let mut hist_buffer = vec![0u16; (hist_len + 1) as usize];
                GetWindowTextW(history, &mut hist_buffer);
                let current_history = String::from_utf16_lossy(&hist_buffer[..hist_len as usize]);

                let new_message = format!("{}[Recipient]: {}\r\n\r\n", current_history, plaintext);
                let new_wide: Vec<u16> = new_message.encode_utf16().chain(std::iter::once(0)).collect();
                SetWindowTextW(history, PCWSTR(new_wide.as_ptr())).ok();

                // Scroll to bottom
                SendMessageW(history, 0x00B1, WPARAM(new_message.len()), LPARAM(new_message.len() as isize));
                SendMessageW(history, 0x00B7, WPARAM(0), LPARAM(0));
            }
        }
        Err(e) => {
            eprintln!("Failed to decrypt message: {}", e);

            // Add error to chat history instead of blocking with a dialog
            if let Ok(history) = GetDlgItem(hwnd, chat::ID_EDIT_MESSAGE_HISTORY as i32) {
                let hist_len = GetWindowTextLengthW(history);
                let mut hist_buffer = vec![0u16; (hist_len + 1) as usize];
                GetWindowTextW(history, &mut hist_buffer);
                let current_history = String::from_utf16_lossy(&hist_buffer[..hist_len as usize]);

                let error_note = format!("{}[System]: ⚠ Could not decrypt message (wrong key or corrupted data)\r\n\r\n", current_history);
                let error_wide: Vec<u16> = error_note.encode_utf16().chain(std::iter::once(0)).collect();
                SetWindowTextW(history, PCWSTR(error_wide.as_ptr())).ok();

                // Scroll to bottom
                SendMessageW(history, 0x00B1, WPARAM(error_note.len()), LPARAM(error_note.len() as isize));
                SendMessageW(history, 0x00B7, WPARAM(0), LPARAM(0));
            }
        }
    }
}

unsafe fn handle_request_received(hwnd: HWND, lparam: LPARAM) {
    use cryptochat_messaging::requests::MessageRequest;
    use cryptochat_messaging::ConversationId;

    // Reconstruct request data from pointer (now includes sender_listening_port)
    let request_ptr = lparam.0 as *mut (String, String, String, u16, String);
    let (sender_fingerprint, sender_public_key, sender_device_id, sender_listening_port, first_message) = *Box::from_raw(request_ptr);

    println!("UI: Received request from port {}", sender_listening_port);

    // Store the sender's listening port for later use when accepting
    SENDER_PORTS.with(|ports| {
        ports.borrow_mut().insert(sender_fingerprint.clone(), sender_listening_port);
    });

    // Decrypt the first message to use as preview
    let preview = if let Some(app_state) = crate::get_app_state() {
        // Import sender's public key temporarily to decrypt
        if let Ok(sender_keypair) = PgpKeyPair::from_public_key(&sender_public_key) {
            // Save original recipient key
            let original_recipient = app_state.recipient_keypair.read().unwrap().clone();

            // Temporarily set sender as recipient to decrypt
            app_state.set_recipient_keypair(sender_keypair);

            // Decrypt the message
            let result = app_state.decrypt_message(&first_message).ok();

            // Restore original recipient key if it existed
            if let Some(orig) = original_recipient {
                app_state.set_recipient_keypair(orig);
            }

            result
        } else {
            None
        }
    } else {
        None
    };

    // Create message request
    let device_id = sender_device_id.parse::<uuid::Uuid>()
        .map(|uuid| cryptochat_messaging::DeviceId(uuid))
        .unwrap_or_else(|_| cryptochat_messaging::DeviceId::new());

    let request = MessageRequest::new(
        ConversationId::new(),
        sender_fingerprint.clone(),
        device_id,
        sender_public_key,
        preview.clone(),
    );

    // Save request to disk
    if let Err(e) = crate::request_store::save_request(&request) {
        eprintln!("Failed to save message request: {}", e);
        let error_msg = format!("Failed to save request: {}", e);
        let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
        MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()), w!("Error"), MB_OK | MB_ICONERROR);
        return;
    }

    // Show notification
    let preview_text = preview.unwrap_or_else(|| "[Could not decrypt preview]".to_string());
    let notification = format!(
        "New Message Request!\n\nFrom: {}\n\nMessage: {}\n\nOpen 'View Requests' to accept or reject.",
        crate::qr_exchange::format_fingerprint(&sender_fingerprint),
        &preview_text[..preview_text.len().min(50)]
    );
    let notification_wide: Vec<u16> = notification.encode_utf16().chain(std::iter::once(0)).collect();
    MessageBoxW(
        hwnd,
        PCWSTR(notification_wide.as_ptr()),
        w!("New Request"),
        MB_OK | MB_ICONINFORMATION,
    );
}

// Requests window class name
const REQUESTS_WINDOW_CLASS: &str = "CryptoChatRequestsWindow";

/// Open the requests window
unsafe fn open_requests_window() {
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;

    let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();

    // Register window class for requests window
    let class_name_wide: Vec<u16> = REQUESTS_WINDOW_CLASS.encode_utf16().chain(std::iter::once(0)).collect();

    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(requests_window_proc),
        hInstance: hinstance,
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        lpszClassName: PCWSTR(class_name_wide.as_ptr()),
        ..Default::default()
    };

    RegisterClassW(&wc);

    // Create requests window
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        PCWSTR(class_name_wide.as_ptr()),
        w!("Message Requests"),
        WS_OVERLAPPEDWINDOW | WS_VISIBLE,
        100, 100, 620, 630,
        None,
        None,
        hinstance,
        None,
    ).ok();
}

/// Window procedure for requests window
unsafe extern "system" fn requests_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            requests::create_requests_controls(hwnd);
            requests::populate_requests_list(hwnd);
            LRESULT(0)
        }
        WM_COMMAND => {
            let control_id = (wparam.0 & 0xFFFF) as isize;
            let notification_code = ((wparam.0 >> 16) & 0xFFFF) as u16;

            // Handle listbox selection change (LBN_SELCHANGE = 1)
            if control_id == requests::ID_LIST_REQUESTS && notification_code == 1 {
                handle_request_selection(hwnd);
            } else {
                handle_requests_command(hwnd, control_id);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // Don't quit the whole app, just close this window
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Handle request selection in the listbox
unsafe fn handle_request_selection(hwnd: HWND) {
    use windows::Win32::UI::Controls::*;

    if let Ok(listbox) = GetDlgItem(hwnd, requests::ID_LIST_REQUESTS as i32) {
        // Get selected index (LB_GETCURSEL = 0x0188)
        let selected_index = SendMessageW(listbox, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;

        if selected_index >= 0 {
            // Load requests and get the selected one
            match crate::request_store::load_pending_requests() {
                Ok(requests_list) => {
                    if let Some(request) = requests_list.get(selected_index as usize) {
                        // Display fingerprint
                        if let Ok(fp_ctrl) = GetDlgItem(hwnd, requests::ID_STATIC_FINGERPRINT as i32) {
                            let fp_display = crate::qr_exchange::format_fingerprint(&request.sender_fingerprint);
                            let fp_wide: Vec<u16> = fp_display.encode_utf16().chain(std::iter::once(0)).collect();
                            SetWindowTextW(fp_ctrl, PCWSTR(fp_wide.as_ptr())).ok();
                        }

                        // Display message preview
                        if let Ok(preview_ctrl) = GetDlgItem(hwnd, requests::ID_STATIC_PREVIEW as i32) {
                            let preview_text = request.first_message_preview
                                .as_ref()
                                .map(|s| s.as_str())
                                .unwrap_or("(No preview available)");
                            let preview_wide: Vec<u16> = preview_text.encode_utf16().chain(std::iter::once(0)).collect();
                            SetWindowTextW(preview_ctrl, PCWSTR(preview_wide.as_ptr())).ok();
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("Error loading request details: {}", e);
                    let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                    MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()), w!("Error"), MB_OK | MB_ICONERROR);
                }
            }
        }
    }
}

/// Handle commands in the requests window
unsafe fn handle_requests_command(hwnd: HWND, control_id: isize) {
    use windows::Win32::UI::Controls::*;

    match control_id {
        requests::ID_BUTTON_REFRESH => {
            requests::populate_requests_list(hwnd);
        }
        requests::ID_BUTTON_ACCEPT => {
            // Get selected request
            if let Ok(listbox) = GetDlgItem(hwnd, requests::ID_LIST_REQUESTS as i32) {
                let selected_index = SendMessageW(listbox, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;

                if selected_index >= 0 {
                    match crate::request_store::load_pending_requests() {
                        Ok(mut requests_list) => {
                            if let Some(mut request) = requests_list.get_mut(selected_index as usize).cloned() {
                                // Accept the request
                                request.accept();

                                // Save updated request
                                if let Err(e) = crate::request_store::save_request(&request) {
                                    let error_msg = format!("Failed to update request: {}", e);
                                    let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                    MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()), w!("Error"), MB_OK | MB_ICONERROR);
                                    return;
                                }

                                // Import sender's public key to app state for immediate chat
                                if let Some(app_state) = crate::get_app_state() {
                                    match PgpKeyPair::from_public_key(&request.sender_public_key) {
                                        Ok(sender_keypair) => {
                                            app_state.set_recipient_keypair(sender_keypair);
                                            
                                            // Send AcceptedResponse back to the original sender
                                            // so they get our key and can decrypt our messages
                                            let sender_port = SENDER_PORTS.with(|ports| {
                                                ports.borrow().get(&request.sender_fingerprint).copied()
                                            });
                                            
                                            if let Some(port) = sender_port {
                                                let sender_address = format!("127.0.0.1:{}", port);
                                                
                                                // Get our keypair to send our public key back
                                                if let Some(my_keypair) = app_state.keypair.read().unwrap().as_ref() {
                                                    let my_port = app_state.network.read().unwrap()
                                                        .as_ref()
                                                        .map(|n| n.port())
                                                        .unwrap_or(DEFAULT_PORT);
                                                    
                                                    let accept_response = crate::network::MessageEnvelope::AcceptedResponse {
                                                        sender_fingerprint: my_keypair.fingerprint(),
                                                        sender_public_key: my_keypair.export_public_key().unwrap_or_default(),
                                                        sender_listening_port: my_port,
                                                    };
                                                    
                                                    println!("Sending AcceptedResponse to {}", sender_address);
                                                    if let Err(e) = crate::network::NetworkHandle::send_message(&sender_address, accept_response) {
                                                        eprintln!("Warning: Failed to send accept response: {}", e);
                                                        // Continue anyway - they can still receive messages from us
                                                    } else {
                                                        println!("AcceptedResponse sent successfully");
                                                    }
                                                }
                                                
                                                // Also set peer address so we can send messages
                                                app_state.set_peer_address(sender_address);
                                            } else {
                                                println!("Warning: Could not find sender's port to send AcceptedResponse");
                                            }
                                        }
                                        Err(e) => {
                                            let error_msg = format!("Warning: Could not import sender's key: {}", e);
                                            let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                            MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()), w!("Warning"), MB_OK | MB_ICONWARNING);
                                        }
                                    }
                                }

                                // Create contact from request
                                let contact = crate::request_store::Contact::from_request(&request);

                                // Save contact
                                match crate::request_store::save_contact(&contact) {
                                    Ok(_) => {
                                        let success_msg = format!(
                                            "✓ Request accepted!\\n\\nContact added:\\n{}\\n\\nYou can now chat with them.",
                                            crate::qr_exchange::format_fingerprint(&contact.fingerprint)
                                        );
                                        let success_wide: Vec<u16> = success_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                        MessageBoxW(hwnd, PCWSTR(success_wide.as_ptr()), w!("Success"), MB_OK | MB_ICONINFORMATION);

                                        // Notify main window to show chat interface with this contact
                                        // Use dynamic window class name for multi-instance support
                                        let window_class_name = format!("CryptoChat.Window{}", crate::get_instance_suffix());
                                        let main_class: Vec<u16> = window_class_name.encode_utf16().chain(std::iter::once(0)).collect();
                                        println!("Looking for main window with class: {}", window_class_name);
                                        match FindWindowW(PCWSTR(main_class.as_ptr()), None) {
                                            Ok(main_hwnd) => {
                                                println!("Found main window: {:?}", main_hwnd);
                                                // Box the fingerprint and send pointer
                                                let fingerprint_box = Box::new(contact.fingerprint.clone());
                                                let fingerprint_ptr = Box::into_raw(fingerprint_box);
                                                println!("Posting WM_CONTACT_ACCEPTED message with fingerprint: {}", contact.fingerprint);
                                                PostMessageW(main_hwnd, WM_CONTACT_ACCEPTED, WPARAM(0), LPARAM(fingerprint_ptr as isize)).ok();
                                            }
                                            Err(e) => {
                                                println!("Failed to find main window: {:?}", e);
                                                MessageBoxW(hwnd, w!("Warning: Could not find main window to notify"), w!("Warning"), MB_OK | MB_ICONWARNING);
                                            }
                                        }

                                        // Close the requests window
                                        PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)).ok();
                                    }
                                    Err(e) => {
                                        let error_msg = format!("Failed to save contact: {}", e);
                                        let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                        MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()), w!("Error"), MB_OK | MB_ICONERROR);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            let error_msg = format!("Error loading requests: {}", e);
                            let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                            MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()), w!("Error"), MB_OK | MB_ICONERROR);
                        }
                    }
                } else {
                    MessageBoxW(hwnd, w!("Please select a request first!"), w!("Info"), MB_OK | MB_ICONINFORMATION);
                }
            }
        }
        requests::ID_BUTTON_REJECT => {
            // Get selected request
            if let Ok(listbox) = GetDlgItem(hwnd, requests::ID_LIST_REQUESTS as i32) {
                let selected_index = SendMessageW(listbox, LB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;

                if selected_index >= 0 {
                    // Confirm rejection
                    let confirm_msg = "Are you sure you want to reject this request?\n\nThis cannot be undone.";
                    let confirm_wide: Vec<u16> = confirm_msg.encode_utf16().chain(std::iter::once(0)).collect();

                    if MessageBoxW(
                        hwnd,
                        PCWSTR(confirm_wide.as_ptr()),
                        w!("Confirm Rejection"),
                        MB_YESNO | MB_ICONWARNING
                    ).0 == IDYES.0 {
                        match crate::request_store::load_pending_requests() {
                            Ok(requests_list) => {
                                if let Some(request) = requests_list.get(selected_index as usize) {
                                    let request_id = request.request_id.to_string();

                                    // Delete the request
                                    match crate::request_store::delete_request(&request_id) {
                                        Ok(_) => {
                                            MessageBoxW(
                                                hwnd,
                                                w!("Request rejected and deleted."),
                                                w!("Rejected"),
                                                MB_OK | MB_ICONINFORMATION
                                            );

                                            // Refresh the list
                                            requests::populate_requests_list(hwnd);

                                            // Clear detail fields
                                            if let Ok(fp_ctrl) = GetDlgItem(hwnd, requests::ID_STATIC_FINGERPRINT as i32) {
                                                SetWindowTextW(fp_ctrl, w!("Select a request to view details")).ok();
                                            }
                                            if let Ok(preview_ctrl) = GetDlgItem(hwnd, requests::ID_STATIC_PREVIEW as i32) {
                                                SetWindowTextW(preview_ctrl, w!("")).ok();
                                            }
                                        }
                                        Err(e) => {
                                            let error_msg = format!("Failed to delete request: {}", e);
                                            let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                            MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()), w!("Error"), MB_OK | MB_ICONERROR);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                let error_msg = format!("Error loading requests: {}", e);
                                let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                                MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()), w!("Error"), MB_OK | MB_ICONERROR);
                            }
                        }
                    }
                } else {
                    MessageBoxW(hwnd, w!("Please select a request first!"), w!("Info"), MB_OK | MB_ICONINFORMATION);
                }
            }
        }
        _ => {}
    }
}

/// Handle contact accepted notification from requests window
unsafe fn handle_contact_accepted(hwnd: HWND, lparam: LPARAM) {
    // Reconstruct fingerprint from pointer
    let fingerprint_ptr = lparam.0 as *mut String;
    let fingerprint = *Box::from_raw(fingerprint_ptr);

    println!("handle_contact_accepted called with fingerprint: {}", fingerprint);

    // Load the contact to get their details
    match crate::request_store::load_contacts() {
        Ok(contacts) => {
            println!("Loaded {} contacts", contacts.len());
            if let Some(contact) = contacts.iter().find(|c| c.fingerprint == fingerprint) {
                println!("Found contact, showing UI elements");

                // Show the peer address field and Start Chat button
                if let Ok(label) = GetDlgItem(hwnd, onboarding::ID_LABEL_PEER_ADDRESS as i32) {
                    println!("Showing peer address label");
                    let _ = ShowWindow(label, SW_SHOW);
                }
                if let Ok(input) = GetDlgItem(hwnd, onboarding::ID_EDIT_PEER_ADDRESS as i32) {
                    // Get the sender's listening port from our stored map
                    let sender_port = SENDER_PORTS.with(|ports| {
                        ports.borrow().get(&fingerprint).copied().unwrap_or(DEFAULT_PORT)
                    });

                    println!("Retrieved sender listening port: {}", sender_port);

                    // Pre-fill with sender's address
                    let peer_addr = format!("127.0.0.1:{}", sender_port);
                    let addr_wide: Vec<u16> = peer_addr.encode_utf16().chain(std::iter::once(0)).collect();
                    SetWindowTextW(input, PCWSTR(addr_wide.as_ptr())).ok();
                    println!("Showing peer address input with: {}", peer_addr);
                    let _ = ShowWindow(input, SW_SHOW);
                }
                if let Ok(btn) = GetDlgItem(hwnd, onboarding::ID_BUTTON_START_CHAT as i32) {
                    println!("Showing Start Chat button");
                    let _ = ShowWindow(btn, SW_SHOW);
                }

                // Get the sender's port for the notification
                let sender_port = SENDER_PORTS.with(|ports| {
                    ports.borrow().get(&fingerprint).copied().unwrap_or(DEFAULT_PORT)
                });

                // Show notification with instructions
                let msg = format!(
                    "Ready to chat with {}!\n\n\
                    Peer address has been set to: 127.0.0.1:{}\n\
                    (Update IP if needed)\n\n\
                    Click 'Start Chat' when ready.\n\
                    Their key is already imported.",
                    crate::qr_exchange::format_fingerprint(&contact.fingerprint),
                    sender_port
                );
                let msg_wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                MessageBoxW(hwnd, PCWSTR(msg_wide.as_ptr()), w!("Chat Ready"), MB_OK | MB_ICONINFORMATION);
            } else {
                println!("Contact not found in list!");
                MessageBoxW(hwnd, w!("Error: Contact not found after accepting request."), w!("Error"), MB_OK | MB_ICONERROR);
            }
        }
        Err(e) => {
            eprintln!("Failed to load contacts: {}", e);
            let error_msg = format!("Failed to load contacts: {}", e);
            let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
            MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()), w!("Error"), MB_OK | MB_ICONERROR);
        }
    }
}

/// Handle acceptance response from recipient - they accepted our request and sent their key
unsafe fn handle_accept_received(hwnd: HWND, lparam: LPARAM) {
    // Reconstruct response data from pointer
    let response_ptr = lparam.0 as *mut (String, String, u16);
    let (sender_fingerprint, sender_public_key, sender_listening_port) = *Box::from_raw(response_ptr);

    println!("handle_accept_received: fingerprint={}, port={}", 
        &sender_fingerprint[..16.min(sender_fingerprint.len())], sender_listening_port);

    // Import sender's public key (the person who accepted our request)
    match PgpKeyPair::from_public_key(&sender_public_key) {
        Ok(recipient_keypair) => {
            if let Some(app_state) = crate::get_app_state() {
                // Set their key as the recipient keypair
                app_state.set_recipient_keypair(recipient_keypair);
                
                // Set the peer address so we can send messages to them
                let peer_address = format!("127.0.0.1:{}", sender_listening_port);
                app_state.set_peer_address(peer_address.clone());

                // Store the port for reference
                SENDER_PORTS.with(|ports| {
                    ports.borrow_mut().insert(sender_fingerprint.clone(), sender_listening_port);
                });

                // Show notification and prompt to start chat
                let msg = format!(
                    "✓ Your request was accepted!\n\n\
                    Contact: {}\n\
                    Address set to: {}\n\n\
                    Click OK to start chatting.",
                    crate::qr_exchange::format_fingerprint(&sender_fingerprint),
                    peer_address
                );
                let msg_wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                MessageBoxW(hwnd, PCWSTR(msg_wide.as_ptr()), w!("Request Accepted!"), MB_OK | MB_ICONINFORMATION);

                // Show peer address and Start Chat button
                if let Ok(label) = GetDlgItem(hwnd, onboarding::ID_LABEL_PEER_ADDRESS as i32) {
                    let _ = ShowWindow(label, SW_SHOW);
                }
                if let Ok(input) = GetDlgItem(hwnd, onboarding::ID_EDIT_PEER_ADDRESS as i32) {
                    let addr_wide: Vec<u16> = peer_address.encode_utf16().chain(std::iter::once(0)).collect();
                    SetWindowTextW(input, PCWSTR(addr_wide.as_ptr())).ok();
                    let _ = ShowWindow(input, SW_SHOW);
                }
                if let Ok(btn) = GetDlgItem(hwnd, onboarding::ID_BUTTON_START_CHAT as i32) {
                    let _ = ShowWindow(btn, SW_SHOW);
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to import recipient's key: {}", e);
            let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
            MessageBoxW(hwnd, PCWSTR(error_wide.as_ptr()), w!("Error"), MB_OK | MB_ICONERROR);
        }
    }
}
