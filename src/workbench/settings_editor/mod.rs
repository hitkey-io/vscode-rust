//! The Settings editor (settingsEditor2.ts, settingsTree.ts,
//! settingsEditor2.css, modern UI tabs.css / roundedCorners.css), measured
//! against VS Code 1.145.2 in its modal at 1300 × 820:
//!
//! - header: search box 26px tall 14px below the top, 24px in from both
//!   sides (Clear Settings Search Input and Filter Settings inside on the
//!   right, "N Settings Found" before them while searching); 10px below it
//!   the User / Workspace tabs (22px, 2px 8px padding, semibold 13,
//!   radius 4, `modernTab.activeBackground` on the checked one) and the
//!   Backup and Sync Settings button (26px) on the right;
//! - body 90px below the top: the table of contents (200px, 24px left
//!   padding, 22px rows, the selected entry bold) and a 1px
//!   `settings.sashBorder` line, then the settings list;
//! - groups: titles 26 / 22 / 18px semibold; settings: 12px top padding,
//!   "Category: " and the label semibold 13 in `settings.headerForeground`,
//!   the description 24px under the title, the control 9px under the
//!   description (number 200px, text 420px, enum 320px × 26), 18px bottom
//!   padding (26 for checkboxes); hover `settings.rowHoverBackground`,
//!   radius 6; the modified bar 2px wide 5px in; the gear 22px left of the
//!   row on hover.

pub mod markdown;
pub mod model;

use std::collections::{HashMap, HashSet};

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use serde_json::{Map, Value};

use crate::icons;
use crate::theme::Palette;
use markdown::{Block, Link};
use model::{SettingDef, TocNode, ValueType};

const PAD_X: f32 = 24.0;
const SEARCH_TOP: f32 = 14.0;
const SEARCH_H: f32 = 26.0;
const TABS_TOP: f32 = 50.0;
const BODY_TOP: f32 = 90.0;
const BODY_BOTTOM: f32 = 7.0;
const TOC_WIDTH: f32 = 200.0;
const TOC_ROW: f32 = 22.0;
const GROUP_ROW: f32 = 41.0;
const ITEM_PAD_X: f32 = 14.0;
const TITLE_CENTRE: f32 = 21.1;
const DESC_TOP: f32 = 36.0;
const CONTROL_GAP: f32 = 9.0;
const PAD_BOTTOM: f32 = 18.0;
const PAD_BOTTOM_BOOL: f32 = 26.0;
const INPUT_H: f32 = 25.0;
const SELECT_H: f32 = 26.0;

/// What Filter Settings offers (`SettingsEditor2.SUGGESTIONS`).
pub const FILTERS: &[&str] = &[
    "@modified",
    "@tag:notebookLayout",
    "@tag:notebookOutputLayout",
    "@tag:requireTrustedWorkspace",
    "@tag:workspaceTrust",
    "@tag:sync",
    "@tag:usesOnlineServices",
    "@tag:telemetry",
    "@tag:accessibility",
    "@tag:preview",
    "@tag:experimental",
    "@tag:expassigned",
    "@tag:agentMerge",
    "@tag:chatPermissions",
    "@tag:advanced",
    "@id:",
    "@ext:",
    "@feature:scm",
    "@feature:explorer",
    "@feature:search",
    "@feature:debug",
    "@feature:extensions",
    "@feature:terminal",
    "@feature:task",
    "@feature:problems",
    "@feature:output",
    "@feature:comments",
    "@feature:remote",
    "@feature:timeline",
    "@feature:notebook",
    "@feature:chat",
    "@hasPolicy",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Target {
    #[default]
    User,
    Workspace,
}

#[derive(Clone, Debug)]
enum RowKind {
    Group { label: String, level: u8, toc: String },
    Setting { index: usize, group: String, toc: String },
    NoResults,
}

#[derive(Clone, Debug)]
struct Row {
    kind: RowKind,
    y: f32,
    h: f32,
}

/// Rows laid out for one width / query / target / values state.
#[derive(Default)]
struct Layout {
    key: (u32, String, Target, u64),
    rows: Vec<Row>,
    total: f32,
    tree: TocNode,
    /// Matching settings while searching.
    found: Option<usize>,
}

/// Caller-owned state of one Settings editor.
#[derive(Default)]
pub struct SettingsEditorState {
    pub query: String,
    pub target: Target,
    pub focus_search: bool,
    scroll: f32,
    toc_expanded: HashSet<String>,
    toc_selected: Option<String>,
    focused_row: Option<String>,
    /// The table of contents has the keyboard (its selection is outlined).
    toc_focused: bool,
    layout: Layout,
    /// Text typed into number / text inputs, by setting key.
    inputs: HashMap<String, String>,
    dropdown: Option<(String, Rect)>,
    /// Scroll to this setting (or group) on the next frame.
    reveal: Option<String>,
    values_generation: u64,
    last_values_hash: u64,
    dragging_scrollbar: Option<f32>,
}

impl SettingsEditorState {
    pub fn new() -> Self {
        SettingsEditorState { focus_search: true, toc_selected: Some("commonlyUsed".into()), ..Default::default() }
    }

    /// Opens with `query` in the search box (`workbench.action.openSettings`
    /// with a query).
    pub fn with_query(query: &str) -> Self {
        SettingsEditorState { query: query.to_string(), ..Self::new() }
    }
}

/// What the person did.
#[derive(Default, Debug)]
pub struct SettingsEditorOutput {
    /// Set (`Some`) or reset (`None`) a value in a target.
    pub set: Option<(Target, String, Option<Value>)>,
    /// "Edit in settings.json".
    pub edit_in_json: Option<(Target, String)>,
    pub open_url: Option<String>,
    /// The row's gear: its key, where to open the menu, configured.
    pub gear_menu: Option<(String, Pos2, bool)>,
    pub filter_menu: Option<Pos2>,
    pub backup_and_sync: bool,
    /// Escape wasn't used (no search text to clear): close the editor.
    pub escape: bool,
}

/// The values the editor shows.
pub struct Values<'a> {
    pub user: &'a Map<String, Value>,
    /// `None` without a folder (the Workspace tab is disabled).
    pub workspace: Option<&'a Map<String, Value>>,
}

