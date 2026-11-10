//! The Search view (`contrib/search/browser/searchView.ts`, searchWidget.ts,
//! patternInputWidget.ts, searchview.css, Modern UI padding) as VS Code
//! 1.145 shows it: title actions, Toggle Replace, the find input with its
//! three toggles, the replace input with Preserve Case and Replace All, the
//! "…" search details (files to include / exclude with their toggles), the
//! result message and the result list (or tree). Searches as you type
//! (`search.searchOnType`, 300ms debounce). Geometry measured against
//! VS Code 1.145.2.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Align2, FontId, Key, Pos2, Rect, ScrollArea, Sense, Stroke, Ui};

use crate::icons::{self, codicon_font};
use crate::theme::Palette;

use super::engine::{run, SearchHit, SearchOutcome, SearchQuery};
use super::glob::GlobSet;

/// `search.searchOnTypeDebouncePeriod`.
const DEBOUNCE: Duration = Duration::from_millis(300);

pub struct SearchState {
    pub query: String,
    pub match_case: bool,
    pub whole_word: bool,
    pub regex: bool,
    pub outcome: Option<SearchOutcome>,
    pub focus_input: bool,
    pub collapsed: HashSet<PathBuf>,
    pub show_replace: bool,
    pub replace: String,
    pub preserve_case: bool,
    pub show_details: bool,
    pub include: String,
    pub exclude: String,
    pub only_open_editors: bool,
    pub only_changed_files: bool,
    /// "Use Exclude Settings and Ignore Files" (on by default).
    pub use_excludes: bool,
    /// View as Tree (`search.defaultViewMode` is list).
    pub tree: bool,
    /// Results dismissed with × (file, line, byte).
    pub dismissed: HashSet<(PathBuf, usize, usize)>,
    /// When the query or an option last changed (search-on-type).
    pub edited_at: Option<Instant>,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            match_case: false,
            whole_word: false,
            regex: false,
            outcome: None,
            focus_input: false,
            collapsed: HashSet::new(),
            show_replace: false,
            replace: String::new(),
            preserve_case: false,
            show_details: false,
            include: String::new(),
            exclude: String::new(),
            only_open_editors: false,
            only_changed_files: false,
            use_excludes: true,
            tree: false,
            dismissed: HashSet::new(),
            edited_at: None,
        }
    }
}

impl SearchState {
    /// The query the options describe; `only` comes from the caller.
    pub fn engine_query(&self, only: Option<Vec<PathBuf>>) -> SearchQuery {
        SearchQuery {
            text: self.query.clone(),
            match_case: self.match_case,
            whole_word: self.whole_word,
            regex: self.regex,
            include: GlobSet::parse(&self.include),
            exclude: GlobSet::parse(&self.exclude),
            use_excludes: self.use_excludes,
            only,
        }
    }
}

/// Files the "only in" toggles restrict to.
#[derive(Default)]
pub struct SearchScope {
    pub open_editors: Vec<PathBuf>,
    pub changed_files: Vec<PathBuf>,
}

#[derive(Default)]
pub struct SearchOutput {
    pub navigate_to: Option<(PathBuf, usize, usize)>,
    /// Replace every result (Replace All, ⌥⌘Enter).
    pub replace_all: bool,
    /// Replace one result, or all of a file's (`hit` None).
    pub replace: Option<(PathBuf, Option<(usize, usize)>)>,
    /// "Open in editor" / New Search Editor.
    pub open_search_editor: bool,
}

/// From the view title's bottom, measured.
const INPUT_TOP: f32 = 6.0;
const INPUT_H: f32 = 26.0;
const INPUT_LEFT: f32 = 20.0;
const INPUT_RIGHT: f32 = 9.0;
const ROW_GAP: f32 = 6.0;
/// The include / exclude boxes (24px with their border).
const DETAIL_INPUT_H: f32 = 24.0;
const ROW_H: f32 = 22.0;
/// Rows are inset 4px (Modern UI padding.css).
const INSET: f32 = 4.0;

