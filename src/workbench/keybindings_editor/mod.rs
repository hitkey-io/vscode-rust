//! The Keyboard Shortcuts editor (contrib/preferences/browser/
//! keybindingsEditor.ts, keybindingWidgets.ts, media/keybindingsEditor.css,
//! media/keybindings.css), shown in the modal editor part like Settings.
//!
//! Measured against 1.145.2 at 1300×820 (modal body from x 131, y 130):
//! search box 30px tall at +27 / +11, 12px from the right, placeholder
//! "Type to search in keybindings", toggles Record Keys, Sort by
//! Precedence and Clear (26px apart, the last centred 23.5px from the
//! right); the table 11px below: a 30px header in
//! `keybindingTable.headerBackground` (bold labels), then 24px rows, odd
//! ones in `keybindingTable.rowsBackground`. Columns: 40px actions (pencil
//! / plus on hover and selection), then Command, Keybinding, When, Source
//! weighted 0.3 / 0.2 / 0.35 / 0.15 of the width with SplitView's 120px
//! minimum taking the 40px back from the last columns first; cells 10px
//! padded. Column lines and the header's bottom border
//! (`tree.tableColumnsBorder`) show while the table is hovered. Keys are
//! keybindingLabel boxes (11px, 17px tall, 4px apart, 10px between
//! chords); When is monospace at 90%. Define Keybinding is a 400px widget
//! centred over the editor.

pub mod model;

use std::ops::Range;

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::icons;
use crate::theme::Palette;
use model::{Entry, Item, KeyMatch, Keybinding, Model, Source, UserBinding};

const PAD_LEFT: f32 = 27.0;
const PAD_RIGHT: f32 = 12.0;
const SEARCH_TOP: f32 = 11.0;
const SEARCH_H: f32 = 30.0;
const TABLE_TOP: f32 = 52.0;
const HEADER_H: f32 = 30.0;
const ACTIONS_W: f32 = 40.0;
const MIN_COLUMN: f32 = 120.0;
const DEFINE_W: f32 = 400.0;
const DEFINE_H: f32 = 110.0;

