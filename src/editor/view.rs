use std::collections::BTreeMap;

use egui::text::{CCursor, CCursorRange};
use egui::{Align2, FontId, Sense, TextEdit, Ui};

use crate::git::DiffKind;
use crate::icons::{codicon_font, CHEVRON_DOWN, CHEVRON_RIGHT};
use crate::theme::Palette;

use super::buffer::Document;
use super::fold;
use super::highlight::build_layout_job;

const EDITOR_FONT_SIZE: f32 = 13.5;
const LINE_HEIGHT: f32 = EDITOR_FONT_SIZE * 1.4;
const CHEVRON_W: f32 = 16.0;
/// Width of the diff decoration strip on the far left of the gutter.
const DIFF_W: f32 = 3.0;

pub fn show(ui: &mut Ui, doc: &mut Document, diff: &BTreeMap<usize, DiffKind>) {
    // Diff tabs render a read-only inline diff against their HEAD base.
    if let Some(base) = &doc.diff_base {
        super::diff_view::show(ui, base, &doc.text, doc.language);
        return;
    }

    // Recompute foldable regions every frame and drop any folded header that
    // no longer starts a region (e.g. after an edit changed the indentation).
    let ranges = fold::foldable_ranges(&doc.text);
    doc.folded.retain(|h| ranges.contains_key(h));

    let all_lines: Vec<String> = doc.text.split('\n').map(|s| s.to_string()).collect();
    let line_count = all_lines.len().max(1);

    // Original (1-based) line numbers that remain visible after folding.
    let visible: Vec<usize> = (1..=line_count)
        .filter(|&ln| !fold::is_hidden(ln, &doc.folded, &ranges))
        .collect();

    let gutter_digits = line_count.to_string().len();
    let char_w = 8.0_f32;
    let number_w = (gutter_digits as f32 * char_w + 12.0).max(36.0);
    let gutter_width = number_w + CHEVRON_W;

    let has_folds = !doc.folded.is_empty();

    if has_folds {
        show_folded(ui, doc, &all_lines, &visible, &ranges, gutter_width, number_w, diff);
    } else {
        show_editable(ui, doc, line_count, &ranges, gutter_width, number_w, diff);
    }
}

/// Paint the Git diff strip for one row at vertical centre `y_center`.
fn paint_diff(ui: &Ui, gutter_left: f32, y_center: f32, kind: DiffKind) {
    let color = match kind {
        DiffKind::Added => Palette::GIT_GUTTER_ADDED,
        DiffKind::Modified => Palette::GIT_GUTTER_MODIFIED,
        DiffKind::DeletedAbove => Palette::GIT_GUTTER_DELETED,
    };
    if matches!(kind, DiffKind::DeletedAbove) {
        // A small downward caret marking where lines were removed.
        let top = y_center - LINE_HEIGHT / 2.0;
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(gutter_left, top - 1.0),
                egui::vec2(DIFF_W + 3.0, 2.0),
            ),
            0.0,
            color,
        );
    } else {
        let r = egui::Rect::from_min_size(
            egui::pos2(gutter_left, y_center - LINE_HEIGHT / 2.0),
            egui::vec2(DIFF_W, LINE_HEIGHT),
        );
        ui.painter().rect_filled(r, 0.0, color);
    }
}

/// Paint the gutter (line numbers + fold chevrons) for the given visible
/// rows, and collect any chevron click as a fold-toggle. `rows` pairs the
/// row index (0-based, for vertical placement) with the original 1-based
/// line number. Returns the header line whose chevron was clicked, if any.
fn paint_gutter(
    ui: &Ui,
    gutter_rect: egui::Rect,
    number_w: f32,
    rows: &[usize],
    cursor_line: usize,
    ranges: &std::collections::BTreeMap<usize, usize>,
    folded: &std::collections::BTreeSet<usize>,
    gutter_hovered: bool,
    diff: &BTreeMap<usize, DiffKind>,
) -> Option<usize> {
    let painter = ui.painter();
    let mut toggled = None;

    for (row, &ln) in rows.iter().enumerate() {
        let y = gutter_rect.top() + 4.0 + row as f32 * LINE_HEIGHT + LINE_HEIGHT / 2.0;

        // Git diff decoration on the far-left edge of the gutter.
        if let Some(&kind) = diff.get(&ln) {
            paint_diff(ui, gutter_rect.left(), y, kind);
        }

        // Line number, right-aligned within the number sub-column.
        let color = if ln == cursor_line {
            Palette::LINE_NUMBER_ACTIVE_FG
        } else {
            Palette::LINE_NUMBER_FG
        };
        painter.text(
            egui::pos2(gutter_rect.left() + number_w - 4.0, y),
            Align2::RIGHT_CENTER,
            ln.to_string(),
            FontId::monospace(13.0),
            color,
        );

        // Fold chevron in the chevron sub-column.
        let is_header = ranges.contains_key(&ln);
        if !is_header {
            continue;
        }
        let is_folded = folded.contains(&ln);
        // VS Code shows the chevron for folded regions always, and for
        // unfolded foldable lines only while the editor is hovered.
        if !is_folded && !gutter_hovered {
            continue;
        }
        let glyph = if is_folded { CHEVRON_RIGHT } else { CHEVRON_DOWN };
        let cx = gutter_rect.left() + number_w + CHEVRON_W / 2.0;
        painter.text(
            egui::pos2(cx, y),
            Align2::CENTER_CENTER,
            glyph.to_string(),
            codicon_font(12.0),
            Palette::FG_DESCRIPTION,
        );

        let hit = egui::Rect::from_center_size(
            egui::pos2(cx, y),
            egui::vec2(CHEVRON_W, LINE_HEIGHT),
        );
        let resp = ui.interact(hit, ui.id().with(("fold", ln)), Sense::click());
        if resp.clicked() {
            toggled = Some(ln);
        }
    }

    toggled
}

