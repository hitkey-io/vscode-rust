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
    // The find widget docks to this area, left of the minimap and scrollbar.
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(super::find::ANCHOR), (full, mm_w + SCROLLBAR_W)));

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

/// egui memory key of an editor action queued by a menu command.
pub const EDITOR_ACTION: &str = "editor-action";

/// egui memory key of the text ⌘F seeds the find input with.
pub const FIND_SEED: &str = "find-seed";

/// The editing a plain text box lacks (`smart.rs`): auto-closing pairs, auto
/// indent, Backspace over a pair, Tab, line comments and the line commands.
/// Reads this frame's keys and typed characters while the editor has the
/// keyboard, consumes the ones it handles and applies the edit to the text and
/// the selection. `true` when the text changed.
fn smart_keys(ctx: &egui::Context, id: egui::Id, doc: &mut Document) -> bool {
    use super::smart;
    use egui::{Event, Key, Modifiers};
    // An action asked for by a menu applies to the editor whether or not it
    // holds the keyboard; keys only count while it does.
    let action = ctx.data_mut(|d| d.remove_temp::<&'static str>(egui::Id::new(EDITOR_ACTION)));
    let focused = ctx.memory(|m| m.has_focus(id));
    if action.is_none() && !focused {
        return false;
    }
    let Some(state) = TextEdit::load_state(ctx, id) else { return false };
    let Some(range) = state.cursor.char_range() else { return false };
    let (a, h) = (range.secondary.index, range.primary.index);
    let (tabs, size) = doc.indentation();
    let unit = if tabs { "\t".to_string() } else { " ".repeat(size) };
    let language = doc.language;
    let text = doc.text.clone();
    let alt = Modifiers::ALT;
    let multi_action = action.filter(|a| matches!(*a, "add-next" | "select-all" | "cursor-above" | "cursor-below"));
    if (focused && action.is_none()) || multi_action.is_some() {
        if let Some(changed) = multi_keys(ctx, id, doc, state.clone(), (a, h), multi_action) {
            return changed;
        }
    }
    // Expand / Shrink Selection: the ranges left behind are kept to shrink back.
    let pre: Option<smart::Edit> = match action {
        Some("expand") | Some("shrink") => {
            let key = egui::Id::new(("expand-stack", &doc.path));
            let mut stack: Vec<(usize, usize)> = ctx.data(|d| d.get_temp(key)).unwrap_or_default();
            let sel = (a.min(h), a.max(h));
            let result = if action == Some("expand") {
                smart::expand_selection(&text, a, h).map(|(x, y)| {
                    stack.push(sel);
                    smart::Edit { text: text.clone(), anchor: x, head: y }
                })
            } else {
                stack.pop().map(|(x, y)| smart::Edit { text: text.clone(), anchor: x, head: y })
            };
            ctx.data_mut(|d| d.insert_temp(key, stack));
            result
        }
        _ => None,
    };
    let edit: Option<smart::Edit> = if pre.is_some() {
        pre
    } else if !focused {
        (action == Some("duplicate")).then(|| smart::duplicate_selection(&text, a, h))
    } else { ctx.input_mut(|i| {
        if action == Some("duplicate") {
            return Some(smart::duplicate_selection(&text, a, h));
        }
        // Keys that don't produce text: exact modifiers, consumed when handled.
        if i.consume_key(Modifiers::NONE, Key::Enter) {
            return Some(smart::enter(&text, a, h, &unit, language));
        }
        if i.consume_key(Modifiers::COMMAND, Key::Enter) {
            return Some(smart::insert_line(&text, a, h, true, &unit));
        }
        if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Enter) {
            return Some(smart::insert_line(&text, a, h, false, &unit));
        }
        if i.consume_key(Modifiers::NONE, Key::Tab) {
            return Some(smart::tab(&text, a, h, &unit, size));
        }
        if i.consume_key(Modifiers::SHIFT, Key::Tab) {
            return smart::outdent_lines(&text, a, h, size);
        }
        if i.consume_key(Modifiers::COMMAND, Key::OpenBracket) {
            return smart::outdent_lines(&text, a, h, size);
        }
        if i.consume_key(Modifiers::COMMAND, Key::CloseBracket) {
            return Some(smart::indent_lines(&text, a, h, &unit));
        }
        if i.consume_key(Modifiers::COMMAND, Key::Slash) {
            return smart::toggle_line_comment(&text, a, h, language);
        }
        let mut alt_combo = None;
        if i.consume_key(alt | Modifiers::SHIFT, Key::A) {
            alt_combo = Some(smart::toggle_block_comment(&text, a, h, language));
        } else if i.consume_key(alt, Key::ArrowUp) {
            alt_combo = Some(smart::move_lines(&text, a, h, true));
        } else if i.consume_key(alt, Key::ArrowDown) {
            alt_combo = Some(smart::move_lines(&text, a, h, false));
        } else if i.consume_key(alt | Modifiers::SHIFT, Key::ArrowUp) {
            alt_combo = Some(Some(smart::copy_lines(&text, a, h, true)));
        } else if i.consume_key(alt | Modifiers::SHIFT, Key::ArrowDown) {
            alt_combo = Some(Some(smart::copy_lines(&text, a, h, false)));
        }
        if let Some(result) = alt_combo {
            // Option makes macOS type a character too (Å, ˚): not for the text.
            i.events.retain(|e| !matches!(e, Event::Text(t) if t.chars().any(|c| !c.is_ascii())));
            return result;
        }
        if i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::K) {
            return Some(smart::delete_lines(&text, a, h));
        }
        // Backspace over an empty pair or in the indentation.
        if i.key_pressed(Key::Backspace) && i.modifiers.is_none() {
            if let Some(e) = smart::backspace(&text, a, h, size) {
                i.consume_key(Modifiers::NONE, Key::Backspace);
                return Some(e);
            }
        }
        // Typed characters that open, close or surround.
        let quotes = smart::closes_quotes(language);
        let hit = i.events.iter().enumerate().find_map(|(n, e)| match e {
            Event::Text(t) if t.chars().count() == 1 => smart::type_char(&text, a, h, t.chars().next()?, quotes).map(|e| (n, e)),
            _ => None,
        });
        if let Some((n, e)) = hit {
            i.events.remove(n);
            return Some(e);
        }
        None
    }) };
    let Some(e) = edit else { return false };
    let changed = e.text != doc.text;
    doc.text = e.text;
    let mut state = state;
    state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(e.anchor), CCursor::new(e.head))));
    state.store(ctx, id);
    if changed {
        doc.check_dirty();
    }
    changed
}