/// An edit to keybindings.json the editor asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    /// Give `item` the keybinding `key` (`add`: keep its current one too).
    Define { command: String, old_key: Option<String>, when: String, user: bool, key: String, add: bool },
    /// Change the `when` of a keybinding.
    When { command: String, key: String, old_when: String, user: bool, when: String },
    Remove { command: String, key: String, when: String, user: bool },
    Reset { command: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItem {
    Copy,
    CopyCommandId,
    CopyCommandTitle,
    Define,
    Add,
    DefineWhen,
    Remove,
    Reset,
    ShowSame,
}

/// What a row's context menu offers (`onContextMenu`).
#[derive(Clone, Debug)]
pub struct RowMenu {
    pub item: usize,
    pub pos: Pos2,
    pub has_key: bool,
    pub user: bool,
}

struct Define {
    item: usize,
    add: bool,
    chords: Vec<String>,
    focus: bool,
}

struct WhenEdit {
    item: usize,
    text: String,
    focus: bool,
}

pub struct KeybindingsEditorState {
    pub query: String,
    pub focus_search: bool,
    pub record_keys: bool,
    pub by_precedence: bool,
    /// Selected item (by `Item::id`).
    selected: Option<String>,
    pub table_focused: bool,
    /// The search box had the keyboard last frame.
    focus_search_owned: bool,
    scroll: f32,
    dragging_scrollbar: Option<f32>,
    define: Option<Define>,
    when_edit: Option<WhenEdit>,
    model: Option<Model>,
    model_user: Vec<UserBinding>,
    entries: Vec<Entry>,
    entries_key: Option<(String, bool)>,
    reveal: Option<String>,
}

impl Default for KeybindingsEditorState {
    fn default() -> Self {
        KeybindingsEditorState {
            query: String::new(),
            focus_search: true,
            record_keys: false,
            by_precedence: false,
            selected: None,
            table_focused: false,
            focus_search_owned: false,
            scroll: 0.0,
            dragging_scrollbar: None,
            define: None,
            when_edit: None,
            model: None,
            model_user: Vec::new(),
            entries: Vec::new(),
            entries_key: None,
            reveal: None,
        }
    }
}

impl KeybindingsEditorState {
    pub fn with_query(q: &str) -> Self {
        KeybindingsEditorState { query: q.to_string(), ..Default::default() }
    }

    /// Keys typed now belong to the editor (recording or defining), not to
    /// the app's shortcuts.
    pub fn captures_keys(&self) -> bool {
        self.define.is_some() || (self.record_keys && self.focus_search_owned)
    }

    /// Starts Define Keybinding for an item (`defineKeybinding`).
    pub fn define(&mut self, item: usize, add: bool) {
        self.define = Some(Define { item, add, chords: Vec::new(), focus: true });
        if let Some(m) = &self.model {
            self.selected = m.items.get(item).map(Item::id);
        }
    }

    pub fn define_when(&mut self, item: usize) {
        if let Some(m) = &self.model {
            if let Some(it) = m.items.get(item).filter(|i| i.key.is_some()) {
                self.selected = Some(it.id());
                self.when_edit = Some(WhenEdit { item, text: it.when.clone(), focus: true });
            }
        }
    }

    pub fn item(&self, index: usize) -> Option<&Item> {
        self.model.as_ref()?.items.get(index)
    }

    /// Shows the keybindings with the same key (`showSimilarKeybindings`).
    pub fn show_same(&mut self, item: usize) {
        if let Some(k) = self.item(item).and_then(|i| i.key.as_ref()) {
            self.query = format!("\"{}\"", k.raw);
            self.focus_search = true;
            self.scroll = 0.0;
        }
    }
}

#[derive(Default)]
pub struct KeybindingsEditorOutput {
    pub escape: bool,
    pub edit: Option<Edit>,
    pub menu: Option<RowMenu>,
    pub copy: Option<String>,
    pub open_extension: Option<String>,
}

/// keybindings.json's copy of an item (`copyKeybinding`).
pub fn copy_text(item: &Item) -> String {
    let mut s = format!("{{\n  \"key\": {},\n  \"command\": {}", json(item.key.as_ref().map(|k| k.raw.as_str()).unwrap_or("")), json(&item.command));
    if !item.when.is_empty() {
        s.push_str(&format!(",\n  \"when\": {}", json(&item.when)));
    }
    s.push_str("\n}");
    s
}

fn json(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// Column widths: actions, command, keybinding, when, source.
fn columns(width: f32) -> [f32; 5] {
    let mut c = [ACTIONS_W, (0.3 * width).round(), (0.2 * width).round(), (0.35 * width).round(), (0.15 * width).round()];
    let mut over = c.iter().sum::<f32>() - width;
    for i in (1..5).rev() {
        if over <= 0.0 {
            break;
        }
        let take = (c[i] - MIN_COLUMN).max(0.0).min(over);
        c[i] -= take;
        over -= take;
    }
    if over > 0.0 {
        c[1] -= over;
    }
    c
}

/// A single-line job with `ranges` (char indices) highlighted.
fn highlighted(text: &str, ranges: &[Range<usize>], font: FontId, bold: FontId, colour: Color32, width: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let normal = egui::TextFormat { font_id: font.clone(), color: colour, ..Default::default() };
    let hl = egui::TextFormat { font_id: bold, color: Palette::VSCE_LIST_HIGHLIGHT_FG(), ..Default::default() };
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut sorted: Vec<Range<usize>> = ranges.iter().filter(|r| r.start < chars.len()).cloned().collect();
    sorted.sort_by_key(|r| r.start);
    for r in sorted {
        if r.start > i {
            job.append(&chars[i..r.start].iter().collect::<String>(), 0.0, normal.clone());
        }
        let s = r.start.max(i);
        let e = r.end.min(chars.len());
        if e > s {
            job.append(&chars[s..e].iter().collect::<String>(), 0.0, hl.clone());
        }
        i = i.max(e);
    }
    if i < chars.len() {
        job.append(&chars[i..].iter().collect::<String>(), 0.0, normal);
    }
    job.wrap = egui::text::TextWrapping { max_width: width.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    job
}

/// Paints keybinding-label boxes from `x` centred on `cy`; returns the width.
fn paint_keys(painter: &egui::Painter, x: f32, cy: f32, kb: &Keybinding, matches: &[KeyMatch], max_right: f32) -> f32 {
    let font = FontId::proportional(11.0);
    let h = 17.0;
    let top = (cy - h / 2.0 + 0.5).round() - 0.5;
    let mut at = x;
    for (ci, chord) in kb.chords.iter().enumerate() {
        if ci > 0 {
            at += 6.0 + 2.0;
        }
        let m = matches.get(ci).copied().unwrap_or_default();
        for (ki, (part, label)) in chord.parts().into_iter().enumerate() {
            if ki > 0 || ci > 0 {
                at += if ki > 0 { 4.0 } else { 2.0 };
            }
            let hl = m.has(part);
            let f = if hl { crate::ui_font::bold(11.0) } else { font.clone() };
            let g = painter.layout_no_wrap(label, f, Color32::PLACEHOLDER);
            let w = g.size().x.max(12.0) + 12.0;
            if at + w > max_right {
                return at - x;
            }
            let r = Rect::from_min_size(Pos2::new(at, top), Vec2::new(w, h));
            painter.rect(r, 3.0, Palette::VSCE_KEYBINDING_LABEL_BG(), Stroke::new(1.0, Palette::VSCE_KEYBINDING_LABEL_BORDER()), egui::StrokeKind::Inside);
            painter.hline((r.left() + 2.0)..=(r.right() - 2.0), r.bottom() - 0.5, Stroke::new(1.0, Palette::VSCE_KEYBINDING_LABEL_BOTTOM_BORDER()));
            let fg = if hl { Palette::VSCE_LIST_HIGHLIGHT_FG() } else { Palette::VSCE_KEYBINDING_LABEL_FG() };
            painter.galley(Pos2::new(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, fg);
            at += w;
        }
    }
    at - x
}

/// Draws the editor in `rect` (the modal's body). `user` is the parsed
/// keybindings.json.
pub fn show(ui: &mut Ui, rect: Rect, state: &mut KeybindingsEditorState, user: &[UserBinding]) -> KeybindingsEditorOutput {
    let mut out = KeybindingsEditorOutput::default();
    let id = ui.id().with("keybindings-editor");
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 0.0, Palette::EDITOR_BG());

    if state.model.is_none() || state.model_user != user {
        state.model = Some(Model::new(user));
        state.model_user = user.to_vec();
        state.entries_key = None;
    }
    let key = (state.query.clone(), state.by_precedence);
    if state.entries_key.as_ref() != Some(&key) {
        state.entries = state.model.as_ref().map(|m| m.fetch(&state.query, state.by_precedence)).unwrap_or_default();
        state.entries_key = Some(key);
    }

    search_box(ui, rect, state, &mut out, id);
    table(ui, rect, state, &mut out, id);
    if state.define.is_some() {
        define_widget(ui, rect, state, &mut out, id);
    }

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) && state.define.is_none() && state.when_edit.is_none() && !state.record_keys {
        if !state.query.is_empty() {
            state.query.clear();
        } else {
            out.escape = true;
        }
    }
    out
}

fn toggle(ui: &mut Ui, centre: Pos2, icon: &str, on: bool, tip: &str, id: egui::Id) -> bool {
    let r = Rect::from_center_size(centre, Vec2::splat(20.0));
    let resp = ui.interact(r, id, Sense::CLICK);
    let p = ui.painter();
    if on {
        p.rect(r, 3.0, Palette::INPUT_OPTION_ACTIVE_BG(), Stroke::new(1.0, Palette::INPUT_OPTION_ACTIVE_BORDER()), egui::StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(r, 3.0, Palette::VSCE_INPUT_OPTION_HOVER_BG());
    }
    let fg = if on { Palette::INPUT_OPTION_ACTIVE_FG() } else { Palette::INPUT_FG() };
    p.text(r.center(), Align2::CENTER_CENTER, icons::codicon_by_name(icon).unwrap_or('?').to_string(), icons::codicon_font(16.0), fg);
    let t = tip.to_string();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, t.clone()));
    resp.on_hover_text(tip).clicked()
}

fn search_box(ui: &mut Ui, rect: Rect, state: &mut KeybindingsEditorState, out: &mut KeybindingsEditorOutput, id: egui::Id) {
    let painter = ui.painter().clone();
    let search = Rect::from_min_max(
        Pos2::new(rect.left() + PAD_LEFT, rect.top() + SEARCH_TOP),
        Pos2::new(rect.right() - PAD_RIGHT, rect.top() + SEARCH_TOP + SEARCH_H),
    );
    let slot = painter.add(egui::Shape::Noop);
    let cy = search.center().y;
    let clear_c = Pos2::new(search.right() - 23.5, cy);
    let sort_c = Pos2::new(clear_c.x - 25.5, cy);
    let record_c = Pos2::new(sort_c.x - 26.0, cy);
    let mut widgets_left = record_c.x - 12.0;
    if state.record_keys {
        let g = painter.layout_no_wrap("Recording Keys".to_string(), FontId::proportional(11.0), Palette::BADGE_FG());
        let badge = Rect::from_min_size(Pos2::new(widgets_left - 8.0 - g.size().x - 8.0, cy - 9.0), Vec2::new(g.size().x + 8.0, 18.0));
        painter.rect_filled(badge, 9.0, Palette::BADGE_BG());
        painter.galley(Pos2::new(badge.left() + 4.0, cy - g.size().y / 2.0), g, Palette::BADGE_FG());
        widgets_left = badge.left() - 4.0;
    }

    if toggle(ui, record_c, "record-keys", state.record_keys, "Record Keys (⌥⌘K)", id.with("record")) {
        state.record_keys = !state.record_keys;
        state.focus_search = true;
    }
    if toggle(ui, sort_c, "sort-precedence", state.by_precedence, "Sort by Precedence (Highest first) (⌥⌘P)", id.with("sort")) {
        state.by_precedence = !state.by_precedence;
        state.scroll = 0.0;
    }
    {
        let r = Rect::from_center_size(clear_c, Vec2::splat(20.0));
        let enabled = !state.query.is_empty();
        let resp = ui.interact(r, id.with("clear"), if enabled { Sense::CLICK } else { Sense::hover() });
        if enabled && resp.hovered() {
            ui.painter().rect_filled(r, 3.0, Palette::VSCE_TOOLBAR_HOVER_BG());
        }
        let fg = if enabled { Palette::INPUT_FG() } else { Palette::INPUT_FG().gamma_multiply(0.4) };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, icons::codicon_by_name("clear-all").unwrap_or('x').to_string(), icons::codicon_font(16.0), fg);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Clear Keybindings Search Input"));
        if resp.clicked() {
            state.query.clear();
            state.scroll = 0.0;
            state.focus_search = true;
        }
    }

    // The text sits 1.5px above the box's centre.
    let text_rect = Rect::from_min_max(Pos2::new(search.left() + 11.0, search.top() - 0.5), Pos2::new(widgets_left - 4.0, search.bottom() - 2.5));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(text_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.visuals_mut().weak_text_color = Some(Palette::INPUT_PLACEHOLDER_FG());
    child.visuals_mut().text_cursor.stroke.color = Palette::INPUT_FG();
    let hint = if state.record_keys { "Recording Keys. Press Escape to exit" } else { "Type to search in keybindings" };
    let edit_id = id.with("search");
    let focused = ui.memory(|m| m.has_focus(edit_id));
    if state.record_keys && focused {
        record_into_search(ui, state);
    }
    let before = state.query.clone();
    let resp = child.add_sized(
        text_rect.size(),
        egui::TextEdit::singleline(&mut state.query)
            .id(edit_id)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .hint_text(egui::RichText::new(hint).font(FontId::proportional(13.5)))
            .font(FontId::proportional(13.5))
            .text_color(Palette::INPUT_FG())
            .vertical_align(egui::Align::Center),
    );
    if state.record_keys && state.query != before {
        // Recording: only recorded keys go in.
        state.query = before;
    }
    state.focus_search_owned = resp.has_focus();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Type to search in keybindings"));
    if std::mem::take(&mut state.focus_search) {
        resp.request_focus();
    }
    if resp.changed() {
        state.scroll = 0.0;
    }
    if resp.has_focus() {
        state.table_focused = false;
        // ⌘↓ focuses the keybindings.
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::ArrowDown)) && !state.entries.is_empty() {
            state.table_focused = true;
            if state.selected.is_none() {
                state.selected = state.model.as_ref().map(|m| m.items[state.entries[0].index].id());
            }
            ui.memory_mut(|m| m.surrender_focus(edit_id));
        }
    }
    let border = if resp.has_focus() { Palette::FOCUS_BORDER() } else { Palette::SETTINGS_TEXT_INPUT_BORDER() };
    painter.set(slot, egui::epaint::RectShape::new(search, 4.0, Palette::INPUT_BG(), Stroke::new(1.0, border), egui::StrokeKind::Inside));
    let _ = out;
}

