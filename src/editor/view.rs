use egui::text::{CCursor, CCursorRange};
use egui::{FontId, RichText, ScrollArea, TextEdit, Ui};

use crate::theme::Palette;

use super::buffer::Document;
use super::highlight::build_layout_job;

const EDITOR_FONT_SIZE: f32 = 13.5;

pub fn show(ui: &mut Ui, doc: &mut Document) {
    let line_count = doc.text.lines().count().max(1);
    let gutter_digits = line_count.to_string().len();
    let char_w = 8.0_f32;
    let gutter_width = (gutter_digits as f32 * char_w + 18.0).max(48.0);

    let pending = doc.pending_nav.take();

    // Use a single ScrollArea wrapping both gutter and editor so they stay in sync.
    let ctx = ui.ctx().clone();
    let style = ui.style().clone();
    let language = doc.language;
    let mut layouter = move |ui: &Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
        let job = build_layout_job(&ctx, &style, text.as_str(), language, wrap_width);
        ui.ctx().fonts_mut(|f| f.layout_job(job))
    };

    let line_height = EDITOR_FONT_SIZE * 1.4;
    let cursor_line = doc.cursor_line;
    let response = ScrollArea::both()
        .id_salt("editor_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                // Gutter column — painted manually, advances in lockstep with the editor.
                let (gutter_rect, _) = ui.allocate_exact_size(
                    egui::vec2(gutter_width, line_height * line_count as f32 + 8.0),
                    egui::Sense::hover(),
                );
                let painter = ui.painter();
                for n in 1..=line_count {
                    let is_current = n == cursor_line;
                    let color = if is_current {
                        Palette::LINE_NUMBER_ACTIVE_FG
                    } else {
                        Palette::LINE_NUMBER_FG
                    };
                    let y = gutter_rect.top() + 4.0 + (n - 1) as f32 * line_height + line_height / 2.0;
                    painter.text(
                        egui::pos2(gutter_rect.right() - 8.0, y),
                        egui::Align2::RIGHT_CENTER,
                        n.to_string(),
                        FontId::monospace(13.0),
                        color,
                    );
                }

                ui.add(
                    TextEdit::multiline(&mut doc.text)
                        .font(FontId::monospace(EDITOR_FONT_SIZE))
                        .code_editor()
                        .desired_rows(40)
                        .desired_width(f32::INFINITY)
                        .lock_focus(true)
                        .frame(
                            egui::Frame::default()
                                .fill(Palette::EDITOR_BG)
                                .inner_margin(egui::Margin::symmetric(6, 4)),
                        )
                        .layouter(&mut layouter),
                )
            })
            .inner
        })
        .inner;

    if response.changed() {
        doc.check_dirty();
    }

    let ctx = ui.ctx();
    if let Some((line, byte_in_line)) = pending {
        let char_offset = line_col_byte_to_char_offset(&doc.text, line, byte_in_line);
        if let Some(mut state) = TextEdit::load_state(ctx, response.id) {
            state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(char_offset))));
            state.store(ctx, response.id);
        }
        response.request_focus();
        ctx.request_repaint();
        doc.cursor_line = line;
    }

    if let Some(state) = TextEdit::load_state(ctx, response.id) {
        if let Some(range) = state.cursor.char_range() {
            let pos = range.primary.index;
            let (line, col) = char_to_line_col(&doc.text, pos);
            doc.cursor_line = line;
            doc.cursor_col = col;
        }
    }
}

fn char_to_line_col(text: &str, char_pos: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in text.chars().enumerate() {
        if i >= char_pos {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn line_col_byte_to_char_offset(text: &str, target_line: usize, target_byte_in_line: usize) -> usize {
    let mut current_line = 1usize;
    let mut char_offset = 0usize;
    for line in text.split('\n') {
        if current_line == target_line {
            let mut byte_pos = 0usize;
            for ch in line.chars() {
                if byte_pos >= target_byte_in_line {
                    break;
                }
                byte_pos += ch.len_utf8();
                char_offset += 1;
            }
            return char_offset;
        }
        char_offset += line.chars().count() + 1;
        current_line += 1;
    }
    char_offset
}