pub fn show(
    ui: &mut Ui,
    workspace_root: &Option<PathBuf>,
    state: &mut SearchState,
    title: Rect,
    scope: &SearchScope,
) -> SearchOutput {
    let mut out = SearchOutput::default();
    let area = ui.available_rect_before_wrap();
    let left = area.left();
    let right = area.right();

    title_actions(ui, title, state, &mut out);

    // Toggle Replace, then the find input.
    let mut y = title.bottom() + INPUT_TOP;
    let find_rect = Rect::from_min_max(Pos2::new(left + INPUT_LEFT, y), Pos2::new(right - INPUT_RIGHT, y + INPUT_H));
    let mut changed = false;
    let mut focus_include = false;
    {
        let replace_block_h = if state.show_replace { INPUT_H * 2.0 + ROW_GAP } else { INPUT_H };
        let chevron = Rect::from_center_size(Pos2::new(left + 10.0, y + replace_block_h / 2.0), egui::vec2(16.0, replace_block_h));
        let resp = ui.interact(chevron, ui.id().with("search-toggle-replace"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(chevron, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
        }
        let glyph = if state.show_replace { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT };
        ui.painter().text(chevron.center(), Align2::CENTER_CENTER, glyph.to_string(), codicon_font(16.0), Palette::FG());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Toggle Replace"));
        if resp.on_hover_text("Toggle Replace").clicked() {
            state.show_replace = !state.show_replace;
        }
    }
    let toggles: [(&str, &str, bool); 3] = [
        ("case-sensitive", "Match Case (⌥⌘C)", state.match_case),
        ("whole-word", "Match Whole Word (⌥⌘W)", state.whole_word),
        ("regex", "Use Regular Expression (⌥⌘R)", state.regex),
    ];
    let (find_changed, find_enter, clicked) = input_box(ui, find_rect, "search-find", &mut state.query, "Search", &toggles, state.focus_input);
    state.focus_input = false;
    changed |= find_changed;
    match clicked {
        Some(0) => {
            state.match_case = !state.match_case;
            changed = true;
        }
        Some(1) => {
            state.whole_word = !state.whole_word;
            changed = true;
        }
        Some(2) => {
            state.regex = !state.regex;
            changed = true;
        }
        _ => {}
    }
    y += INPUT_H;

    // Replace input (Preserve Case inside, Replace All beside it).
    if state.show_replace {
        y += ROW_GAP;
        let r = Rect::from_min_max(Pos2::new(left + INPUT_LEFT, y), Pos2::new(right - INPUT_RIGHT - 28.5, y + INPUT_H));
        let (_, enter, clicked) = input_box(ui, r, "search-replace", &mut state.replace, "Replace", &[("preserve-case", "Preserve Case (⌥⌘P)", state.preserve_case)], false);
        if clicked == Some(0) {
            state.preserve_case = !state.preserve_case;
        }
        let has_results = state.outcome.as_ref().is_some_and(|o| o.total_hits > 0);
        let b = Rect::from_center_size(Pos2::new(right - INPUT_RIGHT - 14.0, r.center().y), egui::vec2(22.0, 22.0));
        let resp = ui.interact(b, ui.id().with("search-replace-all"), Sense::click());
        if resp.hovered() && has_results {
            ui.painter().rect_filled(b, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
        }
        let colour = if has_results { Palette::VSCE_ICON_2026_FG() } else { Palette::VSCE_DISABLED_FG() };
        ui.painter().text(b.center(), Align2::CENTER_CENTER, icon("replace-all").to_string(), codicon_font(16.0), colour);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, has_results, "Replace All"));
        if (resp.on_hover_text("Replace All (⌥⌘Enter)").clicked() || enter) && has_results {
            out.replace_all = true;
        }
        y += INPUT_H;
    }

    // "…" Toggle Search Details.
    {
        let c = Pos2::new(right - INPUT_RIGHT - 11.0, y + 8.0);
        let r = Rect::from_center_size(c, egui::vec2(22.0, 14.0));
        let resp = ui.interact(r, ui.id().with("search-details"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
        }
        ui.painter().text(c, Align2::CENTER_CENTER, icon("ellipsis").to_string(), codicon_font(16.0), Palette::VSCE_ICON_2026_FG());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Toggle Search Details"));
        if resp.on_hover_text("Toggle Search Details (⇧⌘J)").clicked() {
            state.show_details = !state.show_details;
            focus_include = state.show_details;
        }
    }

    if !state.show_details {
        // `.query-details { min-height: 1em }` under the inputs; the "…"
        // floats over it.
        y += 13.0;
    } else {
        // files to include
        y = details_label(ui, left + INPUT_LEFT, y, "files to include");
        let r = Rect::from_min_max(Pos2::new(left + INPUT_LEFT, y), Pos2::new(right - INPUT_RIGHT, y + DETAIL_INPUT_H));
        let include_toggles = [
            ("edit-code", "Search only in Changed Files (Source Control)", state.only_changed_files),
            ("book", "Search only in Open Editors", state.only_open_editors),
        ];
        let (c1, _, clicked) = input_box(ui, r, "search-include", &mut state.include, "e.g. *.ts, src/**/include", &include_toggles, focus_include);
        changed |= c1;
        match clicked {
            Some(0) => {
                state.only_changed_files = !state.only_changed_files;
                changed = true;
            }
            Some(1) => {
                state.only_open_editors = !state.only_open_editors;
                changed = true;
            }
            _ => {}
        }
        y += DETAIL_INPUT_H;
        // files to exclude
        y = details_label(ui, left + INPUT_LEFT, y - 0.5, "files to exclude");
        let r = Rect::from_min_max(Pos2::new(left + INPUT_LEFT, y), Pos2::new(right - INPUT_RIGHT, y + DETAIL_INPUT_H + 1.0));
        let exclude_toggles = [("exclude", "Use Exclude Settings and Ignore Files", state.use_excludes)];
        let (c2, _, clicked) = input_box(ui, r, "search-exclude", &mut state.exclude, "", &exclude_toggles, false);
        changed |= c2;
        if clicked == Some(0) {
            state.use_excludes = !state.use_excludes;
            changed = true;
        }
        y += DETAIL_INPUT_H + 1.0 + 3.5;
    }

    // Search on type.
    if changed {
        state.edited_at = Some(Instant::now());
        state.dismissed.clear();
        if state.query.is_empty() {
            state.outcome = None;
        }
    }
    let due = state.edited_at.is_some_and(|t| t.elapsed() >= DEBOUNCE);
    if state.edited_at.is_some() && !due {
        ui.ctx().request_repaint_after(DEBOUNCE);
    }
    if (due || find_enter) && workspace_root.is_some() {
        state.edited_at = None;
        execute(workspace_root.as_ref().unwrap(), state, scope);
    }

    ui.allocate_rect(Rect::from_min_max(area.min, Pos2::new(right, y)), Sense::hover());

    let Some(root) = workspace_root else {
        // No folder: VS Code's message.
        message(ui, left, y, "You have not yet opened a folder.", None);
        return out;
    };
    let Some(o) = state.outcome.take() else { return out };
    if let Some(err) = &o.error {
        message(ui, left, y, err, None);
        state.outcome = Some(o);
        return out;
    }

    // "N results in M files - Open in editor"
    let visible: Vec<(&super::engine::FileResult, Vec<&SearchHit>)> = o
        .results
        .iter()
        .map(|f| (f, f.hits.iter().filter(|h| !state.dismissed.contains(&(f.path.clone(), h.line, h.byte_start))).collect::<Vec<_>>()))
        .filter(|(_, hits)| !hits.is_empty())
        .collect();
    let total: usize = visible.iter().map(|(_, h)| h.len()).sum();
    let text = if total == 0 {
        "No results found. Review your settings for configured exclusions and check your gitignore files - ".to_string()
    } else {
        format!("{total} result{} in {} file{} - ", plural(total), visible.len(), plural(visible.len()))
    };
    let (msg_bottom, link) = message(ui, left, y, &text, (total > 0).then_some("Open in editor"));
    if link {
        out.open_search_editor = true;
    }
    // The first row is centred 28.5px under the message.
    let results_top = msg_bottom - 11.0 + 28.5 - ROW_H / 2.0;

    let list = Rect::from_min_max(Pos2::new(left, results_top), Pos2::new(right, area.bottom()));
    let mut list_ui = ui.new_child(egui::UiBuilder::new().max_rect(list).layout(egui::Layout::top_down(egui::Align::Min)));
    ScrollArea::vertical().auto_shrink([false, false]).show(&mut list_ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        if state.tree {
            render_tree(ui, root, &visible, state, &mut out);
        } else {
            for (file, hits) in &visible {
                file_row(ui, root, file.path.as_path(), hits.len(), 0, state, &mut out);
                if !state.collapsed.contains(&file.path) {
                    for hit in hits {
                        match_row(ui, &file.path, hit, 0, state, &mut out);
                    }
                }
            }
        }
    });
    drop(visible);
    if state.outcome.is_none() {
        state.outcome = Some(o);
    }
    out
}

fn icon(name: &str) -> char {
    icons::codicon_by_name(name).unwrap_or('?')
}

/// The view title's actions (searchActionsTopBar), 26px apart, the last
/// centred 15px from the right edge: Refresh, Clear Search Results, New
/// Search Editor, View as Tree / List, Collapse / Expand All.
fn title_actions(ui: &mut Ui, title: Rect, state: &mut SearchState, out: &mut SearchOutput) {
    let has_results = state.outcome.is_some();
    let all_collapsed = state
        .outcome
        .as_ref()
        .is_some_and(|o| !o.results.is_empty() && o.results.iter().all(|f| state.collapsed.contains(&f.path)));
    let actions: [(&str, &str, bool); 5] = [
        ("refresh", "Refresh", !state.query.is_empty()),
        ("clear-all", "Clear Search Results", has_results || !state.query.is_empty()),
        ("new-file", "New Search Editor", true),
        (if state.tree { "list-flat" } else { "list-tree" }, if state.tree { "View as List" } else { "View as Tree" }, true),
        (if all_collapsed { "expand-all" } else { "collapse-all" }, if all_collapsed { "Expand All" } else { "Collapse All" }, has_results),
    ];
    let last = title.right() - 15.0;
    for (i, (name, tip, enabled)) in actions.into_iter().enumerate() {
        let c = Pos2::new(last - 26.0 * (4 - i) as f32, title.center().y);
        let r = Rect::from_center_size(c, egui::vec2(22.0, 22.0));
        let resp = ui.interact(r, ui.id().with(("search-title", i)), Sense::click());
        if resp.hovered() && enabled {
            ui.painter().rect_filled(r, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
        }
        let colour = if enabled { Palette::VSCE_ICON_2026_FG() } else { Palette::VSCE_DISABLED_FG() };
        ui.painter().text(c, Align2::CENTER_CENTER, icon(name).to_string(), codicon_font(16.0), colour);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tip));
        if !(resp.on_hover_text(tip).clicked() && enabled) {
            continue;
        }
        match i {
            0 => state.edited_at = Some(Instant::now() - DEBOUNCE),
            1 => {
                state.query.clear();
                state.replace.clear();
                state.outcome = None;
                state.dismissed.clear();
            }
            2 => out.open_search_editor = true,
            3 => state.tree = !state.tree,
            _ => {
                if let Some(o) = &state.outcome {
                    if all_collapsed {
                        state.collapsed.clear();
                    } else {
                        state.collapsed.extend(o.results.iter().map(|f| f.path.clone()));
                    }
                }
            }
        }
    }
}

