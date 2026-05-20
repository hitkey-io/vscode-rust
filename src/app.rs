use std::path::PathBuf;

use eframe::CreationContext;
use egui::{Context, Frame, Key, Margin, Modifiers, SidePanel, TopBottomPanel};

use crate::editor::Document;
use crate::fs::FileNode;
use crate::menubar::MenuIds;
use crate::search::SearchState;
use crate::theme::{self, Palette};
use crate::workbench::command_palette::{self, CommandId, CommandPaletteState};
use crate::workbench::{
    activity_bar,
    sidebar::{self, SidebarOutput},
    status_bar, tabs, ActivityView,
};

pub struct App {
    workspace_root: Option<PathBuf>,
    file_tree: Option<FileNode>,
    documents: Vec<Document>,
    active_doc: Option<usize>,
    active_view: ActivityView,
    sidebar_visible: bool,
    sidebar_width: f32,
    status_message: String,
    palette: CommandPaletteState,
    search: SearchState,
    show_welcome: bool,
    menu_ids: Option<MenuIds>,
    /// Open tab context menu: (document index, screen position where it
    /// was summoned). `None` when no menu is showing.
    tab_menu: Option<(usize, egui::Pos2)>,
}

impl App {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        crate::icons::register_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx);
        Self::fresh()
    }

    /// Bare constructor for tests / harnesses that already prepared fonts+theme
    /// against the egui Context themselves.
    pub fn for_testing(ctx: &egui::Context) -> Self {
        crate::icons::register_fonts(ctx);
        theme::apply(ctx);
        Self::fresh()
    }

    fn fresh() -> Self {
        Self {
            workspace_root: None,
            file_tree: None,
            documents: Vec::new(),
            active_doc: None,
            active_view: ActivityView::Explorer,
            sidebar_visible: true,
            sidebar_width: 260.0,
            status_message: String::new(),
            palette: CommandPaletteState::default(),
            search: SearchState::default(),
            show_welcome: true,
            menu_ids: None,
            tab_menu: None,
        }
    }

    pub fn attach_menu_ids(&mut self, ids: MenuIds) {
        self.menu_ids = Some(ids);
    }

    fn dispatch_menu_event(&mut self, ctx: &Context, id: &muda::MenuId) {
        let Some(ids) = self.menu_ids.clone() else { return };
        let cmd = if id == &ids.open_folder {
            Some(CommandId::OpenFolder)
        } else if id == &ids.open_file {
            Some(CommandId::OpenFile)
        } else if id == &ids.save {
            Some(CommandId::Save)
        } else if id == &ids.save_all {
            Some(CommandId::SaveAll)
        } else if id == &ids.close_editor {
            Some(CommandId::CloseFile)
        } else if id == &ids.close_all {
            Some(CommandId::CloseAllFiles)
        } else if id == &ids.palette {
            self.palette.open();
            None
        } else if id == &ids.toggle_sidebar {
            Some(CommandId::ToggleSidebar)
        } else if id == &ids.show_explorer {
            Some(CommandId::ShowExplorer)
        } else if id == &ids.show_search {
            Some(CommandId::ShowSearch)
        } else if id == &ids.welcome {
            self.show_welcome = true;
            None
        } else {
            None
        };
        if let Some(c) = cmd {
            self.execute_command(ctx, c);
        }
    }

    pub fn bootstrap_workspace(&mut self, path: PathBuf) {
        self.set_workspace(path);
    }

    pub fn bootstrap_open_file(&mut self, path: PathBuf) {
        self.open_file(path);
        self.show_welcome = false;
    }

    /// Force-hide the Welcome tab on launch (used by `--no-welcome` CLI flag).
    pub fn bootstrap_hide_welcome(&mut self) {
        self.show_welcome = false;
    }

    /// Pre-fill the Search view with a query and run it once on startup.
    pub fn bootstrap_search(&mut self, query: String) {
        self.show_view(ActivityView::Search);
        self.search.query = query;
        if let Some(root) = &self.workspace_root {
            let q = crate::search::engine::SearchQuery {
                text: self.search.query.clone(),
                match_case: self.search.match_case,
                whole_word: self.search.whole_word,
                regex: self.search.regex,
            };
            self.search.outcome = Some(crate::search::engine::run(root, &q));
        }
    }

    fn open_folder_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.set_workspace(path);
        }
    }

    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_file() {
            self.open_file(path);
        }
    }

    fn set_workspace(&mut self, path: PathBuf) {
        self.workspace_root = Some(path.clone());
        self.file_tree = Some(FileNode::root(path.clone()));
        self.status_message = format!("Opened folder: {}", path.display());
    }

    fn open_file(&mut self, path: PathBuf) {
        if let Some(idx) = self.documents.iter().position(|d| d.path == path) {
            self.active_doc = Some(idx);
            return;
        }
        match Document::open(path.clone()) {
            Ok(doc) => {
                self.documents.push(doc);
                self.active_doc = Some(self.documents.len() - 1);
                self.status_message = format!("Opened {}", path.display());
            }
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                // Binary file — silently refuse, like VS Code's untrusted/binary
                // editor flow. Don't pollute the status bar with raw UTF-8 errors.
                self.status_message =
                    format!("Cannot show binary file: {}", path.display());
            }
            Err(e) => {
                self.status_message = format!("Failed to open {}: {}", path.display(), e);
            }
        }
    }

    fn close_doc(&mut self, idx: usize) {
        if idx >= self.documents.len() {
            return;
        }
        self.documents.remove(idx);
        if self.documents.is_empty() {
            self.active_doc = None;
        } else {
            let new_active = match self.active_doc {
                Some(a) if a == idx => idx.saturating_sub(1).min(self.documents.len() - 1),
                Some(a) if a > idx => a - 1,
                other => other.unwrap_or(0).min(self.documents.len() - 1),
            };
            self.active_doc = Some(new_active);
        }
    }

    fn close_active(&mut self) {
        if let Some(idx) = self.active_doc {
            self.close_doc(idx);
        }
    }

    fn close_all(&mut self) {
        self.documents.clear();
        self.active_doc = None;
        self.status_message = "Closed all editors".into();
    }

    /// Close every editor except `keep` (pinned tabs are never closed).
    fn close_others(&mut self, keep: usize) {
        let Some(keep_path) = self.documents.get(keep).map(|d| d.path.clone()) else {
            return;
        };
        self.documents
            .retain(|d| d.pinned || d.path == keep_path);
        self.active_doc = self
            .documents
            .iter()
            .position(|d| d.path == keep_path)
            .or(if self.documents.is_empty() { None } else { Some(0) });
    }

    /// Render the per-tab right-click context menu, if one is open.
    fn show_tab_context_menu(&mut self, ctx: &Context) {
        use crate::vscode_widgets::composite::{context_menu, ContextMenuItem, ContextMenuProps};

        let Some((idx, pos)) = self.tab_menu else {
            return;
        };
        if idx >= self.documents.len() {
            self.tab_menu = None;
            return;
        }
        let pinned = self.documents[idx].pinned;

        let items = [
            ContextMenuItem::new("Close").shortcut("⌘W"),
            ContextMenuItem::new("Close Others"),
            ContextMenuItem::new("Close All"),
            ContextMenuItem::separator(),
            ContextMenuItem::new(if pinned { "Unpin" } else { "Pin" }),
        ];

        let mut chosen: Option<usize> = None;
        let area = egui::Area::new(egui::Id::new("tab_context_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                let resp = context_menu(ui, &ContextMenuProps::default(), &items);
                chosen = resp.selected;
            });

        // Dismiss when an item is chosen or the user clicks elsewhere /
        // presses Escape.
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if let Some(sel) = chosen {
            match sel {
                0 => self.close_doc(idx),
                1 => self.close_others(idx),
                2 => self.close_all(),
                4 => {
                    if let Some(doc) = self.documents.get_mut(idx) {
                        doc.pinned = !doc.pinned;
                    }
                }
                _ => {}
            }
            self.tab_menu = None;
        } else if escape || area.response.clicked_elsewhere() {
            self.tab_menu = None;
        }
    }

    fn save_active(&mut self) {
        if let Some(idx) = self.active_doc {
            if let Some(doc) = self.documents.get_mut(idx) {
                match doc.save() {
                    Ok(()) => {
                        self.status_message = format!("Saved {}", doc.path.display());
                    }
                    Err(e) => {
                        self.status_message = format!("Save failed: {}", e);
                    }
                }
            }
        }
    }

    fn save_all(&mut self) {
        let mut saved = 0usize;
        let mut errs = 0usize;
        for d in self.documents.iter_mut() {
            if d.dirty {
                match d.save() {
                    Ok(()) => saved += 1,
                    Err(_) => errs += 1,
                }
            }
        }
        self.status_message = if errs > 0 {
            format!("Saved {} file(s), {} errors", saved, errs)
        } else {
            format!("Saved {} file(s)", saved)
        };
    }

    fn show_view(&mut self, view: ActivityView) {
        self.active_view = view;
        self.sidebar_visible = true;
        if view == ActivityView::Search {
            self.search.focus_input = true;
        }
    }

    fn navigate_to(&mut self, path: PathBuf, line: usize, byte_in_line: usize) {
        if let Some(idx) = self.documents.iter().position(|d| d.path == path) {
            self.active_doc = Some(idx);
        } else {
            match Document::open(path.clone()) {
                Ok(doc) => {
                    self.documents.push(doc);
                    self.active_doc = Some(self.documents.len() - 1);
                }
                Err(e) => {
                    self.status_message = format!("Failed to open {}: {}", path.display(), e);
                    return;
                }
            }
        }
        if let Some(idx) = self.active_doc {
            if let Some(doc) = self.documents.get_mut(idx) {
                doc.pending_nav = Some((line, byte_in_line));
            }
        }
    }

    fn execute_command(&mut self, ctx: &Context, cmd: CommandId) {
        match cmd {
            CommandId::OpenFolder => self.open_folder_dialog(),
            CommandId::OpenFile => self.open_file_dialog(),
            CommandId::Save => self.save_active(),
            CommandId::SaveAll => self.save_all(),
            CommandId::CloseFile => self.close_active(),
            CommandId::CloseAllFiles => self.close_all(),
            CommandId::ToggleSidebar => self.sidebar_visible = !self.sidebar_visible,
            CommandId::ShowExplorer => self.show_view(ActivityView::Explorer),
            CommandId::ShowSearch => self.show_view(ActivityView::Search),
            CommandId::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    fn title_bar(&mut self, ctx: &Context) {
        let bar_height = 35.0;
        // On macOS we leave space for native traffic lights on the left.
        #[cfg(target_os = "macos")]
        let left_inset = 78.0;
        #[cfg(not(target_os = "macos"))]
        let left_inset = 8.0;

        TopBottomPanel::top("title_bar")
            .exact_height(bar_height)
            .frame(
                Frame::default()
                    .fill(Palette::TITLE_BAR_BG)
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                // bottom border
                let bottom_border = egui::Rect::from_min_size(
                    egui::pos2(rect.left(), rect.bottom() - 1.0),
                    egui::vec2(rect.width(), 1.0),
                );
                ui.painter().rect_filled(bottom_border, 0.0, Palette::BORDER);

                // Centered title
                let title = match (&self.workspace_root, self.active_doc.and_then(|i| self.documents.get(i))) {
                    (Some(root), Some(doc)) => {
                        let dirty = if doc.dirty { " ●" } else { "" };
                        format!(
                            "{}{} — {} — vscode-rust",
                            doc.display_name(),
                            dirty,
                            root.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
                        )
                    }
                    (Some(root), None) => {
                        format!(
                            "{} — vscode-rust",
                            root.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
                        )
                    }
                    (None, Some(doc)) => {
                        let dirty = if doc.dirty { " ●" } else { "" };
                        format!("{}{} — vscode-rust", doc.display_name(), dirty)
                    }
                    _ => "vscode-rust".to_string(),
                };
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    title,
                    egui::FontId::proportional(13.0),
                    Palette::TITLE_BAR_FG,
                );

                // VS Code on macOS uses the native system menu bar (NSMenu) instead of an
                // in-window one. Only render the in-window menu bar on non-macOS platforms,
                // mirroring vscode/src/.../titlebarPart.ts: `hasMenubar = !(!isWeb && isMacintosh)`.
                #[cfg(not(target_os = "macos"))]
                ui.allocate_ui_at_rect(
                    egui::Rect::from_min_size(
                        egui::pos2(rect.left() + left_inset, rect.top()),
                        egui::vec2(rect.width() / 2.0 - left_inset, bar_height),
                    ),
                    |ui| {
                        ui.horizontal_centered(|ui| {
                            let v = &mut ui.style_mut().visuals.widgets;
                            v.inactive.bg_stroke = egui::Stroke::NONE;
                            v.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                            v.inactive.bg_fill = egui::Color32::TRANSPARENT;
                            v.hovered.bg_stroke = egui::Stroke::NONE;
                            v.hovered.weak_bg_fill = Palette::LIST_HOVER_BG;
                            v.hovered.bg_fill = Palette::LIST_HOVER_BG;
                            v.active.bg_stroke = egui::Stroke::NONE;
                            v.open.bg_stroke = egui::Stroke::NONE;
                            v.open.weak_bg_fill = Palette::LIST_HOVER_BG;
                            ui.spacing_mut().item_spacing.x = 0.0;
                            ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);

                            self.menu_file(ui, ctx);
                            self.menu_edit(ui);
                            self.menu_view(ui);
                            self.menu_help(ui);
                        });
                    },
                );

                let _ = left_inset; // silence unused on macOS
            });
    }

    fn menu_file(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        let resp = ui.menu_button(menu_label("File"), |ui| {
            if ui.button("Open Folder…").clicked() {
                self.open_folder_dialog();
                ui.close_menu();
            }
            if ui.button("Open File…").clicked() {
                self.open_file_dialog();
                ui.close_menu();
            }
            ui.separator();
            let save_enabled = self.active_doc.is_some();
            if ui
                .add_enabled(save_enabled, egui::Button::new("Save              ⌘S"))
                .clicked()
            {
                self.save_active();
                ui.close_menu();
            }
            if ui
                .add_enabled(
                    !self.documents.is_empty(),
                    egui::Button::new("Save All        ⌥⌘S"),
                )
                .clicked()
            {
                self.save_all();
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Close Editor      ⌘W").clicked() {
                self.close_active();
                ui.close_menu();
            }
            if ui.button("Close All Editors").clicked() {
                self.close_all();
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Quit              ⌘Q").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        let _ = resp;
    }

    fn menu_edit(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(menu_label("Edit"), |ui| {
            ui.label("Undo / Redo (built-in)");
            ui.separator();
            if ui.button("Find in Files     ⇧⌘F").clicked() {
                self.show_view(ActivityView::Search);
                ui.close_menu();
            }
        });
    }

    fn menu_view(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(menu_label("View"), |ui| {
            if ui.button("Command Palette…  ⇧⌘P").clicked() {
                self.palette.open();
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Toggle Sidebar    ⌘B").clicked() {
                self.sidebar_visible = !self.sidebar_visible;
                ui.close_menu();
            }
            if ui.button("Explorer          ⇧⌘E").clicked() {
                self.show_view(ActivityView::Explorer);
                ui.close_menu();
            }
            if ui.button("Search            ⇧⌘F").clicked() {
                self.show_view(ActivityView::Search);
                ui.close_menu();
            }
        });
    }

    fn menu_help(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(menu_label("Help"), |ui| {
            ui.label("Welcome");
            ui.separator();
            ui.label("Visual Studio Code (Rust port)");
            ui.label("v0.1.0");
        });
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        // Cmd+Shift+P always toggles palette, even when it's open
        let toggle_palette = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::P,
            ))
        });
        if toggle_palette {
            self.palette.toggle();
            return;
        }

        // While the palette owns input, skip other shortcuts
        if self.palette.visible {
            return;
        }

        let save = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::S))
        });
        if save {
            self.save_active();
        }

        let save_all = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::ALT,
                Key::S,
            ))
        });
        if save_all {
            self.save_all();
        }

        let toggle_sidebar = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::B))
        });
        if toggle_sidebar {
            self.sidebar_visible = !self.sidebar_visible;
        }

        let close_editor = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::W))
        });
        if close_editor {
            self.close_active();
        }

        let show_search = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::F,
            ))
        });
        if show_search {
            self.show_view(ActivityView::Search);
        }

        let show_explorer = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::E,
            ))
        });
        if show_explorer {
            self.show_view(ActivityView::Explorer);
        }
    }
}

