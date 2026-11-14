use std::collections::BTreeMap;

use egui::text::{CCursor, CCursorRange};
use egui::{Align2, FontId, Sense, TextEdit, Ui};

use crate::git::DiffKind;
use crate::icons::{codicon_font, editor_mono_font, CHEVRON_DOWN, CHEVRON_RIGHT};
use crate::theme::Palette;

use super::buffer::Document;
use super::fold;
use super::highlight::build_layout_job;

// VS Code macOS defaults: Menlo, fontSize 12, lineHeight = 1.5 * fontSize = 18.
const EDITOR_FONT_SIZE: f32 = 12.0;
const LINE_HEIGHT: f32 = EDITOR_FONT_SIZE * 1.5;
const CHEVRON_W: f32 = 16.0;
/// The TextEdit's own left inset before the first character.
const TEXT_PAD: f32 = 6.0;
/// `scrollbar.verticalScrollbarSize`: the strip right of the minimap.
const SCROLLBAR_W: f32 = 14.0;
/// Temp-data key: the editable view's (scroll id, offset, content height,
/// viewport height) for `paint_scrollbar`.
const SCROLL_INFO: &str = "vr-editor-scroll-info";
/// Width of the diff decoration strip on the far left of the gutter.
const DIFF_W: f32 = 3.0;

/// A wavy underline under a byte range (a problem from an extension).
#[derive(Clone, Debug, PartialEq)]
pub struct Squiggle {
    pub start: usize,
    pub end: usize,
    pub color: egui::Color32,
}

/// What the code view reports back for language features.
#[derive(Default)]
pub struct EditorOutput {
    /// Screen rect of the caret, when the editor shows editable text.
    pub caret: Option<egui::Rect>,
    /// Character index under the mouse pointer, and the pointer position.
    pub hovered: Option<(usize, egui::Pos2)>,
    /// Character index ⌘-clicked (go to definition).
    pub command_clicked: Option<usize>,
    /// Whether the text changed this frame.
    pub changed: bool,
    /// Character index of the caret.
    pub caret_char: Option<usize>,
}

/// The scroll info of the editor whose area starts at `origin`: each
/// editor group keeps its own.
fn scroll_key(origin: egui::Pos2) -> egui::Id {
    egui::Id::new(SCROLL_INFO).with((origin.x.round() as i32, origin.y.round() as i32))
}

pub fn show(
    ui: &mut Ui,
    doc: &mut Document,
    diff: &BTreeMap<usize, DiffKind>,
    find: Option<&super::highlight::FindHighlight>,
) -> EditorOutput {
    show_with_squiggles(ui, doc, diff, find, &[])
}

