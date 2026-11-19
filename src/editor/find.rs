//! In-editor find and replace widget (⌘F, ⌥⌘F).
//! VS Code analogue: src/vs/editor/contrib/find/browser/findWidget.ts + findWidget.css
//! (1.145 modern UI), measured against a live 1.145.2 over CDP:
//!
//! A card 419px wide (33px; 62px with the replace row), radius 8, 1px
//! `editorWidget.border`, shadow 12px, 4px below the editor's top and 13px
//! left of the minimap. Inside, from the card's left: the Toggle Replace
//! chevron (6px in, 18px wide), the find input (27px in, 216 × 25, radius 4,
//! Match Case / Whole Word / Regex toggles 20px each in its right 66px), the
//! match count (69px, 12px, centred), Previous / Next (22px), Find in
//! Selection (22px, 40% while disabled) and Close; the replace input below
//! with Preserve Case in it and Replace / Replace All beside it.
//!
//! The editor paints the matches through `highlight::FindHighlight` (all
//! washed, the current one brighter).

use egui::{Align2, Context, CornerRadius, FontId, Key, Modifiers, Order, Rect, Sense, Stroke, StrokeKind, TextEdit, Ui};

use crate::icons::{self, codicon_font};
use crate::theme::Palette;

use super::highlight::FindHighlight;

#[derive(Default)]
pub struct FindState {
    pub open: bool,
    pub query: String,
    /// Index of the current match within the computed match list.
    pub current: usize,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
    /// The replace row is shown (⌥⌘F, the chevron).
    pub replace_open: bool,
    pub replace: String,
    pub preserve_case: bool,
    /// Find in Selection: the byte range being searched, while it is on.
    pub scope: Option<(usize, usize)>,
    /// The editor has a selection the scope could be taken from.
    pub can_scope: bool,
    request_focus: bool,
    request_replace_focus: bool,
    /// The input should select its whole text when it takes the focus.
    select_on_focus: bool,
}

impl FindState {
    pub fn open(&mut self) {
        self.open = true;
        self.request_focus = true;
        self.select_on_focus = true;
    }

    /// ⌘F / ⌥⌘F: opens the widget, seeding the query with `seed` (the
    /// selection or the word at the caret; a multi-line selection or nothing
    /// leaves the query as it was).
    pub fn open_seeded(&mut self, seed: &str, replace: bool) {
        if !seed.is_empty() && !seed.contains('\n') {
            self.query = seed.to_string();
            self.current = 0;
        }
        if replace {
            self.open_replace();
        } else {
            self.open();
        }
    }

