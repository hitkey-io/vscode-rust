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
    ctx.set_fonts(fonts);
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

// Status bar
codepoint!(GIT_BRANCH = 0xec6f);
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

