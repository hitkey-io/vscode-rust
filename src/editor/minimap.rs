//! Editor minimap — the miniature code overview on the right edge.
//! VS Code analogue: src/vs/editor/browser/viewParts/minimap/minimap.ts
//!
//! Renders each source line as a row of tiny colour blocks (one per
//! non-whitespace token run), with leading indentation preserved as gaps, and
//! a translucent slider over the lines currently visible in the editor.

use egui::{Rect, Sense, Ui};

use crate::theme::Palette;

use super::buffer::Document;
use super::highlight::line_runs;

/// The minimap's width (editorOptions `_computeMinimapLayout`, scale 1 at
/// a 2x display: 1px per character, `maxColumn` 120, 8px gutter) for the
/// width left after the editor's gutter; 0 when nothing fits.
pub fn width(remaining: f32, char_w: f32) -> f32 {
    const MAX_COLUMN: f32 = 120.0;
    const GUTTER: f32 = 8.0;
    const SCROLLBAR: f32 = 14.0;
    let w = ((remaining - SCROLLBAR - 2.0) / (char_w + 1.0)).floor().max(0.0);
    if w < 1.0 {
        return 0.0;
    }
    w.min(MAX_COLUMN) + GUTTER
}

/// 2 device px per line at scale 1 on a 2x display.
const LINE_H: f32 = 2.0;
/// Each character is a 1px column.
const CHAR_W: f32 = 1.0;

/// Paint the minimap into `area` (the right-hand strip of the editor). `top`
/// and `rows` describe the editor's currently visible line window for the
/// slider; pass `0`/`0` to omit the slider.
pub fn show(ui: &Ui, area: Rect, doc: &Document, top_line: usize, visible_rows: usize) {
    let p = ui.painter();
    p.rect_filled(area, 0.0, Palette::EDITOR_BG());

    let runs = line_runs(&doc.text, doc.language);
    let max_rows = (area.height() / LINE_H).floor() as usize;
    let right = area.right() - 8.0;

    // renderCharacters: each non-blank character is a 1×2px cell in its
    // token colour (VS Code's prerendered minimap font at this scale reads
    // as short bars); blanks stay empty, tabs are 4 wide.
    for (i, line) in runs.iter().enumerate() {
        if i >= max_rows {
            break;
        }
        let y = area.top() + i as f32 * LINE_H;
        let mut x = area.left();
        for (text, color) in line {
            // A cell holds a downscaled glyph: about half covered.
            let c = color.gamma_multiply(0.45);
            for ch in text.chars() {
                if x >= right {
                    break;
                }
                let w = if ch == '\t' { 4.0 * CHAR_W } else { CHAR_W };
                if !ch.is_whitespace() {
                    p.rect_filled(Rect::from_min_size(egui::pos2(x + 0.15, y + 0.4), egui::vec2(CHAR_W * 0.7, LINE_H - 0.8)), 0.0, c);
                }
                x += w;
            }
        }
    }

    // `minimap.showSlider: mouseover`: the viewport slider while hovered.
    if visible_rows > 0 && !runs.is_empty() && ui.rect_contains_pointer(area) {
        let sy = area.top() + top_line as f32 * LINE_H;
        let sh = (visible_rows as f32 * LINE_H).min(area.height());
        let slider = Rect::from_min_size(egui::pos2(area.left(), sy), egui::vec2(area.width(), sh));
        ui.painter().rect_filled(slider, 0.0, Palette::MINIMAP_SLIDER_BG());
    }

    // Left divider hairline.
    ui.painter().vline(
        area.left(),
        area.y_range(),
        egui::Stroke::new(1.0, Palette::EDITOR_BG()),
    );
    let _ = Sense::hover();
}
