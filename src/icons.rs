// Codicon glyph codepoints. Source:
// vscode/src/vs/base/common/codiconsLibrary.ts
// Font file (assets/codicon.ttf) is loaded into egui as font family "codicon".

pub const CODICON_FAMILY: &str = "codicon";

pub fn codicon_family() -> egui::FontFamily {
    egui::FontFamily::Name(CODICON_FAMILY.into())
}

pub fn codicon_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, codicon_family())
}

/// Editor monospace family name. VS Code on macOS renders code in **Menlo**;
/// we load the system Menlo so glyph metrics match pixel-for-pixel.
pub const EDITOR_MONO_FAMILY: &str = "editor-mono";

pub fn editor_mono_font(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name(EDITOR_MONO_FAMILY.into()))
}

pub fn register_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        CODICON_FAMILY.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/codicon.ttf"
        ))),
    );
    fonts.families.insert(
        codicon_family(),
        vec![CODICON_FAMILY.to_owned()],
    );

    // Seti file-type icon font (VS Code default file-icon theme).
    crate::file_icons::register(&mut fonts);

    // UI text in the macOS system font, as VS Code does.
    crate::ui_font::register(&mut fonts);

    // Load the system Menlo (Regular = face 0 of the .ttc) for the editor, so
    // code metrics match VS Code on macOS. Fall back to egui's bundled
    // monospace if Menlo is unavailable.
    if let Ok(bytes) = std::fs::read("/System/Library/Fonts/Menlo.ttc") {
        let mut data = egui::FontData::from_owned(bytes);
        data.index = 0; // Menlo-Regular
        fonts.font_data.insert("Menlo".to_owned(), std::sync::Arc::new(data));
        fonts.families.insert(
            egui::FontFamily::Name(EDITOR_MONO_FAMILY.into()),
            vec!["Menlo".to_owned()],
        );
        // Also make Menlo the head of the generic Monospace family so any
        // FontId::monospace(...) renders in it.
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "Menlo".to_owned());
        // Append Menlo as a fallback to the Proportional family so glyphs the
        // bundled UI font lacks (e.g. the ⇧ ⌥ ⌘ ⌃ modifier symbols in keyboard
        // hints) still render instead of showing missing-glyph boxes.
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("Menlo".to_owned());
    } else {
        // No Menlo: alias editor-mono to the default monospace family.
        fonts.families.insert(
            egui::FontFamily::Name(EDITOR_MONO_FAMILY.into()),
            fonts
                .families
                .get(&egui::FontFamily::Monospace)
                .cloned()
                .unwrap_or_default(),
        );
    }

    ctx.set_fonts(fonts);
}

/// The application/dock icon. Decodes the bundled `assets/app-icon.png`
/// (the VS Code application icon) into RGBA for the viewport. Falls back to an
/// empty icon if decoding fails.
pub fn app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/app-icon.png"))
        .unwrap_or_default()
}

macro_rules! codepoint {
    ($name:ident = $code:literal) => {
        #[allow(dead_code)]
        pub const $name: char = unsafe { char::from_u32_unchecked($code) };
    };
}

// Activity bar
codepoint!(FILES = 0xeaf0);
codepoint!(SEARCH = 0xea6d);
codepoint!(SOURCE_CONTROL = 0xea68);
codepoint!(DEBUG_ALT = 0xeb91);
codepoint!(EXTENSIONS = 0xeae6);
codepoint!(ACCOUNT = 0xeb99);
codepoint!(SETTINGS_GEAR = 0xeb51);

// File explorer
codepoint!(FILE = 0xea7b);
codepoint!(FOLDER = 0xea83);
codepoint!(FOLDER_OPENED = 0xeaf7);
codepoint!(CHEVRON_DOWN = 0xeab4);
codepoint!(CHEVRON_RIGHT = 0xeab6);
codepoint!(NEW_FILE = 0xea7f);
codepoint!(NEW_FOLDER = 0xea80);
codepoint!(REFRESH = 0xeb37);
codepoint!(COLLAPSE_ALL = 0xeac5);
codepoint!(EXPAND_ALL = 0xeb95);

// Tabs
codepoint!(CLOSE = 0xea76);
codepoint!(CIRCLE_FILLED = 0xea71);
codepoint!(KEBAB_VERTICAL = 0xeb10);
codepoint!(PINNED = 0xeba0);
codepoint!(PIN = 0xeba1);

// Source Control
codepoint!(REPO = 0xea62);
codepoint!(REPO_SELECTED = 0xec69);
codepoint!(ELLIPSIS = 0xea7c);
codepoint!(DISCARD = 0xeae2);
codepoint!(ADD = 0xea60);
codepoint!(REMOVE = 0xeb3b);
codepoint!(GIT_COMMIT = 0xeafc);
codepoint!(GIT_MERGE = 0xeafe);
codepoint!(CLOUD_UPLOAD = 0xeac3);
codepoint!(ARROW_UP = 0xeaa1);
codepoint!(ARROW_DOWN = 0xea9a);

