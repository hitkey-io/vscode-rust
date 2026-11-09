//! Quick Access widget: the command palette (`>` prefix, ⇧⌘P) and Quick Open
//! file search (no prefix, ⌘P) share one popup, exactly like VS Code where
//! the mode is chosen by the first character of the input.
//!
//! VS Code analogues:
//! - `src/vs/platform/quickinput/browser/quickInput.ts` (popup chrome)
//! - `src/vs/workbench/contrib/quickaccess/browser/commandsQuickAccess.ts`
//! - `src/vs/workbench/contrib/search/browser/anythingQuickAccess.ts`
//! - `src/vs/workbench/contrib/codeEditor/browser/quickaccess/gotoLineQuickAccess.ts`
//!
//! Tokens: `--vscode-list-highlightForeground` (matched characters),
//! `--vscode-descriptionForeground` (folder path / group labels).

use std::path::{Path, PathBuf};

use egui::{
    text::LayoutJob, Color32, Context, CornerRadius, FontId, Frame, Key, Margin, Modifiers, Order,
    Pos2, Sense, Stroke, StrokeKind, TextEdit, TextFormat, Ui,
};

use crate::theme::Palette;
use crate::vscode_widgets::primitives::{label, LabelProps};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum CommandId {
    OpenFolder,
    OpenFile,
    CloseFolder,
    Save,
    SaveAll,
    CloseFile,
    CloseAllFiles,
    ToggleSidebar,
    ShowExplorer,
    ShowSearch,
    QuickOpen,
    Quit,
}

pub struct CommandEntry {
    pub id: CommandId,
    pub title: &'static str,
    /// Empty for commands VS Code lists without a category prefix
    /// (e.g. "Go to File…").
    pub category: &'static str,
    pub shortcut: Option<&'static str>,
}

impl CommandEntry {
    /// The row text, `"{Category}: {Title}"` or just the title.
    pub fn label(&self) -> String {
        if self.category.is_empty() {
            self.title.to_string()
        } else {
            format!("{}: {}", self.category, self.title)
        }
    }
}

pub const COMMANDS: &[CommandEntry] = &[
    CommandEntry { id: CommandId::OpenFolder,    title: "Open Folder…",            category: "File", shortcut: None },
    CommandEntry { id: CommandId::OpenFile,      title: "Open File…",              category: "File", shortcut: None },
    CommandEntry { id: CommandId::CloseFolder,   title: "Close Folder",            category: "File", shortcut: Some("⌘K F") },
    CommandEntry { id: CommandId::Save,          title: "Save",                    category: "File", shortcut: Some("⌘S") },
    CommandEntry { id: CommandId::SaveAll,       title: "Save All",                category: "File", shortcut: Some("⌥⌘S") },
    CommandEntry { id: CommandId::CloseFile,     title: "Close Editor",            category: "File", shortcut: Some("⌘W") },
    CommandEntry { id: CommandId::CloseAllFiles, title: "Close All Editors",       category: "File", shortcut: None },
    CommandEntry { id: CommandId::ToggleSidebar, title: "Toggle Sidebar Visibility", category: "View", shortcut: Some("⌘B") },
    CommandEntry { id: CommandId::ShowExplorer,  title: "Show Explorer",           category: "View", shortcut: Some("⇧⌘E") },
    CommandEntry { id: CommandId::ShowSearch,    title: "Show Search",             category: "View", shortcut: Some("⇧⌘F") },
    CommandEntry { id: CommandId::QuickOpen,     title: "Go to File…",             category: "",     shortcut: Some("⌘P") },
    CommandEntry { id: CommandId::Quit,          title: "Quit",                    category: "File", shortcut: Some("⌘Q") },
];

/// What the user picked in the Quick Access popup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteAction {
    /// A command-palette entry (`>` mode).
    Command(CommandId),
    /// A Quick Open file result, optionally with a `:line` suffix.
    OpenFile { path: PathBuf, line: Option<usize> },
    /// `:N` with no file name — go to line N in the active editor.
    GoToLine(usize),
    /// The row's "Remove from Recently Opened" button: drop the file from
    /// the editor history. The popup stays open.
    RemoveRecent(PathBuf),
}

/// Upper bound on files indexed for Quick Open. Keeps the walk bounded on
/// very large trees.
const QUICK_OPEN_FILE_LIMIT: usize = 20_000;
/// Upper bound on scored file results kept per keystroke.
const MAX_FILE_RESULTS: usize = 200;

const PLACEHOLDER_COMMANDS: &str = "Type the name of a command to run.";
const PLACEHOLDER_FILES: &str =
    "Search files by name (append : to go to line or @ to go to symbol)";