impl App {
    /// Frame-agnostic entry point used both by `eframe::App::update` and by
    /// in-process test harnesses (egui_kittest) that don't have a real
    /// `eframe::Frame` instance.
    pub fn render(&mut self, ctx: &Context) {
        self.render_impl(ctx);
    }

    fn render_impl(&mut self, ctx: &Context) {
        // Drain any native-menu events from the previous frame.
        while let Some(ev) = crate::menubar::poll_event() {
            self.dispatch_menu_event(ctx, &ev.id);
        }

        self.handle_shortcuts(ctx);

        self.title_bar(ctx);

        TopBottomPanel::bottom("status_bar")
            .exact_height(22.0)
            .frame(
                Frame::default()
                    .fill(Palette::STATUS_BAR_BG)
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                let active = self.active_doc.and_then(|i| self.documents.get(i));
                let has_workspace = self.workspace_root.is_some();
                status_bar::show(ui, active, &self.status_message, has_workspace);
            });

        SidePanel::left("activity_bar")
            .exact_width(48.0)
            .resizable(false)
            .frame(
                Frame::default()
                    .fill(Palette::ACTIVITY_BAR_BG)
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                activity_bar::show(ui, &mut self.active_view, &mut self.sidebar_visible);
            });

        if self.sidebar_visible {
            // Use a panel-id keyed by the active view so each view has its own width
            // memory; this way switching Explorer↔Search doesn't carry over a stale
            // resize state which previously stretched the sidebar to fill the window.
            let panel_id = match self.active_view {
                ActivityView::Explorer => "sidebar_explorer",
                ActivityView::Search => "sidebar_search",
            };
            let sidebar_output: SidebarOutput;
            {
                let resp = SidePanel::left(panel_id)
                    .resizable(true)
                    .width_range(170.0..=420.0)
                    .default_width(self.sidebar_width)
                    .frame(
                        Frame::default()
                            .fill(Palette::SIDEBAR_BG)
                            .inner_margin(Margin::ZERO),
                    )
                    .show(ctx, |ui| {
                        sidebar::show(
                            ui,
                            self.active_view,
                            &self.workspace_root,
                            &mut self.file_tree,
                            &mut self.search,
                        )
                    });
                sidebar_output = resp.inner;
                self.sidebar_width = resp.response.rect.width().clamp(170.0, 420.0);
            }

            if sidebar_output.open_folder_requested {
                self.open_folder_dialog();
            }
            if let Some(path) = sidebar_output.file_to_open {
                self.open_file(path);
            }
            if let Some((path, line, col)) = sidebar_output.navigate_to {
                self.navigate_to(path, line, col);
            }
        }

        let mut welcome_pending_folder = false;
        let mut welcome_pending_file = false;

        egui::CentralPanel::default()
            .frame(
                Frame::default()
                    .fill(Palette::EDITOR_BG)
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    let show_welcome_tab =
                        self.show_welcome && self.documents.is_empty();
                    let has_any_tab = !self.documents.is_empty() || show_welcome_tab;

                    if has_any_tab {
                        let tabs_action = ui
                            .allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), 35.0),
                                egui::Layout::left_to_right(egui::Align::Min),
                                |ui| {
                                    tabs::show(
                                        ui,
                                        &self.documents,
                                        self.active_doc,
                                        show_welcome_tab,
                                    )
                                },
                            )
                            .inner;

                        if let Some(idx) = tabs_action.activate {
                            self.active_doc = Some(idx);
                        }
                        if let Some(idx) = tabs_action.close {
                            self.close_doc(idx);
                        }
                        if let Some(idx) = tabs_action.toggle_pin {
                            if let Some(doc) = self.documents.get_mut(idx) {
                                doc.pinned = !doc.pinned;
                            }
                        }
                        if let Some(idx) = tabs_action.right_clicked {
                            let pos = ctx
                                .pointer_interact_pos()
                                .unwrap_or_else(|| egui::pos2(0.0, 0.0));
                            self.tab_menu = Some((idx, pos));
                        }
                        if tabs_action.close_welcome {
                            self.show_welcome = false;
                        }

                        self.show_tab_context_menu(ctx);
                    }

                    if let Some(idx) = self.active_doc {
                        if let Some(doc) = self.documents.get_mut(idx) {
                            crate::editor::view::show(ui, doc);
                        }
                    } else if self.show_welcome {
                        let act = welcome_screen(ui);
                        if act.open_folder {
                            welcome_pending_folder = true;
                        }
                        if act.open_file {
                            welcome_pending_file = true;
                        }
                        if act.open_palette {
                            self.palette.open();
                        }
                    } else {
                        empty_editor_hints(ui);
                    }
                });
            });

        if welcome_pending_folder {
            self.open_folder_dialog();
        }
        if welcome_pending_file {
            self.open_file_dialog();
        }

        if let Some(cmd) = command_palette::show(ctx, &mut self.palette) {
            self.execute_command(ctx, cmd);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // UI is registered via panels in `update`, not via the root Ui.
    }

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.render(ctx);
    }
}

