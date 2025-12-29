//! CryptoChat Dark Theme
//!
//! Signal/Session-inspired dark color palette

use iced::Color;

/// Dark theme color palette - Modern Glassmorphism + Gradient Glow
pub mod colors {
    use super::Color;
    
    // Background colors - Deep space purple theme
    pub const BACKGROUND: Color = Color::from_rgb(0.059, 0.059, 0.102);       // #0f0f1a - deep space
    pub const SIDEBAR_BG: Color = Color::from_rgb(0.078, 0.078, 0.157);       // #141428 - glass base
    pub const CARD_BG: Color = Color::from_rgb(0.102, 0.102, 0.208);          // #1a1a35 - elevated surface
    pub const SURFACE: Color = Color::from_rgb(0.133, 0.133, 0.243);          // #222240 - interactive surface
    
    // Accent colors - Vibrant purple/cyan gradient palette
    pub const ACCENT_PRIMARY: Color = Color::from_rgb(0.486, 0.227, 0.929);   // #7c3aed - vibrant purple
    pub const ACCENT_SECONDARY: Color = Color::from_rgb(0.024, 0.714, 0.831); // #06b6d4 - cyan glow
    pub const ACCENT_TERTIARY: Color = Color::from_rgb(0.925, 0.282, 0.600);  // #ec4899 - pink accent
    pub const SUCCESS_GREEN: Color = Color::from_rgb(0.134, 0.810, 0.514);    // #22cf83 - modern green
    pub const WARNING_AMBER: Color = Color::from_rgb(0.976, 0.659, 0.145);    // #f9a825 - warm amber
    pub const ERROR_RED: Color = Color::from_rgb(0.937, 0.267, 0.267);        // #ef4444 - soft red
    
    // Text colors
    pub const TEXT_PRIMARY: Color = Color::from_rgb(0.949, 0.949, 0.969);     // #f2f2f7 - bright white
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.631, 0.631, 0.690);   // #a1a1b0 - muted
    pub const TEXT_MUTED: Color = Color::from_rgb(0.447, 0.447, 0.510);       // #727282 - subtle
    
    // Glass effect colors
    pub const GLASS_BORDER: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.1);     // White 10% opacity
    pub const GLASS_HIGHLIGHT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.05); // White 5% highlight
    pub const GLOW_PURPLE: Color = Color::from_rgba(0.486, 0.227, 0.929, 0.4); // Purple glow 40%
    pub const GLOW_CYAN: Color = Color::from_rgba(0.024, 0.714, 0.831, 0.4);   // Cyan glow 40%
    
    // Border/Divider
    pub const DIVIDER: Color = Color::from_rgb(0.180, 0.180, 0.255);          // #2e2e41
    pub const BORDER: Color = Color::from_rgb(0.220, 0.220, 0.310);           // #38384f
    
    // Message bubbles - Enhanced with gradients
    pub const BUBBLE_MINE: Color = Color::from_rgb(0.486, 0.227, 0.929);      // #7c3aed (accent purple)
    pub const BUBBLE_THEIRS: Color = Color::from_rgb(0.133, 0.133, 0.243);    // #222240 (surface)
}

/// Container style for dark background
pub fn dark_container() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::BACKGROUND)),
        text_color: Some(colors::TEXT_PRIMARY),
        ..Default::default()
    }
}

/// Sidebar container style - Glassmorphism effect
pub fn sidebar_container() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::SIDEBAR_BG)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            color: colors::GLASS_BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            offset: iced::Vector { x: 4.0, y: 0.0 },
            blur_radius: 16.0,
        },
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

/// Style for reaction pills (small buttons below message) - Updated for new theme
pub fn reaction_pill(_theme: &iced::Theme) -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::SURFACE)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            radius: 12.0.into(),
            color: colors::GLASS_BORDER,
            width: 1.0,
        },
        ..Default::default()
    }
}

// ============================================================================
// MODERN UI STYLES - Glassmorphism, Gradient Glow, Neumorphism
// ============================================================================