/// A monaco input box: 1px border (`focusBorder` while focused), 4px radius,
/// text 7px in, option toggles at the right edge 21.5px apart. Returns
/// (text changed, Enter pressed, toggle clicked).
fn input_box(
    ui: &mut Ui,
    rect: Rect,
    id: &str,
    text: &mut String,
    placeholder: &str,
    toggles: &[(&str, &str, bool)],
    focus: bool,
) -> (bool, bool, Option<usize>) {
    let slot = ui.painter().add(egui::Shape::Noop);
    let toggles_w = toggles.len() as f32 * 21.5 + 4.0;
    let text_rect = Rect::from_min_max(
        Pos2::new(rect.left() + 7.0, rect.top() + 1.0),
        Pos2::new(rect.right() - toggles_w, rect.bottom() - 1.0),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(text_rect).layout(*ui.layout()));
    child.visuals_mut().weak_text_color = Some(Palette::INPUT_PLACEHOLDER());
    child.visuals_mut().text_cursor.stroke.color = Palette::INPUT_FG();
    let resp = child.add_sized(
        text_rect.size(),
        egui::TextEdit::singleline(text)
            .id_salt(id)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .hint_text(placeholder)
            .font(FontId::proportional(13.0))
            .text_color(Palette::INPUT_FG())
            .vertical_align(egui::Align::Center),
    );
    if focus {
        resp.request_focus();
    }
    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    let border = if resp.has_focus() { Palette::FOCUS_BORDER() } else { Palette::INPUT_BORDER() };
    ui.painter().set(
        slot,
        egui::epaint::RectShape::new(rect, 4.0, Palette::INPUT_BG(), Stroke::new(1.0, border), egui::StrokeKind::Inside),
    );
    let mut clicked = None;
    let n = toggles.len();
    for (i, (name, tip, on)) in toggles.iter().enumerate() {
        let c = Pos2::new(rect.right() - 13.5 - 21.5 * (n - 1 - i) as f32, rect.center().y);
        let r = Rect::from_center_size(c, egui::vec2(20.0, 20.0));
        let t = ui.interact(r, ui.id().with((id, "toggle", i)), Sense::click());
        if *on {
            ui.painter().rect(r, 4.0, Palette::INPUT_OPTION_ACTIVE_BG(), Stroke::new(1.0, Palette::INPUT_OPTION_ACTIVE_BORDER()), egui::StrokeKind::Inside);
        } else if t.hovered() {
            ui.painter().rect_filled(r, 4.0, Palette::VSCE_INPUT_OPTION_HOVER_BG());
        }
        let colour = if *on { Palette::INPUT_OPTION_ACTIVE_FG() } else { Palette::VSCE_ICON_2026_FG() };
        ui.painter().text(c, Align2::CENTER_CENTER, icon(name).to_string(), codicon_font(16.0), colour);
        let label = tip.to_string();
        let on = *on;
        t.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, label.clone()));
        if t.on_hover_text(*tip).clicked() {
            clicked = Some(i);
        }
    }
    (resp.changed(), enter, clicked)
}

