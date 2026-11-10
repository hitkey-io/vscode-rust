//! Read-only diff editor — what opens when a changed file is clicked in
//! Source Control or in the GRAPH.
//!
//! VS Code's `DiffEditorWidget` renders side by side (original left,
//! modified right) and falls back to the inline view when the editor is
//! narrower than `renderSideBySideInlineBreakpoint` (900px,
//! `useInlineViewWhenSpaceIsLimited`). Removed lines get
//! `diffEditor.removedLineBackground`, added lines
//! `diffEditor.insertedLineBackground`, the changed characters inside a
//! modified line `removed/insertedTextBackground`, the side without a line
//! the `diffEditor.diagonalFill` stripes, and `+` / `-` indicators sit in
//! the margin (`renderIndicators`). An overview ruler at the right edge
//! marks every change.
//!
//! VS Code analogues: `src/vs/editor/browser/widget/diffEditor/`
//! (`diffEditorWidget.ts`, `diffEditorOptions.ts`, `style.css`).

use egui::{Align2, Color32, Rect, Sense, Stroke, Ui};
use similar::{ChangeTag, DiffOp, TextDiff};

use crate::theme::Palette;

use super::highlight::build_layout_job;

const FONT_SIZE: f32 = 12.0;
const LINE_HEIGHT: f32 = FONT_SIZE * 1.5;
/// `renderSideBySideInlineBreakpoint` default.
const INLINE_BREAKPOINT: f32 = 900.0;
/// The diff editor's overview ruler (`ENTIRE_DIFF_OVERVIEW_WIDTH`).
const OVERVIEW_W: f32 = 30.0;

/// One line of one side.
#[derive(Clone, Copy)]
struct Line<'a> {
    no: usize,
    text: &'a str,
}

/// A row of the aligned diff: what each side shows.
struct Row<'a> {
    left: Option<Line<'a>>,
    right: Option<Line<'a>>,
    /// The row is part of a change (vs. unchanged on both sides).
    changed: bool,
}

fn strip_nl(s: &str) -> &str {
    let s = s.strip_suffix('\n').unwrap_or(s);
    s.strip_suffix('\r').unwrap_or(s)
}

/// Align `base` and `working` line by line: unchanged lines side by side,
/// a replaced block paired line by line, the rest against nothing.
fn align<'a>(base_lines: &[&'a str], work_lines: &[&'a str], ops: &[DiffOp]) -> Vec<Row<'a>> {
    let mut rows = Vec::new();
    for op in ops {
        match *op {
            DiffOp::Equal { old_index, new_index, len } => {
                for i in 0..len {
                    rows.push(Row {
                        left: Some(Line { no: old_index + i + 1, text: base_lines[old_index + i] }),
                        right: Some(Line { no: new_index + i + 1, text: work_lines[new_index + i] }),
                        changed: false,
                    });
                }
            }
            DiffOp::Delete { old_index, old_len, .. } => {
                for i in 0..old_len {
                    rows.push(Row { left: Some(Line { no: old_index + i + 1, text: base_lines[old_index + i] }), right: None, changed: true });
                }
            }
            DiffOp::Insert { new_index, new_len, .. } => {
                for i in 0..new_len {
                    rows.push(Row { left: None, right: Some(Line { no: new_index + i + 1, text: work_lines[new_index + i] }), changed: true });
                }
            }
            DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                for i in 0..old_len.max(new_len) {
                    rows.push(Row {
                        left: (i < old_len).then(|| Line { no: old_index + i + 1, text: base_lines[old_index + i] }),
                        right: (i < new_len).then(|| Line { no: new_index + i + 1, text: work_lines[new_index + i] }),
                        changed: true,
                    });
                }
            }
        }
    }
    rows
}

/// Character ranges that differ between two versions of a line
/// (`[start, end)` in chars), for the left and the right side.
fn char_changes(old: &str, new: &str) -> (Vec<(usize, usize)>, Vec<(usize, usize)>) {
    let diff = TextDiff::from_chars(old, new);
    let (mut l, mut r) = (Vec::new(), Vec::new());
    let (mut oi, mut ni) = (0usize, 0usize);
    for c in diff.iter_all_changes() {
        let n = c.value().chars().count();
        match c.tag() {
            ChangeTag::Equal => {
                oi += n;
                ni += n;
            }
            ChangeTag::Delete => {
                push_range(&mut l, oi, oi + n);
                oi += n;
            }
            ChangeTag::Insert => {
                push_range(&mut r, ni, ni + n);
                ni += n;
            }
        }
    }
    (l, r)
}