    /// ⌥⌘F: the widget with its replace row, the replace input focused.
    pub fn open_replace(&mut self) {
        self.open = true;
        self.replace_open = true;
        self.request_replace_focus = true;
        self.select_on_focus = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

/// The regex the query stands for (`None` when it is empty or isn't a valid
/// pattern): literal text unless Regex is on, whole words when asked.
fn pattern(state: &FindState) -> Option<regex::Regex> {
    if state.query.is_empty() {
        return None;
    }
    let mut source = if state.regex { state.query.clone() } else { regex::escape(&state.query) };
    if state.whole_word {
        let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        if word(state.query.chars().next()) && word(state.query.chars().last()) {
            source = format!(r"\b(?:{source})\b");
        }
    }
    regex::RegexBuilder::new(&source).case_insensitive(!state.case_sensitive).multi_line(true).build().ok()
}

/// Byte ranges of the matches of the query in `text` (empty matches left out).
pub fn compute_matches(text: &str, state: &FindState) -> Vec<(usize, usize)> {
    match pattern(state) {
        Some(re) => re
            .find_iter(text)
            .filter(|m| !m.as_str().is_empty())
            .filter(|m| state.scope.is_none_or(|(s, e)| m.start() >= s && m.end() <= e))
            .map(|m| (m.start(), m.end()))
            .collect(),
        None => Vec::new(),
    }
}

/// The text `found` is replaced with: the replacement (with `$1` groups in a
/// regex), in the case of the match when Preserve Case is on.
fn replacement_for(state: &FindState, re: &regex::Regex, text: &str, range: (usize, usize)) -> String {
    let found = &text[range.0..range.1];
    let mut out = String::new();
    if state.regex {
        if let Some(caps) = re.captures_at(text, range.0).filter(|c| c.get(0).is_some_and(|m| m.start() == range.0)) {
            caps.expand(&state.replace, &mut out);
        } else {
            out = state.replace.clone();
        }
    } else {
        out = state.replace.clone();
    }
    if state.preserve_case {
        let letters: Vec<char> = found.chars().filter(|c| c.is_alphabetic()).collect();
        if !letters.is_empty() && letters.iter().all(|c| c.is_uppercase()) {
            out = out.to_uppercase();
        } else if !letters.is_empty() && letters.iter().all(|c| c.is_lowercase()) {
            out = out.to_lowercase();
        } else if letters.first().is_some_and(|c| c.is_uppercase()) {
            let mut cs = out.chars();
            out = cs.next().map(|f| f.to_uppercase().collect::<String>() + cs.as_str()).unwrap_or_default();
        }
    }
    out
}

/// The edits Replace (`only` = the current match's index) or Replace All
/// make: byte ranges and what goes there.
pub fn replace_edits(text: &str, state: &FindState, only: Option<usize>) -> Vec<((usize, usize), String)> {
    let Some(re) = pattern(state) else { return Vec::new() };
    let matches = compute_matches(text, state);
    matches
        .iter()
        .enumerate()
        .filter(|(i, _)| only.is_none_or(|o| *i == o))
        .map(|(_, &r)| (r, replacement_for(state, &re, text, r)))
        .collect()
}

/// `text` with `edits` (non-overlapping byte ranges) applied.
pub fn apply_replacements(text: &str, edits: &[((usize, usize), String)]) -> String {
    let mut out = text.to_string();
    for ((s, e), with) in edits.iter().rev() {
        out.replace_range(*s..*e, with);
    }
    out
}

pub struct FindResponse {
    /// Navigate the editor to this byte offset (start of the current match).
    pub goto: Option<usize>,
    /// Replace the current match / every match.
    pub replace: Option<ReplaceAction>,
    /// The Find in Selection button was pressed.
    pub toggle_scope: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReplaceAction {
    One,
    All,
}

/// egui memory key: the editor area (`Rect`) and the width of the minimap and
/// scrollbar strip, set by the editor view each frame.
pub const ANCHOR: &str = "find-anchor";

const WIDTH: f32 = 419.0;

/// A 20px toggle inside an input (Match Case, Whole Word, Regex, Preserve
/// Case): `inputOption.active*` while on.
fn toggle(ui: &mut Ui, rect: Rect, glyph: &str, on: bool, tip: &str, id: egui::Id) -> bool {
    let resp = ui.interact(rect, id, Sense::click());
    let p = ui.painter();
    if on {
        p.rect(rect, CornerRadius::same(3), Palette::INPUT_OPTION_ACTIVE_BG(), Stroke::new(1.0, Palette::INPUT_OPTION_ACTIVE_BORDER()), StrokeKind::Inside);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(3), Palette::VSCE_INPUT_OPTION_HOVER_BG());
    }
    let fg = if on { Palette::INPUT_OPTION_ACTIVE_FG() } else { Palette::FG() };
    p.text(rect.center(), Align2::CENTER_CENTER, icons::codicon_by_name(glyph).unwrap_or('?').to_string(), codicon_font(16.0), fg);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, tip));
    resp.on_hover_text(tip).clicked()
}

/// A 22px icon button (radius 5); dimmed and inert while `enabled` is false.
fn button(ui: &mut Ui, rect: Rect, glyph: &str, enabled: bool, tip: &str, id: egui::Id, muted: bool) -> bool {
    let resp = ui.interact(rect, id, Sense::click());
    let p = ui.painter();
    if resp.hovered() && enabled {
        p.rect_filled(rect, CornerRadius::same(5), Palette::VSCE_TOOLBAR_HOVER_BG());
    }
    let fg = if !enabled {
        Palette::FG().gamma_multiply(0.45)
    } else if muted {
        Palette::FG().gamma_multiply(0.73)
    } else {
        Palette::FG()
    };
    p.text(rect.center(), Align2::CENTER_CENTER, icons::codicon_by_name(glyph).unwrap_or('?').to_string(), codicon_font(16.0), fg);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tip));
    resp.on_hover_text(tip).clicked() && enabled
}