/// [`show`] with wavy underlines for problems.
pub fn show_with_squiggles(
    ui: &mut Ui,
    doc: &mut Document,
    diff: &BTreeMap<usize, DiffKind>,
    find: Option<&super::highlight::FindHighlight>,
    squiggles: &[Squiggle],
) -> EditorOutput {
    let mut out = EditorOutput::default();
    // Diff tabs render a read-only inline diff against their HEAD base.
    if let Some(base) = &doc.diff_base {
        super::diff_view::show(ui, base, &doc.text, doc.language);
        return out;
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
    // Real advance width of a Menlo digit at the editor size.
    let char_w = ui
        .painter()
        .layout_no_wrap("0".to_string(), editor_mono_font(EDITOR_FONT_SIZE), Palette::FG())
        .size()
        .x
        .max(6.0);
    // editorOptions computeLayout, measured against VS Code 1.145.2: the
    // numbers (≥ `lineNumbersMinChars` 5 digits) end 12px + their width in,
    // the text starts 19px after them (the TextEdit adds `TEXT_PAD`).
    let numbers = (gutter_digits.max(5) as f32 * char_w).round();
    let number_w = 12.0 + numbers + 4.0;
    let gutter_width = 12.0 + numbers + 18.0 - TEXT_PAD;

    let has_folds = !doc.folded.is_empty();

    // Split the editor area: code body on the left, minimap on the right.
    let full = ui.available_rect_before_wrap();
    let mm_w = super::minimap::width(full.width() - gutter_width - TEXT_PAD, char_w);
    let show_minimap = mm_w > 0.0;
    let editor_rect = egui::Rect::from_min_max(
        full.min,
        egui::pos2(full.right() - mm_w - SCROLLBAR_W, full.bottom()),
    );

    ui.scope_builder(egui::UiBuilder::new().max_rect(editor_rect).layout(*ui.layout()), |ui| {
        if has_folds {
            show_folded(ui, doc, &all_lines, &visible, &ranges, gutter_width, number_w, diff);
        } else {
            out = show_editable(ui, doc, line_count, &ranges, gutter_width, number_w, diff, find, squiggles);
        }
    });

    if show_minimap {
        let mm_rect = egui::Rect::from_min_max(
            egui::pos2(full.right() - mm_w - SCROLLBAR_W, full.top()),
            egui::pos2(full.right() - SCROLLBAR_W, full.bottom()),
        );
        let visible_rows = (editor_rect.height() / LINE_HEIGHT).ceil() as usize;
        let top_line = ui
            .ctx()
            .data(|d| d.get_temp::<(egui::Id, f32, f32, f32)>(scroll_key(full.min)))
            .map(|(_, off, _, _)| (off / LINE_HEIGHT) as usize)
            .unwrap_or(0);
        super::minimap::show(ui, mm_rect, doc, top_line, visible_rows);
    }
    if !has_folds {
        let strip = egui::Rect::from_min_max(egui::pos2(full.right() - SCROLLBAR_W, full.top()), full.max);
        paint_scrollbar(ui, strip, full, doc.cursor_line);
    }
    out
}

/// Paint a wavy underline under characters `[start, end)` of `galley`
/// (VS Code's squiggly: a zigzag along the bottom of each line's run).
fn paint_squiggle(ui: &Ui, galley: &egui::Galley, origin: egui::Pos2, text: &str, sq: &Squiggle) {
    let start_char = text[..sq.start.min(text.len())].chars().count();
    let mut end_char = text[..sq.end.min(text.len())].chars().count();
    if end_char <= start_char {
        end_char = start_char + 1; // empty ranges still get a mark
    }
    let mut a = start_char;
    while a < end_char {
        let ra = galley.pos_from_cursor(CCursor::new(a));
        // Extend along the same row.
        let mut b = a + 1;
        while b < end_char && (galley.pos_from_cursor(CCursor::new(b)).top() - ra.top()).abs() < 1.0 {
            b += 1;
        }
        let rb = galley.pos_from_cursor(CCursor::new(b));
        let x1 = if (rb.top() - ra.top()).abs() < 1.0 { rb.left() } else { ra.left() + 7.0 };
        let y = origin.y + ra.bottom() - 1.0;
        let (x0, x1) = (origin.x + ra.left(), origin.x + x1.max(ra.left() + 3.0));
        let mut points = Vec::new();
        let mut x = x0;
        let mut up = true;
        while x < x1 {
            points.push(egui::pos2(x, if up { y - 1.5 } else { y + 1.0 }));
            x += 2.0;
            up = !up;
        }
        points.push(egui::pos2(x1, y));
        ui.painter().add(egui::Shape::line(points, egui::Stroke::new(1.0, sq.color)));
        // Next row (skip the line break).
        a = b.max(a + 1);
        while a < end_char && (galley.pos_from_cursor(CCursor::new(a)).top() - ra.top()).abs() < 1.0 {
            a += 1;
        }
    }
}

/// Paint the Git diff strip for one row at vertical centre `y_center`.
fn paint_diff(ui: &Ui, gutter_left: f32, y_center: f32, kind: DiffKind) {
    let color = match kind {
        DiffKind::Added => Palette::GIT_GUTTER_ADDED(),
        DiffKind::Modified => Palette::GIT_GUTTER_MODIFIED(),
        DiffKind::DeletedAbove => Palette::GIT_GUTTER_DELETED(),
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
        let y = gutter_rect.top() + row as f32 * LINE_HEIGHT + LINE_HEIGHT / 2.0;

        // Git diff decoration on the far-left edge of the gutter.
        if let Some(&kind) = diff.get(&ln) {
            paint_diff(ui, gutter_rect.left(), y, kind);
        }

        // Line number, right-aligned within the number sub-column.
        let color = if ln == cursor_line {
            Palette::LINE_NUMBER_ACTIVE_FG()
        } else {
            Palette::LINE_NUMBER_FG()
        };
        painter.text(
            egui::pos2(gutter_rect.left() + number_w - 4.0, y),
            Align2::RIGHT_CENTER,
            ln.to_string(),
            editor_mono_font(EDITOR_FONT_SIZE),
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
            Palette::FG_DESCRIPTION(),
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
    find: Option<&super::highlight::FindHighlight>,
    squiggles: &[Squiggle],
) -> EditorOutput {
    let mut out = EditorOutput::default();
    let pending = doc.pending_nav.take();
    // A read-only document keeps its text: edits are undone below.
    let read_only_before = doc.read_only.then(|| doc.text.clone());
    let ctx = ui.ctx().clone();
    let style = ui.style().clone();
    let language = doc.language;
    let find_owned = find.cloned();
    let mut layouter = move |ui: &Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
        let job = build_layout_job(
            &ctx,
            &style,
            text.as_str(),
            language,
            wrap_width,
            find_owned.as_ref(),
        );
        ui.ctx().fonts_mut(|f| f.layout_job(job))
    };

    let cursor_line = doc.cursor_line;
    let rows: Vec<usize> = (1..=line_count).collect();
    let mut toggled = None;

    // `editor.scrollBeyondLastLine`: the last line can scroll to the top.
    let viewport_h = ui.available_height();
    let scroll = egui::ScrollArea::both()
        // Per editor, like VS Code's view state (scroll and selection).
        .id_salt(("editor_scroll", &doc.path))
        .auto_shrink([false, false])
        // The vertical scrollbar is drawn right of the minimap
        // (`paint_scrollbar`), the horizontal one isn't shown.
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                let (gutter_rect, _) = ui.allocate_exact_size(
                    egui::vec2(gutter_width, LINE_HEIGHT * line_count as f32 + (viewport_h - LINE_HEIGHT).max(0.0)),
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

                // Current-line highlight (editor.lineHighlightBackground),
                // painted under the TextEdit — whose frame is therefore
                // transparent (the CentralPanel supplies the editor fill).
                if cursor_line >= 1 {
                    let hl_y = gutter_rect.top() + (cursor_line as f32 - 1.0) * LINE_HEIGHT;
                    // `renderLineHighlight: line`: from the text's start to
                    // the minimap.
                    ui.painter().rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(gutter_rect.right() + TEXT_PAD, hl_y),
                            egui::pos2(ui.max_rect().right(), hl_y + LINE_HEIGHT),
                        ),
                        0.0,
                        Palette::LINE_HIGHLIGHT_BG(),
                    );
                }

                paint_indent_guides(ui, &doc.text, gutter_rect.right() + TEXT_PAD, gutter_rect.top(), cursor_line);

                let text_id = ui.make_persistent_id(("editor_text", &doc.path));
                TextEdit::multiline(&mut doc.text)
                        .id(text_id)
                        .font(FontId::monospace(EDITOR_FONT_SIZE))
                        .code_editor()
                        .desired_rows(40)
                        .desired_width(f32::INFINITY)
                        .lock_focus(true)
                        .frame(
                            egui::Frame::default()
                                .fill(egui::Color32::TRANSPARENT)
                                .inner_margin(egui::Margin { left: TEXT_PAD as i8, right: TEXT_PAD as i8, top: 0, bottom: 0 }),
                        )
                        .layouter(&mut layouter)
                        .show(ui)
            })
            .inner
        })
        ;
    ui.ctx().data_mut(|d| {
        d.insert_temp(
            scroll_key(ui.max_rect().min),
            (scroll.id, scroll.state.offset.y, scroll.content_size.y, scroll.inner_rect.height()),
        )
    });
    let output = scroll.inner;
    if let Some(before) = read_only_before {
        if doc.text != before {
            doc.text = before;
        }
    }
    let galley = output.galley.clone();
    let origin = output.galley_pos;
    for sq in squiggles {
        paint_squiggle(ui, &galley, origin, &doc.text, sq);
    }
    if let Some(range) = output.cursor_range {
        out.caret = Some(galley.pos_from_cursor(range.primary).translate(origin.to_vec2()));
        out.caret_char = Some(range.primary.index);
    }
    let response = output.response.response;
    if let Some(pointer) = response.hover_pos() {
        let c = galley.cursor_from_pos(pointer - origin);
        out.hovered = Some((c.index, pointer));
        if response.clicked() && ui.input(|i| i.modifiers.command) {
            out.command_clicked = Some(c.index);
        }
    }
    out.changed = response.changed() && !doc.read_only;

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
    out
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

                let body_h = LINE_HEIGHT * visible.len() as f32;
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
                    .rect_filled(code_rect, 0.0, Palette::EDITOR_BG());

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
                        None,
                    );
                    let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
                    let y = code_rect.top() + row as f32 * LINE_HEIGHT
                        + (LINE_HEIGHT - galley.size().y) / 2.0;
                    ui.painter().galley(
                        egui::pos2(code_rect.left() + TEXT_PAD, y),
                        galley,
                        Palette::FG(),
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

/// Indentation unit in columns (`editor.detectIndentation`, simplified):
/// the smallest indentation of a space-indented line, else 4.
fn indent_size(text: &str) -> usize {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start_matches(' ').len())
        .filter(|&n| n > 0)
        .min()
        .unwrap_or(4)
        .min(8)
}