/// Glassmorphism container - frosted glass effect
pub fn glass_container() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(Color::from_rgba(0.1, 0.1, 0.2, 0.8))),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            color: colors::GLASS_BORDER,
            width: 1.0,
            radius: 16.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
            offset: iced::Vector { x: 0.0, y: 8.0 },
            blur_radius: 24.0,
        },
    }
}

/// Glass card - elevated panel with glassmorphism
pub fn glass_card() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::CARD_BG)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            color: colors::GLASS_BORDER,
            width: 1.0,
            radius: 12.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.2),
            offset: iced::Vector { x: 0.0, y: 4.0 },
            blur_radius: 12.0,
        },
    }
}

/// Conversation list item - hover/selected state
pub fn conversation_item_selected() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::SURFACE)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            color: colors::ACCENT_PRIMARY,
            width: 0.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow {
            color: colors::GLOW_PURPLE,
            offset: iced::Vector { x: 0.0, y: 0.0 },
            blur_radius: 8.0,
        },
    }
}

/// Conversation list item - default state
pub fn conversation_item() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Input container - text input wrapper with subtle styling
pub fn input_container() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::SURFACE)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

/// Message input bar container - bottom bar styling
pub fn message_input_bar() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::SIDEBAR_BG)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            color: colors::DIVIDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// Header bar - top section styling
pub fn header_bar() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::SIDEBAR_BG)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            color: colors::DIVIDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.15),
            offset: iced::Vector { x: 0.0, y: 2.0 },
            blur_radius: 8.0,
        },
    }
}

/// Unread badge - notification count bubble
pub fn unread_badge() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::ACCENT_TERTIARY)),
        text_color: Some(Color::WHITE),
        border: iced::Border {
            radius: 10.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Status indicator - online/typing dot
pub fn status_dot_online() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::SUCCESS_GREEN)),
        border: iced::Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Emoji picker container
pub fn emoji_picker() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::CARD_BG)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            color: colors::GLASS_BORDER,
            width: 1.0,
            radius: 12.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            offset: iced::Vector { x: 0.0, y: -4.0 },
            blur_radius: 16.0,
        },
    }
}

/// Modern bubble for "my" messages - enhanced with glow
pub fn modern_bubble_mine() -> iced::widget::container::Appearance {
    let packed = CUSTOM_BUBBLE_COLOR.load(Ordering::Relaxed);
    let r = ((packed >> 16) & 0xFF) as f32 / 255.0;
    let g = ((packed >> 8) & 0xFF) as f32 / 255.0;
    let b = (packed & 0xFF) as f32 / 255.0;
    
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(Color::from_rgb(r, g, b))),
        text_color: Some(text_color_for_bg(r, g, b)),
        border: iced::Border {
            radius: 18.0.into(),
            ..Default::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(r * 0.5, g * 0.5, b * 0.5, 0.4),
            offset: iced::Vector { x: 0.0, y: 4.0 },
            blur_radius: 12.0,
        },
    }
}

/// Modern bubble for "their" messages - enhanced glass effect with cyan accent
pub fn modern_bubble_theirs() -> iced::widget::container::Appearance {
    // Use a slightly brighter base for better contrast
    let base_r = 0.18;  // Brighter than default #222240
    let base_g = 0.18;
    let base_b = 0.28;  // Slight purple tint
    
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(Color::from_rgb(base_r, base_g, base_b))),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            color: Color::from_rgba(0.4, 0.7, 0.9, 0.3), // Cyan-ish border at 30% opacity
            width: 1.5,
            radius: 18.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.024, 0.714, 0.831, 0.25), // Cyan glow at 25%
            offset: iced::Vector { x: 0.0, y: 3.0 },
            blur_radius: 10.0,
        },
    }
}

/// Section header style (e.g., "Contacts", "Groups")
pub fn section_header() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(Color::TRANSPARENT)),
        text_color: Some(colors::TEXT_SECONDARY),
        border: iced::Border {
            color: colors::DIVIDER,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// Modal overlay background
pub fn modal_overlay() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.6))),
        ..Default::default()
    }
}

/// Modal content container
pub fn modal_content() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::CARD_BG)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            color: colors::GLASS_BORDER,
            width: 1.0,
            radius: 16.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: iced::Vector { x: 0.0, y: 8.0 },
            blur_radius: 32.0,
        },
    }
}