#[derive(Default)]
pub struct CommandPaletteState {
    pub visible: bool,
    pub query: String,
    pub selected: usize,
    just_opened: bool,
    /// First visible row of the result list (keeps the selection in view).
    scroll_top: usize,
    /// Sub-row remainder of mouse-wheel scrolling, in points.
    scroll_accum: f32,
    /// Workspace root for relative paths in Quick Open descriptions.
    root: Option<PathBuf>,
    /// Quick Open file index, rebuilt every time ⌘P opens the popup.
    files: Vec<PathBuf>,
    /// Editor history, most recent first ("recently opened" group).
    recent: Vec<PathBuf>,
}

impl CommandPaletteState {
    /// Open in command mode (⇧⌘P).
    pub fn open(&mut self) {
        self.visible = true;
        // Pre-fill with ">" so the activator is visible in the input the way
        // VS Code does it when you press Cmd+Shift+P.
        self.query = ">".to_string();
        self.selected = 0;
        self.scroll_top = 0;
        self.just_opened = true;
    }

    /// Open in Quick Open file mode (⌘P). `recent` is the editor history,
    /// most recent first.
    pub fn open_quick_open(&mut self, root: Option<&Path>, recent: &[PathBuf]) {
        self.visible = true;
        self.query.clear();
        self.selected = 0;
        self.scroll_top = 0;
        self.just_opened = true;
        self.root = root.map(Path::to_path_buf);
        self.files = root
            .map(|r| crate::search::engine::list_files(r, QUICK_OPEN_FILE_LIMIT))
            .unwrap_or_default();
        self.recent = recent.iter().filter(|p| p.is_file()).cloned().collect();
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn toggle(&mut self) {
        if self.visible {
            self.close();
        } else {
            self.open();
        }
    }

    /// `>` prefix → command palette; anything else → Quick Open.
    pub fn is_command_mode(&self) -> bool {
        self.query.starts_with('>')
    }

    /// Repeated ⌘P while Quick Open is showing moves the selection down
    /// (VS Code's "quick navigate").
    pub fn select_next(&mut self) {
        self.selected = self.selected.saturating_add(1);
    }
}

// =============================================================================
// Result items
// =============================================================================

#[derive(Clone, Debug)]
enum ItemKind {
    Command(usize),
    File { path: PathBuf, line: Option<usize> },
    GoToLine(usize),
}

#[derive(Clone, Debug)]
struct Item {
    kind: ItemKind,
    label: String,
    /// Char indices of `label` to paint in the highlight colour.
    label_hl: Vec<usize>,
    description: String,
    desc_hl: Vec<usize>,
    /// Group label painted on the right of the first row of a group
    /// ("recently opened", "file results").
    separator: Option<&'static str>,
    /// Editor-history entry: gets the "Remove from Recently Opened" button.
    from_history: bool,
}

impl Item {
    fn action(&self) -> PaletteAction {
        match &self.kind {
            ItemKind::Command(i) => PaletteAction::Command(COMMANDS[*i].id),
            ItemKind::File { path, line } => PaletteAction::OpenFile {
                path: path.clone(),
                line: *line,
            },
            ItemKind::GoToLine(n) => PaletteAction::GoToLine(*n),
        }
    }

    fn accessible_label(&self) -> String {
        if self.description.is_empty() {
            self.label.clone()
        } else {
            format!("{}, {}", self.label, self.description)
        }
    }
}

/// What to show when there are no items.
fn empty_message(state: &CommandPaletteState) -> Option<&'static str> {
    if state.is_command_mode() {
        return Some("No matching commands");
    }
    let q: String = state.query.chars().filter(|c| !c.is_whitespace()).collect();
    if q.is_empty() {
        // Empty Quick Open with no history: just the input box.
        return None;
    }
    if q == ":" {
        return Some("Type a line number to navigate to.");
    }
    Some("No matching results")
}

fn build_items(state: &CommandPaletteState) -> Vec<Item> {
    if state.is_command_mode() {
        command_items(&state.query)
    } else {
        file_items(state)
    }
}

fn command_items(query: &str) -> Vec<Item> {
    // Strip the leading ">" activator before fuzzy matching — it's the mode
    // marker for the command palette, not part of the search term.
    let q = query.trim_start_matches('>').trim().to_lowercase();
    let mut scored: Vec<(i32, usize, Vec<usize>)> = COMMANDS
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            if q.is_empty() {
                return Some((0, i, Vec::new()));
            }
            fuzzy_match(&c.label().to_lowercase(), &q).map(|(s, hl)| (s, i, hl))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .map(|(_, i, hl)| Item {
            kind: ItemKind::Command(i),
            label: COMMANDS[i].label(),
            label_hl: hl,
            description: String::new(),
            desc_hl: Vec::new(),
            separator: None,
            from_history: false,
        })
        .collect()
}