/// Leading whitespace in columns (tabs as 4).
fn indent_columns(line: &str) -> usize {
    let mut cols = 0;
    for c in line.chars() {
        match c {
            ' ' => cols += 1,
            '\t' => cols += 4,
            _ => break,
        }
    }
    cols
}

/// `editor.guides.indentation`: a 1px line at each indentation level a line
/// is inside (blank lines take the smaller level of their neighbours), the
/// cursor's innermost block in `editorIndentGuide.activeBackground1`.
fn paint_indent_guides(ui: &Ui, text: &str, content_left: f32, top: f32, cursor_line: usize) {
    let unit = indent_size(text);
    let char_w = ui
        .painter()
        .layout_no_wrap("0".into(), editor_mono_font(EDITOR_FONT_SIZE), Palette::FG())
        .size()
        .x;
    let lines: Vec<&str> = text.split('\n').collect();
    // Levels per line; blank lines get min(previous, next) of the non-blank.
    let raw: Vec<Option<usize>> = lines
        .iter()
        .map(|l| (!l.trim().is_empty()).then(|| indent_columns(l) / unit))
        .collect();
    let mut levels = vec![0usize; lines.len()];
    for i in 0..lines.len() {
        levels[i] = match raw[i] {
            Some(l) => l,
            None => {
                let prev = raw[..i].iter().rev().flatten().next().copied().unwrap_or(0);
                let next = raw[i + 1..].iter().flatten().next().copied().unwrap_or(0);
                prev.min(next)
            }
        };
    }
    // The active block: the cursor line's level, spread over its neighbours
    // at that level or deeper.
    let cur = cursor_line.saturating_sub(1).min(lines.len().saturating_sub(1));
    let active_level = levels.get(cur).copied().unwrap_or(0);
    let (mut a, mut b) = (cur, cur);
    if active_level > 0 {
        while a > 0 && levels[a - 1] >= active_level {
            a -= 1;
        }
        while b + 1 < lines.len() && levels[b + 1] >= active_level {
            b += 1;
        }
    }
    let visible = ui.clip_rect();
    for (i, &level) in levels.iter().enumerate() {
        let y = top + i as f32 * LINE_HEIGHT;
        if y > visible.bottom() || y + LINE_HEIGHT < visible.top() {
            continue;
        }
        for g in 0..level {
            let x = content_left + (g * unit) as f32 * char_w;
            let active = active_level > 0 && g + 1 == active_level && i >= a && i <= b;
            let colour = if active { Palette::INDENT_GUIDE_ACTIVE() } else { Palette::INDENT_GUIDE() };
            ui.painter().rect_filled(
                egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(1.0, LINE_HEIGHT)),
                0.0,
                colour,
            );
        }
    }
}