/// "files to include" / "files to exclude": centred 13.5px under what's
/// above, its input 9.5px under that.
fn details_label(ui: &Ui, x: f32, top: f32, text: &str) -> f32 {
    ui.painter().text(Pos2::new(x, top + 12.0), Align2::LEFT_CENTER, text, FontId::proportional(13.0), Palette::FG());
    top + 23.0
}

/// The result message row, with an optional trailing link. Returns its
/// bottom and whether the link was clicked.
fn message(ui: &mut Ui, left: f32, top: f32, text: &str, link: Option<&str>) -> (f32, bool) {
    let y = top + 10.5;
    let g = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(13.0), Palette::FG_DESCRIPTION());
    let w = g.size().x;
    ui.painter().galley(Pos2::new(left + INPUT_LEFT + 2.0, y - g.size().y / 2.0), g, Palette::FG_DESCRIPTION());
    let mut clicked = false;
    if let Some(link) = link {
        let lg = ui.painter().layout_no_wrap(link.to_string(), FontId::proportional(13.0), Palette::TEXT_LINK_FG());
        let r = Rect::from_min_size(Pos2::new(left + INPUT_LEFT + 2.0 + w, y - 10.0), egui::vec2(lg.size().x, 20.0));
        let resp = ui.interact(r, ui.id().with("search-message-link"), Sense::click());
        let colour = if resp.hovered() { Palette::TEXT_LINK_ACTIVE_FG() } else { Palette::TEXT_LINK_FG() };
        let size = lg.size();
        ui.painter().galley(Pos2::new(r.left(), y - size.y / 2.0), lg, colour);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, link));
        clicked = resp.clicked();
    }
    (y + 11.0, clicked)
}

