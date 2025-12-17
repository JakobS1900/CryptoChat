//! Onboarding UI for key generation

use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
};

pub const ID_BUTTON_GENERATE: isize = 1001;
pub const ID_BUTTON_COPY_KEY: isize = 1002;
pub const ID_BUTTON_GENERATE_QR: isize = 1010;
pub const ID_EDIT_FINGERPRINT: isize = 1003;
pub const ID_EDIT_PUBLIC_KEY: isize = 1004;
pub const ID_EDIT_RECIPIENT_KEY: isize = 1005;
pub const ID_BUTTON_IMPORT_KEY: isize = 1006;
pub const ID_BUTTON_SCAN_QR: isize = 1011;
pub const ID_EDIT_PEER_ADDRESS: isize = 1008;
pub const ID_LABEL_PEER_ADDRESS: isize = 1009;
pub const ID_BUTTON_START_CHAT: isize = 1007;
pub const ID_BUTTON_VIEW_REQUESTS: isize = 1012;
pub const ID_LABEL_MY_PORT: isize = 1013;
pub const ID_EDIT_MY_PORT: isize = 1014;
pub const ID_BUTTON_CONTINUE: isize = 1015;

pub unsafe fn create_onboarding_controls(hwnd: HWND) {
    let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();

    // View Requests button (top-right)
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("View Requests"),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP,
        880, 15, 180, 30,
        hwnd,
        HMENU(ID_BUTTON_VIEW_REQUESTS as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Title text
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Welcome to CryptoChat"),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(0x00000001), // SS_CENTER
        300, 20, 600, 30,
        hwnd,
        None,
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Generate Keys button
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("Generate Encryption Keys"),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP,
        425, 65, 350, 35,
        hwnd,
        HMENU(ID_BUTTON_GENERATE as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Fingerprint label
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Your Fingerprint:"),
        WS_VISIBLE | WS_CHILD,
        30, 120, 150, 18,
        hwnd,
        None,
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Fingerprint display (read-only edit control)
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0x00000200), // WS_EX_CLIENTEDGE
        w!("EDIT"),
        w!(""),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(0x0800) | WINDOW_STYLE(0x0001), // ES_AUTOHSCROLL | ES_READONLY
        30, 140, 380, 24,
        hwnd,
        HMENU(ID_EDIT_FINGERPRINT as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // My Port label
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("My Port:"),
        WS_VISIBLE | WS_CHILD,
        420, 120, 80, 18,
        hwnd,
        HMENU(ID_LABEL_MY_PORT as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // My Port display (read-only edit control)
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0x00000200), // WS_EX_CLIENTEDGE
        w!("EDIT"),
        w!(""),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(0x0800) | WINDOW_STYLE(0x0001), // ES_AUTOHSCROLL | ES_READONLY
        420, 140, 170, 24,
        hwnd,
        HMENU(ID_EDIT_MY_PORT as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Copy Public Key button (moved next to fingerprint)
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("Copy Key"),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP,
        600, 140, 130, 24,
        hwnd,
        HMENU(ID_BUTTON_COPY_KEY as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Generate QR Code button (moved next to Copy)
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("QR Code"),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP,
        740, 140, 130, 24,
        hwnd,
        HMENU(ID_BUTTON_GENERATE_QR as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Public key label
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Your Public Key (share with contacts):"),
        WS_VISIBLE | WS_CHILD,
        30, 180, 320, 18,
        hwnd,
        None,
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Public key display (multiline edit control)
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0x00000200), // WS_EX_CLIENTEDGE
        w!("EDIT"),
        w!(""),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(0x0004) | WINDOW_STYLE(0x0001) | WS_VSCROLL, // ES_MULTILINE | ES_READONLY
        30, 200, 840, 150,
        hwnd,
        HMENU(ID_EDIT_PUBLIC_KEY as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Separator line (using static text)
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("--- Import Recipient's Key ---"),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(0x00000001), // SS_CENTER
        300, 365, 600, 18,
        hwnd,
        None,
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Recipient key label
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Paste Recipient's Public Key:"),
        WS_VISIBLE | WS_CHILD,
        30, 395, 250, 18,
        hwnd,
        None,
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Recipient key input (multiline edit control)
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0x00000200), // WS_EX_CLIENTEDGE
        w!("EDIT"),
        w!(""),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(0x0004) | WS_VSCROLL, // ES_MULTILINE
        30, 415, 840, 120,
        hwnd,
        HMENU(ID_EDIT_RECIPIENT_KEY as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Import Key button
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("Import & Verify Key"),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP,
        300, 545, 220, 30,
        hwnd,
        HMENU(ID_BUTTON_IMPORT_KEY as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Scan QR Code button (next to Import)
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("OR Scan QR Code"),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP,
        530, 545, 220, 30,
        hwnd,
        HMENU(ID_BUTTON_SCAN_QR as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Peer address label
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Peer Address (e.g. 127.0.0.1:62780):"),
        WS_CHILD, // Hidden initially
        30, 590, 300, 18,
        hwnd,
        HMENU(ID_LABEL_PEER_ADDRESS as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Peer address input
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0x00000200), // WS_EX_CLIENTEDGE
        w!("EDIT"),
        w!("127.0.0.1:62780"),
        WS_CHILD | WINDOW_STYLE(0x0800), // ES_AUTOHSCROLL, hidden initially
        30, 610, 620, 24,
        hwnd,
        HMENU(ID_EDIT_PEER_ADDRESS as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Start Chat button (initially hidden until key imported)
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("Start Chat"),
        WS_CHILD | WS_TABSTOP, // Not visible initially
        660, 610, 210, 24,
        hwnd,
        HMENU(ID_BUTTON_START_CHAT as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Continue button (hidden until keys are generated) - for users who just want to receive messages
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("Continue (Ready to Receive)"),
        WS_CHILD | WS_TABSTOP, // Not visible initially
        425, 365, 350, 35,
        hwnd,
        HMENU(ID_BUTTON_CONTINUE as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }
}

pub unsafe fn paint_onboarding(hwnd: HWND) {
    use windows::Win32::Graphics::Gdi::*;

    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    if !hdc.is_invalid() {
        // Just validate the paint region - child controls handle their own painting
        let _ = EndPaint(hwnd, &ps);
    }
}