/// An input box with a text field: `input.background`, 1px border (focus
/// border while focused), radius 4, 6px of padding before the text.
fn input_box(ui: &mut Ui, rect: Rect, text: &mut String, hint: &str, text_w: f32, id: egui::Id, focus: bool) -> egui::Response {
    let focused = ui.memory(|m| m.has_focus(id));
    let border = if focused { Palette::FOCUS_BORDER() } else { Palette::INPUT_BORDER() };
    ui.painter().rect(rect, CornerRadius::same(4), Palette::VSCE_INPUT_BG(), Stroke::new(1.0, border), StrokeKind::Inside);
    let inner = Rect::from_min_size(rect.min + egui::vec2(7.0, 1.0), egui::vec2(text_w - 6.0, rect.height() - 2.0));
    let resp = ui.put(
        inner,
        TextEdit::singleline(text)
            .id(id)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .vertical_align(egui::Align::Center)
            .hint_text(hint)
            .font(FontId::proportional(13.0))
            .text_color(Palette::FG())
            .desired_width(inner.width()),
    );
    if focus {
        resp.request_focus();
        // The whole text selected, so typing replaces the seed.
        let mut st = TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        st.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()))));
        st.store(ui.ctx(), id);
    }
    resp
}

/// Render the widget (only when `state.open`). `matches` must be the ranges
/// computed from the active document's current text.
pub fn show(ctx: &Context, state: &mut FindState, matches: &[(usize, usize)]) -> FindResponse {
    let mut out = FindResponse { goto: None, replace: None, toggle_scope: false };
    if !state.open {
        return out;
    }
    if state.current >= matches.len() {
        state.current = matches.len().saturating_sub(1);
    }

    // Escape closes the widget from anywhere while it is open.
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        state.open = false;
        return out;
    }
    // ⌥⌘C / ⌥⌘W / ⌥⌘R / ⌥⌘P: the toggles' keys.
    let alt_cmd = Modifiers::COMMAND | Modifiers::ALT;
    ctx.input_mut(|i| {
        if i.consume_key(alt_cmd, Key::C) {
            state.case_sensitive = !state.case_sensitive;
        }
        if i.consume_key(alt_cmd, Key::W) {
            state.whole_word = !state.whole_word;
        }
        if i.consume_key(alt_cmd, Key::R) {
            state.regex = !state.regex;
        }
        if i.consume_key(alt_cmd, Key::L) {
            out.toggle_scope = true;
        }
        if i.consume_key(alt_cmd, Key::P) {
            state.preserve_case = !state.preserve_case;
        }
    });

    let (anchor, inset) = ctx
        .data(|d| d.get_temp::<(Rect, f32)>(egui::Id::new(ANCHOR)))
        .unwrap_or((Rect::from_min_max(egui::pos2(0.0, 92.0), egui::pos2(ctx.content_rect().right() - 4.0, 700.0)), 127.0));
    let height = if state.replace_open { 62.0 } else { 33.0 };
    let width = WIDTH.min((anchor.width() - inset - 20.0).max(220.0));
    let pos = egui::pos2(anchor.right() - inset - 14.0 - width, anchor.top() + 4.0);

    egui::Area::new(egui::Id::new("editor_find_widget"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            let card = Rect::from_min_size(pos, egui::vec2(width, height));
            ui.set_clip_rect(card.expand(16.0));
            let p = ui.painter().clone();
            p.add(egui::epaint::Shadow { offset: [0, 0], blur: 12, spread: 0, color: Palette::VSCE_WIDGET_SHADOW() }.as_shape(card, CornerRadius::same(8)));
            p.rect(card, CornerRadius::same(8), Palette::EDITOR_WIDGET_BG(), Stroke::new(1.0, Palette::EDITOR_WIDGET_BORDER()), StrokeKind::Inside);
            ui.allocate_rect(card, Sense::hover());
            let at = |dx: f32, dy: f32, w: f32, h: f32| Rect::from_min_size(card.min + egui::vec2(dx, dy), egui::vec2(w, h));
            let id = ui.id();

            // Toggle Replace: a 18px strip down the card's left.
            let strip = at(6.0, 5.0, 18.0, if state.replace_open { 52.0 } else { 23.0 });
            let sresp = ui.interact(strip, id.with("toggle-replace"), Sense::click());
            if sresp.hovered() {
                p.rect_filled(strip, CornerRadius::same(4), Palette::VSCE_TOOLBAR_HOVER_BG());
            }
            let chevron = if state.replace_open { "chevron-down" } else { "chevron-right" };
            p.text(strip.center_top() + egui::vec2(0.0, 11.5), Align2::CENTER_CENTER, icons::codicon_by_name(chevron).unwrap_or('>').to_string(), codicon_font(16.0), Palette::FG().gamma_multiply(0.73));
            sresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Toggle Replace"));
            if sresp.on_hover_text("Toggle Replace").clicked() {
                state.replace_open = !state.replace_open;
            }

            // Find input with its three toggles.
            let find_box = at(27.0, 4.0, 216.0, 25.0);
            let before = (state.query.clone(), state.case_sensitive, state.whole_word, state.regex);
            let focus = std::mem::take(&mut state.request_focus);
            let find_id = id.with("find-input");
            let resp = input_box(ui, find_box, &mut state.query, "Find", 148.0, find_id, focus);
            for (i, (glyph, on, tip)) in [("case-sensitive", state.case_sensitive, "Match Case (⌥⌘C)"), ("whole-word", state.whole_word, "Match Whole Word (⌥⌘W)"), ("regex", state.regex, "Use Regular Expression (⌥⌘R)")].into_iter().enumerate() {
                let r = Rect::from_min_size(find_box.right_top() + egui::vec2(-66.0 + 22.0 * i as f32, 3.0), egui::vec2(20.0, 20.0));
                if toggle(ui, r, glyph, on, tip, id.with(("find-toggle", i))) {
                    match i {
                        0 => state.case_sensitive = !state.case_sensitive,
                        1 => state.whole_word = !state.whole_word,
                        _ => state.regex = !state.regex,
                    }
                }
            }
            if (state.query.clone(), state.case_sensitive, state.whole_word, state.regex) != before {
                state.current = 0;
            }
            // Enter / Shift+Enter cycle matches while the input is focused.
            if resp.has_focus() && !matches.is_empty() {
                let (enter, shift) = ui.input(|i| (i.key_pressed(Key::Enter), i.modifiers.shift));
                if enter {
                    state.current = if shift { (state.current + matches.len() - 1) % matches.len() } else { (state.current + 1) % matches.len() };
                    out.goto = Some(matches[state.current].0);
                    resp.request_focus();
                }
            }

            // Match count, Previous / Next, Find in Selection, Close.
            let has = !matches.is_empty();
            let count = if has { format!("{} of {}", state.current + 1, matches.len()) } else { "No results".to_string() };
            let count_box = at(246.0, 4.0, 69.0, 25.0);
            // Left in its box with 2px of padding (measured; not centred).
            p.text(egui::pos2(count_box.left() + 2.0, count_box.center().y), Align2::LEFT_CENTER, count, FontId::proportional(12.0), Palette::FG());
            if button(ui, at(318.0, 5.5, 22.0, 22.0), "arrow-up", has, "Previous Match (⇧Enter)", id.with("prev"), false) {
                state.current = (state.current + matches.len() - 1) % matches.len();
                out.goto = Some(matches[state.current].0);
            }
            if button(ui, at(343.0, 5.5, 22.0, 22.0), "arrow-down", has, "Next Match (Enter)", id.with("next"), false) {
                state.current = (state.current + 1) % matches.len();
                out.goto = Some(matches[state.current].0);
            }
            // Find in Selection: 40% while there is no selection to search.
            let sel = at(367.0, 5.5, 22.0, 22.0);
            if state.scope.is_some() || state.can_scope {
                if toggle(ui, sel, "selection", state.scope.is_some(), "Find in Selection (⌥⌘L)", id.with("scope")) {
                    out.toggle_scope = true;
                }
            } else {
                p.text(sel.center(), Align2::CENTER_CENTER, icons::codicon_by_name("selection").unwrap_or('?').to_string(), codicon_font(16.0), Palette::FG().gamma_multiply(0.4));
            }
            if button(ui, at(392.0, 6.0, 22.0, 22.0), "close", true, "Close (Escape)", id.with("close"), true) {
                state.open = false;
            }

            // Replace row.
            if state.replace_open {
                let replace_box = at(27.0, 32.0, 216.0, 25.0);
                let focus = std::mem::take(&mut state.request_replace_focus);
                let replace_id = id.with("replace-input");
                let rresp = input_box(ui, replace_box, &mut state.replace, "Replace", 192.0, replace_id, focus);
                let pr = Rect::from_min_size(replace_box.right_top() + egui::vec2(-22.0, 3.0), egui::vec2(20.0, 20.0));
                if toggle(ui, pr, "preserve-case", state.preserve_case, "Preserve Case (⌥⌘P)", id.with("preserve")) {
                    state.preserve_case = !state.preserve_case;
                }
                if rresp.has_focus() && has && ui.input(|i| i.key_pressed(Key::Enter)) {
                    out.replace = Some(ReplaceAction::One);
                    rresp.request_focus();
                }
                if button(ui, at(246.0, 33.5, 22.0, 22.0), "replace", has, "Replace (Enter)", id.with("replace-one"), false) {
                    out.replace = Some(ReplaceAction::One);
                }
                if button(ui, at(271.0, 33.5, 22.0, 22.0), "replace-all", has, "Replace All (⌘Enter)", id.with("replace-all"), false) {
                    out.replace = Some(ReplaceAction::All);
                }
            }
        });

    out
}