fn execute(root: &Path, state: &mut SearchState, scope: &SearchScope) {
    if state.query.trim().is_empty() {
        state.outcome = None;
        return;
    }
    let only = match (state.only_open_editors, state.only_changed_files) {
        (false, false) => None,
        (open, changed) => {
            let mut files = Vec::new();
            if open {
                files.extend(scope.open_editors.iter().cloned());
            }
            if changed {
                files.extend(scope.changed_files.iter().cloned());
            }
            Some(files)
        }
    };
    state.outcome = Some(run(root, &state.engine_query(only)));
}

/// List mode groups by file; tree mode adds the folders above them.
fn render_tree(
    ui: &mut Ui,
    root: &Path,
    files: &[(&super::engine::FileResult, Vec<&SearchHit>)],
    state: &mut SearchState,
    out: &mut SearchOutput,
) {
    let mut shown_dirs: Vec<PathBuf> = Vec::new();
    for (file, hits) in files {
        let rel_parent = file.path.parent().and_then(|p| p.strip_prefix(root).ok()).map(Path::to_path_buf).unwrap_or_default();
        let mut depth = 0;
        let mut acc = root.to_path_buf();
        let mut hidden = false;
        for comp in rel_parent.components() {
            acc.push(comp);
            if !shown_dirs.contains(&acc) {
                if !hidden {
                    folder_row(ui, &acc, depth, state);
                }
                shown_dirs.push(acc.clone());
            }
            hidden |= state.collapsed.contains(&acc);
            depth += 1;
        }
        if hidden {
            continue;
        }
        file_row(ui, root, &file.path, hits.len(), depth, state, out);
        if !state.collapsed.contains(&file.path) {
            for hit in hits {
                match_row(ui, &file.path, hit, depth, state, out);
            }
        }
    }
}