/// Record Keys: each key pressed in the search box becomes `"chord"`.
fn record_into_search(ui: &mut Ui, state: &mut KeybindingsEditorState) {
    let pressed: Vec<(egui::Key, egui::Modifiers)> = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                _ => None,
            })
            .collect()
    });
    for (key, m) in pressed {
        if key == egui::Key::Escape && m.is_none() {
            state.record_keys = false;
            continue;
        }
        if let Some(chord) = model::chord_from_key(key, m) {
            state.query = format!("\"{chord}\"");
            state.scroll = 0.0;
        }
    }
    ui.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Text(_) | egui::Event::Key { .. } | egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_))));
}

fn table(ui: &mut Ui, rect: Rect, state: &mut KeybindingsEditorState, out: &mut KeybindingsEditorOutput, id: egui::Id) {
    let painter = ui.painter().clone();
    let area = Rect::from_min_max(Pos2::new(rect.left() + PAD_LEFT, rect.top() + TABLE_TOP), rect.max);
    let cols = columns(area.width());
    let mut xs = [area.left(); 6];
    for i in 0..5 {
        xs[i + 1] = xs[i] + cols[i];
    }
    let header = Rect::from_min_size(area.min, Vec2::new(area.width(), HEADER_H));
    let list = Rect::from_min_max(Pos2::new(area.left(), header.bottom() + 1.0), area.max);
    let hovered = ui.rect_contains_pointer(area);

    painter.rect_filled(header, 0.0, Palette::KEYBINDING_TABLE_HEADER_BG());
    for (i, label) in ["", "Command", "Keybinding", "When", "Source"].iter().enumerate() {
        if !label.is_empty() {
            painter.text(Pos2::new(xs[i] + 10.0, header.center().y - 1.0), Align2::LEFT_CENTER, *label, crate::ui_font::bold(13.0), Palette::FG());
        }
    }

    let model = state.model.take().unwrap_or_else(|| Model::new(&[]));
    let entries = std::mem::take(&mut state.entries);
    let heights: Vec<f32> = entries.iter().map(|e| e.height(&model.items[e.index])).collect();
    let total: f32 = heights.iter().sum();
    let viewport = list.height();
    let max_scroll = (total - viewport).max(0.0);

    // Keyboard: ↑/↓, Page Up/Down, Home/End, Enter, ⌘C, ⌘⌫ while the table has focus.
    let sel_pos = state.selected.as_ref().and_then(|s| entries.iter().position(|e| &model.items[e.index].id() == s));
    if state.table_focused && state.define.is_none() && state.when_edit.is_none() && !entries.is_empty() {
        let (up, down, pgup, pgdn, home, end, enter, back) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp) && i.modifiers.is_none(),
                i.key_pressed(egui::Key::ArrowDown) && i.modifiers.is_none(),
                i.key_pressed(egui::Key::PageUp),
                i.key_pressed(egui::Key::PageDown),
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
                i.key_pressed(egui::Key::Enter) && i.modifiers.is_none(),
                i.key_pressed(egui::Key::Backspace) && i.modifiers.command,
            )
        });
        let page = (viewport / 24.0).floor().max(1.0) as usize;
        let cur = sel_pos.unwrap_or(0);
        let next = if up {
            Some(cur.saturating_sub(1))
        } else if down {
            Some(if sel_pos.is_none() { 0 } else { (cur + 1).min(entries.len() - 1) })
        } else if pgup {
            Some(cur.saturating_sub(page))
        } else if pgdn {
            Some((cur + page).min(entries.len() - 1))
        } else if home {
            Some(0)
        } else if end {
            Some(entries.len() - 1)
        } else {
            None
        };
        if let Some(n) = next {
            state.selected = Some(model.items[entries[n].index].id());
            state.reveal = state.selected.clone();
        }
        if let Some(p) = sel_pos {
            let item = &model.items[entries[p].index];
            if enter {
                state.define = Some(Define { item: entries[p].index, add: false, chords: Vec::new(), focus: true });
            }
            if back {
                if let Some(k) = &item.key {
                    out.edit = Some(Edit::Remove { command: item.command.clone(), key: k.raw.clone(), when: item.when.clone(), user: item.source == Source::User });
                }
            }
            if ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy))) {
                out.copy = Some(copy_text(item));
            }
        }
    }
    // Reveal the selection (keyboard moves).
    if let Some(sel) = state.reveal.take() {
        if let Some(p) = entries.iter().position(|e| model.items[e.index].id() == sel) {
            let top: f32 = heights[..p].iter().sum();
            if top < state.scroll {
                state.scroll = top;
            } else if top + heights[p] > state.scroll + viewport {
                state.scroll = top + heights[p] - viewport;
            }
        }
    }

    if ui.rect_contains_pointer(list) {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        state.scroll -= dy;
    }
    state.scroll = state.scroll.clamp(0.0, max_scroll);

    let clip = list;
    let p = painter.with_clip_rect(clip);
    let mut y = list.top() - state.scroll;
    let mut clicked_row = None;
    for (n, e) in entries.iter().enumerate() {
        let h = heights[n];
        if y + h < list.top() {
            y += h;
            continue;
        }
        if y > list.bottom() {
            break;
        }
        let row = Rect::from_min_size(Pos2::new(area.left(), y), Vec2::new(area.width(), h));
        let item = &model.items[e.index];
        let item_id = item.id();
        let resp = ui.interact(row.intersect(clip), id.with(("row", &item_id)), Sense::click());
        let selected = state.selected.as_deref() == Some(item_id.as_str());
        let bg = if selected {
            if state.table_focused { Palette::LIST_ACTIVE_SELECTION_BG() } else { Palette::LIST_INACTIVE_SELECTION_BG() }
        } else if resp.hovered() {
            Palette::LIST_HOVER_BG()
        } else if n % 2 == 1 {
            Palette::KEYBINDING_TABLE_ROWS_BG()
        } else {
            Color32::TRANSPARENT
        };
        if bg != Color32::TRANSPARENT {
            p.rect_filled(row, 0.0, bg);
        }
        if selected && state.table_focused {
            p.rect_stroke(row, 0.0, Stroke::new(1.0, Palette::FOCUS_BORDER()), egui::StrokeKind::Inside);
        }
        let fg = if selected && state.table_focused { Palette::LIST_ACTIVE_SELECTION_FG() } else { Palette::FG() };
        let cy = row.top() + 12.0;
        // Text sits a little above the row's centre (24px rows, 13px text).
        let ty = cy - 0.75;

        // Actions: edit / add.
        if resp.hovered() || selected || ui.rect_contains_pointer(Rect::from_min_size(row.min, Vec2::new(ACTIONS_W, h)).intersect(clip)) {
            let icon_r = Rect::from_center_size(Pos2::new(xs[0] + 28.0, cy), Vec2::splat(20.0));
            let ir = ui.interact(icon_r.intersect(clip), id.with(("act", &item_id)), Sense::CLICK);
            if ir.hovered() {
                p.rect_filled(icon_r, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
            }
            let icon = if item.key.is_some() { "edit" } else { "add" };
            p.text(icon_r.center(), Align2::CENTER_CENTER, icons::codicon_by_name(icon).unwrap_or('+').to_string(), icons::codicon_font(16.0), fg);
            let tip = if item.key.is_some() { "Change Keybinding (Enter)" } else { "Add Keybinding (Enter)" };
            ir.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
            if ir.on_hover_text(tip).clicked() {
                state.define = Some(Define { item: e.index, add: false, chords: Vec::new(), focus: true });
                state.selected = Some(item_id.clone());
            }
        }

        // Command: the label, or the id in monospace; the id under the label
        // when the search matched it.
        let cw = cols[1] - 20.0;
        let cx = xs[1] + 10.0;
        if !item.label.is_empty() {
            let job = highlighted(&item.label, &e.label, FontId::proportional(13.0), crate::ui_font::bold(13.0), fg, cw);
            let g = ui.fonts_mut(|f| f.layout_job(job));
            let gy = if h > 24.0 { row.top() + 4.0 } else { ty - g.size().y / 2.0 };
            p.galley(Pos2::new(cx, gy), g, fg);
            if let Some(m) = &e.command_id {
                let job = highlighted(&item.command, m, crate::ui_font::mono(11.7), crate::ui_font::mono(11.7), fg, cw);
                let g = ui.fonts_mut(|f| f.layout_job(job));
                p.galley(Pos2::new(cx, row.top() + 22.0), g, fg);
            }
        } else {
            let job = highlighted(&item.command, e.command_id.as_deref().unwrap_or(&[]), crate::ui_font::mono(11.7), crate::ui_font::mono(11.7), fg, cw);
            let g = ui.fonts_mut(|f| f.layout_job(job));
            p.galley(Pos2::new(cx, cy - g.size().y / 2.0), g, fg);
        }

        // Keybinding.
        if let Some(kb) = &item.key {
            paint_keys(&p, xs[2] + 10.0, cy, kb, &e.keys, xs[3] - 4.0);
        }

        // When (editable inline).
        let wx = xs[3] + 10.0;
        let ww = cols[3] - 20.0;
        if state.when_edit.as_ref().is_some_and(|w| w.item == e.index) {
            let w = state.when_edit.as_mut().unwrap();
            let er = Rect::from_min_max(Pos2::new(xs[3] + 4.0, row.top() + 2.0), Pos2::new(xs[4] - 4.0, row.top() + 22.0));
            p.rect(er, 2.0, Palette::INPUT_BG(), Stroke::new(1.0, Palette::FOCUS_BORDER()), egui::StrokeKind::Inside);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(er.shrink2(Vec2::new(6.0, 1.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
            child.visuals_mut().text_cursor.stroke.color = Palette::INPUT_FG();
            let r = child.add_sized(
                er.shrink2(Vec2::new(6.0, 1.0)).size(),
                egui::TextEdit::singleline(&mut w.text).id(id.with("when-edit")).frame(egui::Frame::NONE).margin(egui::Margin::ZERO).font(crate::ui_font::mono(11.7)).text_color(Palette::INPUT_FG()),
            );
            if std::mem::take(&mut w.focus) {
                r.request_focus();
            }
            let (enter, esc) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
            if enter {
                if let Some(k) = &item.key {
                    out.edit = Some(Edit::When { command: item.command.clone(), key: k.raw.clone(), old_when: item.when.clone(), user: item.source == Source::User, when: w.text.trim().to_string() });
                }
                state.when_edit = None;
                state.table_focused = true;
            } else if esc || r.lost_focus() {
                state.when_edit = None;
                state.table_focused = true;
            }
        } else if item.when.is_empty() {
            p.text(Pos2::new(wx + 4.0, cy), Align2::LEFT_CENTER, "-", FontId::proportional(13.0), fg);
        } else {
            let job = highlighted(&item.when, &e.when, crate::ui_font::mono(11.7), crate::ui_font::mono(11.7), fg, ww);
            let g = ui.fonts_mut(|f| f.layout_job(job));
            p.galley(Pos2::new(wx, cy - 0.5 - g.size().y / 2.0), g, fg);
        }

        // Source: System / User, or the extension (a link).
        let sx = xs[4] + 10.0;
        match &item.source {
            Source::Extension(name) => {
                let colour = if selected { fg } else { Palette::TEXT_LINK_FG() };
                let g = p.layout_no_wrap(name.clone(), FontId::proportional(13.0), colour);
                let lr = Rect::from_min_size(Pos2::new(sx, cy - g.size().y / 2.0), g.size());
                let lresp = ui.interact(lr.intersect(clip), id.with(("ext", &item_id)), Sense::CLICK);
                if lresp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    p.hline(lr.x_range(), lr.bottom() - 1.0, Stroke::new(1.0, colour));
                }
                if lresp.clicked() {
                    out.open_extension = Some(name.clone());
                }
                p.galley(lr.min, g, colour);
            }
            src => {
                let job = highlighted(src.label(), &e.source, FontId::proportional(13.0), crate::ui_font::bold(13.0), fg, cols[4] - 20.0);
                let g = ui.fonts_mut(|f| f.layout_job(job));
                p.galley(Pos2::new(sx, ty - g.size().y / 2.0), g, fg);
            }
        }

        let label = if item.label.is_empty() { item.command.clone() } else { item.label.clone() };
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, label.clone()));
        if resp.clicked() || resp.secondary_clicked() || resp.double_clicked() {
            clicked_row = Some((e.index, item_id.clone()));
        }
        if resp.double_clicked() {
            state.define = Some(Define { item: e.index, add: false, chords: Vec::new(), focus: true });
        }
        if resp.secondary_clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                out.menu = Some(RowMenu { item: e.index, pos, has_key: item.key.is_some(), user: item.source == Source::User });
            }
        }
        y += h;
    }
    if let Some((_, item_id)) = clicked_row {
        state.selected = Some(item_id);
        state.table_focused = true;
        ui.memory_mut(|m| m.surrender_focus(id.with("search")));
    } else if ui.input(|i| i.pointer.any_pressed()) && !ui.rect_contains_pointer(list) && state.define.is_none() {
        state.table_focused = false;
    }

    if hovered {
        let line = Stroke::new(1.0, Palette::TABLE_COLUMNS_BORDER());
        painter.hline(header.x_range(), header.bottom() + 0.5, line);
        for x in &xs[1..5] {
            painter.vline(x.round() + 0.5, (header.bottom() + 1.0)..=area.bottom(), line);
        }
    }

    // Overlay scrollbar (10px).
    if max_scroll > 0.0 {
        let track = Rect::from_min_max(Pos2::new(list.right() - 10.0, list.top()), list.right_bottom());
        let thumb_h = (viewport / total * viewport).max(20.0);
        let thumb_top = track.top() + (state.scroll / max_scroll) * (viewport - thumb_h);
        let thumb = Rect::from_min_size(Pos2::new(track.left(), thumb_top), Vec2::new(10.0, thumb_h));
        let resp = ui.interact(track, id.with("scrollbar"), Sense::click_and_drag());
        if resp.drag_started() {
            if let Some(p) = resp.interact_pointer_pos() {
                state.dragging_scrollbar = Some(if thumb.contains(p) { p.y - thumb.top() } else { thumb_h / 2.0 });
            }
        }
        if let (Some(grab), Some(p)) = (state.dragging_scrollbar, resp.interact_pointer_pos()) {
            if resp.dragged() {
                let frac = ((p.y - grab - track.top()) / (viewport - thumb_h).max(1.0)).clamp(0.0, 1.0);
                state.scroll = frac * max_scroll;
            }
        }
        if resp.drag_stopped() {
            state.dragging_scrollbar = None;
        }
        if hovered || resp.dragged() {
            let colour = if resp.dragged() {
                Palette::VSCE_SCROLLBAR_SLIDER_ACTIVE()
            } else if resp.hovered() {
                Palette::VSCE_SCROLLBAR_SLIDER_HOVER()
            } else {
                Palette::VSCE_SCROLLBAR_SLIDER_BG()
            };
            painter.rect_filled(thumb, 0.0, colour);
        }
    }

    state.entries = entries;
    state.model = Some(model);
}

