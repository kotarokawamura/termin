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

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

const EMBEDDED_EN_PO: &str = include_str!("../../po/en.po");
const EMBEDDED_JA_PO: &str = include_str!("../../po/ja.po");

/// Represents a parsed GNU gettext `.po` translation catalog.
#[derive(Clone, Debug, Default)]
pub struct PoCatalog {
    pub locale: String,
    pub headers: HashMap<String, String>,
    pub messages: HashMap<String, String>,
    pub plural_messages: HashMap<String, Vec<String>>,
}

impl PoCatalog {
    /// Parse a GNU gettext `.po` file content into a `PoCatalog`.
    pub fn parse(locale: &str, content: &str) -> Self {
        let mut catalog = Self {
            locale: locale.to_string(),
            headers: HashMap::new(),
            messages: HashMap::new(),
            plural_messages: HashMap::new(),
        };

        #[derive(PartialEq, Eq)]
        enum Section {
            None,
            Msgctxt,
            Msgid,
            MsgidPlural,
            Msgstr,
            MsgstrPlural(usize),
        }

        let mut current_section = Section::None;
        let mut msgctxt: Option<String> = None;
        let mut msgid = String::new();
        let mut msgid_plural: Option<String> = None;
        let mut msgstr = String::new();
        let mut msgstr_plural: Vec<(usize, String)> = Vec::new();
        let mut is_fuzzy = false;
        let mut has_entry = false;

        let commit_entry = |catalog: &mut Self,
                            msgctxt: &mut Option<String>,
                            msgid: &mut String,
                            msgid_plural: &mut Option<String>,
                            msgstr: &mut String,
                            msgstr_plural: &mut Vec<(usize, String)>,
                            is_fuzzy: &mut bool,
                            has_entry: &mut bool| {
            if !*has_entry {
                return;
            }
            if !*is_fuzzy {
                if msgid.is_empty() && msgctxt.is_none() {
                    // Parse gettext header metadata lines
                    for line in msgstr.lines() {
                        if let Some((k, v)) = line.split_once(':') {
                            catalog
                                .headers
                                .insert(k.trim().to_string(), v.trim().to_string());
                        }
                    }
                } else {
                    let key = match msgctxt.as_deref() {
                        Some(ctx) if !ctx.is_empty() => format!("{}\u{0004}{}", ctx, msgid),
                        _ => msgid.clone(),
                    };

                    if !msgstr_plural.is_empty() {
                        msgstr_plural.sort_by_key(|(idx, _)| *idx);
                        let forms: Vec<String> = msgstr_plural
                            .iter()
                            .map(|(_, s)| s.clone())
                            .filter(|s| !s.is_empty())
                            .collect();
                        if !forms.is_empty() {
                            if let Some(first) = forms.first() {
                                catalog.messages.insert(key.clone(), first.clone());
                            }
                            catalog.plural_messages.insert(key, forms);
                        }
                    } else if !msgstr.is_empty() {
                        catalog.messages.insert(key, msgstr.clone());
                    }
                }
            }

            *msgctxt = None;
            msgid.clear();
            *msgid_plural = None;
            msgstr.clear();
            msgstr_plural.clear();
            *is_fuzzy = false;
            *has_entry = false;
        };

        for raw_line in content.lines() {
            let line = raw_line.trim();

            if line.is_empty() {
                commit_entry(
                    &mut catalog,
                    &mut msgctxt,
                    &mut msgid,
                    &mut msgid_plural,
                    &mut msgstr,
                    &mut msgstr_plural,
                    &mut is_fuzzy,
                    &mut has_entry,
                );
                current_section = Section::None;
                continue;
            }

            if line.starts_with('#') {
                if current_section != Section::None {
                    commit_entry(
                        &mut catalog,
                        &mut msgctxt,
                        &mut msgid,
                        &mut msgid_plural,
                        &mut msgstr,
                        &mut msgstr_plural,
                        &mut is_fuzzy,
                        &mut has_entry,
                    );
                    current_section = Section::None;
                }
                if let Some(flags) = line.strip_prefix("#,") {
                    if flags.split(',').any(|f| f.trim() == "fuzzy") {
                        is_fuzzy = true;
                    }
                }
                continue;
            }

            if let Some(rest) = line.strip_prefix("msgctxt") {
                if matches!(
                    current_section,
                    Section::Msgstr | Section::MsgstrPlural(_)
                ) {
                    commit_entry(
                        &mut catalog,
                        &mut msgctxt,
                        &mut msgid,
                        &mut msgid_plural,
                        &mut msgstr,
                        &mut msgstr_plural,
                        &mut is_fuzzy,
                        &mut has_entry,
                    );
                }
                has_entry = true;
                current_section = Section::Msgctxt;
                msgctxt = Some(unquote_po_string(rest.trim()));
            } else if let Some(rest) = line.strip_prefix("msgid_plural") {
                has_entry = true;
                current_section = Section::MsgidPlural;
                msgid_plural = Some(unquote_po_string(rest.trim()));
            } else if let Some(rest) = line.strip_prefix("msgid") {
                if matches!(
                    current_section,
                    Section::Msgstr | Section::MsgstrPlural(_)
                ) {
                    commit_entry(
                        &mut catalog,
                        &mut msgctxt,
                        &mut msgid,
                        &mut msgid_plural,
                        &mut msgstr,
                        &mut msgstr_plural,
                        &mut is_fuzzy,
                        &mut has_entry,
                    );
                }
                has_entry = true;
                current_section = Section::Msgid;
                msgid = unquote_po_string(rest.trim());
            } else if let Some(rest) = line.strip_prefix("msgstr[") {
                if let Some((idx_str, after_bracket)) = rest.split_once(']') {
                    if let Ok(idx) = idx_str.trim().parse::<usize>() {
                        has_entry = true;
                        current_section = Section::MsgstrPlural(idx);
                        let val = unquote_po_string(after_bracket.trim());
                        if let Some((_, existing)) =
                            msgstr_plural.iter_mut().find(|(i, _)| *i == idx)
                        {
                            *existing = val;
                        } else {
                            msgstr_plural.push((idx, val));
                        }
                    }
                }
            } else if let Some(rest) = line.strip_prefix("msgstr") {
                has_entry = true;
                current_section = Section::Msgstr;
                msgstr = unquote_po_string(rest.trim());
            } else if line.starts_with('"') && line.ends_with('"') {
                let cont = unquote_po_string(line);
                match current_section {
                    Section::Msgctxt => {
                        if let Some(ctx) = &mut msgctxt {
                            ctx.push_str(&cont);
                        }
                    }
                    Section::Msgid => msgid.push_str(&cont),
                    Section::MsgidPlural => {
                        if let Some(p) = &mut msgid_plural {
                            p.push_str(&cont);
                        }
                    }
                    Section::Msgstr => msgstr.push_str(&cont),
                    Section::MsgstrPlural(idx) => {
                        if let Some((_, existing)) =
                            msgstr_plural.iter_mut().find(|(i, _)| *i == idx)
                        {
                            existing.push_str(&cont);
                        }
                    }
                    Section::None => {}
                }
            }
        }

        commit_entry(
            &mut catalog,
            &mut msgctxt,
            &mut msgid,
            &mut msgid_plural,
            &mut msgstr,
            &mut msgstr_plural,
            &mut is_fuzzy,
            &mut has_entry,
        );

        catalog
    }