fn row(ui: &mut Ui) -> (Rect, egui::Response) {
    let (slot, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::click());
    let rect = slot.shrink2(egui::vec2(INSET, 0.0));
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, Palette::LIST_HOVER_BG());
    }
    (rect, resp)
}

fn twistie(ui: &Ui, rect: Rect, depth: usize, collapsed: bool) {
    let x = rect.left() + 18.5 + 8.0 * depth as f32;
    let glyph = if collapsed { icons::CHEVRON_RIGHT } else { icons::CHEVRON_DOWN };
    ui.painter().text(Pos2::new(x, rect.center().y), Align2::CENTER_CENTER, glyph.to_string(), codicon_font(16.0), Palette::FG());
}

fn folder_row(ui: &mut Ui, dir: &Path, depth: usize, state: &mut SearchState) {
    let (rect, resp) = row(ui);
    let collapsed = state.collapsed.contains(dir);
    twistie(ui, rect, depth, collapsed);
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    ui.painter().text(Pos2::new(rect.left() + 30.0 + 8.0 * depth as f32, rect.center().y), Align2::LEFT_CENTER, &name, FontId::proportional(13.0), Palette::FG());
    if resp.clicked() {
        if !state.collapsed.remove(dir) {
            state.collapsed.insert(dir.to_path_buf());
        }
    }
}