/// `DefineKeybindingWidget`: "Press desired key combination and then press
/// ENTER." over the editor; Escape clears, then closes.
fn define_widget(ui: &mut Ui, rect: Rect, state: &mut KeybindingsEditorState, out: &mut KeybindingsEditorOutput, id: egui::Id) {
    let Some(def) = state.define.as_mut() else { return };
    // Every key goes to the widget.
    let pressed: Vec<(egui::Key, egui::Modifiers)> = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                _ => None,
            })
            .collect()
    });
    let mut accept = false;
    let mut close = false;
    if !std::mem::take(&mut def.focus) {
        for (key, m) in pressed {
            match key {
                egui::Key::Enter if m.is_none() => accept = true,
                egui::Key::Escape if m.is_none() => {
                    if def.chords.is_empty() {
                        close = true;
                    } else {
                        def.chords.clear();
                    }
                }
                _ => {
                    if let Some(c) = model::chord_from_key(key, m) {
                        if def.chords.len() == 2 {
                            def.chords.clear();
                        }
                        def.chords.push(c);
                    }
                }
            }
        }
    }
    ui.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Text(_) | egui::Event::Key { .. } | egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_))));

    let w = DEFINE_W + 22.0;
    let h = DEFINE_H + 22.0;
    let r = Rect::from_min_size(Pos2::new((rect.left() + (rect.width() - w) / 2.0).round(), (rect.top() + (rect.height() - h) / 2.0).round()), Vec2::new(w, h));
    let painter = ui.painter().clone();
    painter.add(egui::epaint::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(90) }.as_shape(r, 8));
    painter.rect(r, 8.0, Palette::EDITOR_WIDGET_BG(), Stroke::new(1.0, Palette::EDITOR_WIDGET_BORDER()), egui::StrokeKind::Inside);
    ui.interact(r, id.with("define-body"), Sense::click());
    let inner = r.shrink(11.0);
    painter.text(Pos2::new(inner.center().x, inner.top() + 9.0), Align2::CENTER_CENTER, "Press desired key combination and then press ENTER.", FontId::proportional(13.0), Palette::EDITOR_WIDGET_FG());
    let input = Rect::from_min_size(Pos2::new(inner.left(), inner.top() + 28.0), Vec2::new(DEFINE_W, 26.0));
    painter.rect(input, 2.0, Palette::INPUT_BG(), Stroke::new(1.0, Palette::FOCUS_BORDER()), egui::StrokeKind::Inside);
    let value = def.chords.join(" ");
    painter.text(input.center(), Align2::CENTER_CENTER, &value, FontId::proportional(13.0), Palette::INPUT_FG());
    let mut existing = 0;
    if let Some(kb) = Keybinding::parse(&value) {
        // Keys with "chord to" between chords.
        let mut parts: Vec<(Option<Keybinding>, f32)> = Vec::new();
        for (i, c) in kb.chords.iter().enumerate() {
            let single = Keybinding { raw: String::new(), chords: vec![c.clone()] };
            let width = single.chords[0].parts().iter().map(|(_, s)| painter.layout_no_wrap(s.clone(), FontId::proportional(11.0), Color32::PLACEHOLDER).size().x.max(12.0) + 12.0).sum::<f32>() + 4.0 * (single.chords[0].parts().len() as f32 - 1.0);
            if i > 0 {
                parts.push((None, painter.layout_no_wrap("chord to".into(), FontId::proportional(13.0), Color32::PLACEHOLDER).size().x));
            }
            parts.push((Some(single), width));
        }
        let total: f32 = parts.iter().map(|(_, w)| w + 8.0).sum();
        let mut x = inner.center().x - total / 2.0;
        let cy = input.bottom() + 10.0 + 9.0;
        for (kb, w) in parts {
            x += 4.0;
            match kb {
                Some(k) => {
                    paint_keys(&painter, x, cy, &k, &[], f32::MAX);
                }
                None => {
                    painter.text(Pos2::new(x, cy), Align2::LEFT_CENTER, "chord to", FontId::proportional(13.0), Palette::EDITOR_WIDGET_FG());
                }
            }
            x += w + 4.0;
        }
        if let Some(m) = &state.model {
            existing = m.items.iter().filter(|i| i.key.as_ref().is_some_and(|k| k.raw == kb.raw)).count();
        }
    }
    if existing > 0 {
        let text = if existing == 1 { "1 existing command has this keybinding".to_string() } else { format!("{existing} existing commands have this keybinding") };
        let g = painter.layout_no_wrap(text.clone(), FontId::proportional(13.0), Palette::EDITOR_WIDGET_FG());
        let tr = Rect::from_min_size(Pos2::new(inner.center().x - g.size().x / 2.0, input.bottom() + 38.0), g.size());
        let resp = ui.interact(tr, id.with("existing"), Sense::CLICK);
        painter.hline(tr.x_range(), tr.bottom() - 1.0, Stroke::new(1.0, Palette::EDITOR_WIDGET_FG()));
        painter.galley(tr.min, g, Palette::EDITOR_WIDGET_FG());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, text.clone()));
        if resp.clicked() {
            state.query = format!("\"{value}\"");
            state.scroll = 0.0;
            close = true;
        }
    }

    if accept {
        let def = state.define.take().unwrap();
        if let (Some(kb), Some(item)) = (Keybinding::parse(&value), state.model.as_ref().and_then(|m| m.items.get(def.item))) {
            let old = item.key.as_ref().map(|k| k.raw.clone());
            if old.as_deref() != Some(kb.raw.as_str()) {
                out.edit = Some(Edit::Define { command: item.command.clone(), old_key: old, when: item.when.clone(), user: item.source == Source::User, key: kb.raw, add: def.add });
            }
        }
        state.table_focused = true;
    } else if close {
        state.define = None;
        state.table_focused = true;
    }
}

