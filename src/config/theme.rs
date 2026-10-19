// Copyright (C) [2026/10/19] kotaro,termin
//
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; version 2
// of the License.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program; if not, write to the Free Software
// Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301, USA.
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeSettings {
    pub title_active_bg: String,
    pub title_active_fg: String,
    pub title_inactive_bg: String,
    pub title_inactive_fg: String,
    pub frame_bg_color: String,
    pub frame_border_color: String,
    pub separator_color: String,
    pub frame_border_width: f32,

    // Font configuration
    pub font_path: String,
    pub ui_font_size: f32,
    pub terminal_font_size: f32,
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
            font_path: default_debian_font_path().unwrap_or_default(),
            ui_font_size: 13.0,
            terminal_font_size: 14.0,
        }
    }
}

impl ThemeSettings {
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config/termini/config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(mut parsed) = serde_json::from_str::<Self>(&content) {
                if parsed.font_path.is_empty() {
                    parsed.font_path = default_debian_font_path().unwrap_or_default();
                }
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

    /// Apply configured font file and UI font sizes to the egui Context.
    pub fn apply_fonts(&self, ctx: &egui::Context) {
        let mut fonts = FontDefinitions::default();

        let target_path = if !self.font_path.trim().is_empty() {
            Some(self.font_path.trim().to_string())
        } else {
            default_debian_font_path()
        };

        if let Some(path) = target_path {
            if let Ok(bytes) = fs::read(&path) {
                fonts.font_data.insert(
                    "custom_font".to_owned(),
                    Arc::new(FontData::from_owned(bytes)),
                );

                // Put custom font at the highest priority for both Proportional and Monospace
                fonts
                    .families
                    .entry(FontFamily::Proportional)
                    .or_default()
                    .insert(0, "custom_font".to_owned());

                fonts
                    .families
                    .entry(FontFamily::Monospace)
                    .or_default()
                    .insert(0, "custom_font".to_owned());
            }
        }

        // Also register a Japanese fallback font on Debian if the primary font is ASCII-only
        if let Some(fallback_path) = default_debian_cjk_fallback() {
            if fallback_path != self.font_path {
                if let Ok(bytes) = fs::read(&fallback_path) {
                    fonts.font_data.insert(
                        "cjk_fallback".to_owned(),
                        Arc::new(FontData::from_owned(bytes)),
                    );
                    fonts
                        .families
                        .entry(FontFamily::Proportional)
                        .or_default()
                        .push("cjk_fallback".to_owned());
                    fonts
                        .families
                        .entry(FontFamily::Monospace)
                        .or_default()
                        .push("cjk_fallback".to_owned());
                }
            }
        }

        ctx.set_fonts(fonts);

        // Apply UI font size across standard text styles
        let ui_size = self.effective_ui_font_size();
        ctx.all_styles_mut(|style| {
            style.text_styles.insert(
                TextStyle::Body,
                FontId::new(ui_size, FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Button,
                FontId::new(ui_size, FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Monospace,
                FontId::new(ui_size, FontFamily::Monospace),
            );
            style.text_styles.insert(
                TextStyle::Small,
                FontId::new((ui_size - 2.0).max(8.0), FontFamily::Proportional),
            );
            style.text_styles.insert(
                TextStyle::Heading,
                FontId::new(ui_size + 4.0, FontFamily::Proportional),
            );
        });
    }

    pub fn effective_ui_font_size(&self) -> f32 {
        self.ui_font_size.clamp(8.0, 32.0)
    }

    pub fn effective_terminal_font_size(&self) -> f32 {
        self.terminal_font_size.clamp(8.0, 48.0)
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

/// Find a sensible default Japanese-capable font on Debian systems.
fn default_debian_font_path() -> Option<String> {
    default_debian_cjk_fallback()
}

fn default_debian_cjk_fallback() -> Option<String> {
    let candidates = [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansMonoCJKjp-Regular.otf",
        "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
        "/usr/share/fonts/truetype/vlgothic/VL-Gothic-regular.ttf",
        "/usr/share/fonts/opentype/ipafont-gothic/ipag.ttf",
        "/usr/share/fonts/truetype/ipafont-gothic/ipag.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    ];
    for path in candidates {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    None
}

/// Scan standard Debian font directories for available font files.
pub fn discover_system_fonts() -> Vec<(String, String)> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/fonts"));
        dirs.push(PathBuf::from(&home).join(".fonts"));
    }

    let mut found = Vec::new();
    for dir in dirs {
        collect_fonts_recursive(&dir, &mut found);
    }

    found.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    found.dedup_by(|a, b| a.1 == b.1);
    found
}

fn collect_fonts_recursive(dir: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_fonts_recursive(&path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_ascii_lowercase();
            if matches!(ext_lower.as_str(), "ttf" | "otf" | "ttc") {
                if let (Some(name), Some(full_path)) =
                    (path.file_name().and_then(|n| n.to_str()), path.to_str())
                {
                    out.push((name.to_string(), full_path.to_string()));
                }
            }
        }
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