/// Editable path — no folds active. Standard `TextEdit` over the full text
/// with the fold gutter painted alongside.
fn show_editable(
    ui: &mut Ui,
    doc: &mut Document,
    line_count: usize,
    ranges: &std::collections::BTreeMap<usize, usize>,
    gutter_width: f32,
    number_w: f32,
    diff: &BTreeMap<usize, DiffKind>,
) {
    let pending = doc.pending_nav.take();
    let ctx = ui.ctx().clone();
    let style = ui.style().clone();
    let language = doc.language;
    let mut layouter = move |ui: &Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
        let job = build_layout_job(&ctx, &style, text.as_str(), language, wrap_width);
        ui.ctx().fonts_mut(|f| f.layout_job(job))
    };

    let cursor_line = doc.cursor_line;
    let rows: Vec<usize> = (1..=line_count).collect();
    let mut toggled = None;

    let response = egui::ScrollArea::both()
        .id_salt("editor_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                let (gutter_rect, _) = ui.allocate_exact_size(
                    egui::vec2(gutter_width, LINE_HEIGHT * line_count as f32 + 8.0),
                    Sense::hover(),
                );
                let gutter_hovered = ui.rect_contains_pointer(gutter_rect)
                    || ui.rect_contains_pointer(ui.max_rect());
                toggled = paint_gutter(
                    ui,
                    gutter_rect,
                    number_w,
                    &rows,
                    cursor_line,
                    ranges,
                    &doc.folded,
                    gutter_hovered,
                    diff,
                );

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

    if let Some(h) = toggled {
        doc.folded.insert(h);
    }

    if response.changed() {
        doc.check_dirty();
    }

    let ctx = ui.ctx();
    if let Some((line, byte_in_line)) = pending {
        let char_offset = line_col_byte_to_char_offset(&doc.text, line, byte_in_line);
        if let Some(mut state) = TextEdit::load_state(ctx, response.id) {
            state
                .cursor
                .set_char_range(Some(CCursorRange::one(CCursor::new(char_offset))));
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

/// Folded path — at least one region is collapsed. Renders a read-only,
/// syntax-highlighted view of the visible lines (with a `⋯` marker on each
/// folded header). Editing requires unfolding, which keeps the text model
/// trivially consistent.
fn show_folded(
    ui: &mut Ui,
    doc: &mut Document,
    all_lines: &[String],
    visible: &[usize],
    ranges: &std::collections::BTreeMap<usize, usize>,
    gutter_width: f32,
    number_w: f32,
    diff: &BTreeMap<usize, DiffKind>,
) {
    let ctx = ui.ctx().clone();
    let style = ui.style().clone();
    let language = doc.language;
    let cursor_line = doc.cursor_line;
    let mut toggled = None;

    egui::ScrollArea::both()
        .id_salt("editor_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                let body_h = LINE_HEIGHT * visible.len() as f32 + 8.0;
                let (gutter_rect, _) =
                    ui.allocate_exact_size(egui::vec2(gutter_width, body_h), Sense::hover());
                let gutter_hovered = true; // chevrons always visible while folded
                toggled = paint_gutter(
                    ui,
                    gutter_rect,
                    number_w,
                    visible,
                    cursor_line,
                    ranges,
                    &doc.folded,
                    gutter_hovered,
                    diff,
                );

                // Code column — read-only, painted per visible line so it
                // stays row-aligned with the gutter.
                let avail_w = ui.available_width().max(200.0);
                let (code_rect, _) =
                    ui.allocate_exact_size(egui::vec2(avail_w, body_h), Sense::hover());
                ui.painter()
                    .rect_filled(code_rect, 0.0, Palette::EDITOR_BG);

                for (row, &ln) in visible.iter().enumerate() {
                    let mut line_text = all_lines[ln - 1].clone();
                    if doc.folded.contains(&ln) {
                        line_text.push_str("  ⋯");
                    }
                    let job = build_layout_job(
                        &ctx,
                        &style,
                        &line_text,
                        language,
                        f32::INFINITY,
                    );
                    let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
                    let y = code_rect.top() + 4.0 + row as f32 * LINE_HEIGHT
                        + (LINE_HEIGHT - galley.size().y) / 2.0;
                    ui.painter().galley(
                        egui::pos2(code_rect.left() + 6.0, y),
                        galley,
                        Palette::FG,
                    );
                }
            });
        });

    if let Some(h) = toggled {
        // Toggling a visible chevron either folds a new region or unfolds an
        // already-folded one.
        if !doc.folded.remove(&h) {
            doc.folded.insert(h);
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