/// Applies an edit to keybindings.json's entries (`KeybindingsEditingService`):
/// a default keybinding changed or removed gets a `-command` entry; a user
/// one is edited in place.
pub fn apply_edit(entries: &mut Vec<UserBinding>, edit: &Edit) {
    let norm = |k: &str| Keybinding::parse(k).map(|k| k.raw).unwrap_or_else(|| k.to_string());
    let find_user = |entries: &Vec<UserBinding>, command: &str, key: &str, when: &str| {
        entries.iter().position(|u| u.command == command && norm(&u.key) == norm(key) && u.when.as_deref().unwrap_or("") == when)
    };
    let when_opt = |w: &str| if w.is_empty() { None } else { Some(w.to_string()) };
    match edit {
        Edit::Define { command, old_key, when, user, key, add } => {
            if *user && !add {
                if let Some(i) = old_key.as_ref().and_then(|k| find_user(entries, command, k, when)) {
                    entries[i].key = key.clone();
                    return;
                }
            }
            entries.push(UserBinding { key: key.clone(), command: command.clone(), when: when_opt(when) });
            if let (Some(old), false, false) = (old_key, *user, *add) {
                entries.push(UserBinding { key: old.clone(), command: format!("-{command}"), when: when_opt(when) });
            }
        }
        Edit::When { command, key, old_when, user, when } => {
            if *user {
                if let Some(i) = find_user(entries, command, key, old_when) {
                    entries[i].when = when_opt(when);
                    return;
                }
            }
            entries.push(UserBinding { key: key.clone(), command: command.clone(), when: when_opt(when) });
            entries.push(UserBinding { key: key.clone(), command: format!("-{command}"), when: when_opt(old_when) });
        }
        Edit::Remove { command, key, when, user } => {
            if *user {
                if let Some(i) = find_user(entries, command, key, when) {
                    entries.remove(i);
                    return;
                }
            }
            entries.push(UserBinding { key: key.clone(), command: format!("-{command}"), when: when_opt(when) });
        }
        Edit::Reset { command } => {
            let minus = format!("-{command}");
            entries.retain(|u| u.command != *command && u.command != minus);
        }
    }
}

