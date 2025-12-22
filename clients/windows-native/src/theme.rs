//! CryptoChat Dark Theme
//!
//! Signal/Session-inspired dark color palette

use iced::Color;

/// Dark theme color palette
pub mod colors {
    use super::Color;
    
    // Background colors
    pub const BACKGROUND: Color = Color::from_rgb(0.102, 0.102, 0.118);       // #1a1a1e
    pub const SIDEBAR_BG: Color = Color::from_rgb(0.078, 0.078, 0.090);       // #141417
    pub const CARD_BG: Color = Color::from_rgb(0.165, 0.165, 0.180);          // #2a2a2e
    
    // Accent colors
    pub const ACCENT_BLUE: Color = Color::from_rgb(0.173, 0.482, 0.898);      // #2c7be5
    pub const LINK_BLUE: Color = Color::from_rgb(0.039, 0.518, 1.0);          // #0a84ff
    pub const SUCCESS_GREEN: Color = Color::from_rgb(0.196, 0.706, 0.196);    // #32b432
    pub const WARNING_ORANGE: Color = Color::from_rgb(1.0, 0.584, 0.0);       // #ff9500
    pub const ERROR_RED: Color = Color::from_rgb(1.0, 0.231, 0.188);          // #ff3b30
    
    // Text colors
    pub const TEXT_PRIMARY: Color = Color::WHITE;
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.557, 0.557, 0.576);   // #8e8e93
    pub const TEXT_MUTED: Color = Color::from_rgb(0.400, 0.400, 0.420);       // #666666
    
    // Border/Divider
    pub const DIVIDER: Color = Color::from_rgb(0.227, 0.227, 0.235);          // #3a3a3c
    pub const BORDER: Color = Color::from_rgb(0.300, 0.300, 0.320);           // #4d4d52
    
    // Message bubbles
    pub const BUBBLE_MINE: Color = Color::from_rgb(0.173, 0.482, 0.898);      // #2c7be5 (accent blue)
    pub const BUBBLE_THEIRS: Color = Color::from_rgb(0.165, 0.165, 0.180);    // #2a2a2e (card bg)
}

/// Container style for dark background
pub fn dark_container() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::BACKGROUND)),
        text_color: Some(colors::TEXT_PRIMARY),
        ..Default::default()
    }
}

/// Sidebar container style
pub fn sidebar_container() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::SIDEBAR_BG)),
        text_color: Some(colors::TEXT_PRIMARY),
        ..Default::default()
    }
}

/// Card container style (for message bubbles, panels)
pub fn card_container() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::CARD_BG)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            radius: 12.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// My message bubble style
pub fn my_bubble() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::BUBBLE_MINE)),
        text_color: Some(Color::WHITE),
        border: iced::Border {
            radius: 16.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.2),
            offset: iced::Vector { x: 2.0, y: 2.0 },
            blur_radius: 8.0,
        },
    }
}

/// Their message bubble style
pub fn their_bubble() -> iced::widget::container::Appearance {
    let packed = THEIR_BUBBLE_COLOR.load(Ordering::Relaxed);
    let r = ((packed >> 16) & 0xFF) as f32 / 255.0;
    let g = ((packed >> 8) & 0xFF) as f32 / 255.0;
    let b = (packed & 0xFF) as f32 / 255.0;

    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(Color::from_rgb(r, g, b))),
        text_color: Some(text_color_for_bg(r, g, b)),
        border: iced::Border {
            radius: 16.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.15),
            offset: iced::Vector { x: 2.0, y: 2.0 },
            blur_radius: 6.0,
        },
    }
}

/// Custom colored bubble for user's messages
pub fn custom_bubble(r: f32, g: f32, b: f32) -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(Color::from_rgb(r, g, b))),
        text_color: Some(Color::WHITE),
        border: iced::Border {
            radius: 16.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

use std::sync::atomic::{AtomicU32, Ordering};

// Static storage for custom bubble color (packed as RGB u32)
static CUSTOM_BUBBLE_COLOR: AtomicU32 = AtomicU32::new(0x2c7be5); // Default blue
// Second color for gradient mode
static GRADIENT_COLOR_2: AtomicU32 = AtomicU32::new(0x9b59b6); // Default purple
// Static storage for "their" bubble color
static THEIR_BUBBLE_COLOR: AtomicU32 = AtomicU32::new(0x2a2a2e); // Default dark gray

/// Set the custom bubble color from RGB floats
pub fn set_bubble_color(r: f32, g: f32, b: f32) {
    let packed = ((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32;
    CUSTOM_BUBBLE_COLOR.store(packed, Ordering::Relaxed);
}

/// Set "their" bubble color from RGB floats
pub fn set_their_bubble_color(r: f32, g: f32, b: f32) {
    let packed = ((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32;
    THEIR_BUBBLE_COLOR.store(packed, Ordering::Relaxed);
}

/// Set the gradient second color
pub fn set_gradient_color2(r: f32, g: f32, b: f32) {
    let packed = ((r * 255.0) as u32) << 16 | ((g * 255.0) as u32) << 8 | (b * 255.0) as u32;
    GRADIENT_COLOR_2.store(packed, Ordering::Relaxed);
}

/// Calculate luminance and determine if color is light (needs dark text)
fn is_light_color(r: f32, g: f32, b: f32) -> bool {
    // Standard luminance formula
    let luminance = 0.299 * r + 0.587 * g + 0.114 * b;
    luminance > 0.5
}

/// Get text color based on background luminance
fn text_color_for_bg(r: f32, g: f32, b: f32) -> Color {
    if is_light_color(r, g, b) {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

/// Get my_bubble with custom color from static storage (color 1)
pub fn my_bubble_custom() -> iced::widget::container::Appearance {
    let packed = CUSTOM_BUBBLE_COLOR.load(Ordering::Relaxed);
    let r = ((packed >> 16) & 0xFF) as f32 / 255.0;
    let g = ((packed >> 8) & 0xFF) as f32 / 255.0;
    let b = (packed & 0xFF) as f32 / 255.0;
    
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(Color::from_rgb(r, g, b))),
        text_color: Some(text_color_for_bg(r, g, b)),
        border: iced::Border {
            radius: 16.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.2),
            offset: iced::Vector { x: 2.0, y: 2.0 },
            blur_radius: 8.0,
        },
    }
}

/// Get my_bubble with gradient color 2 (for alternating messages)
pub fn my_bubble_gradient2() -> iced::widget::container::Appearance {
    let packed = GRADIENT_COLOR_2.load(Ordering::Relaxed);
    let r = ((packed >> 16) & 0xFF) as f32 / 255.0;
    let g = ((packed >> 8) & 0xFF) as f32 / 255.0;
    let b = (packed & 0xFF) as f32 / 255.0;
    
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(Color::from_rgb(r, g, b))),
        text_color: Some(text_color_for_bg(r, g, b)),
        border: iced::Border {
            radius: 16.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.2),
            offset: iced::Vector { x: 2.0, y: 2.0 },
            blur_radius: 8.0,
        },
    }
}

/// Style for reaction pills (small buttons below message)
pub fn reaction_pill(_theme: &iced::Theme) -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(Color::from_rgb(0.18, 0.18, 0.20))), // Slightly lighter than bg
        text_color: Some(Color::WHITE),
        border: iced::Border {
            radius: 12.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

