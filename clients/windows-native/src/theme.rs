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
        ..Default::default()
    }
}

/// Their message bubble style
pub fn their_bubble() -> iced::widget::container::Appearance {
    iced::widget::container::Appearance {
        background: Some(iced::Background::Color(colors::BUBBLE_THEIRS)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: iced::Border {
            radius: 16.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