/// keybindings.json text for `entries` (the header VS Code writes, four
/// spaces like its default formatting).
pub fn to_json(entries: &[UserBinding]) -> String {
    let mut s = String::from("// Place your key bindings in this file to override the defaults\n[");
    for (i, e) in entries.iter().enumerate() {
        s.push_str(if i == 0 { "\n" } else { ",\n" });
        s.push_str(&format!("    {{\n        \"key\": {},\n        \"command\": {}", json(&e.key), json(&e.command)));
        if let Some(w) = &e.when {
            s.push_str(&format!(",\n        \"when\": {}", json(w)));
        }
        s.push_str("\n    }");
    }
    s.push_str("\n]");
    s
}

/// What VS Code writes when it creates keybindings.json.
pub const EMPTY_JSON: &str = "// Place your key bindings in this file to override the defaults\n[\n]";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_take_the_actions_column_from_the_last_ones() {
        // 1300×820 window: the table is 1011px wide.
        assert_eq!(columns(1011.0), [40.0, 303.0, 202.0, 346.0, 120.0]);
    }

    #[test]
    fn editing_a_default_keybinding_adds_a_removal() {
        let mut e = Vec::new();
        apply_edit(&mut e, &Edit::Define { command: "a.b".into(), old_key: Some("cmd+s".into()), when: String::new(), user: false, key: "alt+cmd+s".into(), add: false });
        assert_eq!(to_json(&e), "// Place your key bindings in this file to override the defaults\n[\n    {\n        \"key\": \"alt+cmd+s\",\n        \"command\": \"a.b\"\n    },\n    {\n        \"key\": \"cmd+s\",\n        \"command\": \"-a.b\"\n    }\n]");
        apply_edit(&mut e, &Edit::Reset { command: "a.b".into() });
        assert!(e.is_empty());
    }
}