fn menu_label(label: &str) -> egui::RichText {
    egui::RichText::new(label)
        .size(13.0)
        .color(Palette::TITLE_BAR_FG)
}

fn title_icon(ui: &mut egui::Ui, glyph: char, tip: &str) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, Palette::LIST_HOVER_BG);
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph.to_string(),
        crate::icons::codicon_font(15.0),
        Palette::TITLE_BAR_FG,
    );
    resp.on_hover_text(tip)
}

pub fn empty_editor_hints(ui: &mut egui::Ui) {
    let rect = ui.max_rect();
    let painter = ui.painter();

    // Empty-editor watermark: the stylised VS Code "<" angle-bracket logo.
    // Source: codicon `vscode` (0xEC29) in vscode-original/src/vs/base/common/codiconsLibrary.ts.
    let watermark_size = (rect.height() * 0.5).min(rect.width() * 0.4);
    let cx = rect.center().x;
    let cy = rect.center().y - 60.0;
    painter.text(
        egui::pos2(cx, cy),
        egui::Align2::CENTER_CENTER,
        crate::icons::VSCODE.to_string(),
        crate::icons::codicon_font(watermark_size),
        // VS Code paints it at ~12% alpha black on dark editor.bg, which our
        // tint approximates as faint white. Real source uses opacity:0.3 in CSS.
        egui::Color32::from_white_alpha(22),
    );

    // Keyboard shortcut hints centered below the watermark
    let hints: &[(&str, &str)] = &[
        ("Show All Commands", "⇧⌘P"),
        ("Open File or Folder", "⌘O"),
        ("Toggle Sidebar", "⌘B"),
        ("Save", "⌘S"),
        ("Save All", "⌥⌘S"),
    ];

    let label_w = 220.0_f32;
    let shortcut_w = 110.0_f32;
    let row_h = 22.0;
    let total_w = label_w + shortcut_w;
    let start_y = cy + watermark_size * 0.5;
    for (i, (label, shortcut)) in hints.iter().enumerate() {
        let y = start_y + i as f32 * row_h;
        painter.text(
            egui::pos2(cx - total_w / 2.0 + label_w, y),
            egui::Align2::RIGHT_CENTER,
            *label,
            egui::FontId::proportional(13.0),
            Palette::FG_DESCRIPTION,
        );
        painter.text(
            egui::pos2(cx - total_w / 2.0 + label_w + 40.0, y),
            egui::Align2::LEFT_CENTER,
            *shortcut,
            egui::FontId::proportional(13.0),
            Palette::FG_DESCRIPTION,
        );
    }
}