impl Values<'_> {
    fn of(&self, target: Target) -> Option<&Map<String, Value>> {
        match target {
            Target::User => Some(self.user),
            Target::Workspace => self.workspace,
        }
    }

    fn hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        serde_json::to_string(self.user).unwrap_or_default().hash(&mut h);
        self.workspace.map(|w| serde_json::to_string(w).unwrap_or_default()).hash(&mut h);
        h.finish()
    }
}

fn text_fg() -> Color32 {
    Palette::FG().gamma_multiply(0.9)
}


/// Draws the editor in `rect`.
pub fn show(ui: &mut Ui, rect: Rect, state: &mut SettingsEditorState, values: &Values<'_>) -> SettingsEditorOutput {
    let mut out = SettingsEditorOutput::default();
    let id = ui.id().with("settings-editor");
    ui.painter().rect_filled(rect, 0.0, Palette::EDITOR_BG());
    if values.workspace.is_none() {
        state.target = Target::User;
    }
    let hash = values.hash();
    if hash != state.last_values_hash {
        state.last_values_hash = hash;
        state.values_generation += 1;
    }

    // Max width 1400, centred.
    let inner = {
        let w = rect.width().min(1400.0);
        Rect::from_min_size(Pos2::new(rect.center().x - w / 2.0, rect.top()), Vec2::new(w, rect.height()))
    };

    header(ui, inner, state, values, &mut out, id);

    let body = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + BODY_TOP), Pos2::new(rect.right(), rect.bottom() - BODY_BOTTOM));
    let narrow = rect.width() < 700.0;
    let toc_rect = Rect::from_min_size(body.min, Vec2::new(if narrow { 0.0 } else { TOC_WIDTH }, body.height()));
    // The separator is drawn over the list's first pixel column.
    let tree_rect = Rect::from_min_max(Pos2::new(toc_rect.right(), body.top()), body.max);
    if !narrow {
        ui.painter().vline(toc_rect.right() + 0.5, body.y_range(), Stroke::new(1.0, Palette::SETTINGS_SASH_BORDER()));
    }

    relayout(ui, state, values, tree_rect.width());
    if !narrow {
        toc(ui, toc_rect, state, id);
    }
    tree(ui, tree_rect, state, values, &mut out, id);
    dropdown_popup(ui, state, values, &mut out, id);

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) && state.dropdown.is_none() {
        if !state.query.is_empty() {
            state.query.clear();
        } else {
            out.escape = true;
        }
    }
    out
}

// ── Header ────────────────────────────────────────────────────────────────