    pub fn get<'a>(&'a self, msgid: &'a str) -> &'a str {
        self.messages
            .get(msgid)
            .map(|s| s.as_str())
            .unwrap_or(msgid)
    }

    pub fn get_plural<'a>(&'a self, msgid: &'a str, msgid_plural: &'a str, n: usize) -> &'a str {
        if let Some(forms) = self.plural_messages.get(msgid) {
            if forms.len() == 1 {
                return forms[0].as_str();
            }
            let idx = if n == 1 { 0 } else { 1.min(forms.len() - 1) };
            return forms[idx].as_str();
        }
        if let Some(single) = self.messages.get(msgid) {
            return single.as_str();
        }
        if n == 1 {
            msgid
        } else {
            msgid_plural
        }
    }
}

fn unquote_po_string(s: &str) -> String {
    let s = s.trim();
    let inner = if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        &s[1..s.len() - 1]
    } else {
        s
    };

    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

static GLOBAL_CATALOG: OnceLock<RwLock<PoCatalog>> = OnceLock::new();

fn catalog_lock() -> &'static RwLock<PoCatalog> {
    GLOBAL_CATALOG.get_or_init(|| {
        let lang = detect_system_language();
        RwLock::new(load_catalog_for_language(&lang, ""))
    })
}

/// Detect the system language code (e.g. `"ja"`, `"en"`) from environment variables.
pub fn detect_system_language() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            let trimmed = val.trim();
            if !trimmed.is_empty() && trimmed != "C" && trimmed != "POSIX" {
                let code = trimmed
                    .split('.')
                    .next()
                    .unwrap_or(trimmed)
                    .split('_')
                    .next()
                    .unwrap_or(trimmed)
                    .to_lowercase();
                if !code.is_empty() {
                    return code;
                }
            }
        }
    }
    "en".to_string()
}

