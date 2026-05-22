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

/// Rasterise the VS Code product glyph (codicon `vscode`, 0xEC29) into a
/// rounded-square app icon — white mark on the VS Code blue, matching the
/// dock/taskbar identity. Returns `eframe::egui::IconData` for the viewport.
pub fn app_icon() -> egui::IconData {
    use ab_glyph::{Font, FontRef, ScaleFont};

    const SIZE: usize = 256;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];

    // Rounded-square background in VS Code blue (#0098FF), corner radius ~22%.
    let (bg_r, bg_g, bg_b) = (0x00u8, 0x98u8, 0xFFu8);
    let radius = (SIZE as f32) * 0.22;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let fx = x as f32;
            let fy = y as f32;
            // Distance into the rounded-rect corner mask (anti-aliased edge).
            let cx = fx.max(radius).min(SIZE as f32 - radius);
            let cy = fy.max(radius).min(SIZE as f32 - radius);
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            let cov = (radius - d + 0.5).clamp(0.0, 1.0);
            if cov > 0.0 {
                let i = (y * SIZE + x) * 4;
                rgba[i] = bg_r;
                rgba[i + 1] = bg_g;
                rgba[i + 2] = bg_b;
                rgba[i + 3] = (cov * 255.0) as u8;
            }
        }
    }

    // Rasterise the glyph centred on the background.
    if let Ok(font) = FontRef::try_from_slice(include_bytes!("../assets/codicon.ttf")) {
        let px = SIZE as f32 * 0.62;
        let scaled = font.as_scaled(px);
        let glyph_id = font.glyph_id(VSCODE);
        let glyph = glyph_id.with_scale(px);
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            let gw = bounds.width();
            let gh = bounds.height();
            let _ = scaled;
            let ox = (SIZE as f32 - gw) / 2.0 - bounds.min.x;
            let oy = (SIZE as f32 - gh) / 2.0 - bounds.min.y;
            outline.draw(|gx, gy, c| {
                let px = (gx as f32 + bounds.min.x + ox).round() as i32;
                let py = (gy as f32 + bounds.min.y + oy).round() as i32;
                if px < 0 || py < 0 || px >= SIZE as i32 || py >= SIZE as i32 {
                    return;
                }
                let i = ((py as usize) * SIZE + px as usize) * 4;
                // Alpha-composite white glyph over the blue background.
                let a = c.clamp(0.0, 1.0);
                rgba[i] = (255.0 * a + rgba[i] as f32 * (1.0 - a)) as u8;
                rgba[i + 1] = (255.0 * a + rgba[i + 1] as f32 * (1.0 - a)) as u8;
                rgba[i + 2] = (255.0 * a + rgba[i + 2] as f32 * (1.0 - a)) as u8;
                rgba[i + 3] = rgba[i + 3].max((a * 255.0) as u8);
            });
        }
    }

    egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
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