fn header(ui: &mut Ui, inner: Rect, state: &mut SettingsEditorState, values: &Values<'_>, out: &mut SettingsEditorOutput, id: egui::Id) {
    let painter = ui.painter().clone();
    let search = Rect::from_min_max(
        Pos2::new(inner.left() + PAD_X, inner.top() + SEARCH_TOP),
        Pos2::new(inner.right() - PAD_X, inner.top() + SEARCH_TOP + SEARCH_H),
    );
    let slot = painter.add(egui::Shape::Noop);
    // Filter Settings and Clear Settings Search Input, 22px each, 1px in.
    let filter_c = Pos2::new(search.right() - 12.5, search.center().y);
    let clear_c = Pos2::new(search.right() - 36.0, search.center().y);
    let mut widgets_left = clear_c.x - 11.0;
    if let Some(found) = state.layout.found.filter(|_| !state.query.trim().is_empty()) {
        let label = match found {
            0 => "No Settings Found".to_string(),
            1 => "1 Setting Found".to_string(),
            n => format!("{n} Settings Found"),
        };
        let g = painter.layout_no_wrap(label, FontId::proportional(13.0), Palette::FG_DESCRIPTION());
        widgets_left -= g.size().x + 3.0;
        painter.galley(Pos2::new(widgets_left, search.center().y - g.size().y / 2.0 - 1.5), g, Color32::PLACEHOLDER);
    }
    let text_rect = Rect::from_min_max(Pos2::new(search.left() + 7.0, search.top() + 1.0), Pos2::new(widgets_left - 4.0, search.bottom() - 1.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(text_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.visuals_mut().weak_text_color = Some(Palette::INPUT_PLACEHOLDER_FG());
    child.visuals_mut().text_cursor.stroke.color = Palette::INPUT_FG();
    let resp = child.add_sized(
        text_rect.size(),
        egui::TextEdit::singleline(&mut state.query)
            .id_salt(id.with("search"))
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .hint_text("Search settings")
            .font(FontId::proportional(13.0))
            .text_color(Palette::INPUT_FG())
            .vertical_align(egui::Align::Center),
    );
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Search settings"));
    if std::mem::take(&mut state.focus_search) {
        resp.request_focus();
    }
    if resp.changed() {
        state.scroll = 0.0;
    }
    let border = if resp.has_focus() { Palette::FOCUS_BORDER() } else { Palette::SETTINGS_TEXT_INPUT_BORDER() };
    painter.set(slot, egui::epaint::RectShape::new(search, 4.0, Palette::INPUT_BG(), Stroke::new(1.0, border), egui::StrokeKind::Inside));
    for (c, name, tip, enabled) in [
        (clear_c, "clear-all", "Clear Settings Search Input", !state.query.is_empty()),
        (filter_c, "filter", "Filter Settings", true),
    ] {
        let r = Rect::from_center_size(c, Vec2::splat(22.0));
        let resp = ui.interact(r, id.with(name), Sense::click());
        if resp.hovered() && enabled {
            painter.rect_filled(r, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
        }
        let colour = if enabled { Palette::FG() } else { Palette::FG().gamma_multiply(0.5) };
        painter.text(c, Align2::CENTER_CENTER, icons::codicon_by_name(name).unwrap_or('?').to_string(), icons::codicon_font(16.0), colour);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tip));
        let clicked = resp.on_hover_text(tip).clicked();
        if clicked && enabled {
            if name == "clear-all" {
                state.query.clear();
                state.focus_search = true;
            } else {
                out.filter_menu = Some(r.left_bottom());
            }
        }
    }

    // User / Workspace.
    let tabs_top = inner.top() + TABS_TOP;
    let mut x = inner.left() + PAD_X;
    for (label, target) in [("User", Target::User), ("Workspace", Target::Workspace)] {
        let enabled = target == Target::User || values.workspace.is_some();
        let g = painter.layout_no_wrap(label.to_string(), crate::ui_font::semibold(13.0), Color32::PLACEHOLDER);
        let r = Rect::from_min_size(Pos2::new(x, tabs_top), Vec2::new(g.size().x + 16.0, 22.0));
        let resp = ui.interact(r, id.with(("target", label)), Sense::click());
        let checked = state.target == target;
        let colour = if !enabled {
            Palette::FG().gamma_multiply(0.4)
        } else if checked {
            painter.rect_filled(r, 4.0, Palette::MODERN_TAB_ACTIVE_BG());
            Palette::MODERN_TAB_ACTIVE_FG()
        } else if resp.hovered() {
            painter.rect_filled(r, 4.0, Palette::MODERN_TAB_HOVER_BG());
            Palette::MODERN_TAB_HOVER_FG()
        } else {
            Palette::FG().gamma_multiply(0.9)
        };
        painter.galley(Pos2::new(r.left() + 8.0, r.center().y - g.size().y / 2.0), g, colour);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, checked, label));
        if resp.clicked() && enabled && !checked {
            state.target = target;
            state.inputs.clear();
        }
        x = r.right();
    }

    // Backup and Sync Settings.
    let label = "Backup and Sync Settings";
    let g = painter.layout_no_wrap(label.to_string(), FontId::proportional(12.0), Palette::BUTTON_FG());
    let r = Rect::from_min_max(Pos2::new(inner.right() - PAD_X - g.size().x - 20.0, tabs_top), Pos2::new(inner.right() - PAD_X, tabs_top + 26.0));
    let resp = ui.interact(r, id.with("sync"), Sense::click());
    painter.rect_filled(r, 4.0, if resp.hovered() { Palette::BUTTON_HOVER() } else { Palette::BUTTON_BG() });
    painter.galley(Pos2::new(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, Palette::BUTTON_FG());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    if resp.clicked() {
        out.backup_and_sync = true;
    }
}

// ── Model → rows ──────────────────────────────────────────────────────────

fn configured_in(values: &Values<'_>, target: Target, key: &str) -> bool {
    values.of(target).is_some_and(|m| m.contains_key(key))
}

fn relayout(ui: &Ui, state: &mut SettingsEditorState, values: &Values<'_>, width: f32) {
    let key = (width.to_bits(), state.query.trim().to_string(), state.target, state.values_generation);
    if state.layout.key == key && !state.layout.rows.is_empty() {
        return;
    }
    let target = state.target;
    let configured = |k: &str| configured_in(values, target, k);
    let query = model::Query::parse(&state.query);
    let searching = !query.is_empty();
    let tree = if searching {
        model::build_tree(&|i| query.matches(i, &configured) && (!model::settings()[i].has_tag("advanced") || query.tags.iter().any(|t| t == "advanced")))
    } else {
        model::build_tree(&|i| !model::hidden_by_default(i, &configured))
    };
    let content_w = (width - 2.0 * PAD_X).min(1400.0 - 2.0 * PAD_X) - 2.0 * ITEM_PAD_X;
    let mut rows = Vec::new();
    let mut y = 0.0;
    let mut seen = HashSet::new();
    let mut push_setting = |rows: &mut Vec<Row>, y: &mut f32, index: usize, group: &str, toc: &str| {
        let h = setting_height(ui, &model::settings()[index], group, content_w);
        rows.push(Row { kind: RowKind::Setting { index, group: group.to_string(), toc: toc.to_string() }, y: *y, h });
        *y += h;
    };
    if searching {
        // Results in table-of-contents order, without group titles.
        fn walk(node: &TocNode, f: &mut dyn FnMut(usize, &str)) {
            for &s in &node.settings {
                f(s, &node.id);
            }
            for c in &node.children {
                walk(c, f);
            }
        }
        let mut found = 0;
        for c in &tree.children {
            walk(c, &mut |s, group| {
                if c.id != "commonlyUsed" && seen.insert(s) {
                    push_setting(&mut rows, &mut y, s, group, group);
                    found += 1;
                }
            });
        }
        if rows.is_empty() {
            rows.push(Row { kind: RowKind::NoResults, y: 0.0, h: 40.0 });
            y = 40.0;
        }
        state.layout.found = Some(found);
    } else {
        fn walk(node: &TocNode, level: u8, rows: &mut Vec<Row>, y: &mut f32, push: &mut dyn FnMut(&mut Vec<Row>, &mut f32, usize, &str, &str)) {
            rows.push(Row { kind: RowKind::Group { label: node.label.clone(), level, toc: node.id.clone() }, y: *y, h: GROUP_ROW });
            *y += GROUP_ROW;
            for &s in &node.settings {
                push(rows, y, s, &node.id, &node.id);
            }
            for c in &node.children {
                walk(c, (level + 1).min(3), rows, y, push);
            }
        }
        for c in &tree.children {
            walk(c, 1, &mut rows, &mut y, &mut push_setting);
        }
        state.layout.found = None;
    }
    state.layout.rows = rows;
    state.layout.total = y;
    state.layout.tree = tree;
    state.layout.key = key;
}

fn description_blocks(s: &SettingDef) -> Vec<Block> {
    let mut text = s.description.clone();
    // At most 20 lines (`MAX_DESC_LINES`).
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.len() > 20 {
        text = format!("{}\n[...]", lines[..20].join("\n"));
    }
    markdown::blocks(&text, s.markdown, markdown::Style::default())
}

fn blocks_height(ui: &Ui, blocks: &[Block], width: f32) -> f32 {
    if blocks.iter().all(|b| b.job.text.is_empty()) {
        return 0.0;
    }
    let mut h = 0.0;
    for (i, b) in blocks.iter().enumerate() {
        let mut job = b.job.clone();
        job.wrap.max_width = width - b.indent;
        let g = ui.fonts_mut(|f| f.layout_job(job));
        h += g.rows.len().max(1) as f32 * markdown::LINE_HEIGHT;
        if i + 1 < blocks.len() {
            h += markdown::PARAGRAPH_GAP;
        }
    }
    h
}

fn setting_height(ui: &Ui, s: &SettingDef, _group: &str, width: f32) -> f32 {
    let blocks = description_blocks(s);
    match s.value_type {
        ValueType::Boolean => DESC_TOP + blocks_height(ui, &blocks, width - 27.0).max(18.0) + PAD_BOTTOM_BOOL,
        vt => {
            let desc = blocks_height(ui, &blocks, width);
            let control = match vt {
                ValueType::Enum => SELECT_H,
                ValueType::String | ValueType::MultilineString | ValueType::Integer | ValueType::Number | ValueType::NullableInteger | ValueType::NullableNumber => INPUT_H,
                _ => markdown::LINE_HEIGHT,
            };
            DESC_TOP + desc + CONTROL_GAP + control + PAD_BOTTOM
        }
    }
}

// ── Table of contents ─────────────────────────────────────────────────────

fn toc(ui: &mut Ui, rect: Rect, state: &mut SettingsEditorState, id: egui::Id) {
    let painter = ui.painter().with_clip_rect(rect);
    let searching = state.layout.found.is_some();
    let counts: HashMap<String, usize> = if searching {
        let mut m = HashMap::new();
        for r in &state.layout.rows {
            if let RowKind::Setting { toc, .. } = &r.kind {
                // Count in the group and each of its parents.
                let mut path = String::new();
                for part in toc.split('/') {
                    if !path.is_empty() {
                        path.push('/');
                    }
                    path.push_str(part);
                    *m.entry(path.clone()).or_insert(0) += 1;
                }
            }
        }
        m
    } else {
        HashMap::new()
    };
    struct Entry {
        id: String,
        label: String,
        depth: usize,
        has_children: bool,
        expanded: bool,
    }
    let mut entries = Vec::new();
    fn walk(node: &TocNode, depth: usize, state: &SettingsEditorState, searching: bool, counts: &HashMap<String, usize>, out: &mut Vec<Entry>) {
        for c in &node.children {
            if searching && counts.get(&c.id).copied().unwrap_or(0) == 0 && !counts.keys().any(|k| k.starts_with(&format!("{}/", c.id))) {
                // Extension groups use their own ids: count their settings.
                if c.count() == 0 || c.id == "commonlyUsed" {
                    continue;
                }
            }
            let expanded = searching || state.toc_expanded.contains(&c.id);
            out.push(Entry { id: c.id.clone(), label: c.label.clone(), depth, has_children: !c.children.is_empty(), expanded });
            if expanded {
                walk(c, depth + 1, state, searching, counts, out);
            }
        }
    }
    walk(&state.layout.tree, 0, state, searching, &counts, &mut entries);

    let mut y = rect.top();
    let mut clicked: Option<(String, bool)> = None;
    for e in &entries {
        let row = Rect::from_min_size(Pos2::new(rect.left(), y), Vec2::new(rect.width(), TOC_ROW));
        y += TOC_ROW;
        if row.top() > rect.bottom() {
            break;
        }
        // Rows start after the 24px padding of the TOC wrapper.
        let row = Rect::from_min_max(Pos2::new(rect.left() + PAD_X, row.top()), row.max);
        let resp = ui.interact(row.intersect(rect), id.with(("toc", &e.id)), Sense::click());
        let selected = state.toc_selected.as_deref() == Some(e.id.as_str());
        if selected && state.toc_focused {
            painter.rect_stroke(row, 3.0, Stroke::new(1.0, Palette::FOCUS_BORDER()), egui::StrokeKind::Inside);
        } else if resp.hovered() {
            painter.rect_filled(row, 3.0, Palette::LIST_HOVER_BG());
        }
        let x = rect.left() + PAD_X + 8.0 * e.depth as f32;
        if e.has_children {
            let glyph = if e.expanded { "chevron-down" } else { "chevron-right" };
            painter.text(
                Pos2::new(x + 18.0, row.center().y),
                Align2::CENTER_CENTER,
                icons::codicon_by_name(glyph).unwrap_or('>').to_string(),
                icons::codicon_font(16.0),
                text_fg(),
            );
        }
        let font = if selected { crate::ui_font::bold(13.0) } else { FontId::proportional(13.0) };
        let colour = if selected { Palette::FG() } else { text_fg() };
        let g = painter.layout_no_wrap(e.label.clone(), font, colour);
        let label_x = x + 30.0;
        let gw = g.size().x;
        painter.galley(Pos2::new(label_x, row.center().y - g.size().y / 2.0 + 0.5), g, colour);
        if searching {
            let n = counts.get(&e.id).copied().unwrap_or_else(|| count_in(&state.layout.tree, &e.id));
            painter.text(Pos2::new(label_x + gw + 3.0, row.center().y), Align2::LEFT_CENTER, format!("({n})"), FontId::proportional(13.0), Palette::FG().gamma_multiply(0.8 * 0.9));
        }
        let label = format!("toc:{}", e.label);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
        if resp.clicked() {
            let on_twistie = resp.interact_pointer_pos().is_some_and(|p| p.x < x + 27.0);
            clicked = Some((e.id.clone(), on_twistie && e.has_children));
        }
    }
    if clicked.is_some() {
        state.toc_focused = true;
    } else if ui.input(|i| i.pointer.any_pressed()) && !ui.rect_contains_pointer(rect) {
        state.toc_focused = false;
    }
    if let Some((toc_id, twistie_only)) = clicked {
        let expanded = state.toc_expanded.contains(&toc_id);
        if expanded {
            state.toc_expanded.remove(&toc_id);
        } else {
            state.toc_expanded.insert(toc_id.clone());
        }
        if !twistie_only {
            state.toc_selected = Some(toc_id.clone());
            state.reveal = Some(format!("group:{toc_id}"));
        }
    }
}

fn count_in(node: &TocNode, id: &str) -> usize {
    if node.id == id {
        return node.count();
    }
    node.children.iter().map(|c| count_in(c, id)).find(|&n| n > 0).unwrap_or(0)
}

// ── Settings list ─────────────────────────────────────────────────────────

fn tree(ui: &mut Ui, rect: Rect, state: &mut SettingsEditorState, values: &Values<'_>, out: &mut SettingsEditorOutput, id: egui::Id) {
    let viewport = rect.height();
    let max_scroll = (state.layout.total - viewport).max(0.0);

    // Reveal a group (TOC click) or a setting (link).
    if let Some(target) = state.reveal.take() {
        let y = state.layout.rows.iter().find_map(|r| match &r.kind {
            RowKind::Group { toc, .. } if target == format!("group:{toc}") => Some(r.y),
            RowKind::Setting { index, .. } if target == format!("setting:{}", model::settings()[*index].key) => Some(r.y),
            _ => None,
        });
        if let Some(y) = y {
            state.scroll = y.min(max_scroll);
        } else if let Some(key) = target.strip_prefix("setting:") {
            // Not shown in the current view: search for it.
            state.query = format!("@id:{key}");
        }
    }

    let hovered = ui.rect_contains_pointer(rect) && state.dropdown.is_none();
    if hovered {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        state.scroll -= dy;
    }
    state.scroll = state.scroll.clamp(0.0, max_scroll);

    // The TOC follows the first group in view.
    if state.layout.found.is_none() {
        let top = state.scroll + 1.0;
        let current = state.layout.rows.iter().take_while(|r| r.y <= top).filter_map(|r| match &r.kind {
            RowKind::Group { toc, .. } => Some(toc.clone()),
            RowKind::Setting { toc, .. } => Some(toc.clone()),
            _ => None,
        }).last();
        if let Some(cur) = current {
            if state.toc_selected.as_deref() != Some(&cur) {
                // Reveal it: expand its parents.
                let mut path = String::new();
                for part in cur.split('/').collect::<Vec<_>>().iter().rev().skip(1).rev() {
                    if !path.is_empty() {
                        path.push('/');
                    }
                    path.push_str(part);
                    state.toc_expanded.insert(path.clone());
                }
                state.toc_selected = Some(cur);
            }
        }
    }

    let painter = ui.painter().with_clip_rect(rect);
    let content_left = (rect.left() + PAD_X).max(rect.center().x - 700.0 + PAD_X);
    let content_right = (rect.right() - PAD_X).min(rect.center().x + 700.0 - PAD_X);
    let target = state.target;
    let rows: Vec<Row> = state
        .layout
        .rows
        .iter()
        .filter(|r| r.y + r.h >= state.scroll && r.y <= state.scroll + viewport)
        .cloned()
        .collect();
    for row in rows {
        let top = rect.top() + row.y - state.scroll;
        let row_rect = Rect::from_min_max(Pos2::new(content_left, top), Pos2::new(content_right, top + row.h));
        match &row.kind {
            RowKind::NoResults => {
                painter.text(Pos2::new(rect.left() + PAD_X, top + 20.0), Align2::LEFT_TOP, "No Settings Found", FontId::proportional(13.0), Palette::FG());
            }
            RowKind::Group { label, level, .. } => {
                let size = match level {
                    1 => 26.0,
                    2 => 22.0,
                    _ => 18.0,
                };
                painter.text(
                    Pos2::new(content_left + 15.0, top + 10.0 + 9.1),
                    Align2::LEFT_CENTER,
                    label,
                    crate::ui_font::heading(size),
                    Palette::SETTINGS_HEADER_FG(),
                );
            }
            RowKind::Setting { index, group, .. } => {
                setting_row(ui, &painter, row_rect, *index, group, state, values, target, out, id, rect);
            }
        }
    }

    // Overlay scrollbar (10px).
    if max_scroll > 0.0 {
        let track = Rect::from_min_max(Pos2::new(rect.right() - 10.0, rect.top()), rect.right_bottom());
        let thumb_h = (viewport / state.layout.total * viewport).max(20.0);
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
}

#[allow(clippy::too_many_arguments)]
fn setting_row(
    ui: &mut Ui,
    painter: &egui::Painter,
    row: Rect,
    index: usize,
    group: &str,
    state: &mut SettingsEditorState,
    values: &Values<'_>,
    target: Target,
    out: &mut SettingsEditorOutput,
    id: egui::Id,
    clip: Rect,
) {
    let s = &model::settings()[index];
    let key = s.key.clone();
    let configured = configured_in(values, target, &key);
    let value = values.of(target).and_then(|m| m.get(&key)).cloned().unwrap_or_else(|| s.default.clone());
    let row_id = id.with(("row", &key));
    let hover_rect = row.intersect(clip);
    let row_resp = ui.interact(hover_rect, row_id, Sense::click());
    let focused = state.focused_row.as_deref() == Some(key.as_str());
    if focused {
        painter.rect_filled(row, 6.0, Palette::SETTINGS_FOCUSED_ROW_BG());
        painter.rect_stroke(row, 6.0, Stroke::new(1.0, Palette::SETTINGS_FOCUSED_ROW_BORDER()), egui::StrokeKind::Inside);
    } else if ui.rect_contains_pointer(hover_rect) && state.dropdown.is_none() {
        painter.rect_filled(row, 6.0, Palette::SETTINGS_ROW_HOVER_BG());
    }
    if row_resp.clicked() {
        state.focused_row = Some(key.clone());
    }
    let label_for_a11y = format!("setting:{key}");
    row_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label_for_a11y.clone()));

    // Modified indicator.
    if configured {
        let bottom = if s.value_type == ValueType::Boolean { 23.0 } else { 18.0 };
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(row.left() + 5.0, row.top() + 15.0), Pos2::new(row.left() + 7.0, row.bottom() - bottom)),
            0.0,
            Palette::SETTINGS_MODIFIED_INDICATOR(),
        );
    }

    // Gear (More Actions...) 22px left of the row.
    let gear = Rect::from_min_size(Pos2::new(row.left() - 22.0, row.top() + 8.0), Vec2::splat(22.0));
    let gear_resp = ui.interact(gear.intersect(clip), id.with(("gear", &key)), Sense::click());
    if ui.rect_contains_pointer(hover_rect) || focused || gear_resp.hovered() {
        if gear_resp.hovered() {
            painter.rect_filled(gear, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
        }
        painter.text(gear.center(), Align2::CENTER_CENTER, icons::codicon_by_name("gear").unwrap_or('*').to_string(), icons::codicon_font(16.0), Palette::FG());
    }
    if gear_resp.on_hover_text("More Actions...").clicked() {
        out.gear_menu = Some((key.clone(), gear.left_bottom(), configured));
    }

    // Title: "Category: " + label (+ Preview / Experimental badge).
    let left = row.left() + ITEM_PAD_X;
    let width = row.width() - 2.0 * ITEM_PAD_X;
    let (category, label) = model::display_format(s, group);
    let ty = row.top() + TITLE_CENTRE;
    let mut x = left;
    if !category.is_empty() {
        let g = painter.layout_no_wrap(format!("{category}: "), crate::ui_font::semibold(13.0), Palette::SETTINGS_HEADER_FG().gamma_multiply(0.9));
        let w = g.size().x;
        painter.galley(Pos2::new(x, ty - g.size().y / 2.0), g, Color32::PLACEHOLDER);
        x += w;
    }
    let g = painter.layout_no_wrap(label, crate::ui_font::semibold(13.0), Palette::SETTINGS_HEADER_FG());
    let w = g.size().x;
    painter.galley(Pos2::new(x, ty - g.size().y / 2.0), g, Color32::PLACEHOLDER);
    x += w + 7.0;
    for (tag, text) in [("preview", "Preview"), ("experimental", "Experimental")] {
        if s.has_tag(tag) {
            let g = painter.layout_no_wrap(text.into(), crate::ui_font::italic(13.0), Palette::BADGE_FG());
            let r = Rect::from_min_size(Pos2::new(x, ty - g.size().y / 2.0 - 1.0), g.size() + Vec2::new(8.0, 2.0));
            painter.rect_filled(r, 4.0, Palette::BADGE_BG());
            painter.galley(Pos2::new(r.left() + 4.0, r.top()), g, Palette::BADGE_FG());
            x = r.right() + 4.0;
        }
    }

    // Description (beside the checkbox for booleans).
    let desc_top = row.top() + DESC_TOP;
    let blocks = description_blocks(s);
    let bool_type = s.value_type == ValueType::Boolean;
    let desc_left = if bool_type { left + 27.0 } else { left };
    let desc_w = if bool_type { width - 27.0 } else { width };
    let (desc_h, link) = paint_blocks(ui, painter, &blocks, Pos2::new(desc_left, desc_top), desc_w, id.with(("desc", &key)), clip);
    if let Some(link) = link {
        match link {
            Link::Url(u) => out.open_url = Some(u),
            Link::Setting(k) => {
                state.reveal = Some(format!("setting:{k}"));
            }
        }
    }
    let control_top = desc_top + desc_h + CONTROL_GAP + 0.5;

    match s.value_type {
        ValueType::Boolean => {
            let on = value.as_bool().unwrap_or(false);
            let r = Rect::from_min_size(Pos2::new(left, desc_top + 0.5), Vec2::splat(18.0));
            let desc_area = Rect::from_min_size(Pos2::new(desc_left, desc_top), Vec2::new(desc_w, desc_h.max(18.0)));
            let resp = ui.interact(r.intersect(clip), id.with(("check", &key)), Sense::click());
            let desc_resp = ui.interact(desc_area.intersect(clip), id.with(("check-desc", &key)), Sense::click());
            painter.rect(r, 3.0, Palette::SETTINGS_CHECKBOX_BG(), Stroke::new(1.0, Palette::SETTINGS_CHECKBOX_BORDER()), egui::StrokeKind::Inside);
            if on {
                painter.text(r.center(), Align2::CENTER_CENTER, icons::codicon_by_name("check").unwrap_or('v').to_string(), icons::codicon_font(16.0), Palette::SETTINGS_CHECKBOX_FG());
            }
            let label = key.clone();
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, label.clone()));
            if resp.clicked() || desc_resp.clicked() {
                out.set = Some((target, key.clone(), Some(Value::Bool(!on))));
                state.focused_row = Some(key.clone());
            }
        }
        ValueType::Enum => {
            let r = Rect::from_min_size(Pos2::new(left, control_top), Vec2::new(320.0f32.min(width), SELECT_H));
            let resp = ui.interact(r.intersect(clip), id.with(("select", &key)), Sense::click());
            let open = state.dropdown.as_ref().is_some_and(|(k, _)| *k == key);
            let border = if open { Palette::FOCUS_BORDER() } else { Palette::SETTINGS_DROPDOWN_BORDER() };
            painter.rect(r, 4.0, Palette::SETTINGS_DROPDOWN_BG(), Stroke::new(1.0, border), egui::StrokeKind::Inside);
            let current = s.enum_values.iter().position(|v| *v == value);
            let text = current.map(|i| s.enum_label(i)).unwrap_or_else(|| display_value(&value));
            painter.text(Pos2::new(r.left() + 7.0, r.center().y), Align2::LEFT_CENTER, text, FontId::proportional(13.0), Palette::SETTINGS_DROPDOWN_FG());
            painter.text(Pos2::new(r.right() - 12.5, r.center().y), Align2::CENTER_CENTER, icons::codicon_by_name("chevron-down").unwrap_or('v').to_string(), icons::codicon_font(16.0), Palette::SETTINGS_DROPDOWN_FG());
            let label = key.clone();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, label.clone()));
            if resp.clicked() {
                state.dropdown = if open { None } else { Some((key.clone(), r)) };
                state.focused_row = Some(key.clone());
            }
            if open {
                state.dropdown = Some((key.clone(), r));
            }
        }
        ValueType::String | ValueType::MultilineString | ValueType::Integer | ValueType::Number | ValueType::NullableInteger | ValueType::NullableNumber => {
            let numeric = !matches!(s.value_type, ValueType::String | ValueType::MultilineString);
            let w = if numeric { 200.0 } else { 420.0f32.min(width) };
            let r = Rect::from_min_size(Pos2::new(left, control_top), Vec2::new(w, INPUT_H));
            let (bg, fg, border) = if numeric {
                (Palette::SETTINGS_NUMBER_INPUT_BG(), Palette::SETTINGS_NUMBER_INPUT_FG(), Palette::SETTINGS_NUMBER_INPUT_BORDER())
            } else {
                (Palette::SETTINGS_TEXT_INPUT_BG(), Palette::SETTINGS_TEXT_INPUT_FG(), Palette::SETTINGS_TEXT_INPUT_BORDER())
            };
            let slot = painter.add(egui::Shape::Noop);
            let text = state.inputs.entry(key.clone()).or_insert_with(|| display_value(&value));
            let text_rect = Rect::from_min_max(Pos2::new(r.left() + 7.0, r.top() + 1.0), Pos2::new(r.right() - 6.0, r.bottom() - 1.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(text_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
            child.set_clip_rect(text_rect.intersect(clip));
            child.visuals_mut().text_cursor.stroke.color = fg;
            let resp = child.add_sized(
                text_rect.size(),
                egui::TextEdit::singleline(text)
                    .id_salt(id.with(("input", &key)))
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::ZERO)
                    .font(FontId::proportional(13.0))
                    .text_color(fg)
                    .vertical_align(egui::Align::Center),
            );
            let error = validate(s, text);
            let stroke = if error.is_some() {
                Palette::VSCE_INPUT_ERROR_BORDER()
            } else if resp.has_focus() {
                Palette::FOCUS_BORDER()
            } else {
                border
            };
            painter.set(slot, egui::epaint::RectShape::new(r, 4.0, bg, Stroke::new(1.0, stroke), egui::StrokeKind::Inside));
            if resp.has_focus() {
                state.focused_row = Some(key.clone());
            }
            if resp.changed() && error.is_none() {
                let new = parse_value(s, text);
                if new != value || !configured {
                    out.set = Some((target, key.clone(), Some(new)));
                }
            }
            if let Some(msg) = error {
                let g = painter.layout(msg, FontId::proportional(13.0), Palette::FG(), w - 10.0);
                let br = Rect::from_min_size(Pos2::new(r.left(), r.bottom() - 1.0), Vec2::new(w, g.size().y + 10.0));
                painter.rect(br, 0.0, Palette::INPUT_VALIDATION_ERROR_BG(), Stroke::new(1.0, Palette::VSCE_INPUT_ERROR_BORDER()), egui::StrokeKind::Inside);
                painter.galley(br.min + Vec2::splat(5.0), g, Color32::PLACEHOLDER);
            }
        }
        _ => {
            // Edit in settings.json
            let g = painter.layout_no_wrap("Edit in settings.json".into(), FontId::proportional(13.0), Palette::TEXT_LINK_FG());
            let r = Rect::from_min_size(Pos2::new(left, control_top), Vec2::new(g.size().x, markdown::LINE_HEIGHT));
            let resp = ui.interact(r.intersect(clip), id.with(("json", &key)), Sense::click());
            let colour = if resp.hovered() { Palette::TEXT_LINK_ACTIVE_FG() } else { Palette::TEXT_LINK_FG() };
            let gw = g.size().x;
            painter.galley(Pos2::new(r.left(), r.center().y - g.size().y / 2.0), g, colour.gamma_multiply(0.9));
            if resp.hovered() {
                painter.hline(r.left()..=r.left() + gw, r.center().y + 7.0, Stroke::new(1.0, colour));
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, "Edit in settings.json"));
            if resp.clicked() {
                out.edit_in_json = Some((target, key.clone()));
            }
        }
    }
}