/// Discover available `.po` files in `./po` and `~/.config/termin/po`.
pub fn discover_po_files() -> Vec<(String, PathBuf)> {
    let mut dirs = vec![PathBuf::from("po")];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("po"));
            if let Some(grandparent) = parent.parent() {
                dirs.push(grandparent.join("po"));
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(&home).join(".config/termin/po"));
    }

    let mut found = Vec::new();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("po") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let lang = stem.to_string();
                    if !found.iter().any(|(l, _)| l == &lang) {
                        found.push((lang, path));
                    }
                }
            }
        }
    }

    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

/// Load a `PoCatalog` for the given language setting (`"auto"`, `"en"`, `"ja"`, etc.)
/// or an explicit custom `.po` file path.
pub fn load_catalog_for_language(language: &str, custom_po_path: &str) -> PoCatalog {
    let custom_trimmed = custom_po_path.trim();
    if !custom_trimmed.is_empty() {
        if let Ok(content) = fs::read_to_string(custom_trimmed) {
            let name = Path::new(custom_trimmed)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("custom");
            return PoCatalog::parse(name, &content);
        }
    }

    let effective_lang = if language.trim().is_empty() || language.eq_ignore_ascii_case("auto") {
        detect_system_language()
    } else {
        language.trim().to_string()
    };

    // 1. Check external .po files in ./po/<lang>.po or ~/.config/termin/po/<lang>.po
    for (lang, path) in discover_po_files() {
        if lang.eq_ignore_ascii_case(&effective_lang) {
            if let Ok(content) = fs::read_to_string(&path) {
                return PoCatalog::parse(&effective_lang, &content);
            }
        }
    }

    // 2. Fallback to embedded .po catalogs
    let normalized = effective_lang.to_lowercase();
    if normalized.starts_with("ja") {
        PoCatalog::parse("ja", EMBEDDED_JA_PO)
    } else {
        PoCatalog::parse("en", EMBEDDED_EN_PO)
    }
}

/// Apply the language / `.po` settings globally.
pub fn set_active_language(language: &str, custom_po_path: &str) {
    let catalog = load_catalog_for_language(language, custom_po_path);
    if let Ok(mut guard) = catalog_lock().write() {
        *guard = catalog;
    }
}

/// Return information about the currently active catalog (locale code, number of loaded entries).
pub fn active_catalog_info() -> (String, usize) {
    if let Ok(guard) = catalog_lock().read() {
        (guard.locale.clone(), guard.messages.len())
    } else {
        ("en".to_string(), 0)
    }
}

/// Translate a message ID using the active `.po` catalog.
pub fn tr(msgid: &str) -> String {
    if let Ok(guard) = catalog_lock().read() {
        guard.get(msgid).to_string()
    } else {
        msgid.to_string()
    }
}

/// Translate a plural message ID using the active `.po` catalog.
#[allow(dead_code)]
pub fn tr_plural(msgid: &str, msgid_plural: &str, n: usize) -> String {
    if let Ok(guard) = catalog_lock().read() {
        guard.get_plural(msgid, msgid_plural, n).to_string()
    } else if n == 1 {
        msgid.to_string()
    } else {
        msgid_plural.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_po_parser_basic_and_multiline() {
        let sample = r#"
# Comment line
msgid ""
msgstr ""
"Content-Type: text/plain; charset=UTF-8\n"
"Language: ja\n"

msgid "Copy"
msgstr "コピー"

msgid ""
"Line one\n"
"Line two"
msgstr ""
"1行目\n"
"2行目"

#, fuzzy
msgid "Fuzzy entry"
msgstr "曖昧な翻訳"
"#;
        let cat = PoCatalog::parse("ja", sample);
        assert_eq!(cat.headers.get("Language").map(|s| s.as_str()), Some("ja"));
        assert_eq!(cat.get("Copy"), "コピー");
        assert_eq!(cat.get("Line one\nLine two"), "1行目\n2行目");
        // Fuzzy entries are ignored and fall back to msgid
        assert_eq!(cat.get("Fuzzy entry"), "Fuzzy entry");
        assert_eq!(cat.get("Untranslated"), "Untranslated");
    }

    #[test]
    fn test_embedded_catalogs_load() {
        let ja = load_catalog_for_language("ja", "");
        assert_eq!(ja.get("Copy"), "コピー");
        assert_eq!(ja.get("Paste"), "ペースト");

        let en = load_catalog_for_language("en", "");
        assert_eq!(en.get("Copy"), "Copy");
        assert_eq!(en.get("Paste"), "Paste");
    }
}