/// Split a Quick Open query into the file-name part and an optional
/// `:line` suffix (`"main.rs:42"` → `("main.rs", Some(42))`).
pub fn split_line_suffix(query: &str) -> (String, Option<usize>) {
    let q: String = query.chars().filter(|c| !c.is_whitespace()).collect();
    if let Some(pos) = q.rfind(':') {
        let tail = &q[pos + 1..];
        if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
            return (q[..pos].to_string(), tail.parse().ok());
        }
        if tail.is_empty() {
            return (q[..pos].to_string(), None);
        }
    }
    (q, None)
}

/// `(name, parent folder relative to root)` for a Quick Open row.
fn name_and_dir(path: &Path, root: Option<&Path>) -> (String, String) {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parent = path.parent().unwrap_or(Path::new(""));
    let dir = match root.and_then(|r| parent.strip_prefix(r).ok()) {
        Some(rel) => rel.to_string_lossy().replace('\\', "/"),
        None => parent.to_string_lossy().into_owned(),
    };
    (name, dir)
}

/// Score one file against `needle` (already lowercased, no whitespace).
/// Name matches outrank path matches, mirroring VS Code's label-first
/// scorer. Returns `(score, label_hl, desc_hl)`.
fn score_file(name: &str, dir: &str, needle: &str) -> Option<(i32, Vec<usize>, Vec<usize>)> {
    if let Some((s, hl)) = fuzzy_match(&name.to_lowercase(), needle) {
        return Some((s + 10_000, hl, Vec::new()));
    }
    if dir.is_empty() {
        return None;
    }
    let full = format!("{}/{}", dir, name).to_lowercase();
    let (s, hl) = fuzzy_match(&full, needle)?;
    let dir_len = dir.chars().count();
    let mut label_hl = Vec::new();
    let mut desc_hl = Vec::new();
    for p in hl {
        if p < dir_len {
            desc_hl.push(p);
        } else if p > dir_len {
            label_hl.push(p - dir_len - 1);
        }
    }
    Some((s, label_hl, desc_hl))
}

fn file_items(state: &CommandPaletteState) -> Vec<Item> {
    let root = state.root.as_deref();
    let (needle_raw, line) = split_line_suffix(&state.query);
    let needle = needle_raw.to_lowercase();

    if needle.is_empty() {
        if let Some(n) = line {
            return vec![Item {
                kind: ItemKind::GoToLine(n),
                label: format!("Go to line {n}."),
                label_hl: Vec::new(),
                description: String::new(),
                desc_hl: Vec::new(),
                separator: None,
                from_history: false,
            }];
        }
        if state.query.contains(':') {
            return Vec::new();
        }
        // Empty input: editor history only.
        return state
            .recent
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let (name, dir) = name_and_dir(p, root);
                Item {
                    kind: ItemKind::File { path: p.clone(), line: None },
                    label: name,
                    label_hl: Vec::new(),
                    description: dir,
                    desc_hl: Vec::new(),
                    separator: (i == 0).then_some("recently opened"),
                    from_history: true,
                }
            })
            .collect();
    }

    let mut items = Vec::new();

    // Group 1: matching editor-history entries, in history order.
    for p in &state.recent {
        let (name, dir) = name_and_dir(p, root);
        if let Some((_, label_hl, desc_hl)) = score_file(&name, &dir, &needle) {
            items.push(Item {
                kind: ItemKind::File { path: p.clone(), line },
                label: name,
                label_hl,
                description: dir,
                desc_hl,
                separator: items.is_empty().then_some("recently opened"),
                from_history: true,
            });
        }
    }
    let recent_count = items.len();

    // Group 2: workspace files, best score first.
    let mut scored: Vec<(i32, usize, Item)> = Vec::new();
    for p in &state.files {
        if state.recent.contains(p) {
            continue;
        }
        let (name, dir) = name_and_dir(p, root);
        if let Some((score, label_hl, desc_hl)) = score_file(&name, &dir, &needle) {
            let depth = dir.chars().count() + name.chars().count();
            scored.push((
                score,
                depth,
                Item {
                    kind: ItemKind::File { path: p.clone(), line },
                    label: name,
                    label_hl,
                    description: dir,
                    desc_hl,
                    separator: None,
                    from_history: false,
                },
            ));
        }
    }
    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(a.1.cmp(&b.1))
            .then_with(|| a.2.label.cmp(&b.2.label))
    });
    items.extend(scored.into_iter().take(MAX_FILE_RESULTS).map(|(_, _, it)| it));
    if items.len() > recent_count {
        items[recent_count].separator = Some("file results");
    }
    items
}

// =============================================================================
// Rendering
// =============================================================================