/// Paints description blocks; returns their height and a clicked link.
fn paint_blocks(ui: &mut Ui, painter: &egui::Painter, blocks: &[Block], at: Pos2, width: f32, id: egui::Id, clip: Rect) -> (f32, Option<Link>) {
    if blocks.iter().all(|b| b.job.text.is_empty()) {
        return (0.0, None);
    }
    let mut y = at.y;
    let mut clicked = None;
    for (i, b) in blocks.iter().enumerate() {
        let mut job = b.job.clone();
        job.wrap.max_width = width - b.indent;
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        // Text sits 1.5px lower in its 18px line than egui centres it.
        let origin = Pos2::new(at.x + b.indent, y + 1.5);
        let h = galley.rows.len().max(1) as f32 * markdown::LINE_HEIGHT;
        if !b.links.is_empty() {
            let area = Rect::from_min_size(origin, Vec2::new(width - b.indent, h));
            let resp = ui.interact(area.intersect(clip), id.with(i), Sense::click());
            if let Some(p) = resp.hover_pos() {
                let cursor = galley.cursor_from_pos(p - origin);
                let ci = cursor.index;
                if let Some((_, link)) = b.links.iter().find(|(r, _)| r.contains(&ci)) {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    if resp.clicked() {
                        clicked = Some(link.clone());
                    }
                }
            }
        }
        for range in &b.codes {
            paint_code_box(painter, &galley, origin, range.clone());
        }
        painter.galley(origin, galley, Color32::PLACEHOLDER);
        y += h;
        if i + 1 < blocks.len() {
            y += markdown::PARAGRAPH_GAP;
        }
    }
    (y - at.y, clicked)
}

