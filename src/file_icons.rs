//! VS Code default file-icon theme ("Seti") port.
//!
//! Source assets (from `vscode-original/extensions/theme-seti/icons/`):
//!   - `assets/seti.ttf`             — the icon glyph font (converted from seti.woff)
//!   - `assets/seti-icon-theme.json` — extension / filename / language → glyph + colour
//!
//! Each file row renders its glyph from the seti font, tinted by the theme's
//! per-icon colour. Folders are not covered by Seti (it defines none); the
//! Explorer keeps its codicon folder glyphs.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use egui::{Color32, FontFamily, FontId};
use once_cell::sync::Lazy;

pub const SETI_FAMILY: &str = "seti";

pub fn seti_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SETI_FAMILY.into()))
}

/// Register the seti icon font into an existing FontDefinitions.
pub fn register(fonts: &mut egui::FontDefinitions) {
    fonts.font_data.insert(
        SETI_FAMILY.to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/seti.ttf"
        ))),
    );
    fonts
        .families
        .insert(FontFamily::Name(SETI_FAMILY.into()), vec![SETI_FAMILY.to_owned()]);
}

struct Theme {
    defs: HashMap<String, (char, Color32)>,
    file_extensions: HashMap<String, String>,
    file_names: HashMap<String, String>,
    language_ids: HashMap<String, String>,
    default_def: String,
}

static THEME: Lazy<Theme> = Lazy::new(|| {
    let raw = include_str!("../assets/seti-icon-theme.json");
    let v: serde_json::Value = serde_json::from_str(raw).expect("seti theme json");

    let mut defs = HashMap::new();
    if let Some(obj) = v.get("iconDefinitions").and_then(|x| x.as_object()) {
        for (name, d) in obj {
            let fc = d.get("fontCharacter").and_then(|x| x.as_str()).unwrap_or("");
            let color = d.get("fontColor").and_then(|x| x.as_str()).unwrap_or("#d4d7d6");
            if let (Some(ch), Some(col)) = (parse_font_char(fc), parse_hex(color)) {
                defs.insert(name.clone(), (ch, col));
            }
        }
    }
    let map = |key: &str| -> HashMap<String, String> {
        v.get(key)
            .and_then(|x| x.as_object())
            .map(|o| {
                o.iter()
                    .filter_map(|(k, val)| val.as_str().map(|s| (k.to_lowercase(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default()
    };
    Theme {
        defs,
        file_extensions: map("fileExtensions"),
        file_names: map("fileNames"),
        language_ids: map("languageIds"),
        default_def: v.get("file").and_then(|x| x.as_str()).unwrap_or("_default").to_string(),
    }
});

/// Map our extension token → the seti languageId, for the gaps where the theme
/// only lists an icon under `languageIds` (e.g. ts → typescript).
fn ext_to_language(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescriptreact",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascriptreact",
        "rs" => "rust",
        "go" => "go",
        "py" => "python",
        "rb" => "ruby",
        "json" | "jsonc" => "json",
        "md" | "markdown" => "markdown",
        "html" | "htm" => "html",
        "css" => "css",
        "yaml" | "yml" => "yaml",
        "sh" | "bash" | "zsh" => "shellscript",
        "c" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        _ => return None,
    })
}

/// Resolve a file's seti icon glyph and colour. Returns `None` only if the font
/// failed to map (caller falls back to a codicon).
pub fn icon_for(path: &Path) -> Option<(char, Color32)> {
    let t = &*THEME;
    let name = path.file_name()?.to_string_lossy().to_lowercase();

    // 1. exact filename
    if let Some(def) = t.file_names.get(&name) {
        if let Some(hit) = t.defs.get(def) {
            return Some(*hit);
        }
    }
    // 2. longest matching extension (foo.test.ts → "test.ts" then "ts")
    let parts: Vec<&str> = name.split('.').collect();
    for i in 1..parts.len() {
        let ext = parts[i..].join(".");
        if let Some(def) = t.file_extensions.get(&ext) {
            if let Some(hit) = t.defs.get(def) {
                return Some(*hit);
            }
        }
    }
    // 3. language id derived from the final extension
    if let Some(last) = parts.last() {
        if let Some(lang) = ext_to_language(last) {
            if let Some(def) = t.language_ids.get(lang) {
                if let Some(hit) = t.defs.get(def) {
                    return Some(*hit);
                }
            }
        }
    }
    // 4. default file icon
    t.defs.get(&t.default_def).copied()
}

fn parse_font_char(s: &str) -> Option<char> {
    // theme stores e.g. "\\E099" → strip the leading backslash, parse hex.
    let hex = s.trim_start_matches('\\');
    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
}

fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim_start_matches('#');
    if s.len() < 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}