/// Editable path — no folds active. Standard `TextEdit` over the full text
/// with the fold gutter painted alongside.
/// The extra cursors of an editor (besides the `TextEdit`'s own, which is
/// the primary one), kept in egui memory per file.
pub fn extra_cursors_key(path: &std::path::Path) -> egui::Id {
    egui::Id::new(("multi-sel", path))
}

/// Keys for several cursors (`editor/multi.rs`): ⌘D, ⇧⌘L, ⌥⌘↑ / ⌥⌘↓ add
/// cursors; while there are extra ones, typing, Backspace, Delete, Enter,
/// Tab, ← / →, Home / End (with ⇧) and paste act at all of them, ⌘C / ⌘X
/// take every selection (one per line), Escape keeps the primary only, and
/// any other key drops the extras before the editor handles it. `None` when
/// nothing here applied.
fn multi_keys(ctx: &egui::Context, id: egui::Id, doc: &mut Document, mut state: egui::text_edit::TextEditState, primary: (usize, usize), action: Option<&str>) -> Option<bool> {
    use super::multi::{self, Op};
    use egui::{Event, Key, Modifiers};
    let key = extra_cursors_key(&doc.path);
    let word_key = egui::Id::new(("multi-word", &doc.path));
    let mut extras: Vec<multi::Sel> = ctx.data(|d| d.get_temp(key)).unwrap_or_default();
    let mut word_mode: bool = ctx.data(|d| d.get_temp(word_key)).unwrap_or(false);
    let mut all = extras.clone();
    all.push(primary);
    let mut next: Option<(String, Vec<multi::Sel>)> = None;
    let mut handled = false;
    ctx.input_mut(|i| {
        if action == Some("add-next") || (action.is_none() && i.consume_key(Modifiers::COMMAND, Key::D)) {
            handled = true;
            if extras.is_empty() && primary.0 == primary.1 {
                word_mode = false;
            }
            if let Some(s) = multi::add_next_occurrence(&doc.text, &all, &mut word_mode) {
                next = Some((doc.text.clone(), s));
            }
        } else if action == Some("select-all") || (action.is_none() && i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::L)) {
            handled = true;
            next = Some((doc.text.clone(), multi::select_all_occurrences(&doc.text, primary)));
        } else if action == Some("cursor-above") || (action.is_none() && i.consume_key(Modifiers::COMMAND | Modifiers::ALT, Key::ArrowUp)) {
            handled = true;
            next = Some((doc.text.clone(), multi::add_cursor_vertical(&doc.text, &all, true)));
        } else if action == Some("cursor-below") || (action.is_none() && i.consume_key(Modifiers::COMMAND | Modifiers::ALT, Key::ArrowDown)) {
            handled = true;
            next = Some((doc.text.clone(), multi::add_cursor_vertical(&doc.text, &all, false)));
        }
        if handled || extras.is_empty() || action.is_some() {
            return;
        }
        if i.consume_key(Modifiers::NONE, Key::Escape) {
            handled = true;
            next = Some((doc.text.clone(), vec![primary]));
            return;
        }
        let keys: [(Modifiers, Key, Op); 12] = [
            (Modifiers::NONE, Key::Backspace, Op::Backspace),
            (Modifiers::NONE, Key::Delete, Op::Delete),
            (Modifiers::NONE, Key::Enter, Op::Insert("\n".into())),
            (Modifiers::NONE, Key::Tab, Op::Insert("\t".into())),
            (Modifiers::NONE, Key::ArrowLeft, Op::Left { shift: false }),
            (Modifiers::SHIFT, Key::ArrowLeft, Op::Left { shift: true }),
            (Modifiers::NONE, Key::ArrowRight, Op::Right { shift: false }),
            (Modifiers::SHIFT, Key::ArrowRight, Op::Right { shift: true }),
            (Modifiers::NONE, Key::Home, Op::Home { shift: false }),
            (Modifiers::SHIFT, Key::Home, Op::Home { shift: true }),
            (Modifiers::NONE, Key::End, Op::End { shift: false }),
            (Modifiers::SHIFT, Key::End, Op::End { shift: true }),
        ];
        let mut op = None;
        for (m, k, o) in keys {
            if i.consume_key(m, k) {
                op = Some(o);
                break;
            }
        }
        if op.is_none() {
            // Typed text and paste go to every cursor; copy / cut take them all.
            let mut copied = None;
            let mut cut = false;
            i.events.retain(|e| match e {
                Event::Text(t) | Event::Paste(t) if op.is_none() => {
                    op = Some(Op::Insert(t.clone()));
                    false
                }
                Event::Copy | Event::Cut => {
                    cut |= matches!(e, Event::Cut);
                    copied = Some(());
                    false
                }
                _ => true,
            });
            if copied.is_some() {
                let chars: Vec<char> = doc.text.chars().collect();
                let mut sorted = all.clone();
                sorted.sort_by_key(|(a, h)| (*a).min(*h));
                let text: Vec<String> = sorted.iter().map(|&(a, h)| chars[a.min(h)..a.max(h).min(chars.len())].iter().collect()).collect();
                ctx.copy_text(text.join("\n"));
                handled = true;
                if cut {
                    op = Some(Op::Insert(String::new()));
                }
            }
        }
        match op {
            Some(o) => {
                handled = true;
                next = Some(multi::apply(&doc.text, &all, &o));
            }
            None => {
                // Another key: back to one cursor, the editor handles it.
                let other = i.events.iter().any(|e| matches!(e, Event::Key { pressed: true, .. }));
                if other {
                    extras.clear();
                    ctx.data_mut(|d| d.remove::<Vec<multi::Sel>>(key));
                }
            }
        }
    });
    ctx.data_mut(|d| d.insert_temp(word_key, word_mode));
    let (text, mut sels) = next?;
    let changed = text != doc.text;
    doc.text = text;
    let primary = sels.pop().unwrap_or(primary);
    ctx.data_mut(|d| d.insert_temp(key, sels));
    state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(primary.0), CCursor::new(primary.1))));
    state.store(ctx, id);
    if changed {
        doc.check_dirty();
    }
    handled.then_some(changed)
}