/// Build the editor highlight decoration from the current widget state.
pub fn highlight_for(state: &FindState, matches: &[(usize, usize)]) -> Option<FindHighlight> {
    if !state.open || matches.is_empty() {
        return None;
    }
    Some(FindHighlight { ranges: matches.to_vec(), current: state.current.min(matches.len() - 1), decorations: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(query: &str) -> FindState {
        FindState { query: query.to_string(), ..Default::default() }
    }

    #[test]
    fn matches_follow_case_word_and_regex() {
        let text = "Foo foo FOO food";
        assert_eq!(compute_matches(text, &state("foo")).len(), 4);
        let mut s = state("foo");
        s.case_sensitive = true;
        assert_eq!(compute_matches(text, &s), vec![(4, 7), (12, 15)]);
        s.case_sensitive = false;
        s.whole_word = true;
        assert_eq!(compute_matches(text, &s).len(), 3);
        let mut r = state(r"f(o+)\b");
        r.regex = true;
        assert_eq!(compute_matches(text, &r).len(), 3);
        // A bad pattern or an empty query finds nothing.
        r.query = "(".into();
        assert!(compute_matches(text, &r).is_empty());
        assert!(compute_matches(text, &state("")).is_empty());
        // Plain text takes regex characters literally.
        assert_eq!(compute_matches("a.b axb", &state("a.b")).len(), 1);
    }

    #[test]
    fn find_in_selection_limits_the_matches() {
        let mut s = FindState { query: "a".into(), ..Default::default() };
        assert_eq!(compute_matches("a a a", &s).len(), 3);
        s.scope = Some((2, 5));
        assert_eq!(compute_matches("a a a", &s), vec![(2, 3), (4, 5)]);
    }

    #[test]
    fn opening_seeds_the_query_with_the_selection_or_word() {
        let mut s = FindState::default();
        s.open_seeded("needle", false);
        assert!(s.open && s.query == "needle" && !s.replace_open);
        // Nothing under the caret, or several lines selected: the query stays.
        s.open_seeded("", false);
        assert_eq!(s.query, "needle");
        s.open_seeded("two\nlines", true);
        assert_eq!(s.query, "needle");
        assert!(s.replace_open);
    }

    #[test]
    fn replacing_one_all_groups_and_preserving_case() {
        let text = "Foo foo FOO";
        let mut s = state("foo");
        s.replace = "bar".into();
        assert_eq!(apply_replacements(text, &replace_edits(text, &s, Some(1))), "Foo bar FOO");
        assert_eq!(apply_replacements(text, &replace_edits(text, &s, None)), "bar bar bar");
        s.preserve_case = true;
        assert_eq!(apply_replacements(text, &replace_edits(text, &s, None)), "Bar bar BAR");
        let mut r = state(r"(\w+)@(\w+)");
        r.regex = true;
        r.replace = "$2:$1".into();
        assert_eq!(apply_replacements("ann@home", &replace_edits("ann@home", &r, None)), "home:ann");
    }
}