/// The `textPreformat` box behind inline code: 1px 3px padding, 1px
/// border, radius 4, 15px tall (measured), one per row the span covers.
fn paint_code_box(painter: &egui::Painter, galley: &egui::Galley, origin: Pos2, range: std::ops::Range<usize>) {
    use egui::text::CCursor;
    let mut spans: Vec<(f32, f32, f32)> = Vec::new(); // (row centre y, min x, max x)
    for i in range.start..=range.end {
        let r = galley.pos_from_cursor(CCursor::new(i));
        let cy = r.center().y;
        match spans.iter_mut().find(|s| (s.0 - cy).abs() < 1.0) {
            Some(s) => {
                s.1 = s.1.min(r.left());
                s.2 = s.2.max(r.left());
            }
            None => spans.push((cy, r.left(), r.left())),
        }
    }
    let text: Vec<char> = galley.job.text.chars().collect();
    let mut start = range.start;
    for (cy, x0, x1) in spans {
        if x1 <= x0 {
            continue;
        }
        let r = Rect::from_min_max(Pos2::new(origin.x + x0 - 4.0, origin.y + cy - 8.5), Pos2::new(origin.x + x1 + 4.0, origin.y + cy + 6.5));
        painter.rect(r, 4.0, Palette::TEXT_PREFORMAT_BG(), Stroke::new(1.0, Palette::TEXT_PREFORMAT_BORDER()), egui::StrokeKind::Inside);
        // The characters on this row.
        let n = (start..range.end).take_while(|&i| galley.pos_from_cursor(CCursor::new(i)).center().y - cy < 1.0).count();
        let part: String = text[start.min(text.len())..(start + n).min(text.len())].iter().collect();
        start += n;
        painter.text(Pos2::new(origin.x + x0 - 1.0, r.center().y + 0.5), Align2::LEFT_CENTER, part, crate::icons::editor_mono_font(11.0), Palette::TEXT_PREFORMAT_FG());
    }
}