fn push_range(v: &mut Vec<(usize, usize)>, a: usize, b: usize) {
    match v.last_mut() {
        Some(last) if last.1 == a => last.1 = b,
        _ => v.push((a, b)),
    }
}

pub fn show(ui: &mut Ui, base: &str, working: &str, language: &str) {
    let base_lines: Vec<&str> = if base.is_empty() { Vec::new() } else { base.split_inclusive('\n').map(strip_nl).collect() };
    let work_lines: Vec<&str> = if working.is_empty() { Vec::new() } else { working.split_inclusive('\n').map(strip_nl).collect() };
    let diff = similar::capture_diff_slices(similar::Algorithm::Myers, &base_lines, &work_lines);
    let rows = align(&base_lines, &work_lines, &diff);
    let side_by_side = ui.available_width() >= INLINE_BREAKPOINT;

    let ctx = ui.ctx().clone();
    let style = ui.style().clone();
    let font = crate::icons::editor_mono_font(FONT_SIZE);
    let char_w = ui.painter().layout_no_wrap("0".into(), font.clone(), Color32::WHITE).size().x;
    let digits = base_lines.len().max(work_lines.len()).max(1).to_string().len();
    let number_w = (digits as f32 * char_w + 12.0).max(36.0);
    // Line numbers, then the +/- indicator margin.
    let gutter_w = number_w + 20.0;

    let full = ui.available_rect_before_wrap();
    ui.painter().rect_filled(full, 0.0, Palette::EDITOR_BG());
    let content = Rect::from_min_max(full.min, egui::pos2(full.right() - OVERVIEW_W, full.bottom()));
    let line = |ui: &Ui, rect: Rect, side: Option<Line>, removed: bool, highlights: &[(usize, usize)], changed: bool| {
        let p = ui.painter_at(rect);
        let Some(l) = side else {
            // No line on this side: diagonal stripes (`diffEditor.diagonalFill`).
            if changed {
                let mut x = rect.left() - rect.height();
                while x < rect.right() {
                    p.line_segment([egui::pos2(x, rect.bottom()), egui::pos2(x + rect.height(), rect.top())], Stroke::new(1.0, Palette::DIFF_DIAGONAL_FILL()));
                    x += 6.0;
                }
            }
            return;
        };
        let text_x = rect.left() + gutter_w;
        if changed {
            let wash = if removed { Palette::DIFF_REMOVED_BG() } else { Palette::DIFF_INSERTED_BG() };
            p.rect_filled(Rect::from_min_max(egui::pos2(rect.left() + number_w, rect.top()), rect.max), 0.0, wash);
            let text_wash = if removed { Palette::DIFF_REMOVED_TEXT_BG() } else { Palette::DIFF_INSERTED_TEXT_BG() };
            for (a, b) in highlights {
                let r = Rect::from_min_max(
                    egui::pos2(text_x + *a as f32 * char_w, rect.top()),
                    egui::pos2(text_x + *b as f32 * char_w, rect.bottom()),
                );
                p.rect_filled(r, 0.0, text_wash);
            }
            let (sign, color) = if removed { ("-", Palette::GIT_DELETED_FG()) } else { ("+", Palette::GIT_ADDED_FG()) };
            p.text(egui::pos2(rect.left() + number_w + 10.0, rect.center().y), Align2::CENTER_CENTER, sign, font.clone(), color);
        }
        p.text(
            egui::pos2(rect.left() + number_w - 4.0, rect.center().y),
            Align2::RIGHT_CENTER,
            l.no.to_string(),
            font.clone(),
            Palette::LINE_NUMBER_FG(),
        );
        let job = build_layout_job(&ctx, &style, l.text, language, f32::INFINITY, None);
        let g = ui.ctx().fonts_mut(|f| f.layout_job(job));
        p.galley(egui::pos2(text_x, rect.center().y - g.size().y / 2.0), g, Palette::FG());
    };

    let mut viewport = (0.0, 0.0);
    let total_rows = if side_by_side {
        rows.len()
    } else {
        rows.iter().map(|r| match (r.changed, r.left.is_some(), r.right.is_some()) {
            (true, true, true) => 2,
            _ => 1,
        }).sum()
    };
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(content));
    egui::ScrollArea::vertical()
        .id_salt("diff_scroll")
        .auto_shrink([false, false])
        .show_viewport(&mut child, |ui, vp| {
            viewport = (vp.top(), vp.bottom());
            let total_h = LINE_HEIGHT * total_rows as f32 + 8.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), total_h), Sense::hover());
            let first = ((vp.top() - 4.0) / LINE_HEIGHT).floor().max(0.0) as usize;
            let last = ((vp.bottom() / LINE_HEIGHT).ceil() as usize + 1).min(total_rows);
            if side_by_side {
                let half = rect.width() / 2.0;
                for (i, row) in rows.iter().enumerate().take(last).skip(first) {
                    let y = rect.top() + 4.0 + i as f32 * LINE_HEIGHT;
                    let left = Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(half - 1.0, LINE_HEIGHT));
                    let right = Rect::from_min_size(egui::pos2(rect.left() + half, y), egui::vec2(half, LINE_HEIGHT));
                    let (lh, rh) = match (row.changed, row.left, row.right) {
                        (true, Some(l), Some(r)) => char_changes(l.text, r.text),
                        _ => (Vec::new(), Vec::new()),
                    };
                    line(ui, left, row.left, true, &lh, row.changed);
                    line(ui, right, row.right, false, &rh, row.changed);
                }
                // The divider between the two editors.
                ui.painter().vline(rect.left() + half - 0.5, rect.y_range(), Stroke::new(1.0, Palette::DIFF_DIVIDER()));
            } else {
                // Inline: a changed pair shows the removed line, then the added one.
                let mut i = 0usize;
                for row in &rows {
                    let parts: Vec<(Option<Line>, bool)> = match (row.changed, row.left, row.right) {
                        (false, _, r) => vec![(r, false)],
                        (true, Some(l), Some(r)) => vec![(Some(l), true), (Some(r), false)],
                        (true, l @ Some(_), None) => vec![(l, true)],
                        (true, None, r) => vec![(r, false)],
                    };
                    let (lh, rh) = match (row.changed, row.left, row.right) {
                        (true, Some(l), Some(r)) => char_changes(l.text, r.text),
                        _ => (Vec::new(), Vec::new()),
                    };
                    for (side, removed) in parts {
                        if i >= first && i < last {
                            let y = rect.top() + 4.0 + i as f32 * LINE_HEIGHT;
                            let r = Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(rect.width(), LINE_HEIGHT));
                            line(ui, r, side, removed, if removed { &lh } else { &rh }, row.changed);
                        }
                        i += 1;
                    }
                }
            }
        });

    // Overview ruler: where the changes are, and the visible part.
    let ruler = Rect::from_min_max(egui::pos2(content.right(), full.top()), full.max);
    let p = ui.painter();
    p.vline(ruler.left(), ruler.y_range(), Stroke::new(1.0, Palette::DIFF_DIVIDER()));
    if !rows.is_empty() {
        let scale = ruler.height() / rows.len() as f32;
        let half = ruler.width() / 2.0;
        for (i, row) in rows.iter().enumerate().filter(|(_, r)| r.changed) {
            let y = ruler.top() + i as f32 * scale;
            let h = scale.max(2.0);
            if row.left.is_some() {
                p.rect_filled(Rect::from_min_size(egui::pos2(ruler.left() + 1.0, y), egui::vec2(half - 1.0, h)), 0.0, Palette::GIT_DELETED_FG().gamma_multiply(0.6));
            }
            if row.right.is_some() {
                p.rect_filled(Rect::from_min_size(egui::pos2(ruler.left() + half, y), egui::vec2(half, h)), 0.0, Palette::GIT_ADDED_FG().gamma_multiply(0.6));
            }
        }
        let total_h = LINE_HEIGHT * total_rows as f32 + 8.0;
        if total_h > content.height() {
            let top = ruler.top() + viewport.0 / total_h * ruler.height();
            let bottom = ruler.top() + viewport.1 / total_h * ruler.height();
            p.rect_filled(Rect::from_min_max(egui::pos2(ruler.left(), top), egui::pos2(ruler.right(), bottom)), 0.0, Palette::DIFF_RULER_SLIDER());
        }
    }
    ui.allocate_rect(full, Sense::hover());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_pair_replaced_lines_and_leave_gaps() {
        let base = ["a", "b", "c"];
        let work = ["a", "B", "c", "d"];
        let ops = similar::capture_diff_slices(similar::Algorithm::Myers, &base, &work);
        let rows = align(&base, &work, &ops);
        let shape: Vec<(bool, bool, bool)> = rows.iter().map(|r| (r.left.is_some(), r.right.is_some(), r.changed)).collect();
        assert_eq!(shape, [(true, true, false), (true, true, true), (true, true, false), (false, true, true)]);
    }

    #[test]
    fn changed_characters_are_found() {
        let (l, r) = char_changes("println!(\"hi\");", "println!(\"hello\");");
        assert!(!l.is_empty() && !r.is_empty());
        assert_eq!(r[0].0, 11);
    }
}
