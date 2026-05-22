//! Read-only inline diff renderer (the "(Working Tree)" editor tab).
//!
//! VS Code opens a `DiffEditor` when you click a changed file in Source
//! Control. It offers side-by-side and inline modes; we render the inline
//! mode — a single column where removed lines have a red wash and a `-`
//! gutter, added lines a green wash and a `+` gutter, unchanged lines show
//! both old/new numbers. Alignment comes from `similar::TextDiff` rather than
//! parsing `git diff`, so it tracks the in-memory buffer live.

use egui::{Align2, FontId, Sense, Ui};
use similar::{ChangeTag, TextDiff};

use crate::theme::Palette;

use super::highlight::build_layout_job;

const FONT_SIZE: f32 = 13.5;
const LINE_HEIGHT: f32 = FONT_SIZE * 1.4;

struct Row<'a> {
    tag: ChangeTag,
    old_no: Option<usize>,
    new_no: Option<usize>,
    text: &'a str,
}

pub fn show(ui: &mut Ui, base: &str, working: &str, language: &str) {
    // similar borrows the inputs; collect aligned rows up front.
    let diff = TextDiff::from_lines(base, working);
    let mut rows: Vec<Row> = Vec::new();
    let mut old_no = 0usize;
    let mut new_no = 0usize;
    for change in diff.iter_all_changes() {
        let text = change.value();
        // Strip the trailing newline similar keeps on each line.
        let text = text.strip_suffix('\n').unwrap_or(text);
        match change.tag() {
            ChangeTag::Equal => {
                old_no += 1;
                new_no += 1;
                rows.push(Row {
                    tag: ChangeTag::Equal,
                    old_no: Some(old_no),
                    new_no: Some(new_no),
                    text,
                });
            }
            ChangeTag::Delete => {
                old_no += 1;
                rows.push(Row {
                    tag: ChangeTag::Delete,
                    old_no: Some(old_no),
                    new_no: None,
                    text,
                });
            }
            ChangeTag::Insert => {
                new_no += 1;
                rows.push(Row {
                    tag: ChangeTag::Insert,
                    old_no: None,
                    new_no: Some(new_no),
                    text,
                });
            }
        }
    }

    let num_w = 84.0; // two number columns + sign
    let ctx = ui.ctx().clone();
    let style = ui.style().clone();

    egui::ScrollArea::both()
        .id_salt("diff_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let total_h = LINE_HEIGHT * rows.len() as f32 + 8.0;
            let width = ui.available_width().max(400.0);
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(width, total_h), Sense::hover());
            ui.painter().rect_filled(rect, 0.0, Palette::EDITOR_BG);

            for (i, row) in rows.iter().enumerate() {
                let y_top = rect.top() + 4.0 + i as f32 * LINE_HEIGHT;
                let row_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.left(), y_top),
                    egui::vec2(width, LINE_HEIGHT),
                );

                // Full-width line wash for inserted / removed rows.
                let (wash, sign, sign_color) = match row.tag {
                    ChangeTag::Insert => (
                        Some(Palette::DIFF_INSERTED_BG),
                        "+",
                        Palette::GIT_ADDED_FG,
                    ),
                    ChangeTag::Delete => {
                        (Some(Palette::DIFF_REMOVED_BG), "-", Palette::GIT_DELETED_FG)
                    }
                    ChangeTag::Equal => (None, " ", Palette::LINE_NUMBER_FG),
                };
                if let Some(bg) = wash {
                    ui.painter().rect_filled(row_rect, 0.0, bg);
                }

                let cy = row_rect.center().y;
                // Old line number.
                if let Some(n) = row.old_no {
                    ui.painter().text(
                        egui::pos2(rect.left() + 30.0, cy),
                        Align2::RIGHT_CENTER,
                        n.to_string(),
                        FontId::monospace(12.0),
                        Palette::LINE_NUMBER_FG,
                    );
                }
                // New line number.
                if let Some(n) = row.new_no {
                    ui.painter().text(
                        egui::pos2(rect.left() + 62.0, cy),
                        Align2::RIGHT_CENTER,
                        n.to_string(),
                        FontId::monospace(12.0),
                        Palette::LINE_NUMBER_FG,
                    );
                }
                // +/- sign.
                ui.painter().text(
                    egui::pos2(rect.left() + 72.0, cy),
                    Align2::CENTER_CENTER,
                    sign,
                    FontId::monospace(12.0),
                    sign_color,
                );

                // Syntax-highlighted line content.
                let job = build_layout_job(&ctx, &style, row.text, language, f32::INFINITY);
                let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
                let ty = cy - galley.size().y / 2.0;
                ui.painter()
                    .galley(egui::pos2(rect.left() + num_w, ty), galley, Palette::FG);
            }
        });
}