fn display_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// preferencesValidation.ts messages for numbers.
fn validate(s: &SettingDef, text: &str) -> Option<String> {
    let numeric = matches!(s.value_type, ValueType::Integer | ValueType::Number | ValueType::NullableInteger | ValueType::NullableNumber);
    if !numeric {
        return None;
    }
    let t = text.trim();
    if t.is_empty() {
        return if matches!(s.value_type, ValueType::NullableInteger | ValueType::NullableNumber) { None } else { Some("Value must be a number.".into()) };
    }
    let Ok(n) = t.parse::<f64>() else { return Some("Value must be a number.".into()) };
    if matches!(s.value_type, ValueType::Integer | ValueType::NullableInteger) && n.fract() != 0.0 {
        return Some("Value must be an integer.".into());
    }
    if let Some(min) = s.minimum.filter(|m| n < *m) {
        return Some(format!("Value must be greater than or equal to {}.", trim_num(min)));
    }
    if let Some(max) = s.maximum.filter(|m| n > *m) {
        return Some(format!("Value must be less than or equal to {}.", trim_num(max)));
    }
    None
}

fn trim_num(n: f64) -> String {
    if n.fract() == 0.0 { format!("{}", n as i64) } else { n.to_string() }
}

fn parse_value(s: &SettingDef, text: &str) -> Value {
    match s.value_type {
        ValueType::Integer | ValueType::Number | ValueType::NullableInteger | ValueType::NullableNumber => {
            let t = text.trim();
            if t.is_empty() {
                return Value::Null;
            }
            let n: f64 = t.parse().unwrap_or(0.0);
            if n.fract() == 0.0 && n.abs() < 9e15 { serde_json::json!(n as i64) } else { serde_json::json!(n) }
        }
        _ => Value::String(text.to_string()),
    }
}