pub fn show(ctx: &Context, state: &mut CommandPaletteState) -> Option<PaletteAction> {
    if !state.visible {
        return None;
    }

    let items = build_items(state);
    if state.selected >= items.len() {
        state.selected = items.len().saturating_sub(1);
    }

    // Handle nav keys BEFORE the TextEdit consumes them.
    let mut chosen: Option<PaletteAction> = None;
    ctx.input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::Escape) {
            state.visible = false;
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowDown) && !items.is_empty() {
            state.selected = (state.selected + 1).min(items.len() - 1);
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            state.selected = state.selected.saturating_sub(1);
        }
        if i.consume_key(Modifiers::NONE, Key::Enter) {
            if let Some(item) = items.get(state.selected) {
                chosen = Some(item.action());
                state.visible = false;
            }
        }
    });

    if !state.visible {
        return chosen;
    }

    // Keep the selected row inside the visible window.
    if state.selected < state.scroll_top {
        state.scroll_top = state.selected;
    } else if state.selected >= state.scroll_top + MAX_VISIBLE_ROWS {
        state.scroll_top = state.selected + 1 - MAX_VISIBLE_ROWS;
    }
    state.scroll_top = state
        .scroll_top
        .min(items.len().saturating_sub(MAX_VISIBLE_ROWS));

    // Snapshot the just_opened flag now: palette_input (inside the area)
    // consumes it to grab focus, but the click-outside guard below must still
    // see it (otherwise the click that opened the palette gets re-evaluated as
    // "outside" and closes it immediately).
    let was_just_opened = state.just_opened;
    // With the command center in the title bar, VS Code lays the popup over
    // the title bar, 6px from the top of the window (layout.ts quickPickTop).
    // `left = (width - 600) / 2` for the 600px content box; the 1px border
    // sits outside it, so the popup is 602px wide and starts at that left.
    let content = ctx.content_rect();
    let left = content.left() + ((content.width() - POPUP_W) / 2.0).round();
    let area = egui::Area::new(egui::Id::new("command_palette_area"))
        .order(Order::Foreground)
        .fixed_pos(egui::pos2(left, content.top() + POPUP_TOP))
        .show(ctx, |ui| {
            Frame::default()
                .fill(Palette::VSCE_QUICK_INPUT_BG)
                .stroke(Stroke::new(1.0, Palette::VSCE_WIDGET_BORDER))
                // --vscode-cornerRadius-xLarge
                .corner_radius(12)
                // --vscode-shadow-xl: 0 0 20px rgba(0, 0, 0, 0.15)
                .shadow(egui::epaint::Shadow {
                    offset: [0, 0],
                    blur: 20,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(38),
                })
                .inner_margin(Margin::ZERO)
                .show(ui, |ui| {
                    ui.set_width(POPUP_W);
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                    palette_input(ui, state);
                    palette_list(ui, &items, state, &mut chosen);
                });
        });

    if !was_just_opened && area.response.clicked_elsewhere() {
        state.visible = false;
    }

    chosen
}

/// Popup top edge, measured from the top of the window.
const POPUP_TOP: f32 = 6.0;
/// `.quick-input-widget { width: 600px }`
const POPUP_W: f32 = 600.0;
/// `.quick-input-header { padding: 6px 6px 4px 6px }`
const HEADER_PAD: f32 = 6.0;
/// Input box height including its 1px border (13px text, 4px padding).
const INPUT_H: f32 = 25.0;
/// Space between the input box and the first row.
const LIST_GAP: f32 = 6.0;
/// `.quick-input-list .monaco-list { padding-bottom: 7px }`
const LIST_PAD_BOTTOM: f32 = 7.0;
/// `.quick-input-list .monaco-scrollable-element { padding: 0 6px }`
const LIST_PAD_X: f32 = 6.0;
const ROW_H: f32 = 22.0;
/// `.quick-input-list-entry { padding: 0 6px }`
const ENTRY_PAD_X: f32 = 6.0;
const MAX_VISIBLE_ROWS: usize = 16;
/// `.quick-input-list-rows { margin-left: 5px }`
const ROWS_MARGIN: f32 = 5.0;
/// File icon column (`.monaco-icon-label::before`): 16px + 6px padding-right.
const ICON_W: f32 = 16.0;
const ICON_PAD: f32 = 6.0;
/// Seti glyph size: the icon theme draws at 150% of 13px; egui sizes fonts
/// by line height, so the matching size is larger than 19.5.
const ICON_FONT: f32 = 20.0;
/// Row action button: 16px codicon, 2px padding, 4px margin-right.
const ACTION_BOX: f32 = 20.0;
const ACTION_MARGIN: f32 = 4.0;
/// `.quick-input-list-entry-action-bar { margin-right: 4px }`
const ACTION_BAR_MARGIN: f32 = 4.0;
/// `.quick-input-list-separator { margin-right: 4px }`
const SEPARATOR_MARGIN: f32 = 4.0;

