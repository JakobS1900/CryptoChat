//! Chat UI window

use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
};

pub const ID_EDIT_MESSAGE_HISTORY: isize = 2001;
pub const ID_EDIT_MESSAGE_INPUT: isize = 2002;
pub const ID_BUTTON_SEND: isize = 2003;

pub unsafe fn create_chat_controls(hwnd: HWND) {
    let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();

    // Title
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("CryptoChat - End-to-End Encrypted Messaging"),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(0x00000001), // SS_CENTER
        300, 20, 600, 30,
        hwnd,
        None,
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Message history (read-only multiline edit)
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0x00000200), // WS_EX_CLIENTEDGE
        w!("EDIT"),
        w!(""),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(0x0004) | WINDOW_STYLE(0x0001) | WS_VSCROLL, // ES_MULTILINE | ES_READONLY
        50, 70, 1100, 700,
        hwnd,
        HMENU(ID_EDIT_MESSAGE_HISTORY as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Message input label
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Your Message:"),
        WS_VISIBLE | WS_CHILD,
        50, 790, 150, 20,
        hwnd,
        None,
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Message input (multiline edit)
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0x00000200), // WS_EX_CLIENTEDGE
        w!("EDIT"),
        w!(""),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(0x0004) | WS_VSCROLL, // ES_MULTILINE
        50, 815, 950, 80,
        hwnd,
        HMENU(ID_EDIT_MESSAGE_INPUT as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Send button
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("Send Encrypted Message"),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP,
        1020, 815, 130, 80,
        hwnd,
        HMENU(ID_BUTTON_SEND as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }
}

pub unsafe fn paint_chat(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);

    if !hdc.is_invalid() {
        // Controls handle their own painting
        let _ = EndPaint(hwnd, &ps);
    }
}
