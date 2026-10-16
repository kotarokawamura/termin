use std::fs;
use std::path::PathBuf;
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThemeSettings {
    pub title_active_bg: String,
    pub title_active_fg: String,
    pub title_inactive_bg: String,
    pub title_inactive_fg: String,
    pub frame_bg_color: String,
    pub frame_border_color: String,
    pub separator_color: String,
    pub frame_border_width: f32,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            title_active_bg: String::new(),
            title_active_fg: String::new(),
            title_inactive_bg: String::new(),
            title_inactive_fg: String::new(),
            frame_bg_color: "#2b2b2b".to_string(),
            frame_border_color: "#5a5a5a".to_string(),
            separator_color: String::new(),
            frame_border_width: 5.0,
        }
    }
}

impl ThemeSettings {
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config/terminechan/config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(parsed) = serde_json::from_str::<Self>(&content) {
                return parsed;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }

    pub fn effective_border_width(&self) -> f32 {
        self.frame_border_width.clamp(1.0, 20.0)
    }

    pub fn titlebar_bg(&self, is_active: bool) -> Color32 {
        if is_active {
            parse_color(&self.title_active_bg).unwrap_or(Color32::from_rgb(200, 0, 3))
        } else {
            parse_color(&self.title_inactive_bg).unwrap_or(Color32::from_rgb(75, 75, 75))
        }
    }

    pub fn titlebar_fg(&self, is_active: bool) -> Color32 {
        if is_active {
            parse_color(&self.title_active_fg).unwrap_or(Color32::WHITE)
        } else {
            parse_color(&self.title_inactive_fg).unwrap_or(Color32::from_rgb(210, 210, 210))
        }
    }

    pub fn frame_bg_color(&self) -> Color32 {
        parse_color(&self.frame_bg_color).unwrap_or(Color32::from_rgb(43, 43, 43))
    }

    pub fn frame_border_color(&self) -> Color32 {
        parse_color(&self.frame_border_color).unwrap_or(Color32::from_rgb(90, 90, 90))
    }

    pub fn separator_color(&self) -> Color32 {
        parse_color(&self.separator_color).unwrap_or(Color32::from_rgb(74, 74, 74))
    }
}

/// Parse `#rrggbb` or `rgb(r,g,b)` into `egui::Color32`. Returns `None` if empty or invalid.
pub fn parse_color(input: &str) -> Option<Color32> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }

    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color32::from_rgb(r, g, b));
        }
    }

    if let Some(inner) = s.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() == 3 {
            let r = parts[0].trim().parse::<u8>().ok()?;
            let g = parts[1].trim().parse::<u8>().ok()?;
            let b = parts[2].trim().parse::<u8>().ok()?;
            return Some(Color32::from_rgb(r, g, b));
        }
    }

    None
}

pub fn color_to_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}