fn palette_input(ui: &mut Ui, state: &mut CommandPaletteState) {
    let (header, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), HEADER_PAD + INPUT_H),
        Sense::hover(),
    );
    let box_rect = egui::Rect::from_min_size(
        egui::pos2(header.left() + HEADER_PAD, header.top() + HEADER_PAD),
        egui::vec2(header.width() - HEADER_PAD * 2.0, INPUT_H),
    );
    // Reserve the box's paint slot first so it lands behind the text.
    let box_slot = ui.painter().add(egui::Shape::Noop);

    // The text sits 6px inside the 1px border (`.input { padding: 4px 6px }`).
    let text_rect = box_rect.shrink2(egui::vec2(7.0, 1.0));

    let placeholder = if state.is_command_mode() {
        PLACEHOLDER_COMMANDS
    } else {
        PLACEHOLDER_FILES
    };
    let query_before = state.query.clone();
    let mut focused = false;
    // A child Ui, so the text field doesn't move the parent's cursor back
    // up to its own (shorter) bottom edge.
    {
        let ui = &mut ui.new_child(
            egui::UiBuilder::new().max_rect(text_rect).layout(*ui.layout()),
        );
        {
            // The caret uses the input foreground, as in VS Code.
            ui.visuals_mut().text_cursor.stroke.color = Palette::VSCE_QUICK_INPUT_BOX_FG;
            // egui paints hint text in the weak text colour, whatever the
            // hint's own colour says.
            ui.visuals_mut().weak_text_color = Some(Palette::VSCE_QUICK_INPUT_PLACEHOLDER_FG);
            let edit = TextEdit::singleline(&mut state.query)
                .id_salt("quick_access_input")
                .frame(Frame::NONE)
                .margin(Margin::ZERO)
                .desired_width(text_rect.width())
                .hint_text(placeholder)
                .font(FontId::proportional(13.0))
                .text_color(Palette::VSCE_QUICK_INPUT_BOX_FG)
                .vertical_align(egui::Align::Center);
            let resp = ui.add_sized(text_rect.size(), edit);
            if state.just_opened {
                resp.request_focus();
                state.just_opened = false;
                focused = true;
            }
            focused |= resp.has_focus();
        }
    }

    // Input background with a 1px border that turns into the focus border
    // while the input owns the keyboard.
    let border = if focused {
        Palette::VSCE_QUICK_INPUT_FOCUS_BORDER
    } else {
        Palette::INPUT_BORDER
    };
    ui.painter().set(
        box_slot,
        egui::epaint::RectShape::new(
            box_rect,
            CornerRadius::same(6),
            Palette::VSCE_QUICK_INPUT_BOX_BG,
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        ),
    );

    if state.query != query_before {
        state.selected = 0;
        state.scroll_top = 0;
    }
}

