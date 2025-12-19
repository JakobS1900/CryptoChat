//! Color Store - Persistence for bubble style preferences
//!
//! Supports solid colors, gradients, and animated rainbow modes

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Bubble style options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BubbleStyle {
    /// Single solid color (hex string like "#2c7be5")
    Solid { color: String },
    /// Two-color gradient
    Gradient { color1: String, color2: String },
    /// Animated rainbow cycling
    Rainbow { speed: f32 },
}

impl Default for BubbleStyle {
    fn default() -> Self {
        BubbleStyle::Solid {
            color: "#2c7be5".to_string(), // Default blue
        }
    }
}

/// Color preferences stored in colors.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPreferences {
    pub bubble_style: BubbleStyle,
    pub hue: f32,        // 0.0 - 360.0
    pub saturation: f32, // 0.0 - 1.0
}

impl Default for ColorPreferences {
    fn default() -> Self {
        Self {
            bubble_style: BubbleStyle::default(),
            hue: 210.0,      // Blue hue
            saturation: 0.8, // High saturation
        }
    }
}

/// Get path to colors.json
fn get_colors_path() -> PathBuf {
    let base = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
    let instance_suffix = std::env::args()
        .skip_while(|a| a != "--instance")
        .nth(1)
        .map(|i| format!("_{}", i))
        .unwrap_or_default();
    
    PathBuf::from(format!("{}/.cryptochat{}/colors.json", base, instance_suffix))
}

/// Load color preferences from disk
pub fn load_preferences() -> ColorPreferences {
    let path = get_colors_path();
    if path.exists() {
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(prefs) = serde_json::from_str(&data) {
                return prefs;
            }
        }
    }
    ColorPreferences::default()
}

/// Save color preferences to disk
pub fn save_preferences(prefs: &ColorPreferences) -> Result<(), std::io::Error> {
    let path = get_colors_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(prefs)?;
    fs::write(path, json)
}

/// Convert HSL to hex color string
pub fn hsl_to_hex(hue: f32, saturation: f32, lightness: f32) -> String {
    let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = lightness - c / 2.0;
    
    let (r, g, b) = if hue < 60.0 {
        (c, x, 0.0)
    } else if hue < 120.0 {
        (x, c, 0.0)
    } else if hue < 180.0 {
        (0.0, c, x)
    } else if hue < 240.0 {
        (0.0, x, c)
    } else if hue < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    
    let r = ((r + m) * 255.0) as u8;
    let g = ((g + m) * 255.0) as u8;
    let b = ((b + m) * 255.0) as u8;
    
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

/// Parse hex color to RGB floats
pub fn hex_to_rgb(hex: &str) -> Option<(f32, f32, f32)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    
    let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
    
    Some((r, g, b))
}

/// Get rainbow color based on time offset (0.0 - 1.0 through spectrum)
pub fn rainbow_color(offset: f32) -> String {
    let hue = (offset * 360.0) % 360.0;
    hsl_to_hex(hue, 0.8, 0.5)
}