/// The editor's vertical scrollbar in its 14px strip
/// (`scrollbar.verticalScrollbarSize`): a `scrollbarSlider.*` slider shown
/// while the pointer is over the editor, draggable.
fn paint_scrollbar(ui: &mut Ui, strip: egui::Rect, editor: egui::Rect, cursor_line: usize) {
    let Some((id, offset, content_h, view_h)) =
        ui.ctx().data(|d| d.get_temp::<(egui::Id, f32, f32, f32)>(scroll_key(editor.min)))
    else {
        return;
    };
    // The overview ruler shares the strip (decorationsOverviewRuler.ts):
    // its left border and the cursor (2px, editorCursor.foreground at 70%).
    ui.painter().rect_filled(
        egui::Rect::from_min_size(strip.min, egui::vec2(1.0, strip.height())),
        0.0,
        Palette::OVERVIEW_RULER_BORDER(),
    );
    if content_h > 0.0 {
        let y = strip.top() + (cursor_line.saturating_sub(1) as f32 * LINE_HEIGHT) / content_h * strip.height();
        ui.painter().rect_filled(
            egui::Rect::from_min_size(egui::pos2(strip.left() + 1.0, y), egui::vec2(strip.width() - 1.0, 2.0)),
            0.0,
            Palette::CURSOR_FG().gamma_multiply(0.7),
        );
    }
    let range = content_h - view_h;
    if range <= 0.5 {
        return;
    }
    let slider_h = (view_h / content_h * strip.height()).max(20.0);
    let travel = strip.height() - slider_h;
    let y = strip.top() + offset / range * travel;
    let slider = egui::Rect::from_min_size(egui::pos2(strip.left(), y), egui::vec2(strip.width(), slider_h));
    let resp = ui.interact(slider, id.with("vscode-slider"), Sense::drag());
    if resp.dragged() {
        let dy = resp.drag_delta().y;
        if let Some(mut state) = egui::scroll_area::State::load(ui.ctx(), id) {
            state.offset.y = (state.offset.y + dy * range / travel.max(1.0)).clamp(0.0, range);
            state.store(ui.ctx(), id);
            ui.ctx().request_repaint();
        }
    }
    if ui.rect_contains_pointer(editor) || resp.dragged() {
        let colour = if resp.dragged() {
            Palette::VSCE_SCROLLBAR_SLIDER_ACTIVE()
        } else if resp.hovered() {
            Palette::VSCE_SCROLLBAR_SLIDER_HOVER()
        } else {
            Palette::VSCE_SCROLLBAR_SLIDER_BG()
        };
        ui.painter().rect_filled(slider, 4.0, colour);
    }
}