/// Paints the extra cursors and their selections over the editor's text.
fn paint_extra_cursors(ui: &Ui, clip: egui::Rect, galley: &egui::Galley, origin: egui::Pos2, text: &str, extras: &[(usize, usize)]) {
    let p = ui.painter().with_clip_rect(clip.intersect(ui.clip_rect()));
    let chars: Vec<char> = text.chars().collect();
    let pos = |c: usize| galley.pos_from_cursor(CCursor::new(c.min(chars.len()))).translate(origin.to_vec2());
    for &(a, h) in extras {
        let (s, e) = (a.min(h), a.max(h));
        if s != e {
            // One rect per line of the selection.
            let mut from = s;
            while from < e {
                let line_end = (from..e).find(|&k| chars[k] == '\n').unwrap_or(e);
                let (r0, r1) = (pos(from), pos(line_end));
                let right = if line_end < e { r1.left() + 6.0 } else { r1.left() };
                p.rect_filled(egui::Rect::from_min_max(egui::pos2(r0.left(), r0.top()), egui::pos2(right, r0.bottom())), 0.0, Palette::SELECTION_BG());
                from = line_end + 1;
            }
        }
        let c = pos(h);
        p.rect_filled(egui::Rect::from_min_size(c.left_top(), egui::vec2(2.0, c.height())), 0.0, Palette::CURSOR_FG());
    }
}

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
    let mut find_owned = find.cloned();
    let prev_sel = ctx.data(|d| d.get_temp::<(usize, usize)>(egui::Id::new(("marks-sel", &doc.path))));
    // Marks around the selection from the last frame's caret: other
    // occurrences of the selected text and the bracket pair at the caret.
    if let Some((anchor, head)) = ctx.data(|d| d.get_temp::<(usize, usize)>(egui::Id::new(("marks-sel", &doc.path)))) {
        let mut spans = super::marks::selection_occurrences(&doc.text, anchor, head)
            .into_iter()
            .map(|(start, end)| super::highlight::DecorationSpan { start, end, background: Some(Palette::SELECTION_HIGHLIGHT_BG()), color: None, underline: None })
            .collect::<Vec<_>>();
        if anchor == head {
            if let Some((a, b)) = super::marks::bracket_pair(&doc.text, head) {
                for i in [a, b] {
                    let start = doc.text.char_indices().nth(i).map_or(doc.text.len(), |(b, _)| b);
                    let end = doc.text[start..].chars().next().map_or(start, |c| start + c.len_utf8());
                    spans.push(super::highlight::DecorationSpan { start, end, background: Some(Palette::BRACKET_MATCH_BG()), color: None, underline: None });
                }
            }
        }
        if !spans.is_empty() {
            let f = find_owned.get_or_insert_with(Default::default);
            spans.extend(std::mem::take(&mut f.decorations));
            f.decorations = spans;
        }
    }
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
    let mut smart_changed = false;

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
                let has_extras = ui.ctx().data(|d| d.get_temp::<Vec<(usize, usize)>>(extra_cursors_key(&doc.path))).is_some_and(|v| !v.is_empty());
                if !doc.read_only && smart_keys(ui.ctx(), text_id, doc) {
                    smart_changed = true;
                }
                let shown = TextEdit::multiline(&mut doc.text)
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
                        .show(ui);
                // With extra cursors Escape stays in the editor (it drops
                // them) instead of taking the keyboard away.
                if has_extras && ui.memory(|m| m.has_focus(text_id)) {
                    ui.memory_mut(|m| m.set_focus_lock_filter(text_id, egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, tab: true, escape: true }));
                }
                shown
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
    let text_clip = scroll.inner_rect;
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
    // ⌥-click adds a cursor (the one before stays); a plain click or an
    // edit the editor made itself leaves one cursor.
    {
        let key = extra_cursors_key(&doc.path);
        let resp = &output.response.response;
        let mut extras: Vec<(usize, usize)> = ui.ctx().data(|d| d.get_temp(key)).unwrap_or_default();
        if resp.clicked() || resp.changed() {
            let alt = ui.input(|i| i.modifiers.alt && !i.modifiers.command);
            match (alt && resp.clicked(), prev_sel) {
                (true, Some(prev)) if output.cursor_range.is_some_and(|r| r.primary.index != prev.1) => {
                    if !extras.contains(&prev) {
                        extras.push(prev);
                    }
                }
                _ => extras.clear(),
            }
            ui.ctx().data_mut(|d| d.insert_temp(key, extras.clone()));
        }
        if !extras.is_empty() {
            paint_extra_cursors(ui, text_clip, &galley, origin, &doc.text, &extras);
        }
    }
    if let Some(range) = output.cursor_range {
        out.caret = Some(galley.pos_from_cursor(range.primary).translate(origin.to_vec2()));
        out.caret_char = Some(range.primary.index);
        // What ⌘F seeds the find input with (`seedSearchStringFromSelection`):
        // the selection, or the word at the caret.
        let ch: Vec<char> = doc.text.chars().collect();
        let (s, e) = (range.primary.index.min(range.secondary.index), range.primary.index.max(range.secondary.index));
        let seed: String = if s != e {
            ch[s.min(ch.len())..e.min(ch.len())].iter().take(1000).collect()
        } else {
            let word = |c: &char| c.is_alphanumeric() || *c == '_';
            let mut a = s.min(ch.len());
            let mut b = a;
            while a > 0 && word(&ch[a - 1]) {
                a -= 1;
            }
            while b < ch.len() && word(&ch[b]) {
                b += 1;
            }
            ch[a..b].iter().collect()
        };
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(FIND_SEED), seed));
        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("marks-sel", &doc.path)), (range.secondary.index, range.primary.index)));
    }
    let response = output.response.response;
    if let Some(pointer) = response.hover_pos() {
        let c = galley.cursor_from_pos(pointer - origin);
        out.hovered = Some((c.index, pointer));
        if response.clicked() && ui.input(|i| i.modifiers.command) {
            out.command_clicked = Some(c.index);
        }
    }
    out.changed = (response.changed() || smart_changed) && !doc.read_only;

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
