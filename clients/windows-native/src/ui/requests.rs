//! Pending message requests UI

use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::System::LibraryLoader::GetModuleHandleW,
};

// Control IDs for requests window
pub const ID_LIST_REQUESTS: isize = 3001;
pub const ID_BUTTON_ACCEPT: isize = 3002;
pub const ID_BUTTON_REJECT: isize = 3003;
pub const ID_BUTTON_REFRESH: isize = 3004;
pub const ID_STATIC_FINGERPRINT: isize = 3005;
pub const ID_STATIC_PREVIEW: isize = 3006;

pub unsafe fn create_requests_controls(hwnd: HWND) {
    let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap().into();

    // Title
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Pending Message Requests"),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(0x00000001), // SS_CENTER
        20, 20, 560, 30,
        hwnd,
        None,
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Instructions
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Select a request below to accept or reject:"),
        WS_VISIBLE | WS_CHILD,
        20, 60, 560, 20,
        hwnd,
        None,
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Requests list (using LISTBOX)
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0x00000200), // WS_EX_CLIENTEDGE
        w!("LISTBOX"),
        w!(""),
        WS_VISIBLE | WS_CHILD | WS_VSCROLL | WINDOW_STYLE(0x0001), // LBS_NOTIFY
        20, 90, 560, 200,
        hwnd,
        HMENU(ID_LIST_REQUESTS as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Fingerprint label
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Fingerprint:"),
        WS_VISIBLE | WS_CHILD,
        20, 310, 560, 20,
        hwnd,
        None,
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Fingerprint display
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0x00000200), // WS_EX_CLIENTEDGE
        w!("EDIT"),
        w!("Select a request to view details"),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(0x0804), // ES_READONLY | ES_MULTILINE
        20, 335, 560, 60,
        hwnd,
        HMENU(ID_STATIC_FINGERPRINT as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Message preview label
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("First Message Preview:"),
        WS_VISIBLE | WS_CHILD,
        20, 410, 560, 20,
        hwnd,
        None,
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Message preview
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE(0x00000200), // WS_EX_CLIENTEDGE
        w!("EDIT"),
        w!(""),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(0x0804), // ES_READONLY | ES_MULTILINE
        20, 435, 560, 80,
        hwnd,
        HMENU(ID_STATIC_PREVIEW as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Accept button
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("Accept Request"),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP,
        20, 530, 180, 40,
        hwnd,
        HMENU(ID_BUTTON_ACCEPT as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Reject button
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("Reject Request"),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP,
        220, 530, 180, 40,
        hwnd,
        HMENU(ID_BUTTON_REJECT as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }

    // Refresh button
    if let Ok(ctrl) = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("Refresh"),
        WS_VISIBLE | WS_CHILD | WS_TABSTOP,
        420, 530, 160, 40,
        hwnd,
        HMENU(ID_BUTTON_REFRESH as *mut _),
        hinstance,
        None,
    ) {
        super::set_default_font(ctrl);
    }
}

/// Populate the requests list with pending requests
pub unsafe fn populate_requests_list(hwnd: HWND) {
    use windows::Win32::UI::Controls::*;

    if let Ok(listbox) = GetDlgItem(hwnd, ID_LIST_REQUESTS as i32) {
        // Clear existing items
        SendMessageW(listbox, LB_RESETCONTENT, WPARAM(0), LPARAM(0));

        // Load pending requests from storage
        match crate::request_store::load_pending_requests() {
            Ok(requests) => {
                if requests.is_empty() {
                    // Show "no requests" message
                    let msg = "(No pending requests)";
                    let msg_wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                    SendMessageW(listbox, LB_ADDSTRING, WPARAM(0), LPARAM(msg_wide.as_ptr() as isize));
                } else {
                    // Add each request to the list
                    for request in requests.iter() {
                        // Format: "From: ABC1...XYZ9 (2 minutes ago)"
                        let short_fp = if request.sender_fingerprint.len() > 12 {
                            format!("{}...{}",
                                &request.sender_fingerprint[..6],
                                &request.sender_fingerprint[request.sender_fingerprint.len()-6..])
                        } else {
                            request.sender_fingerprint.clone()
                        };

                        let display_text = format!("From: {} (Request ID: {})",
                            short_fp,
                            &request.request_id.to_string()[..8]);

                        let text_wide: Vec<u16> = display_text.encode_utf16().chain(std::iter::once(0)).collect();
                        SendMessageW(listbox, LB_ADDSTRING, WPARAM(0), LPARAM(text_wide.as_ptr() as isize));
                    }
                }
            }
            Err(e) => {
                let error_msg = format!("Error loading requests: {}", e);
                let error_wide: Vec<u16> = error_msg.encode_utf16().chain(std::iter::once(0)).collect();
                SendMessageW(listbox, LB_ADDSTRING, WPARAM(0), LPARAM(error_wide.as_ptr() as isize));
            }
        }
    }
}