fn palette_list(
    ui: &mut Ui,
    items: &[Item],
    state: &mut CommandPaletteState,
    chosen: &mut Option<PaletteAction>,
) {
    if items.is_empty() {
        match empty_message(state) {
            Some(msg) => {
                ui.add_space(LIST_GAP);
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), ROW_H),
                    Sense::hover(),
                );
                ui.painter().text(
                    egui::pos2(rect.left() + LIST_PAD_X + ENTRY_PAD_X, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    msg,
                    FontId::proportional(13.0),
                    Palette::FG,
                );
                ui.add_space(LIST_PAD_BOTTOM);
            }
            // Just the input box: the header keeps its 4px bottom padding.
            None => ui.add_space(4.0),
        }
        return;
    }

    let top = state.scroll_top.min(items.len().saturating_sub(1));
    let end = (top + MAX_VISIBLE_ROWS).min(items.len());
    ui.add_space(LIST_GAP);
    let list_top = ui.cursor().top();
    let mut removed: Option<PathBuf> = None;
    for (row, item) in items.iter().enumerate().take(end).skip(top) {
        let is_selected = row == state.selected;

        let (slot, resp) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::click());
        let rect = slot.shrink2(egui::vec2(LIST_PAD_X, 0.0));
        let a11y = item.accessible_label();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, a11y.clone()));

        // Row actions show on the selected row and on hover (anythingQuickAccess:
        // "Remove from Recently Opened" for editor-history entries).
        let mut action_clicked = false;
        let mut action_rect = None;
        if item.from_history && (is_selected || resp.hovered()) {
            let right = rect.right() - ENTRY_PAD_X - ACTION_BAR_MARGIN - ACTION_MARGIN;
            let r = egui::Rect::from_center_size(
                egui::pos2(right - ACTION_BOX / 2.0, rect.center().y + 1.0),
                egui::vec2(ACTION_BOX, ACTION_BOX),
            );
            let btn = ui
                .interact(r, resp.id.with("remove_recent"), Sense::click())
                .on_hover_text("Remove from Recently Opened");
            btn.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Remove from Recently Opened")
            });
            action_clicked = btn.clicked();
            action_rect = Some((r, btn.hovered()));
        }

        paint_row(ui, rect, item, is_selected, resp.hovered(), row > top, action_rect);

        if action_clicked {
            if let ItemKind::File { path, .. } = &item.kind {
                removed = Some(path.clone());
            }
        } else if resp.clicked() {
            *chosen = Some(item.action());
            state.visible = false;
        }
    }

    // Mouse-wheel scrolling over the list, one row per ROW_H of delta.
    let list_rect = egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), list_top..=ui.cursor().top());
    if ui.rect_contains_pointer(list_rect) && items.len() > MAX_VISIBLE_ROWS {
        state.scroll_accum += ui.input(|i| i.smooth_scroll_delta.y);
        let rows = (state.scroll_accum / ROW_H).trunc();
        if rows != 0.0 {
            state.scroll_accum -= rows * ROW_H;
            let max_top = items.len() - MAX_VISIBLE_ROWS;
            state.scroll_top =
                (state.scroll_top as isize - rows as isize).clamp(0, max_top as isize) as usize;
            // The wheel scrolls the view only; keep the selection inside it.
            state.selected = state.selected.clamp(state.scroll_top, state.scroll_top + MAX_VISIBLE_ROWS - 1);
            ui.ctx().request_repaint();
        }
    }

    if items.len() > end {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.add_space(LIST_PAD_X + ENTRY_PAD_X);
            let more = format!("… and {} more", items.len() - end);
            label(ui, &LabelProps::new(&more).normal().description().size(11.5));
        });
    }
    ui.add_space(LIST_PAD_BOTTOM);

    if let Some(path) = removed {
        state.recent.retain(|p| p != &path);
        *chosen = Some(PaletteAction::RemoveRecent(path));
        ui.ctx().request_repaint();
    }
}

fn paint_row(
    ui: &Ui,
    rect: egui::Rect,
    item: &Item,
    is_selected: bool,
    hovered: bool,
    not_first: bool,
    action: Option<(egui::Rect, bool)>,
) {
    let painter = ui.painter();

    // Selection / hover fill (`.monaco-list-row { border-radius: 3px }`).
    if is_selected {
        painter.rect_filled(rect, 3.0, Palette::VSCE_QUICK_INPUT_LIST_FOCUS_BG);
    } else if hovered {
        painter.rect_filled(rect, 3.0, Palette::VSCE_QUICK_INPUT_LIST_HOVER_BG);
    }

    // Group separator: 1px top border on the first entry of every group after
    // the first one, hidden while that row is focused.
    if item.separator.is_some() && not_first && !is_selected {
        painter.rect_filled(
            egui::Rect::from_min_size(rect.left_top(), egui::vec2(rect.width(), 1.0)),
            0.0,
            Palette::VSCE_PICKER_GROUP_BORDER,
        );
    }

    let mut x = rect.left() + ENTRY_PAD_X + ROWS_MARGIN;
    let cy = rect.center().y;

    // Selected rows switch every text run to the focus foreground; the
    // description is `.label-description { opacity: .7; font-size: .9em }`.
    let (fg, hl_fg) = if is_selected {
        (
            Palette::VSCE_QUICK_INPUT_LIST_FOCUS_FG,
            Palette::VSCE_QUICK_INPUT_LIST_FOCUS_HIGHLIGHT_FG,
        )
    } else {
        (Palette::FG, Palette::VSCE_LIST_HIGHLIGHT_FG)
    };
    let desc_fg = fg.gamma_multiply(0.7);

    // File rows: Seti icon in a 16px column in front of the name.
    if let ItemKind::File { path, .. } = &item.kind {
        let (glyph, color, font) = match crate::file_icons::icon_for(path) {
            Some((g, c)) => (g, c, crate::file_icons::seti_font(ICON_FONT)),
            None => (
                crate::icons::FILE,
                Palette::FG,
                FontId::new(14.0, egui::FontFamily::Name(crate::icons::CODICON_FAMILY.into())),
            ),
        };
        // The glyph is wider than its 16px column and overflows to the right.
        painter.text(
            egui::pos2(x + ICON_W / 2.0 + 1.5, cy),
            egui::Align2::CENTER_CENTER,
            glyph.to_string(),
            font,
            color,
        );
        x += ICON_W + ICON_PAD;
    }

    x += paint_highlighted(ui, egui::pos2(x, cy), &item.label, &item.label_hl, 13.0, fg, hl_fg);

    if !item.description.is_empty() {
        // margin-left: 0.5em
        x += 6.0;
        paint_highlighted(ui, egui::pos2(x, cy), &item.description, &item.desc_hl, 11.7, desc_fg, hl_fg);
    }

    // Right side: row action button, then keybinding chips (commands) or the
    // group label (files).
    let mut right = rect.right() - ENTRY_PAD_X - ACTION_BAR_MARGIN;
    if let Some((r, btn_hovered)) = action {
        if btn_hovered {
            painter.rect_filled(r, 5.0, Palette::VSCE_QUICK_INPUT_ACTION_HOVER_BG);
        }
        painter.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            crate::icons::CLOSE.to_string(),
            FontId::new(16.0, egui::FontFamily::Name(crate::icons::CODICON_FAMILY.into())),
            fg,
        );
        right = r.left();
    }
    if let ItemKind::Command(i) = item.kind {
        if let Some(sc) = COMMANDS[i].shortcut {
            // `.quick-input-list-entry-keybinding { margin-right: 8px }`, left
            // of the (empty) row action bar and its 4px margin.
            paint_kbd(ui, egui::pos2(rect.right() - ENTRY_PAD_X - ACTION_BAR_MARGIN - 8.0, cy), sc);
        }
    } else if let Some(sep) = item.separator {
        painter.text(
            egui::pos2(right - SEPARATOR_MARGIN, cy),
            egui::Align2::RIGHT_CENTER,
            sep,
            FontId::proportional(11.0),
            if is_selected { fg } else { Palette::VSCE_QUICK_INPUT_SEPARATOR_FG },
        );
    }
}