// ── Enum dropdown (selectBoxCustom.ts) ────────────────────────────────────

fn dropdown_popup(ui: &mut Ui, state: &mut SettingsEditorState, values: &Values<'_>, out: &mut SettingsEditorOutput, id: egui::Id) {
    let Some((key, anchor)) = state.dropdown.clone() else { return };
    let Some(index) = model::find(&key) else {
        state.dropdown = None;
        return;
    };
    let s = &model::settings()[index];
    let target = state.target;
    let value = values.of(target).and_then(|m| m.get(&key)).cloned().unwrap_or_else(|| s.default.clone());
    let row_h = 22.0;
    let n = s.enum_values.len();
    let list_h = (n as f32 * row_h).min(row_h * 12.0);
    let mut close = false;
    let area = egui::Area::new(id.with("dropdown"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::new(anchor.left(), anchor.bottom()))
        .show(ui.ctx(), |ui| {
            let painter = ui.painter().clone();
            let hovered_i = ui
                .ctx()
                .pointer_hover_pos()
                .and_then(|p| {
                    let top = anchor.bottom() + 1.0;
                    (p.x >= anchor.left() && p.x <= anchor.right() && p.y >= top && p.y < top + list_h).then(|| ((p.y - top) / row_h) as usize)
                });
            let current = s.enum_values.iter().position(|v| *v == value);
            let focus_i = hovered_i.or(current);
            let detail = focus_i.and_then(|i| s.enum_description(i)).map(|d| markdown::blocks(d, s.enum_markdown, markdown::Style { size: 13.0, color: Palette::FG() }));
            let detail_h = detail.as_ref().map(|b| blocks_height(ui, b, anchor.width() - 12.0) + 12.0).unwrap_or(0.0);
            let total = Rect::from_min_size(Pos2::new(anchor.left(), anchor.bottom()), Vec2::new(anchor.width(), list_h + 2.0 + detail_h));
            painter.rect(total, 0.0, Palette::SETTINGS_DROPDOWN_BG(), Stroke::new(1.0, Palette::FOCUS_BORDER()), egui::StrokeKind::Inside);
            ui.allocate_rect(total, Sense::click());
            for i in 0..n {
                let r = Rect::from_min_size(Pos2::new(anchor.left() + 1.0, anchor.bottom() + 1.0 + i as f32 * row_h), Vec2::new(anchor.width() - 2.0, row_h));
                if r.bottom() > anchor.bottom() + 1.0 + list_h + 0.5 {
                    break;
                }
                let resp = ui.interact(r, id.with(("opt", i)), Sense::click());
                let selected = Some(i) == focus_i;
                if selected {
                    painter.rect_filled(r, 0.0, Palette::LIST_ACTIVE_SELECTION_BG());
                }
                let fg = if selected { Palette::LIST_ACTIVE_SELECTION_FG() } else { Palette::SETTINGS_DROPDOWN_FG() };
                painter.text(Pos2::new(r.left() + 6.0, r.center().y), Align2::LEFT_CENTER, s.enum_label(i), FontId::proportional(13.0), fg);
                if Some(i) == current && s.enum_values.get(i) == Some(&s.default) {
                    // VS Code marks the default value.
                }
                if s.enum_values.get(i) == Some(&s.default) {
                    painter.text(Pos2::new(r.right() - 6.0, r.center().y), Align2::RIGHT_CENTER, "default", FontId::proportional(13.0), fg.gamma_multiply(0.7));
                }
                let label = s.enum_label(i);
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("option:{label}")));
                if resp.clicked() {
                    let v = s.enum_values[i].clone();
                    out.set = Some((target, key.clone(), Some(v)));
                    close = true;
                }
            }
            if let Some(blocks) = detail {
                let top = anchor.bottom() + 1.0 + list_h;
                painter.hline(total.left() + 1.0..=total.right() - 1.0, top + 0.5, Stroke::new(1.0, Palette::SETTINGS_DROPDOWN_LIST_BORDER()));
                let mut y = top + 6.0;
                for b in &blocks {
                    let mut job = b.job.clone();
                    job.wrap.max_width = anchor.width() - 12.0;
                    let g = ui.fonts_mut(|f| f.layout_job(job));
                    let h = g.rows.len().max(1) as f32 * markdown::LINE_HEIGHT;
                    painter.galley(Pos2::new(anchor.left() + 6.0, y), g, Color32::PLACEHOLDER);
                    y += h + markdown::PARAGRAPH_GAP;
                }
            }
        });
    let clicked_outside = ui.input(|i| i.pointer.any_pressed()) && ui.ctx().pointer_interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !anchor.contains(p));
    if close || clicked_outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.dropdown = None;
    }
}

impl SettingsEditorState {
    /// Adds a filter from Filter Settings to the query.
    pub fn add_filter(&mut self, filter: &str) {
        if !self.query.is_empty() && !self.query.ends_with(' ') {
            self.query.push(' ');
        }
        self.query.push_str(filter);
        if !filter.ends_with(':') {
            self.query.push(' ');
        }
        self.focus_search = true;
        self.scroll = 0.0;
    }

    /// Forgets typed text (after a value changed elsewhere).
    pub fn reset_inputs(&mut self) {
        self.inputs.clear();
    }

    /// Scrolls to a setting and focuses it.
    pub fn reveal_setting(&mut self, key: &str) {
        self.reveal = Some(format!("setting:{key}"));
        self.focused_row = Some(key.to_string());
    }
}

