//! CryptoChat Windows Native Client
//!
//! Pure Rust + WinUI 3 implementation with no web engine dependencies.

#![windows_subsystem = "windows"]

mod app;
mod ui;
mod network;
mod keystore;
mod qr_exchange;
mod request_store;

use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::HBRUSH,
    Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::WindowsAndMessaging::*,
};
use std::sync::Arc;
use std::cell::RefCell;
use std::sync::OnceLock;

/// Instance ID for multi-instance testing (None = default/production)
static INSTANCE_ID: OnceLock<Option<u32>> = OnceLock::new();

/// Get the current instance ID
pub fn get_instance_id() -> Option<u32> {
    INSTANCE_ID.get().copied().flatten()
}

/// Get instance suffix for display (e.g., " #1" or empty for default)
pub fn get_instance_suffix() -> String {
    get_instance_id().map(|id| format!(" #{}", id)).unwrap_or_default()
}

thread_local! {
    static APP_STATE: RefCell<Option<Arc<app::AppState>>> = RefCell::new(None);
}

fn main() -> Result<()> {
    // Parse command-line arguments for instance ID (used for testing)
    let args: Vec<String> = std::env::args().collect();
    let instance_id = args.iter()
        .position(|arg| arg == "--instance")
        .and_then(|pos| args.get(pos + 1))
        .and_then(|s| s.parse::<u32>().ok());
    
    INSTANCE_ID.set(instance_id).ok();
    
    if let Some(id) = instance_id {
        println!("=== CryptoChat Instance #{} ===", id);
    }

    // Initialize application state
    let app_state = Arc::new(app::AppState::new());

    // Try to load existing keys from Windows Credential Manager
    if let Ok(Some(stored_key)) = keystore::load_keypair() {
        match cryptochat_crypto_core::pgp::PgpKeyPair::from_secret_key(&stored_key.secret_key_armored) {
            Ok(keypair) => {
                // Verify fingerprint matches
                if keypair.fingerprint() == stored_key.fingerprint {
                    app_state.set_keypair(keypair);
                    println!("✓ Loaded existing keys from secure storage");
                    println!("  Fingerprint: {}", stored_key.fingerprint);
                } else {
                    eprintln!("⚠ WARNING: Stored key fingerprint mismatch - keys NOT loaded for security");
                }
            }
            Err(e) => {
                eprintln!("⚠ Failed to parse stored key: {} - will generate new keys", e);
            }
        }
    }

    // Store in thread-local for window procedure access
    APP_STATE.with(|state| {
        *state.borrow_mut() = Some(app_state.clone());
    });

    // Create and show main window
    unsafe {
        let module_instance = GetModuleHandleW(None)?;
        
        // Use instance-specific window class name for multi-instance support
        let window_class_name = format!("CryptoChat.Window{}", get_instance_suffix());
        let window_class_wide: Vec<u16> = window_class_name.encode_utf16().chain(std::iter::once(0)).collect();

        let wc = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hInstance: module_instance.into(),
            lpszClassName: PCWSTR(window_class_wide.as_ptr()),
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(ui::window_proc),
            hbrBackground: HBRUSH((5 + 1) as *mut _), // COLOR_WINDOW + 1
            ..Default::default()
        };

        RegisterClassW(&wc);

        let window_title = format!("CryptoChat{} - Secure Messaging", get_instance_suffix());
        let window_title_wide: Vec<u16> = window_title.encode_utf16().chain(std::iter::once(0)).collect();

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(window_class_wide.as_ptr()),
            PCWSTR(window_title_wide.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            920,
            700, // Compact layout fits in smaller window
            None,
            None,
            module_instance,
            None,
        )?;

        let _ = ShowWindow(hwnd, SW_SHOW);

        // Message loop
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).into() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        Ok(())
    }
}

pub fn get_app_state() -> Option<Arc<app::AppState>> {
    APP_STATE.with(|state| state.borrow().clone())
}