/// Paint `text` left-aligned at `pos` (vertically centred), with the chars
/// at `hl` in `hl_color` and bold (`.highlight { font-weight: bold }`).
/// Returns the painted width.
fn paint_highlighted(
    ui: &Ui,
    pos: Pos2,
    text: &str,
    hl: &[usize],
    size: f32,
    color: Color32,
    hl_color: Color32,
) -> f32 {
    let job = highlighted_job(text, hl, FontId::proportional(size), color, hl_color);
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let w = galley.size().x;
    let top = egui::pos2(pos.x, pos.y - galley.size().y / 2.0);
    ui.painter().galley(top, galley, color);
    w
}

/// Lay out `text`, painting the chars at `hl` (char indices) in `hl_color`
/// (`--vscode-list-highlightForeground`, or the focus foreground on the
/// selected row).
fn highlighted_job(
    text: &str,
    hl: &[usize],
    font: FontId,
    color: Color32,
    hl_color: Color32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let plain = TextFormat { font_id: font.clone(), color, ..Default::default() };
    let marked = TextFormat {
        font_id: crate::ui_font::bold(font.size),
        color: hl_color,
        ..Default::default()
    };
    let mut run = String::new();
    let mut run_hl = false;
    for (i, ch) in text.chars().enumerate() {
        let is_hl = hl.contains(&i);
        if is_hl != run_hl && !run.is_empty() {
            job.append(&run, 0.0, if run_hl { marked.clone() } else { plain.clone() });
            run.clear();
        }
        run_hl = is_hl;
        run.push(ch);
    }
    if !run.is_empty() {
        job.append(&run, 0.0, if run_hl { marked } else { plain });
    }
    job
}

/// Paint a shortcut string as keybinding-label chips, right-aligned to
/// `anchor`. Each non-space character becomes its own key (so `"⌘K F"` →
/// `[⌘][K]  [F]`). Metrics from `keybindingLabel.css`: 11px text, 3px 5px
/// padding, 1px border with a heavier bottom edge, 3px radius, 2px margin
/// on each side of a key, 6px between chords. Returns the width painted.
fn paint_kbd(ui: &Ui, anchor: Pos2, text: &str) -> f32 {
    let font = FontId::proportional(11.0);
    let p = ui.painter();
    let key_h: f32 = 10.0 + 3.0 * 2.0 + 2.0;
    let pad_x: f32 = 5.0;
    let key_gap: f32 = 4.0;
    let chord_gap: f32 = 6.0;
    let fg = Palette::VSCE_KEYBINDING_LABEL_FG;

    // Chords, each a list of (key text, key width).
    let mut chords: Vec<Vec<(String, f32)>> = vec![Vec::new()];
    for ch in text.chars() {
        if ch == ' ' {
            chords.push(Vec::new());
            continue;
        }
        let s = ch.to_string();
        let glyph_w = p.layout_no_wrap(s.clone(), font.clone(), fg).size().x;
        let key_w = glyph_w.max(12.0) + pad_x * 2.0 + 2.0;
        chords.last_mut().unwrap().push((s, key_w));
    }
    chords.retain(|c| !c.is_empty());
    let chord_w = |c: &Vec<(String, f32)>| {
        c.iter().map(|(_, w)| *w).sum::<f32>() + key_gap * c.len().saturating_sub(1) as f32
    };
    let total: f32 = chords.iter().map(chord_w).sum::<f32>()
        + chord_gap * chords.len().saturating_sub(1) as f32;

    let mut x = anchor.x - total;
    let y_top = (anchor.y - key_h / 2.0).round();
    for (ci, chord) in chords.iter().enumerate() {
        if ci > 0 {
            x += chord_gap;
        }
        for (ki, (s, w)) in chord.iter().enumerate() {
            if ki > 0 {
                x += key_gap;
            }
            let key = egui::Rect::from_min_size(egui::pos2(x, y_top), egui::vec2(*w, key_h));
            p.rect(
                key,
                CornerRadius::same(3),
                Palette::VSCE_KEYBINDING_LABEL_BG,
                Stroke::new(1.0, Palette::VSCE_KEYBINDING_LABEL_BORDER),
                StrokeKind::Inside,
            );
            // Heavier bottom edge (`border-bottom-color`).
            p.hline(
                (key.left() + 2.0)..=(key.right() - 2.0),
                key.bottom() - 0.5,
                Stroke::new(1.0, Palette::VSCE_KEYBINDING_LABEL_BOTTOM_BORDER),
            );
            p.text(key.center(), egui::Align2::CENTER_CENTER, s, font.clone(), fg);
            x += w;
        }
    }
    total
}

