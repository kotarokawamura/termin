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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutSettings {
    pub copy: String,
    pub paste: String,
    pub split_horizontal: String,
    pub split_vertical: String,
    pub new_tab: String,
    pub close_pane: String,
    pub focus_next_pane: String,
    pub focus_prev_pane: String,
    pub open_preferences: String,
    pub toggle_fullscreen: String,
    pub zoom_in: String,
    pub zoom_out: String,
    pub zoom_reset: String,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            copy: "Ctrl+Shift+C".to_string(),
            paste: "Ctrl+Shift+V".to_string(),
            split_horizontal: "Ctrl+Shift+E".to_string(),
            split_vertical: "Ctrl+Shift+O".to_string(),
            new_tab: "Ctrl+Shift+T".to_string(),
            close_pane: "Ctrl+Shift+W".to_string(),
            focus_next_pane: "Ctrl+Tab".to_string(),
            focus_prev_pane: "Ctrl+Shift+Tab".to_string(),
            open_preferences: "Ctrl+,".to_string(),
            toggle_fullscreen: "F11".to_string(),
            zoom_in: "Ctrl+=".to_string(),
            zoom_out: "Ctrl+-".to_string(),
            zoom_reset: "Ctrl+0".to_string(),
        }
    }
}

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
    pub separator_width: f32,
    pub frame_border_width: f32,
    pub target_fps: u32,

    // Font configuration
    pub font_path: String,
    pub ui_font_size: f32,
    pub terminal_font_size: f32,

    // Internationalization (.po) configuration
    pub language: String,
    pub custom_po_path: String,

    // Shortcut keys configuration
    pub shortcuts: ShortcutSettings,
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
            separator_width: 2.0,
            frame_border_width: 5.0,
            target_fps: 30,
            font_path: default_debian_font_path().unwrap_or_default(),
            ui_font_size: 13.0,
            terminal_font_size: 14.0,
            language: "auto".to_string(),
            custom_po_path: String::new(),
            shortcuts: ShortcutSettings::default(),
        }
    }
}