/// A file: twistie, Seti icon, name, its folder as description, the count
/// badge (round, `badge.*`); Replace All in File on hover while replacing.
fn file_row(ui: &mut Ui, root: &Path, path: &Path, count: usize, depth: usize, state: &mut SearchState, out: &mut SearchOutput) {
    let (rect, resp) = row(ui);
    let collapsed = state.collapsed.contains(path);
    twistie(ui, rect, depth, collapsed);
    let indent = 8.0 * depth as f32;
    let cy = rect.center().y;
    if let Some((glyph, colour)) = crate::file_icons::icon_for(path) {
        ui.painter().text(Pos2::new(rect.left() + 39.0 + indent, cy), Align2::CENTER_CENTER, glyph.to_string(), crate::file_icons::seti_font(16.0), colour);
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let name_x = rect.left() + 52.5 + indent;
    let g = ui.painter().layout_no_wrap(name, FontId::proportional(13.0), Palette::FG());
    let w = g.size().x;
    ui.painter().galley(Pos2::new(name_x, cy - g.size().y / 2.0), g, Palette::FG());
    if !state.tree {
        let dir = path.parent().and_then(|p| p.strip_prefix(root).ok()).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        if !dir.is_empty() {
            ui.painter().text(Pos2::new(name_x + w + 6.0, cy), Align2::LEFT_CENTER, dir, FontId::proportional(12.0), Palette::FG_DESCRIPTION());
        }
    }
    // Count badge: 19px circle (`.monaco-count-badge`, Modern UI).
    let text = count.to_string();
    let tg = ui.painter().layout_no_wrap(text, FontId::proportional(11.0), Palette::BADGE_FG());
    let bw = (tg.size().x + 12.0).max(19.0);
    let badge = Rect::from_center_size(Pos2::new(rect.right() - 6.0 - bw / 2.0, cy), egui::vec2(bw, 19.0));
    if state.show_replace && resp.hovered() {
        let b = Rect::from_center_size(Pos2::new(badge.left() - 14.0, cy), egui::vec2(20.0, 20.0));
        let r = ui.interact(b, ui.id().with(("replace-file", path)), Sense::click());
        ui.painter().text(b.center(), Align2::CENTER_CENTER, icon("replace-all").to_string(), codicon_font(16.0), Palette::FG());
        if r.on_hover_text("Replace All").clicked() {
            out.replace = Some((path.to_path_buf(), None));
        }
    }
    ui.painter().rect_filled(badge, 9.5, Palette::BADGE_BG());
    let size = tg.size();
    ui.painter().galley(badge.center() - size / 2.0, tg, Palette::BADGE_FG());
    let label = path.display().to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
    if resp.clicked() && !state.collapsed.remove(path) {
        state.collapsed.insert(path.to_path_buf());
    }
}

/// A match: the line with the match highlighted
/// (`editor.findMatchHighlightBackground`), the text before it cut to
/// 26 characters at a word boundary (`lcut`, match.ts); Replace and
/// Dismiss on hover.
fn match_row(ui: &mut Ui, path: &Path, hit: &SearchHit, depth: usize, state: &mut SearchState, out: &mut SearchOutput) {
    let (rect, resp) = row(ui);
    let cy = rect.center().y;
    let x0 = rect.left() + 38.5 + 8.0 * depth as f32;
    let preview = &hit.preview;
    let start = hit.preview_start.min(preview.len());
    let end = (start + hit.match_len).min(preview.len());
    let before = lcut(&preview[..start], 26, "…");
    let inside = &preview[start..end];
    let after = &preview[end..];
    let font = FontId::proportional(13.0);
    let fg = Palette::FG();
    let mut x = x0;
    let clip = Rect::from_min_max(rect.min, Pos2::new(rect.right() - if resp.hovered() { 48.0 } else { 0.0 }, rect.bottom()));
    let painter = ui.painter().with_clip_rect(clip.intersect(ui.clip_rect()));
    let g = painter.layout_no_wrap(before, font.clone(), fg);
    let w = g.size().x;
    painter.galley(Pos2::new(x, cy - g.size().y / 2.0), g, fg);
    x += w;
    let g = painter.layout_no_wrap(inside.to_string(), font.clone(), fg);
    let mr = Rect::from_min_size(Pos2::new(x, cy - 9.0), egui::vec2(g.size().x, 18.0));
    painter.rect_filled(mr, 0.0, Palette::SEARCH_MATCH_BG());
    let w = g.size().x;
    painter.galley(Pos2::new(x, cy - g.size().y / 2.0), g, fg);
    x += w;
    // The rest, cut with "…" at the row's end.
    let room = clip.right() - x - 4.0;
    let mut rest = after.to_string();
    let full = painter.layout_no_wrap(rest.clone(), font.clone(), fg);
    if full.size().x > room {
        while !rest.is_empty() && painter.layout_no_wrap(format!("{rest}…"), font.clone(), fg).size().x > room {
            rest.pop();
        }
        rest.push('…');
    }
    let g = painter.layout_no_wrap(rest, font, fg);
    painter.galley(Pos2::new(x, cy - g.size().y / 2.0), g, fg);

    let key = (path.to_path_buf(), hit.line, hit.byte_start);
    let mut action_clicked = false;
    if resp.hovered() {
        let mut bx = rect.right() - 14.0;
        let mut actions = vec![("close", "Dismiss")];
        if state.show_replace {
            actions.insert(0, ("replace", "Replace"));
        }
        for (name, tip) in actions.into_iter().rev() {
            let b = Rect::from_center_size(Pos2::new(bx, cy), egui::vec2(20.0, 20.0));
            let r = ui.interact(b, ui.id().with((name, &key)), Sense::click());
            if r.hovered() {
                ui.painter().rect_filled(b, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
            }
            ui.painter().text(b.center(), Align2::CENTER_CENTER, icon(name).to_string(), codicon_font(16.0), Palette::FG());
            if r.on_hover_text(tip).clicked() {
                action_clicked = true;
                if name == "close" {
                    state.dismissed.insert(key.clone());
                } else {
                    out.replace = Some((path.to_path_buf(), Some((hit.line, hit.byte_start))));
                }
            }
            bx -= 22.0;
        }
    }
    let label = format!("{}{}{}", &preview[..start], inside, after);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
    if resp.clicked() && !action_clicked {
        out.navigate_to = Some((path.to_path_buf(), hit.line, hit.byte_start));
    }
}

/// `strings.ts lcut`: the end of `text` at most `n` characters long, cut at
/// a word boundary, with `prefix` when shortened.
pub fn lcut(text: &str, n: usize, prefix: &str) -> String {
    let trimmed = text.trim_start();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() < n {
        return trimmed.to_string();
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    // Word boundaries, as `\b` positions (char indices).
    let mut cut = 0;
    for i in 1..=chars.len() {
        let boundary = i == chars.len() || is_word(chars[i - 1]) != is_word(chars[i]);
        if !boundary {
            continue;
        }
        if chars.len() - i < n {
            break;
        }
        cut = i;
    }
    if cut == 0 {
        return trimmed.to_string();
    }
    let rest: String = chars[cut..].iter().collect();
    format!("{prefix}{}", rest.trim_start())
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use super::lcut;

    #[test]
    fn lcut_like_vs_code() {
        assert_eq!(lcut("short", 26, "…"), "short");
        assert_eq!(lcut("const root = path.dirname(fileURLTo", 26, "…"), "…root = path.dirname(fileURLTo");
    }
}