// =============================================================================
// Fuzzy matching
// =============================================================================

/// Match `needle` against `haystack` (both lowercase). Returns a score and
/// the matched char indices. A contiguous substring hit always beats a
/// scattered subsequence; word-start hits score higher.
fn fuzzy_match(haystack: &str, needle: &str) -> Option<(i32, Vec<usize>)> {
    if needle.is_empty() {
        return Some((0, Vec::new()));
    }
    let h: Vec<char> = haystack.chars().collect();
    let n: Vec<char> = needle.chars().collect();

    // Contiguous substring: prefer an occurrence at a word start.
    let starts: Vec<usize> = (0..h.len().saturating_sub(n.len() - 1))
        .filter(|&s| h[s..s + n.len()] == n[..])
        .collect();
    if let Some(&start) = starts
        .iter()
        .find(|&&s| s == 0 || !h[s - 1].is_alphanumeric())
        .or(starts.first())
    {
        let boundary = start == 0 || !h[start - 1].is_alphanumeric();
        let score = 1000 + n.len() as i32 * 10 - start as i32 + if boundary { 100 } else { 0 };
        return Some((score, (start..start + n.len()).collect()));
    }

    // Scattered subsequence (greedy, leftmost).
    let mut positions = Vec::with_capacity(n.len());
    let mut ni = 0usize;
    let mut score = 0i32;
    let mut consecutive = 0i32;
    for (hi, &hc) in h.iter().enumerate() {
        if ni == n.len() {
            break;
        }
        if hc == n[ni] {
            let mut delta = 10 + consecutive * 5;
            if hi == 0 || !h[hi - 1].is_alphanumeric() {
                delta += 15;
            }
            score += delta;
            consecutive += 1;
            positions.push(hi);
            ni += 1;
        } else {
            consecutive = 0;
            score -= 1;
        }
    }
    (ni == n.len()).then_some((score, positions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_suffix_is_split_off() {
        assert_eq!(split_line_suffix("main.rs:42"), ("main.rs".into(), Some(42)));
        assert_eq!(split_line_suffix(":7"), (String::new(), Some(7)));
        assert_eq!(split_line_suffix("main.rs:"), ("main.rs".into(), None));
        assert_eq!(split_line_suffix("site header"), ("siteheader".into(), None));
    }

    #[test]
    fn substring_beats_subsequence() {
        let (a, hl) = fuzzy_match("siteheader.vue", "head").unwrap();
        assert_eq!(hl, vec![4, 5, 6, 7]);
        let (b, _) = fuzzy_match("shared.ts", "shd").unwrap();
        assert!(a > b);
        assert!(fuzzy_match("index.vue", "xyz").is_none());
    }

    #[test]
    fn name_match_outranks_path_match() {
        let (by_name, _, _) = score_file("pages.ts", "src", "pages").unwrap();
        let (by_path, label_hl, desc_hl) = score_file("index.vue", "pages", "pages").unwrap();
        assert!(by_name > by_path);
        assert!(label_hl.is_empty());
        assert_eq!(desc_hl, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn path_match_splits_highlights_between_dir_and_name() {
        // "components/SiteHeader.vue" matched by "comp/site".
        let (_, label_hl, desc_hl) = score_file("SiteHeader.vue", "components", "comp/site").unwrap();
        assert_eq!(desc_hl, vec![0, 1, 2, 3]);
        assert_eq!(label_hl, vec![0, 1, 2, 3]);
    }
}