// Title bar
codepoint!(ARROW_LEFT = 0xea9b);
codepoint!(ARROW_RIGHT = 0xea9c);
codepoint!(LAYOUT_SIDEBAR_LEFT = 0xebf3);
codepoint!(LAYOUT_PANEL = 0xebf2);
codepoint!(LAYOUT_SIDEBAR_RIGHT = 0xebf4);
codepoint!(LAYOUT = 0xebeb);

// Status bar
codepoint!(REMOTE = 0xeb3a);
codepoint!(FEEDBACK = 0xeb96);
codepoint!(JSON_BRACES = 0xeb0f);
codepoint!(GIT_BRANCH = 0xec6f);
codepoint!(TAG = 0xea66);
codepoint!(SYNC = 0xea77);
codepoint!(ERROR_ICON = 0xea87);
codepoint!(WARNING_ICON = 0xea6c);
codepoint!(INFO_ICON = 0xea74);
codepoint!(BELL = 0xeaa2);
codepoint!(CHECK = 0xeab2);

// Title bar / window
codepoint!(CHROME_MINIMIZE = 0xeaba);
codepoint!(CHROME_MAXIMIZE = 0xeab9);
codepoint!(CHROME_RESTORE = 0xeabb);
codepoint!(CHROME_CLOSE = 0xeab8);
codepoint!(MENU = 0xeb94);
codepoint!(THREE_BARS = 0xeb6a);

// Misc
codepoint!(COPILOT = 0xec1e);

// VS Code product logo (the stylised "<" angle-bracket).
// Ref: vscode-original/src/vs/base/common/codiconsLibrary.ts
codepoint!(VSCODE = 0xec29);
codepoint!(VSCODE_INSIDERS = 0xec2a);
codepoint!(TERMINAL_ICON = 0xea85);
codepoint!(PLAY = 0xeb2c);



/// Codicon names (`$(name)` in status bar text and labels) → codepoints,
/// from `src/vs/base/common/codiconsLibrary.ts`.
static CODICON_NAMES: once_cell::sync::Lazy<std::collections::HashMap<&'static str, char>> =
    once_cell::sync::Lazy::new(|| {
        include_str!("../assets/codicon-names.txt")
            .lines()
            .filter_map(|l| {
                let (name, hex) = l.split_once(' ')?;
                Some((name, char::from_u32(u32::from_str_radix(hex, 16).ok()?)?))
            })
            .collect()
    });

/// The codicon called `name` (`"smiley"`, `"sync~spin"` → `sync`).
pub fn codicon_by_name(name: &str) -> Option<char> {
    let base = name.split('~').next().unwrap_or(name);
    CODICON_NAMES.get(base).copied()
}

/// A piece of label text: plain text or one codicon.
#[derive(Debug, Clone, PartialEq)]
pub enum LabelPart {
    Text(String),
    Icon(char),
}

/// Split label text with `$(name)` icon references into parts, as VS Code's
/// `renderLabelWithIcons` does. Unknown icons are dropped; `\$(` keeps a
/// literal `$(`.
pub fn parse_label_with_icons(text: &str) -> Vec<LabelPart> {
    let mut parts = Vec::new();
    let mut buf = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("$(") {
        if i > 0 && rest[..i].ends_with('\\') {
            buf.push_str(&rest[..i - 1]);
            buf.push_str("$(");
            rest = &rest[i + 2..];
            continue;
        }
        let Some(end) = rest[i + 2..].find(')') else { break };
        buf.push_str(&rest[..i]);
        let name = &rest[i + 2..i + 2 + end];
        if let Some(glyph) = codicon_by_name(name) {
            if !buf.is_empty() {
                parts.push(LabelPart::Text(std::mem::take(&mut buf)));
            }
            parts.push(LabelPart::Icon(glyph));
        }
        rest = &rest[i + 3 + end..];
    }
    buf.push_str(rest);
    if !buf.is_empty() {
        parts.push(LabelPart::Text(buf));
    }
    parts
}

#[cfg(test)]
mod label_tests {
    use super::*;

    #[test]
    fn splits_icons_and_text() {
        let parts = parse_label_with_icons("$(smiley) Hello $(nope)!");
        assert_eq!(parts[0], LabelPart::Icon(codicon_by_name("smiley").unwrap()));
        assert_eq!(parts[1], LabelPart::Text(" Hello !".into()));
        assert_eq!(codicon_by_name("sync~spin"), codicon_by_name("sync"));
    }
}
