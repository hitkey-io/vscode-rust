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

/// Minimap column width in logical points (VS Code's default sits near this for
/// typical content).
pub const WIDTH: f32 = 72.0;

// VS Code's minimap at scale 1 spends ~2 device px per line; at our 2x DPR
// that is ~2.2 logical points per source line, micro-glyphs slightly larger.
const LINE_H: f32 = 2.2;
const PAD: f32 = 4.0;

/// Paint the minimap into `area` (the right-hand strip of the editor). `top`
/// and `rows` describe the editor's currently visible line window for the
/// slider; pass `0`/`0` to omit the slider.
pub fn show(ui: &Ui, area: Rect, doc: &Document, top_line: usize, visible_rows: usize) {
    let p = ui.painter();
    p.rect_filled(area, 0.0, Palette::EDITOR_BG);

    let runs = line_runs(&doc.text, doc.language);
    let max_rows = ((area.height() - PAD) / LINE_H).floor() as usize;

    // Each run renders as real (micro) text in the editor's monospace font —
    // tiny letterforms like VS Code's renderCharacters minimap, instead of
    // flat blocks. Tabs expand so indentation stays proportional.
    let micro = crate::icons::editor_mono_font(LINE_H + 0.4);
    for (i, line) in runs.iter().enumerate() {
        if i >= max_rows {
            break;
        }
        let y = area.top() + PAD + i as f32 * LINE_H;
        let mut x = area.left() + PAD;
        for (text, color) in line {
            if x > area.right() - 2.0 {
                break;
            }
            let txt = text.replace('\t', "    ");
            let c = color.gamma_multiply(0.85);
            let galley = p.layout_no_wrap(txt, micro.clone(), c);
            let w = galley.size().x.min(area.right() - 2.0 - x);
            p.with_clip_rect(Rect::from_min_size(
                egui::pos2(x, y),
                egui::vec2(w, LINE_H),
            ))
            .galley(egui::pos2(x, y), galley.clone(), c);
            x += galley.size().x;
        }
    }

    // Viewport slider.
    if visible_rows > 0 && !runs.is_empty() {
        let sy = area.top() + PAD + top_line as f32 * LINE_H;
        let sh = (visible_rows as f32 * LINE_H).min(area.height());
        let slider = Rect::from_min_size(
            egui::pos2(area.left(), sy),
            egui::vec2(area.width(), sh),
        );
        ui.painter().rect_filled(
            slider,
            0.0,
            egui::Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x10),
        );
    }

    // Left divider hairline.
    ui.painter().vline(
        area.left(),
        area.y_range(),
        egui::Stroke::new(1.0, Palette::EDITOR_BG),
    );
    let _ = Sense::hover();
}
