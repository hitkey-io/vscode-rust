//! The workbench UI font.
//!
//! VS Code on macOS renders the workbench in the system font
//! (`.monaco-workbench.mac { font-family: -apple-system }`, see
//! `src/vs/workbench/browser/media/style.css`), which is SF Pro. The system
//! ships it as one variable font (`SFNS.ttf`, axes wdth / opsz / GRAD / wght)
//! whose default instance is the large "Display" design. At 11–13px the
//! system uses the "Text" design instead (optical size 17, the bottom of
//! the axis), which has wider letters.
//!
//! egui draws glyph outlines at the coordinates given in `FontTweak`, but
//! measures advances at the font's default instance and caches glyphs
//! without the coordinates. So each weight is loaded as its own font, with
//! the horizontal metrics rewritten to that instance's advances; text then
//! lays out as wide as it does in VS Code.

use egui::{FontData, FontDefinitions, FontFamily, FontId, FontTweak};
use skrifa::{instance::Size, raw::TableProvider, GlyphId, MetadataProvider};

/// macOS system UI font.
const SYSTEM_UI_FONT: &str = "/System/Library/Fonts/SFNS.ttf";
/// Its italic companion (preview tab titles).
const SYSTEM_UI_ITALIC: &str = "/System/Library/Fonts/SFNSItalic.ttf";
const ITALIC: &str = "SF-Italic";
/// Family for italic UI text (`font-style: italic`).
const ITALIC_FAMILY: &str = "ui-italic";
const REGULAR: &str = "SF";
const BOLD: &str = "SF-Bold";
/// Family for bold UI text (`font-weight: bold`).
const BOLD_FAMILY: &str = "ui-bold";
const SEMIBOLD: &str = "SF-SemiBold";
/// Family for semibold UI text (`--vscode-fontWeight-semiBold`, 600).
const SEMIBOLD_FAMILY: &str = "ui-semibold";
/// SF's "Text" optical size, used for every UI size (11–13px).
const OPTICAL_SIZE: f32 = 17.0;

/// Bold UI text at `size`, e.g. matched characters in Quick Open.
/// Falls back to the regular UI font where SF isn't available.
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD_FAMILY.into()))
}

/// SemiBold UI text at `size` (600: view and pane titles in Modern UI).
/// Falls back to the regular UI font where SF isn't available.
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD_FAMILY.into()))
}

/// Italic UI text at `size`, e.g. a preview editor's tab title.
/// Falls back to the regular UI font where SF isn't available.
pub fn italic(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ITALIC_FAMILY.into()))
}

/// Put SF (Regular as the head of the proportional family, Bold as the
/// `ui-bold` family) in front of egui's bundled faces. Without SF both
/// families keep the bundled faces.
pub fn register(fonts: &mut FontDefinitions) {
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut bold_family = fallback.clone();
    let mut italic_family = fallback.clone();
    let mut semibold_family = fallback.clone();
    if let Some(italic) = std::fs::read(SYSTEM_UI_ITALIC).ok().and_then(|b| instance(&b, 400.0)) {
        fonts.font_data.insert(ITALIC.to_owned(), std::sync::Arc::new(italic));
        italic_family.insert(0, ITALIC.to_owned());
    }

    if let Ok(bytes) = std::fs::read(SYSTEM_UI_FONT) {
        let regular = instance(&bytes, 400.0);
        let bold = instance(&bytes, 700.0);
        if let (Some(regular), Some(bold)) = (regular, bold) {
            fonts.font_data.insert(REGULAR.to_owned(), std::sync::Arc::new(regular));
            fonts.font_data.insert(BOLD.to_owned(), std::sync::Arc::new(bold));
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, REGULAR.to_owned());
            bold_family.insert(0, BOLD.to_owned());
        }
        if let Some(semibold) = instance(&bytes, 600.0) {
            fonts.font_data.insert(SEMIBOLD.to_owned(), std::sync::Arc::new(semibold));
            semibold_family.insert(0, SEMIBOLD.to_owned());
        }
    }
    fonts
        .families
        .insert(FontFamily::Name(BOLD_FAMILY.into()), bold_family);
    fonts
        .families
        .insert(FontFamily::Name(ITALIC_FAMILY.into()), italic_family);
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD_FAMILY.into()), semibold_family);
}

/// SF at `weight` and the Text optical size: outlines through the tweak's
/// coordinates, advances baked into `hmtx`.
fn instance(bytes: &[u8], weight: f32) -> Option<FontData> {
    let data = with_advances_at(bytes, &[("wght", weight), ("opsz", OPTICAL_SIZE)])?;
    Some(FontData::from_owned(data).tweak(FontTweak {
        coords: egui::epaint::text::VariationCoords::new([
            (b"wght", weight),
            (b"opsz", OPTICAL_SIZE),
        ]),
        ..Default::default()
    }))
}

/// A copy of `bytes` whose `hmtx` advances are those of the instance at
/// `coords`. Every other table is left alone. `None` if the font can't be
/// read or has no horizontal metrics.
fn with_advances_at(bytes: &[u8], coords: &[(&str, f32)]) -> Option<Vec<u8>> {
    let font = skrifa::FontRef::new(bytes).ok()?;
    let location = font.axes().location(coords.iter().copied());
    let metrics = font.glyph_metrics(Size::unscaled(), &location);
    let count = font.hhea().ok()?.number_of_h_metrics() as usize;
    let hmtx = font.table_data(skrifa::Tag::new(b"hmtx"))?;
    let start = hmtx.as_bytes().as_ptr() as usize - bytes.as_ptr() as usize;
    if start + count * 4 > bytes.len() {
        return None;
    }

    let mut out = bytes.to_vec();
    for gid in 0..count {
        let Some(advance) = metrics.advance_width(GlyphId::new(gid as u32)) else {
            continue;
        };
        let advance = advance.round().clamp(0.0, u16::MAX as f32) as u16;
        let at = start + gid * 4;
        out[at..at + 2].copy_from_slice(&advance.to_be_bytes());
    }
    Some(out)
}