pub struct WelcomeAction {
    pub open_folder: bool,
    pub open_file: bool,
    pub open_palette: bool,
}

pub fn welcome_screen(ui: &mut egui::Ui) -> WelcomeAction {
    let mut action = WelcomeAction {
        open_folder: false,
        open_file: false,
        open_palette: false,
    };

    ui.add_space(72.0);
    ui.horizontal(|ui| {
        let total = ui.available_width();
        let content_w = 820.0_f32.min(total - 40.0);
        let side_pad = ((total - content_w) / 2.0).max(20.0);
        ui.add_space(side_pad);

        ui.vertical(|ui| {
            ui.set_width(content_w);

            ui.label(
                egui::RichText::new("Visual Studio Code")
                    .size(36.0)
                    .color(Palette::FG),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Editing evolved")
                    .size(18.0)
                    .color(Palette::FG_DESCRIPTION),
            );

            ui.add_space(36.0);

            let col_w = (content_w - 32.0) / 2.0;
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(col_w, 240.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.label(
                            egui::RichText::new("Start")
                                .size(20.0)
                                .color(Palette::FG)
                                .strong(),
                        );
                        ui.add_space(8.0);
                        if welcome_link(ui, "Open File…").clicked() {
                            action.open_file = true;
                        }
                        if welcome_link(ui, "Open Folder…").clicked() {
                            action.open_folder = true;
                        }
                        if welcome_link(ui, "Show All Commands  ⇧⌘P").clicked() {
                            action.open_palette = true;
                        }
                    },
                );

                ui.add_space(32.0);

                ui.allocate_ui_with_layout(
                    egui::vec2(col_w, 240.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.label(
                            egui::RichText::new("Recent")
                                .size(20.0)
                                .color(Palette::FG)
                                .strong(),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("No recent folders")
                                .color(Palette::FG_DESCRIPTION),
                        );
                    },
                );
            });
        });
    });

    action
}

fn welcome_link(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let parts: Vec<&str> = text.splitn(2, "  ").collect();
    let label = parts[0];
    let shortcut = parts.get(1).copied().unwrap_or("");

    let row_height = 26.0;
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), row_height),
        egui::Sense::click(),
    );
    let painter = ui.painter();
    let color = if resp.hovered() {
        Palette::FG_BRIGHT
    } else {
        Palette::ACCENT
    };
    painter.text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        color,
    );
    if !shortcut.is_empty() {
        painter.text(
            rect.left_center() + egui::vec2(label.len() as f32 * 7.5 + 16.0, 0.0),
            egui::Align2::LEFT_CENTER,
            shortcut,
            egui::FontId::proportional(12.0),
            Palette::FG_DESCRIPTION,
        );
    }
    resp
}