impl ThemeSettings {
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config/termin/config.json")
    }

    fn legacy_config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config/termini/config.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let content = fs::read_to_string(&path)
            .or_else(|_| fs::read_to_string(Self::legacy_config_path()));
        if let Ok(content) = content {
            if let Ok(mut parsed) = serde_json::from_str::<Self>(&content) {
                if parsed.font_path.is_empty() {
                    parsed.font_path = default_debian_font_path().unwrap_or_default();
                }
                if parsed.language.trim().is_empty() {
                    parsed.language = "auto".to_string();
                }
                if parsed.target_fps == 0 {
                    parsed.target_fps = 30;
                }
                parsed.apply_language();
                return parsed;
            }
        }
        let default_settings = Self::default();
        default_settings.apply_language();
        default_settings
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

    /// Apply configured `.po` translation catalog.
    pub fn apply_language(&self) {
        crate::i18n::set_active_language(&self.language, &self.custom_po_path);
    }



    /// Build `FontDefinitions` containing the primary font, CJK fallback, and any preview sample fonts.
    pub fn build_font_definitions(
        &self,
        preview_fonts: &[(String, Arc<FontData>)],
    ) -> FontDefinitions {
        let mut fonts = FontDefinitions::default();

        let target_path = if !self.font_path.trim().is_empty() {
            Some(self.font_path.trim().to_string())
        } else {
            default_debian_font_path()
        };

        if let Some(path) = target_path {
            if let Ok(bytes) = fs::read(&path) {
                if is_valid_font_bytes(&bytes) {
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
        }

        // Also register a Japanese fallback font on Debian if the primary font is ASCII-only
        let mut has_cjk_fallback = false;
        if let Some(fallback_path) = default_debian_cjk_fallback() {
            if fallback_path != self.font_path {
                if let Ok(bytes) = fs::read(&fallback_path) {
                    if is_valid_font_bytes(&bytes) {
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
                        has_cjk_fallback = true;
                    }
                }
            }
        }

        // Register preview font families for the Font Sample Catalog screen
        for (fam_key, data) in preview_fonts {
            if !is_valid_font_bytes(&data.font) {
                continue;
            }
            fonts.font_data.insert(fam_key.clone(), Arc::clone(data));
            let mut list = vec![fam_key.clone()];
            if has_cjk_fallback {
                list.push("cjk_fallback".to_owned());
            } else if fonts.font_data.contains_key("custom_font") {
                list.push("custom_font".to_owned());
            }
            list.push("Hack".to_owned());
            fonts
                .families
                .insert(FontFamily::Name(fam_key.clone().into()), list);
        }

        fonts
    }

    /// Apply configured font file and UI font sizes to the egui Context.
    pub fn apply_fonts(&self, ctx: &egui::Context) {
        self.apply_fonts_with_previews(ctx, &[]);
    }

    /// Apply configured font file, preview sample fonts, and UI font sizes to the egui Context.
    pub fn apply_fonts_with_previews(
        &self,
        ctx: &egui::Context,
        preview_fonts: &[(String, Arc<FontData>)],
    ) {
        let fonts = self.build_font_definitions(preview_fonts);
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

    pub fn effective_separator_width(&self) -> f32 {
        self.separator_width.clamp(1.0, 16.0)
    }

    pub fn effective_fps(&self) -> u32 {
        self.target_fps.clamp(1, 240)
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
        "/usr/share/fonts/truetype/vlgothic/VL-Gothic-Regular.ttf",
        "/usr/share/fonts/truetype/vlgothic/VL-Gothic-regular.ttf",
        "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
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

pub fn is_valid_font_bytes(bytes: &[u8]) -> bool {
    if bytes.len() < 16 {
        return false;
    }
    let magic = &bytes[0..4];
    if magic == b"ttcf" {
        let num_fonts = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
        if num_fonts == 0 {
            return false;
        }
        let first_offset =
            u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        if first_offset + 12 > bytes.len() {
            return false;
        }
        let sub_magic = &bytes[first_offset..first_offset + 4];
        let num_tables =
            u16::from_be_bytes([bytes[first_offset + 4], bytes[first_offset + 5]]) as usize;
        matches!(sub_magic, [0x00, 0x01, 0x00, 0x00] | b"OTTO" | b"true" | b"typ1")
            && num_tables > 0
            && first_offset + 12 + num_tables * 16 <= bytes.len()
    } else if matches!(magic, [0x00, 0x01, 0x00, 0x00] | b"OTTO" | b"true" | b"typ1") {
        let num_tables = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        num_tables > 0 && 12 + num_tables * 16 <= bytes.len()
    } else {
        false
    }
}

pub fn is_monospace_font(name: &str, path: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let p = path.to_ascii_lowercase();
    n.contains("mono")
        || n.contains("fixed")
        || n.contains("console")
        || n.contains("code")
        || n.contains("courier")
        || n.starts_with("vl-gothic")
        || n.starts_with("ipag.")
        || n.starts_with("ipam.")
        || n.contains("unifont")
        || p.contains("mono")
}

pub fn is_cjk_font(name: &str, path: &str) -> bool {
    let n = name.to_ascii_lowercase();
    let p = path.to_ascii_lowercase();
    n.contains("cjk")
        || n.contains("vl-gothic")
        || n.contains("vl-pgothic")
        || n.contains("ipag")
        || n.contains("ipam")
        || n.contains("japanese")
        || n.contains("gothic")
        || n.contains("mincho")
        || n.contains("droidsansfallback")
        || n.contains("unifont_jp")
        || p.contains("vlgothic")
        || p.contains("ipafont")
        || p.contains("japanese")
}

fn font_sort_priority(name: &str, path: &str) -> u8 {
    let n = name.to_ascii_lowercase();
    let is_regular = n.contains("regular") || (!n.contains("bold") && !n.contains("italic") && !n.contains("oblique") && !n.contains("condensed") && !n.contains("light") && !n.contains("thin") && !n.contains("black"));
    if is_cjk_font(name, path) && is_monospace_font(name, path) {
        if is_regular { 0 } else { 1 }
    } else if is_cjk_font(name, path) {
        if is_regular { 2 } else { 3 }
    } else if is_monospace_font(name, path) {
        if is_regular { 4 } else { 5 }
    } else if n.starts_with("dejavu")
        || n.starts_with("liberation")
        || n.starts_with("roboto")
        || n.starts_with("lato")
        || n.starts_with("freesans")
        || n.starts_with("freeserif")
        || n.starts_with("notosans-")
        || n.starts_with("notoserif-")
    {
        6
    } else {
        7
    }
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

    found.sort_by(|a, b| {
        let pa = font_sort_priority(&a.0, &a.1);
        let pb = font_sort_priority(&b.0, &b.1);
        pa.cmp(&pb)
            .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
            .then_with(|| a.1.cmp(&b.1))
    });
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

/// Checks if a keyboard event matches a configured shortcut string (e.g., `"Ctrl+Shift+C"`, `"Alt+D"`, `"F11"`).
/// Multiple bindings can be separated by `/` or `,` (e.g., `"Ctrl+Shift+E / Alt+D"`).
pub fn shortcut_matches_event(
    binding: &str,
    key: egui::Key,
    physical_key: Option<egui::Key>,
    modifiers: egui::Modifiers,
    super_held: bool,
) -> bool {
    for part in binding.split(['/', '|']) {
        let trimmed = part.trim();
        if !trimmed.is_empty()
            && single_shortcut_matches(trimmed, key, physical_key, modifiers, super_held)
        {
            return true;
        }
    }
    false
}

fn single_shortcut_matches(
    binding: &str,
    key: egui::Key,
    physical_key: Option<egui::Key>,
    modifiers: egui::Modifiers,
    super_held: bool,
) -> bool {
    let mut want_ctrl = false;
    let mut want_shift = false;
    let mut want_alt = false;
    let mut want_super = false;
    let key_token: &str;

    // Handle trailing `+` or `-` keys like `"Ctrl+="`, `"Ctrl++"`, `"Ctrl+-"`
    let s = binding.trim();
    let mut rest = s;
    loop {
        if let Some((prefix, after)) = rest.split_once('+') {
            let p_low = prefix.trim().to_ascii_lowercase();
            match p_low.as_str() {
                "ctrl" | "control" => {
                    want_ctrl = true;
                    rest = after;
                    continue;
                }
                "shift" => {
                    want_shift = true;
                    rest = after;
                    continue;
                }
                "alt" | "meta" => {
                    want_alt = true;
                    rest = after;
                    continue;
                }
                "super" | "win" | "cmd" | "command" => {
                    want_super = true;
                    rest = after;
                    continue;
                }
                _ => {}
            }
        }
        key_token = rest.trim();
        break;
    }

    if key_token.is_empty() {
        return false;
    }

    let has_super = super_held || modifiers.mac_cmd;
    if modifiers.ctrl != want_ctrl
        || modifiers.shift != want_shift
        || modifiers.alt != want_alt
        || has_super != want_super
    {
        return false;
    }

    let Some(target_key) = parse_key_token(key_token) else {
        return false;
    };

    if key == target_key || physical_key == Some(target_key) {
        return true;
    }
    // Treat Plus and Equals as equivalent for zoom shortcuts
    if matches!(target_key, egui::Key::Plus | egui::Key::Equals)
        && (matches!(key, egui::Key::Plus | egui::Key::Equals)
            || matches!(physical_key, Some(egui::Key::Plus | egui::Key::Equals)))
    {
        return true;
    }
    false
}

pub fn parse_key_token(token: &str) -> Option<egui::Key> {
    let t = token.trim();
    match t.to_ascii_uppercase().as_str() {
        "A" => Some(egui::Key::A),
        "B" => Some(egui::Key::B),
        "C" => Some(egui::Key::C),
        "D" => Some(egui::Key::D),
        "E" => Some(egui::Key::E),
        "F" => Some(egui::Key::F),
        "G" => Some(egui::Key::G),
        "H" => Some(egui::Key::H),
        "I" => Some(egui::Key::I),
        "J" => Some(egui::Key::J),
        "K" => Some(egui::Key::K),
        "L" => Some(egui::Key::L),
        "M" => Some(egui::Key::M),
        "N" => Some(egui::Key::N),
        "O" => Some(egui::Key::O),
        "P" => Some(egui::Key::P),
        "Q" => Some(egui::Key::Q),
        "R" => Some(egui::Key::R),
        "S" => Some(egui::Key::S),
        "T" => Some(egui::Key::T),
        "U" => Some(egui::Key::U),
        "V" => Some(egui::Key::V),
        "W" => Some(egui::Key::W),
        "X" => Some(egui::Key::X),
        "Y" => Some(egui::Key::Y),
        "Z" => Some(egui::Key::Z),
        "0" | "NUM0" => Some(egui::Key::Num0),
        "1" | "NUM1" => Some(egui::Key::Num1),
        "2" | "NUM2" => Some(egui::Key::Num2),
        "3" | "NUM3" => Some(egui::Key::Num3),
        "4" | "NUM4" => Some(egui::Key::Num4),
        "5" | "NUM5" => Some(egui::Key::Num5),
        "6" | "NUM6" => Some(egui::Key::Num6),
        "7" | "NUM7" => Some(egui::Key::Num7),
        "8" | "NUM8" => Some(egui::Key::Num8),
        "9" | "NUM9" => Some(egui::Key::Num9),
        "TAB" => Some(egui::Key::Tab),
        "ENTER" | "RETURN" => Some(egui::Key::Enter),
        "SPACE" => Some(egui::Key::Space),
        "BACKSPACE" => Some(egui::Key::Backspace),
        "ESCAPE" | "ESC" => Some(egui::Key::Escape),
        "INSERT" | "INS" => Some(egui::Key::Insert),
        "DELETE" | "DEL" => Some(egui::Key::Delete),
        "HOME" => Some(egui::Key::Home),
        "END" => Some(egui::Key::End),
        "PAGEUP" | "PGUP" => Some(egui::Key::PageUp),
        "PAGEDOWN" | "PGDN" => Some(egui::Key::PageDown),
        "UP" | "ARROWUP" => Some(egui::Key::ArrowUp),
        "DOWN" | "ARROWDOWN" => Some(egui::Key::ArrowDown),
        "LEFT" | "ARROWLEFT" => Some(egui::Key::ArrowLeft),
        "RIGHT" | "ARROWRIGHT" => Some(egui::Key::ArrowRight),
        "," | "COMMA" => Some(egui::Key::Comma),
        "." | "PERIOD" => Some(egui::Key::Period),
        "/" | "SLASH" => Some(egui::Key::Slash),
        ";" | "SEMICOLON" => Some(egui::Key::Semicolon),
        "=" | "EQUALS" | "+" | "PLUS" => Some(egui::Key::Equals),
        "-" | "MINUS" => Some(egui::Key::Minus),
        "[" | "OPENBRACKET" => Some(egui::Key::OpenBracket),
        "]" | "CLOSEBRACKET" => Some(egui::Key::CloseBracket),
        "\\" | "BACKSLASH" => Some(egui::Key::Backslash),
        "F1" => Some(egui::Key::F1),
        "F2" => Some(egui::Key::F2),
        "F3" => Some(egui::Key::F3),
        "F4" => Some(egui::Key::F4),
        "F5" => Some(egui::Key::F5),
        "F6" => Some(egui::Key::F6),
        "F7" => Some(egui::Key::F7),
        "F8" => Some(egui::Key::F8),
        "F9" => Some(egui::Key::F9),
        "F10" => Some(egui::Key::F10),
        "F11" => Some(egui::Key::F11),
        "F12" => Some(egui::Key::F12),
        _ => None,
    }
}

/// Format a pressed key + modifiers into a human-readable shortcut string (e.g. `"Ctrl+Shift+T"`).
pub fn format_key_shortcut(
    key: egui::Key,
    modifiers: egui::Modifiers,
    super_held: bool,
) -> Option<String> {
    if matches!(
        key,
        egui::Key::SuperLeft | egui::Key::SuperRight
    ) {
        return None;
    }
    let key_str = match key {
        egui::Key::Comma => ",".to_string(),
        egui::Key::Period => ".".to_string(),
        egui::Key::Slash => "/".to_string(),
        egui::Key::Semicolon => ";".to_string(),
        egui::Key::Equals | egui::Key::Plus => "=".to_string(),
        egui::Key::Minus => "-".to_string(),
        egui::Key::OpenBracket => "[".to_string(),
        egui::Key::CloseBracket => "]".to_string(),
        egui::Key::Backslash => "\\".to_string(),
        egui::Key::Num0 => "0".to_string(),
        egui::Key::Num1 => "1".to_string(),
        egui::Key::Num2 => "2".to_string(),
        egui::Key::Num3 => "3".to_string(),
        egui::Key::Num4 => "4".to_string(),
        egui::Key::Num5 => "5".to_string(),
        egui::Key::Num6 => "6".to_string(),
        egui::Key::Num7 => "7".to_string(),
        egui::Key::Num8 => "8".to_string(),
        egui::Key::Num9 => "9".to_string(),
        other => format!("{:?}", other),
    };

    let mut parts = Vec::new();
    if super_held || modifiers.mac_cmd {
        parts.push("Super".to_string());
    }
    if modifiers.ctrl {
        parts.push("Ctrl".to_string());
    }
    if modifiers.alt {
        parts.push("Alt".to_string());
    }
    if modifiers.shift {
        parts.push("Shift".to_string());
    }
    parts.push(key_str);
    Some(parts.join("+"))
}
