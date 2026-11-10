use std::path::{Path, PathBuf};

use eframe::CreationContext;
use egui::{Context, Frame, Key, Margin, Modifiers, SidePanel, TopBottomPanel};

use crate::editor::Document;
use crate::fs::FileNode;
use crate::menubar::MenuIds;
use crate::search::SearchState;
use crate::theme::{self, Palette};
use crate::workbench::command_palette::{self, CommandId, CommandPaletteState, PaletteAction};
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
    /// All repositories discovered in the workspace (root + nested).
    git_model: crate::git::Model,
    /// Commit-graph rows for the active repository (the GRAPH section).
    git_history: Vec<crate::git::GraphRow>,
    /// Source Control view UI state (commit messages, expanded repos, graph).
    scm_ui: crate::workbench::source_control::ScmUiState,
    /// Per-line diff decorations for the active document (vs. HEAD), keyed by
    /// 1-based line number. Recomputed on save / file switch.
    git_line_changes: std::collections::BTreeMap<usize, crate::git::DiffKind>,
    /// Explorer git decorations: absolute path → change kind (union of repos).
    git_decorations: std::collections::BTreeMap<PathBuf, crate::git::ChangeKind>,
    /// Filesystem watcher for the workspace; sets a dirty flag the update loop
    /// polls to auto-refresh git status.
    git_watcher: Option<crate::git::Watcher>,
    /// HEAD text of the active document, cached for live (as-you-type) gutter
    /// diffing without spawning `git`.
    git_head_blob: Option<String>,
    /// The working text last diffed against `git_head_blob`; lets the update
    /// loop detect edits and recompute the gutter only when the text changed.
    git_diff_snapshot: String,
    /// Open branch-picker popup: `(repo_root, anchor)`.
    branch_picker: Option<(PathBuf, egui::Pos2)>,
    /// Open Commit-mode dropdown: `(repo_root, anchor)`.
    commit_menu: Option<(PathBuf, egui::Pos2)>,
    /// Explorer OUTLINE / TIMELINE section expanded state.
    outline_expanded: bool,
    timeline_expanded: bool,
    /// In-editor find widget (Cmd+F) state.
    find: crate::editor::find::FindState,
    /// When the current status message was set. VS Code status messages are
    /// transient; ours expire after a few seconds (see STATUS_TTL).
    status_message_at: Option<std::time::Instant>,
    /// Editor history, most recently active first. Feeds Quick Open's
    /// "recently opened" group.
    recent_files: Vec<PathBuf>,
    /// Active editor last recorded in `recent_files`; history only changes
    /// when the active editor changes, so a file removed from "recently
    /// opened" stays out until it is activated again.
    recent_tracked: Option<PathBuf>,
    /// Installed VS Code extensions and their Node.js host.
    extensions: crate::extensions::ExtensionService,
    /// Extensions were loaded but the host hasn't been started (it starts on
    /// the first frame, once there's a context to wake).
    extensions_pending_start: bool,
    /// The marketplace may be contacted (set by the real app, not tests);
    /// it connects the first time the Extensions view opens.
    marketplace_allowed: bool,
    /// Extension language features in the editor.
    lang: LanguageState,
    /// Native web views for extension webviews.
    webview_host: crate::workbench::webviews::WebviewHost,
    /// The extension container showing when the view is
    /// `ActivityView::ExtensionContainer`.
    ext_container: Option<String>,
    /// Open extension pages (editor tabs).
    ext_pages: Vec<ExtensionPage>,
    next_internal_handle: u64,
    /// The native menu an extension page opened: item id → (extension,
    /// action).
    ext_menu_actions: Vec<(muda::MenuId, String, ExtPageAction)>,
    /// An open version picker: (request, extension, versions, download).
    ext_pick: Option<(u64, String, Vec<String>, bool)>,
    /// Text to put on the clipboard on the next frame.
    pending_copy: Option<String>,
    /// Height of the window's content, for placing native menus.
    viewport_height: f32,
}

/// An extension's page in an editor tab.
struct ExtensionPage {
    handle: u64,
    /// `publisher.name`.
    id: String,
    /// Set when it was opened from the catalogue.
    gallery: Option<crate::extensions::marketplace::GalleryExtension>,
    /// What's read from the installed copy's folder.
    local: Option<LocalFiles>,
    /// What the page shows now (rebuilt only when this changes).
    shown: Option<crate::workbench::extension_details::DetailsData>,
    /// Open tab and feature, as the page last reported them.
    tab: String,
    feature: Option<String>,
}

/// README, CHANGELOG, install time and sizes of an installed extension,
/// read once per installed folder.
struct LocalFiles {
    path: PathBuf,
    readme: Option<String>,
    changelog: Option<String>,
    installed_ms: Option<i64>,
    size: u64,
    /// Its global storage: (bytes, folder).
    cache: Option<(u64, String)>,
    /// `package.json` with `package.nls.json` applied.
    manifest: serde_json::Value,
}

impl LocalFiles {
    fn read(dir: &std::path::Path, id: &str, manifest: &serde_json::Value) -> Self {
        let read = |names: &[&str]| -> Option<String> {
            std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
                let n = e.file_name().to_string_lossy().to_lowercase();
                names.contains(&n.as_str()).then(|| std::fs::read_to_string(e.path()).ok()).flatten()
            })
        };
        let size_of = |p: &std::path::Path| -> u64 {
            walkdir::WalkDir::new(p).into_iter().flatten().filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum()
        };
        let installed_ms = std::fs::metadata(dir)
            .ok()
            .and_then(|m| m.created().or_else(|_| m.modified()).ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64);
        let cache = std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|h| h.join(".vscode-rust/extension-storage/global").join(id))
            .filter(|p| p.is_dir())
            .map(|p| (size_of(&p), p.display().to_string()))
            .filter(|(n, _)| *n > 0);
        Self {
            path: dir.to_path_buf(),
            readme: read(&["readme.md", "readme.markdown", "readme.txt", "readme"]),
            changelog: read(&["changelog.md", "changelog.markdown", "changelog.txt", "changelog"]),
            installed_ms,
            size: size_of(dir),
            cache,
            manifest: crate::extensions::manifest::localized_manifest(manifest, dir),
        }
    }
}

/// What a button or menu item on an extension page does.
#[derive(Clone, Debug)]
enum ExtPageAction {
    Install,
    Update,
    InstallVersion(String),
    Uninstall,
    /// `true`: only in this workspace.
    Enable(bool),
    Disable(bool),
    PickVersion,
    PickDownload,
    Copy,
    CopyId,
    CopyLink,
    Recommend(bool),
    Download,
}

/// Request ids of the editor's own version pickers (far above the
/// extension host's).
const EXT_PICK_BASE: u64 = 1 << 50;

/// Completion, hover and go-to-definition requests and what they show.
#[derive(Default)]
struct LanguageState {
    suggest: Option<crate::workbench::language_ui::Suggest>,
    /// Completion request in flight: (request, path, word start byte).
    pending_completion: Option<(u64, PathBuf, usize)>,
    pending_definition: Option<u64>,
    /// Hover: the character under the pointer and since when.
    hover_track: Option<(usize, std::time::Instant, egui::Pos2)>,
    /// Hover request in flight: (request, char, pointer).
    pending_hover: Option<(u64, usize, egui::Pos2)>,
    /// Hover card on screen: (byte range it covers, pointer, markdown blocks).
    hover: Option<((usize, usize), egui::Pos2, Vec<String>)>,
    caret: Option<egui::Rect>,
    caret_char: Option<usize>,
}

/// How long a transient status-bar message stays visible.
const STATUS_TTL: std::time::Duration = std::time::Duration::from_secs(5);

/// How many entries the editor history keeps.
const RECENT_FILES_MAX: usize = 50;

impl App {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        crate::icons::register_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx);
        crate::menubar::repaint_on_event(&cc.egui_ctx);
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
            git_model: crate::git::Model::default(),
            git_history: Vec::new(),
            scm_ui: crate::workbench::source_control::ScmUiState::default(),
            git_line_changes: std::collections::BTreeMap::new(),
            git_decorations: std::collections::BTreeMap::new(),
            git_watcher: None,
            git_head_blob: None,
            git_diff_snapshot: String::new(),
            branch_picker: None,
            commit_menu: None,
            outline_expanded: false,
            timeline_expanded: false,
            find: crate::editor::find::FindState::default(),
            status_message_at: None,
            recent_files: Vec::new(),
            recent_tracked: None,
            extensions: crate::extensions::ExtensionService::default(),
            extensions_pending_start: false,
            marketplace_allowed: false,
            lang: LanguageState::default(),
            webview_host: Default::default(),
            ext_container: None,
            ext_pages: Vec::new(),
            ext_menu_actions: Vec::new(),
            ext_pick: None,
            pending_copy: None,
            viewport_height: 0.0,
            next_internal_handle: crate::workbench::webviews::INTERNAL_HANDLE_BASE,
        }
    }

    /// Set a transient status-bar message (auto-hides after STATUS_TTL,
    /// matching VS Code's `setStatusMessage` behaviour).
    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
        self.status_message_at = Some(std::time::Instant::now());
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
        } else if id == &ids.close_folder {
            Some(CommandId::CloseFolder)
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
        } else if id == &ids.go_to_file {
            self.quick_open_shortcut();
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

    /// Load the extensions installed in `dirs` (VS Code's layout: one folder
    /// per extension with a `package.json`). Their code starts running on
    /// the next frame.
    pub fn bootstrap_extensions(&mut self, dirs: Vec<PathBuf>) {
        // Installs go to the last folder (`~/.vscode-rust/extensions`, or
        // the one `--extensions-dir` names).
        if let Some(dir) = dirs.last() {
            self.extensions.marketplace.install_dir = dir.clone();
        }
        self.extensions.load(&dirs);
        self.extensions_pending_start = true;
    }

    /// Let the Extensions view reach the marketplace (off in tests).
    pub fn bootstrap_marketplace(&mut self) {
        self.marketplace_allowed = true;
    }

    /// Open the Extensions view on launch.
    pub fn bootstrap_extensions_view(&mut self) {
        self.show_view(ActivityView::Extensions);
    }

    /// Start the extension host (if extensions were loaded) and handle what
    /// it sent since the last frame.
    fn pump_extensions(&mut self, ctx: &Context) {
        if self.extensions_pending_start {
            self.extensions_pending_start = false;
            let wake = ctx.clone();
            self.extensions
                .start(self.workspace_root.as_deref(), move || wake.request_repaint());
            self.extensions.marketplace.load_recommendations(self.workspace_root.as_deref());
        }
        let view_open = self.sidebar_visible && self.active_view == ActivityView::Extensions;
        if view_open && self.marketplace_allowed && !self.extensions.marketplace.enabled {
            let wake = ctx.clone();
            self.extensions.marketplace.enable(move || wake.request_repaint());
        }
        if self.extensions.tick_marketplace(ctx) {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        if let Some(lang) = self.active_doc.and_then(|i| self.documents.get(i)).map(|d| d.language_id()) {
            self.extensions.fire_event(&format!("onLanguage:{lang}"));
        }
        self.sync_extension_documents();
        for id in self.palette.take_abandoned() {
            if !self.extension_version_picked(id, None) {
                self.extensions.answer(id, serde_json::Value::Null);
            }
        }
        for request in self.extensions.poll() {
            match request {
                crate::extensions::EditorRequest::ApplyEdits { request_id, path, edits } => {
                    let ok = self.apply_extension_edits(&path, &edits);
                    self.extensions.answer(request_id, serde_json::json!(ok));
                }
                crate::extensions::EditorRequest::OpenEditor { request_id, path } => {
                    self.show_welcome = false;
                    self.open_file(path);
                    self.sync_extension_documents();
                    self.extensions.answer(request_id, serde_json::json!(true));
                }
                crate::extensions::EditorRequest::OpenDiff { request_id, path, base, text, title } => {
                    self.show_welcome = false;
                    self.documents.push(crate::editor::Document::diff(path, base, text, title));
                    self.active_doc = Some(self.documents.len() - 1);
                    self.extensions.answer(request_id, serde_json::json!(true));
                }
                crate::extensions::EditorRequest::CloseTabs { request_id, paths } => {
                    while let Some(i) = self.documents.iter().position(|d| paths.contains(&d.path)) {
                        self.close_doc(i);
                    }
                    self.extensions.answer(request_id, serde_json::json!(true));
                }
                crate::extensions::EditorRequest::SaveDocument { request_id, path } => {
                    let ok = match self.documents.iter_mut().find(|d| d.path == path) {
                        Some(doc) => doc.save().is_ok(),
                        None => false,
                    };
                    if ok {
                        self.extensions.document_saved(&path);
                    }
                    self.extensions.answer(request_id, serde_json::json!(ok));
                }
                crate::extensions::EditorRequest::Reveal { path, line } => {
                    if self.active_doc.and_then(|i| self.documents.get(i)).is_some_and(|d| d.path == path) {
                        self.go_to_line(line + 1);
                    }
                }
                crate::extensions::EditorRequest::SetSelection { path, line, character } => {
                    if let Some(doc) = self.documents.iter_mut().find(|d| d.path == path) {
                        let byte = crate::extensions::text::utf16_to_byte(&doc.text, line, character);
                        let line_start = doc.text[..byte].rfind('\n').map(|p| p + 1).unwrap_or(0);
                        doc.pending_nav = Some((line + 1, byte - line_start));
                    }
                }
                crate::extensions::EditorRequest::QuickPick { request_id, items, placeholder, .. } => {
                    let items = items
                        .into_iter()
                        .map(|i| (i.label, i.description, i.detail, i.separator))
                        .collect();
                    self.palette.open_pick(request_id, items, placeholder);
                }
                crate::extensions::EditorRequest::InputBox { request_id, prompt, placeholder, value, password } => {
                    self.palette.open_input(request_id, prompt, placeholder, value, password);
                }
                crate::extensions::EditorRequest::Command(id) => self.run_editor_command(ctx, &id),
                crate::extensions::EditorRequest::StatusMessage(text) => {
                    if text.is_empty() {
                        self.status_message_at = None;
                    } else {
                        self.set_status(text);
                    }
                }
                crate::extensions::EditorRequest::CopyText(text) => ctx.copy_text(text),
            }
        }
    }

    /// Byte spans to paint for the extensions' decorations of `doc`.
    fn extension_decoration_spans(
        &self,
        doc: &crate::editor::Document,
    ) -> Vec<crate::editor::highlight::DecorationSpan> {
        let Some(per_type) = self.extensions.decorations.get(&doc.path) else { return Vec::new() };
        let mut out = Vec::new();
        for (key, ranges) in per_type {
            let Some(style) = self.extensions.decoration_types.get(key) else { continue };
            if style.background.is_none() && style.color.is_none() && style.border.is_none() {
                continue;
            }
            for (start, end) in crate::extensions::text::decoration_spans(&doc.text, ranges, style.whole_line) {
                out.push(crate::editor::highlight::DecorationSpan {
                    start,
                    end,
                    background: style.background,
                    color: style.color,
                    underline: style.border,
                });
            }
        }
        out
    }

    /// Squiggles for the problems extensions reported in `doc`.
    fn problem_squiggles(&self, doc: &crate::editor::Document) -> Vec<crate::editor::view::Squiggle> {
        use crate::extensions::text::utf16_to_byte;
        self.extensions
            .diagnostics_for(&doc.path)
            .into_iter()
            .filter(|d| d.severity <= 2)
            .map(|d| crate::editor::view::Squiggle {
                start: utf16_to_byte(&doc.text, d.range[0], d.range[1]),
                end: utf16_to_byte(&doc.text, d.range[2], d.range[3]),
                color: match d.severity {
                    0 => Palette::VSCE_EDITOR_ERROR_FG,
                    1 => Palette::VSCE_EDITOR_WARNING_FG,
                    _ => Palette::VSCE_EDITOR_INFO_FG,
                },
            })
            .collect()
    }

    /// The active document and the caret's byte offset in it.
    fn caret_byte(&self) -> Option<(usize, usize)> {
        let idx = self.active_doc?;
        let doc = self.documents.get(idx)?;
        let ch = self.lang.caret_char?;
        Some((idx, crate::extensions::text::char_to_byte(&doc.text, ch)))
    }

    /// Keys for the suggest list (before the editor sees them), Ctrl+Space
    /// to ask for completions, F12 for go to definition.
    fn language_shortcuts(&mut self, ctx: &Context) {
        let mut accept = false;
        if self.lang.suggest.is_some() {
            ctx.input_mut(|i| {
                let s = self.lang.suggest.as_mut().unwrap();
                if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                    s.selected = (s.selected + 1) % s.shown.len().max(1);
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                    s.selected = (s.selected + s.shown.len().max(1) - 1) % s.shown.len().max(1);
                }
                if i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::Tab) {
                    accept = true;
                }
                if i.consume_key(Modifiers::NONE, Key::Escape) {
                    self.lang.suggest = None;
                }
            });
        }
        if accept {
            self.accept_suggestion();
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Space)) {
            self.request_completion(None);
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F12)) {
            if let Some(ch) = self.lang.caret_char {
                self.request_definition(ch);
            }
        }
    }

    /// Ask for completions at the caret; `trigger` is the typed trigger
    /// character, if any.
    fn request_completion(&mut self, trigger: Option<&str>) {
        let Some((idx, caret)) = self.caret_byte() else { return };
        let doc = &self.documents[idx];
        let start = if trigger.is_some() {
            caret
        } else {
            doc.text[..caret]
                .char_indices()
                .rev()
                .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '$')
                .last()
                .map(|(b, _)| b)
                .unwrap_or(caret)
        };
        let (line, ch) = crate::extensions::text::byte_to_position(&doc.text, caret);
        let path = doc.path.clone();
        self.sync_extension_documents();
        if let Some(id) = self.extensions.request_language_feature(
            crate::extensions::LanguageFeature::Completion,
            &path,
            line,
            ch,
            trigger,
        ) {
            self.lang.pending_completion = Some((id, path, start));
        }
    }

    fn request_definition(&mut self, char_index: usize) {
        let Some(doc) = self.active_doc.and_then(|i| self.documents.get(i)) else { return };
        let byte = crate::extensions::text::char_to_byte(&doc.text, char_index);
        let (line, ch) = crate::extensions::text::byte_to_position(&doc.text, byte);
        let path = doc.path.clone();
        self.sync_extension_documents();
        self.lang.pending_definition = self.extensions.request_language_feature(
            crate::extensions::LanguageFeature::Definition,
            &path,
            line,
            ch,
            None,
        );
    }

    /// Replace the typed word with the selected suggestion.
    fn accept_suggestion(&mut self) {
        let Some(s) = self.lang.suggest.take() else { return };
        let Some(item) = s.current().cloned() else { return };
        let Some((idx, caret)) = self.caret_byte() else { return };
        let doc = &mut self.documents[idx];
        if doc.path != s.path {
            return;
        }
        let item_start = crate::extensions::text::utf16_to_byte(&doc.text, item.range[0], item.range[1]);
        // The provider's range start when it's before the caret, else the
        // start of the typed word.
        let start = if item_start <= caret { item_start } else { s.start.min(caret) };
        doc.text.replace_range(start..caret, &item.insert_text);
        doc.check_dirty();
        let end = start + item.insert_text.len();
        let line_start = doc.text[..end].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line = doc.text[..end].matches('\n').count() + 1;
        doc.pending_nav = Some((line, end - line_start));
    }

    /// After the editor drew: remember the caret, follow typing (open,
    /// filter or close completions), track the pointer for hovers, ⌘-click.
    fn after_editor(&mut self, ctx: &Context, out: crate::editor::view::EditorOutput) {
        self.lang.caret = out.caret;
        self.lang.caret_char = out.caret_char;
        if out.changed {
            self.lang.hover = None;
            self.on_text_changed();
        }
        if let Some(ch) = out.command_clicked {
            self.request_definition(ch);
        }
        match out.hovered {
            Some((ch, pos)) => {
                let same = self.lang.hover_track.is_some_and(|(c, _, _)| c == ch);
                if !same {
                    self.lang.hover_track = Some((ch, std::time::Instant::now(), pos));
                    if let Some(((a, b), _, _)) = self.lang.hover {
                        let byte = self
                            .active_doc
                            .and_then(|i| self.documents.get(i))
                            .map(|d| crate::extensions::text::char_to_byte(&d.text, ch))
                            .unwrap_or(0);
                        if byte < a || byte > b {
                            self.lang.hover = None;
                        }
                    }
                }
                let (c, since, p) = self.lang.hover_track.unwrap();
                let waited = since.elapsed() >= std::time::Duration::from_millis(500);
                if waited && self.lang.pending_hover.is_none() && self.lang.hover.is_none() {
                    self.start_hover(c, p);
                } else if !waited {
                    ctx.request_repaint_after(std::time::Duration::from_millis(520) - since.elapsed().min(std::time::Duration::from_millis(500)));
                }
            }
            None => {
                self.lang.hover_track = None;
                self.lang.hover = None;
            }
        }
    }

    fn start_hover(&mut self, char_index: usize, pointer: egui::Pos2) {
        let Some(doc) = self.active_doc.and_then(|i| self.documents.get(i)) else { return };
        let byte = crate::extensions::text::char_to_byte(&doc.text, char_index);
        let (line, ch) = crate::extensions::text::byte_to_position(&doc.text, byte);
        let path = doc.path.clone();
        // Problems under the pointer show even without a hover provider.
        let has_problem = self.problems_at(&path, line, ch).next().is_some();
        match self.extensions.request_language_feature(
            crate::extensions::LanguageFeature::Hover,
            &path,
            line,
            ch,
            None,
        ) {
            Some(id) => self.lang.pending_hover = Some((id, char_index, pointer)),
            None if has_problem => {
                let word = word_bytes(&doc.text, byte);
                self.lang.hover = Some((word, pointer, Vec::new()));
            }
            None => {}
        }
    }

    fn problems_at<'a>(
        &'a self,
        path: &'a std::path::Path,
        line: usize,
        ch: usize,
    ) -> impl Iterator<Item = &'a crate::extensions::Diagnostic> + 'a {
        self.extensions.diagnostics_for(path).into_iter().filter(move |d| {
            let after_start = (line, ch) >= (d.range[0], d.range[1]);
            let before_end = (line, ch) <= (d.range[2], d.range[3]);
            after_start && before_end
        })
    }

    /// Typing: refilter or close the suggest list, or open it after a word
    /// character or a trigger character.
    fn on_text_changed(&mut self) {
        let Some((idx, caret)) = self.caret_byte() else { return };
        let text = &self.documents[idx].text;
        let prev = text[..caret].chars().next_back();
        let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
        if let Some(s) = self.lang.suggest.as_mut() {
            if caret < s.start || text[s.start..caret].chars().any(|c| !is_word(c)) {
                self.lang.suggest = None;
            } else {
                let word = text[s.start..caret].to_string();
                s.refilter(&word);
                if s.shown.is_empty() {
                    self.lang.suggest = None;
                }
                return;
            }
        }
        match prev {
            Some(c) if self.extensions.completion_triggers.contains(&c.to_string()) => {
                self.request_completion(Some(&c.to_string()));
            }
            Some(c) if is_word(c) && self.lang.pending_completion.is_none() => self.request_completion(None),
            _ => {}
        }
    }

    /// Pick up language feature answers and draw the suggest list and the
    /// hover card.
    fn language_widgets(&mut self, ctx: &Context) {
        use crate::extensions::ProviderResult;
        if let Some((id, path, start)) = self.lang.pending_completion.clone() {
            if let Some(ProviderResult::Completion { items, .. }) = self.extensions.take_result(id) {
                self.lang.pending_completion = None;
                if !items.is_empty() {
                    let mut s = crate::workbench::language_ui::Suggest {
                        items,
                        start,
                        shown: Vec::new(),
                        selected: 0,
                        path,
                    };
                    let word = self
                        .caret_byte()
                        .map(|(i, c)| self.documents[i].text.get(start..c).unwrap_or("").to_string())
                        .unwrap_or_default();
                    s.refilter(&word);
                    self.lang.suggest = (!s.shown.is_empty()).then_some(s);
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(30));
            }
        }
        if let Some(id) = self.lang.pending_definition {
            if let Some(ProviderResult::Definition(locs)) = self.extensions.take_result(id) {
                self.lang.pending_definition = None;
                match locs.first() {
                    Some(loc) => {
                        let text = self
                            .documents
                            .iter()
                            .find(|d| d.path == loc.path)
                            .map(|d| d.text.clone())
                            .or_else(|| std::fs::read_to_string(&loc.path).ok())
                            .unwrap_or_default();
                        let byte = crate::extensions::text::utf16_to_byte(&text, loc.range[0], loc.range[1]);
                        let line_start = text[..byte].rfind('\n').map(|p| p + 1).unwrap_or(0);
                        self.navigate_to(loc.path.clone(), loc.range[0] + 1, byte - line_start);
                    }
                    None => self.set_status("No definition found"),
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(30));
            }
        }
        if let Some((id, ch, pointer)) = self.lang.pending_hover {
            if let Some(ProviderResult::Hover { contents, range }) = self.extensions.take_result(id) {
                self.lang.pending_hover = None;
                if let Some(doc) = self.active_doc.and_then(|i| self.documents.get(i)) {
                    let byte = crate::extensions::text::char_to_byte(&doc.text, ch);
                    let (line, col) = crate::extensions::text::byte_to_position(&doc.text, byte);
                    let has_problem = self.problems_at(&doc.path, line, col).next().is_some();
                    let span = match range {
                        Some(r) => (
                            crate::extensions::text::utf16_to_byte(&doc.text, r[0], r[1]),
                            crate::extensions::text::utf16_to_byte(&doc.text, r[2], r[3]),
                        ),
                        None => word_bytes(&doc.text, byte),
                    };
                    if !contents.is_empty() || has_problem {
                        self.lang.hover = Some((span, pointer, contents));
                    }
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(30));
            }
        }

        if let (Some(caret), Some(s)) = (self.lang.caret, self.lang.suggest.as_mut()) {
            let word_len = self
                .active_doc
                .and_then(|i| self.documents.get(i))
                .zip(self.lang.caret_char)
                .map(|(d, c)| {
                    let caret_b = crate::extensions::text::char_to_byte(&d.text, c);
                    d.text.get(s.start..caret_b).map(|w| w.chars().count()).unwrap_or(0)
                })
                .unwrap_or(0);
            if let Some(crate::workbench::language_ui::SuggestEvent::Accept) =
                crate::workbench::language_ui::suggest_widget(ctx, s, caret, word_len)
            {
                self.accept_suggestion();
            }
        }
        if let Some(((a, _), pointer, blocks)) = self.lang.hover.clone() {
            if let Some(doc) = self.active_doc.and_then(|i| self.documents.get(i)) {
                let (line, col) = crate::extensions::text::byte_to_position(&doc.text, a);
                let problems: Vec<&crate::extensions::Diagnostic> = self.problems_at(&doc.path, line, col).collect();
                crate::workbench::language_ui::card(
                    ctx,
                    "hover_card",
                    // Above the hovered line, as VS Code places hovers.
                    pointer - egui::vec2(0.0, 12.0),
                    true,
                    &problems,
                    &blocks,
                    500.0,
                );
            }
        }
    }

    /// Keep webview panel tabs in step with the extensions: open tabs for
    /// new panels, close tabs of disposed ones, follow title changes, and
    /// tell extensions when the user closed a panel's tab.
    fn sync_webview_tabs(&mut self) {
        for handle in std::mem::take(&mut self.extensions.opened_panels) {
            let Some(w) = self.extensions.webviews.get(&handle) else { continue };
            self.documents.push(crate::editor::Document::webview(handle, w.title.clone()));
            self.active_doc = Some(self.documents.len() - 1);
            self.show_welcome = false;
        }
        for handle in std::mem::take(&mut self.extensions.closed_panels) {
            if let Some(i) = self.documents.iter().position(|d| d.webview == Some(handle)) {
                self.documents.remove(i);
                self.active_doc = match self.active_doc {
                    _ if self.documents.is_empty() => None,
                    Some(a) if a > i => Some(a - 1),
                    Some(a) => Some(a.min(self.documents.len() - 1)),
                    None => None,
                };
            }
        }
        // Tabs the user closed.
        let open: std::collections::BTreeSet<u64> = self.documents.iter().filter_map(|d| d.webview).collect();
        let gone: Vec<u64> = self
            .extensions
            .webviews
            .values()
            .filter(|w| matches!(w.kind, crate::extensions::WebviewKind::Panel { .. }) && !open.contains(&w.handle))
            .map(|w| w.handle)
            .collect();
        for h in gone {
            self.extensions.close_webview(h);
        }
        // Titles, and `reveal()`.
        let mut reveal = None;
        for (i, d) in self.documents.iter_mut().enumerate() {
            if let Some(w) = d.webview.and_then(|h| self.extensions.webviews.get_mut(&h)) {
                if d.diff_title.as_deref() != Some(w.title.as_str()) {
                    d.diff_title = Some(w.title.clone());
                }
                if std::mem::take(&mut w.reveal) {
                    reveal = Some(i);
                }
            }
        }
        if let Some(i) = reveal {
            self.active_doc = Some(i);
        }
    }

    /// Open (or focus) an extension's page in an editor tab, as clicking an
    /// extension in VS Code's Extensions view does. A single click opens a
    /// preview editor (italic title) that the next one replaces; `pinned`
    /// (double-click) keeps it.
    fn open_extension_page(&mut self, target: crate::workbench::extension_details::OpenTarget, pinned: bool) {
        use crate::workbench::extension_details::OpenTarget;
        let (id, gallery) = match target {
            OpenTarget::Installed(id) => (id, None),
            OpenTarget::Gallery(g) => (g.id(), Some(g)),
        };
        if let Some(page) = self.ext_pages.iter().find(|p| p.id.eq_ignore_ascii_case(&id)) {
            let handle = page.handle;
            if let Some(i) = self.documents.iter().position(|d| d.webview == Some(handle)) {
                if pinned {
                    self.documents[i].preview = false;
                }
                self.active_doc = Some(i);
                self.show_welcome = false;
                return;
            }
        }
        let title = self
            .extensions
            .installed()
            .find(|e| e.id.eq_ignore_ascii_case(&id))
            .map(|e| e.display_name.clone())
            .or_else(|| gallery.as_ref().map(|g| g.display_name.clone()))
            .unwrap_or_else(|| id.clone());
        let handle = self.next_internal_handle;
        self.next_internal_handle += 1;
        self.ext_pages.push(ExtensionPage {
            handle,
            id: id.clone(),
            gallery,
            local: None,
            shown: None,
            tab: "details".into(),
            feature: None,
        });
        if self.marketplace_allowed && !self.extensions.marketplace.enabled {
            // The page shows install counts and ratings from the catalogue.
            // (Wake-ups come from the repaint after messages.)
            self.extensions.marketplace.enable(|| {});
        }
        if let Some((ns, name)) = id.split_once('.') {
            self.extensions.marketplace.request_details(ns, name);
        }
        let mut doc = crate::editor::Document::webview(handle, format!("Extension: {title}"));
        doc.preview = !pinned;
        // The preview editor is replaced, as in VS Code.
        let replace = self.documents.iter().position(|d| {
            d.preview && d.webview.is_some_and(|h| h >= crate::workbench::webviews::INTERNAL_HANDLE_BASE)
        });
        match replace {
            Some(i) => {
                self.documents[i] = doc;
                self.active_doc = Some(i);
            }
            None => {
                self.documents.push(doc);
                self.active_doc = Some(self.documents.len() - 1);
            }
        }
        self.show_welcome = false;
    }

    /// Rebuild open extension pages whose data changed; drop pages whose
    /// tab closed.
    fn refresh_extension_pages(&mut self) {
        use crate::workbench::extension_details::{
            clean_repo_url, iso_to_ms, DetailsData, Enabled, ExtRef, InstallInfo, InstallState, MarketInfo, Runtime,
        };
        let open: std::collections::BTreeSet<u64> = self.documents.iter().filter_map(|d| d.webview).collect();
        let closed: Vec<u64> = self.ext_pages.iter().map(|p| p.handle).filter(|h| !open.contains(h)).collect();
        for h in closed {
            self.webview_host.remove_internal_page(h);
            self.ext_pages.retain(|p| p.handle != h);
        }
        let workspace = self.workspace_root.is_some();
        for i in 0..self.ext_pages.len() {
            let handle = self.ext_pages[i].handle;
            let page_id = self.ext_pages[i].id.clone();
            let installed = self.extensions.installed().find(|e| e.id.eq_ignore_ascii_case(&page_id)).cloned();
            // Files of the installed copy, read again after an update.
            if let Some(e) = &installed {
                if self.ext_pages[i].local.as_ref().map(|l| &l.path) != Some(&e.path) {
                    self.ext_pages[i].local = Some(LocalFiles::read(&e.path, &e.id, &e.manifest));
                }
            } else {
                self.ext_pages[i].local = None;
            }
            let page = &self.ext_pages[i];
            let market = &self.extensions.marketplace;
            let remote = market
                .details
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(&page_id))
                .and_then(|(_, r)| r.as_ref().ok());
            let json = remote.map(|r| &r.json);
            let jstr = |k: &str| {
                json.and_then(|j| j.get(k)).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(str::to_string)
            };
            let (ns, name) = page_id.split_once('.').unwrap_or((page_id.as_str(), ""));

            let mut d = DetailsData {
                handle,
                id: installed.as_ref().map(|e| e.id.clone()).unwrap_or_else(|| page_id.clone()),
                tab: page.tab.clone(),
                feature: page.feature.clone(),
                has_workspace: workspace,
                ..Default::default()
            };
            let mut roots = Vec::new();
            if let Some(e) = &installed {
                d.name = e.display_name.clone();
                d.publisher = jstr("namespaceDisplayName").unwrap_or_else(|| e.publisher.clone());
                d.description = e.description.clone();
                d.state = if market.installing.contains(&e.id) && !self.extensions.can_uninstall(&e.id) {
                    InstallState::Installing
                } else {
                    InstallState::Installed
                };
                d.enabled = match self.extensions.enablement(&e.id) {
                    crate::extensions::Enablement::Enabled => Enabled::Yes,
                    crate::extensions::Enablement::DisabledGlobally => Enabled::DisabledGlobally,
                    crate::extensions::Enablement::DisabledWorkspace => Enabled::DisabledWorkspace,
                };
                d.can_uninstall = self.extensions.can_uninstall(&e.id);
                d.auto_update = self.extensions.auto_update(&e.id);
                if d.can_uninstall && !market.installing.contains(&e.id) {
                    d.update_to = self.extensions.update_available(&e.id);
                }
                match d.enabled {
                    Enabled::DisabledGlobally => d.status.push(("", "This extension is disabled globally by the user.".into())),
                    Enabled::DisabledWorkspace => {
                        d.status.push(("", "This extension is disabled for this workspace by the user.".into()))
                    }
                    Enabled::Yes => {}
                }
                let activation = self.extensions.activation.get(&e.id).cloned();
                match &activation {
                    Some(crate::extensions::Activation::Failed(err)) => {
                        d.status.push(("error", format!("Activating the extension failed: {err}")))
                    }
                    Some(crate::extensions::Activation::NeedsNode) => {
                        d.status.push(("warning", "This extension needs Node.js to run, and it wasn't found.".into()))
                    }
                    _ => {}
                }
                if d.enabled == Enabled::Yes && e.main.is_some() {
                    d.runtime = Some(Runtime {
                        activated: matches!(activation, Some(crate::extensions::Activation::Active)),
                        error: match activation {
                            Some(crate::extensions::Activation::Failed(err)) => Some(err),
                            _ => None,
                        },
                    });
                }
                d.icon = e
                    .manifest
                    .get("icon")
                    .and_then(|i| i.as_str())
                    .map(|rel| format!("vscode-resource://w{handle}{}", e.path.join(rel).display()));
                if let Some(local) = &page.local {
                    d.readme = local.readme.clone();
                    d.changelog = local.changelog.clone();
                    d.install_info = Some(InstallInfo {
                        version: e.version.clone(),
                        last_updated: local.installed_ms,
                        size: Some((local.size, e.path.display().to_string())),
                        cache: local.cache.clone(),
                    });
                }
                d.base_url = Some(format!("vscode-resource://w{handle}{}/", e.path.display()));
                match &page.local {
                    Some(local) => d.read_manifest(&local.manifest),
                    None => d.read_manifest(&e.manifest),
                }
                roots.push(e.path.clone());
            } else {
                let g = page.gallery.as_ref();
                d.name = jstr("displayName")
                    .or_else(|| g.map(|g| g.display_name.clone()))
                    .unwrap_or_else(|| name.to_string());
                d.publisher = jstr("namespaceDisplayName").unwrap_or_else(|| ns.to_string());
                d.description = jstr("description").or_else(|| g.map(|g| g.description.clone())).unwrap_or_default();
                d.icon = g.and_then(|g| g.icon_url.clone()).or_else(|| {
                    json.and_then(|j| j.pointer("/files/icon")).and_then(|v| v.as_str()).map(str::to_string)
                });
                d.auto_update = self.extensions.auto_update(&page_id);
                d.state = if market.installing.iter().any(|x| x.eq_ignore_ascii_case(&page_id)) {
                    InstallState::Installing
                } else {
                    InstallState::NotInstalled
                };
                d.readme = remote.and_then(|r| r.readme.clone());
                d.changelog = remote.and_then(|r| r.changelog.clone());
                d.base_url = remote.and_then(|r| r.base_url.clone());
                d.loading = remote.is_none()
                    && market.enabled
                    && !market.details.keys().any(|k| k.eq_ignore_ascii_case(&page_id));
                d.pre_release = json.and_then(|j| j.get("preRelease")).and_then(|v| v.as_bool()).unwrap_or(false);
                if let Some(m) = remote.and_then(|r| r.manifest.as_ref()) {
                    d.read_manifest(m);
                }
                if let Some(g) = g {
                    d.install_count = Some(g.downloads);
                    d.rating = g.rating;
                    d.verified = g.verified;
                }
            }
            if let Some(j) = json {
                d.install_count = j.get("downloadCount").and_then(|v| v.as_u64()).or(d.install_count);
                d.rating = j.get("averageRating").and_then(|v| v.as_f64()).map(|r| r as f32).or(d.rating);
                d.rating_count = j.get("reviewCount").and_then(|v| v.as_u64());
                d.verified = j.get("verified").and_then(|v| v.as_bool()).unwrap_or(d.verified);
                d.preview |= j.get("preview").and_then(|v| v.as_bool()).unwrap_or(false);
                d.deprecated = j.get("deprecated").and_then(|v| v.as_bool()).unwrap_or(false);
                d.sponsor_url = jstr("sponsorLink");
                d.url = Some(market.web_url(&format!("extension/{ns}/{name}")));
                d.publisher_url = Some(market.web_url(&format!("namespace/{ns}")));
                d.license_url = j.pointer("/files/license").and_then(|v| v.as_str()).map(str::to_string);
                if let Some(r) = jstr("repository") {
                    d.repository = Some(clean_repo_url(&r));
                }
                if let Some(b) = jstr("bugs") {
                    d.issues = Some(b);
                }
                if let Some(cats) = j.get("categories").and_then(|c| c.as_array()) {
                    d.categories = cats.iter().filter_map(|c| c.as_str()).map(str::to_string).collect();
                }
                d.market_info = Some(MarketInfo {
                    version: jstr("version").unwrap_or_default(),
                    published: remote.and_then(|r| r.published.as_deref()).and_then(iso_to_ms),
                    last_released: jstr("timestamp").as_deref().and_then(iso_to_ms),
                });
            }
            // Dependencies and pack members: installed copies, else what the
            // catalogue says about them.
            let refs = |key: &str| -> Vec<String> {
                d.manifest
                    .as_ref()
                    .and_then(|m| m.get(key))
                    .and_then(|v| v.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_str()).map(str::to_string).collect())
                    .unwrap_or_default()
            };
            let (deps, pack) = (refs("extensionDependencies"), refs("extensionPack"));
            let describe = |id: &str| -> ExtRef {
                if let Some(e) = self.extensions.installed().find(|e| e.id.eq_ignore_ascii_case(id)) {
                    return ExtRef {
                        id: e.id.clone(),
                        name: e.display_name.clone(),
                        publisher: e.publisher.clone(),
                        description: e.description.clone(),
                        icon: e.manifest.get("icon").and_then(|i| i.as_str()).map(|rel| format!("vscode-resource://w{handle}{}", e.path.join(rel).display())),
                    };
                }
                let j = market.details.iter().find(|(k, _)| k.eq_ignore_ascii_case(id)).and_then(|(_, r)| r.as_ref().ok()).map(|r| &r.json);
                let get = |k: &str| j.and_then(|j| j.get(k)).and_then(|v| v.as_str()).map(str::to_string);
                ExtRef {
                    id: id.to_string(),
                    name: get("displayName").unwrap_or_else(|| id.split_once('.').map(|(_, n)| n.to_string()).unwrap_or_default()),
                    publisher: get("namespaceDisplayName").unwrap_or_else(|| id.split_once('.').map(|(p, _)| p.to_string()).unwrap_or_default()),
                    description: get("description").unwrap_or_default(),
                    icon: j.and_then(|j| j.pointer("/files/icon")).and_then(|v| v.as_str()).map(str::to_string),
                }
            };
            d.dependencies = deps.iter().map(|id| describe(id)).collect();
            d.pack = pack.iter().map(|id| describe(id)).collect();
            let wanted: Vec<String> = deps
                .iter()
                .chain(pack.iter())
                .filter(|id| !self.extensions.is_installed(id))
                .cloned()
                .collect();
            for id in wanted {
                if let Some((ns, name)) = id.split_once('.') {
                    self.extensions.marketplace.request_details(ns, name);
                }
            }
            if self.ext_pages[i].shown.as_ref() != Some(&d) {
                let html = crate::workbench::extension_details::page_html(&d);
                self.webview_host.set_internal_page(handle, html, roots);
                self.ext_pages[i].shown = Some(d);
            }
        }
    }

    /// A button, link or menu request on an extension page. `frame` is the
    /// editor window (native menus open over it); `None` in tests.
    fn extension_page_message(&mut self, handle: u64, msg: serde_json::Value, frame: Option<&eframe::Frame>) {
        let Some(idx) = self.ext_pages.iter().position(|p| p.handle == handle) else { return };
        let id = self.ext_pages[idx].shown.as_ref().map(|d| d.id.clone()).unwrap_or_else(|| self.ext_pages[idx].id.clone());
        let str_of = |k: &str| msg.get(k).and_then(|v| v.as_str()).map(str::to_string);
        match msg.get("command").and_then(|c| c.as_str()).unwrap_or_default() {
            "state" => {
                // The page switched tab or feature itself: remember it
                // without reloading the page.
                let page = &mut self.ext_pages[idx];
                if let Some(t) = str_of("tab") {
                    page.tab = t;
                }
                page.feature = str_of("feature").filter(|f| !f.is_empty());
                if let Some(shown) = page.shown.as_mut() {
                    shown.tab = page.tab.clone();
                    shown.feature = page.feature.clone();
                }
                self.pin_extension_page(handle);
            }
            "install" => self.extension_page_action(&id, ExtPageAction::Install),
            "update" => self.extension_page_action(&id, ExtPageAction::Update),
            "uninstall" => self.extension_page_action(&id, ExtPageAction::Uninstall),
            "enable" => self.extension_page_action(&id, ExtPageAction::Enable(false)),
            "disable" => self.extension_page_action(&id, ExtPageAction::Disable(false)),
            "autoUpdate" => {
                let on = self.extensions.auto_update(&id);
                self.extensions.set_auto_update(&id, !on);
            }
            "menu" => {
                let menu = str_of("menu").unwrap_or_default();
                let x = msg.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let y = msg.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let items = self.extension_menu_items(&id, &menu);
                // The page sits at the tab's rect; menus open under the button.
                let origin = self.webview_host.placed_rect(handle).map(|r| r.min).unwrap_or_default();
                if let Some(frame) = frame {
                    self.show_extension_menu(frame, &id, items, origin.x + x, origin.y + y + 2.0);
                }
            }
            "openLink" => {
                if let Some(url) = str_of("url") {
                    if url.starts_with("http://") || url.starts_with("https://") {
                        let _ = std::process::Command::new("open").arg(url).spawn();
                    }
                }
            }
            "reveal" => {
                if let Some(path) = str_of("path") {
                    let _ = std::process::Command::new("open").arg(path).spawn();
                }
            }
            "openExtension" => {
                if let Some(other) = str_of("id") {
                    let target = self.extension_target(&other);
                    self.open_extension_page(target, false);
                }
            }
            "search" => {
                if let Some(q) = str_of("query") {
                    self.show_view(ActivityView::Extensions);
                    self.extensions.marketplace.set_query(q);
                }
            }
            _ => {}
        }
    }

    /// How to open the page of extension `id`: installed, else from the
    /// catalogue.
    fn extension_target(&self, id: &str) -> crate::workbench::extension_details::OpenTarget {
        use crate::workbench::extension_details::OpenTarget;
        if let Some(e) = self.extensions.installed().find(|e| e.id.eq_ignore_ascii_case(id)) {
            return OpenTarget::Installed(e.id.clone());
        }
        let (ns, name) = id.split_once('.').unwrap_or((id, ""));
        let known = self
            .extensions
            .marketplace
            .lists
            .values()
            .flat_map(|l| l.items.iter())
            .find(|g| g.id().eq_ignore_ascii_case(id))
            .cloned();
        OpenTarget::Gallery(known.unwrap_or_else(|| crate::extensions::marketplace::GalleryExtension {
            namespace: ns.to_string(),
            name: name.to_string(),
            display_name: name.to_string(),
            ..Default::default()
        }))
    }

    /// Working in a preview editor keeps it (VS Code pins a preview on
    /// interaction with its content).
    fn pin_extension_page(&mut self, handle: u64) {
        if let Some(doc) = self.documents.iter_mut().find(|d| d.webview == Some(handle)) {
            doc.preview = false;
        }
    }

    /// The items of one of the page's menus: (label, action), `None` for a
    /// separator.
    fn extension_menu_items(&self, id: &str, menu: &str) -> Vec<Option<(String, ExtPageAction)>> {
        let installed = self.extensions.is_installed(id);
        let workspace = self.workspace_root.is_some();
        let item = |label: &str, a: ExtPageAction| Some((label.to_string(), a));
        match menu {
            "disable" => {
                let mut v = vec![item("Disable", ExtPageAction::Disable(false))];
                if workspace {
                    v.push(item("Disable (Workspace)", ExtPageAction::Disable(true)));
                }
                v
            }
            "enable" => {
                let mut v = vec![item("Enable", ExtPageAction::Enable(false))];
                if workspace {
                    v.push(item("Enable (Workspace)", ExtPageAction::Enable(true)));
                }
                v
            }
            "uninstall" => vec![item("Uninstall", ExtPageAction::Uninstall), item("Install Specific Version...", ExtPageAction::PickVersion)],
            _ => {
                let mut v = Vec::new();
                if !installed {
                    v.push(item("Install Specific Version...", ExtPageAction::PickVersion));
                    v.push(None);
                }
                v.push(item("Copy", ExtPageAction::Copy));
                v.push(item("Copy Extension ID", ExtPageAction::CopyId));
                v.push(item("Copy Link", ExtPageAction::CopyLink));
                if workspace {
                    v.push(None);
                    if self.extensions.marketplace.recommendations.iter().any(|r| r.eq_ignore_ascii_case(id)) {
                        v.push(item("Remove from Workspace Recommendations", ExtPageAction::Recommend(false)));
                    } else {
                        v.push(item("Add to Workspace Recommendations", ExtPageAction::Recommend(true)));
                    }
                }
                v.push(None);
                v.push(item("Download VSIX", ExtPageAction::Download));
                v.push(item("Download Specific Version VSIX...", ExtPageAction::PickDownload));
                v
            }
        }
    }

    /// Show a native menu at `x, y` (window points) and remember what its
    /// items do; the choice comes back as a menu event.
    fn show_extension_menu(&mut self, frame: &eframe::Frame, id: &str, items: Vec<Option<(String, ExtPageAction)>>, x: f32, y: f32) {
        use muda::ContextMenu;
        use wry::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let menu = muda::Menu::new();
        let mut actions = Vec::new();
        for entry in items {
            match entry {
                Some((label, action)) => {
                    let item = muda::MenuItem::new(label, true, None);
                    actions.push((item.id().clone(), id.to_string(), action));
                    let _ = menu.append(&item);
                }
                None => {
                    let _ = menu.append(&muda::PredefinedMenuItem::separator());
                }
            }
        }
        self.ext_menu_actions = actions;
        let Ok(handle) = frame.window_handle() else { return };
        if let RawWindowHandle::AppKit(appkit) = handle.as_raw() {
            // winit's view is flipped (y grows down) while muda measures
            // from the bottom, so hand it the distance from the bottom.
            let y = self.viewport_height - y;
            // SAFETY: the view belongs to the live editor window.
            unsafe {
                menu.show_context_menu_for_nsview(
                    appkit.ns_view.as_ptr() as *const std::ffi::c_void,
                    Some(muda::dpi::Position::Logical(muda::dpi::LogicalPosition::new(x as f64, y as f64))),
                );
            }
        }
    }

    /// Do what a page button or menu item asks for extension `id`.
    fn extension_page_action(&mut self, id: &str, action: ExtPageAction) {
        let (ns, name) = id.split_once('.').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or_default();
        let link = self.extensions.marketplace.web_url(&format!("extension/{ns}/{name}"));
        match action {
            ExtPageAction::Install => self.extensions.marketplace.install_version(&ns, &name, None),
            ExtPageAction::Update => {
                let v = self.extensions.update_available(id);
                self.extensions.marketplace.install_version(&ns, &name, v);
            }
            ExtPageAction::InstallVersion(v) => self.extensions.marketplace.install_version(&ns, &name, Some(v)),
            ExtPageAction::Uninstall => self.extensions.uninstall(id),
            ExtPageAction::Enable(ws) => self.extensions.set_enabled(id, true, ws),
            ExtPageAction::Disable(ws) => self.extensions.set_enabled(id, false, ws),
            ExtPageAction::PickVersion | ExtPageAction::PickDownload => {
                let versions = self
                    .extensions
                    .marketplace
                    .details
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(id))
                    .and_then(|(_, r)| r.as_ref().ok())
                    .map(|r| r.versions.clone())
                    .unwrap_or_default();
                if versions.is_empty() {
                    self.extensions.notify_info(format!("No versions of '{id}' are known yet."));
                    return;
                }
                let current = self.extensions.installed().find(|e| e.id.eq_ignore_ascii_case(id)).map(|e| e.version.clone());
                let items = versions
                    .iter()
                    .map(|v| (v.clone(), (Some(v) == current.as_ref()).then(|| "Current".to_string()), None, false))
                    .collect();
                let request = EXT_PICK_BASE + self.ext_pick.as_ref().map(|p| p.0 - EXT_PICK_BASE + 1).unwrap_or(0);
                let download = matches!(action, ExtPageAction::PickDownload);
                self.ext_pick = Some((request, id.to_string(), versions, download));
                let placeholder = if download { "Select Version to Download" } else { "Select Version to Install" };
                self.palette.open_pick(request, items, Some(placeholder.to_string()));
            }
            ExtPageAction::Copy => {
                let e = self.ext_pages.iter().filter_map(|p| p.shown.as_ref()).find(|d| d.id.eq_ignore_ascii_case(id));
                if let Some(d) = e {
                    let version = d.install_info.as_ref().map(|i| i.version.clone()).or_else(|| d.market_info.as_ref().map(|m| m.version.clone())).unwrap_or_default();
                    let text = format!(
                        "Name: {}\nId: {}\nDescription: {}\nVersion: {}\nPublisher: {}\nMarketplace Link: {}",
                        d.name, d.id, d.description, version, d.publisher, link
                    );
                    self.pending_copy = Some(text);
                }
            }
            ExtPageAction::CopyId => self.pending_copy = Some(id.to_string()),
            ExtPageAction::CopyLink => self.pending_copy = Some(link),
            ExtPageAction::Recommend(add) => {
                if let Some(root) = self.workspace_root.clone() {
                    match crate::extensions::marketplace::set_recommended(&root, id, add) {
                        Ok(()) => self.extensions.marketplace.load_recommendations(Some(&root)),
                        Err(e) => self.extensions.notify_error(format!("Couldn't update .vscode/extensions.json: {e}")),
                    }
                }
            }
            ExtPageAction::Download => self.download_extension(id, None),
        }
    }

    /// "Download VSIX": ask where, then fetch it.
    fn download_extension(&mut self, id: &str, version: Option<String>) {
        let v = version.clone().or_else(|| {
            self.extensions
                .marketplace
                .details
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(id))
                .and_then(|(_, r)| r.as_ref().ok())
                .and_then(|r| r.json.get("version").and_then(|v| v.as_str()).map(str::to_string))
        });
        let file = format!("{id}-{}.vsix", v.clone().unwrap_or_else(|| "latest".into()));
        let dest = rfd::FileDialog::new().set_file_name(&file).add_filter("VSIX", &["vsix"]).save_file();
        if let Some(dest) = dest {
            self.extensions.marketplace.download_vsix(id, version, dest);
        }
    }

    /// The version picked for "Install Specific Version..." or "Download
    /// Specific Version VSIX...". `true` when the request was ours.
    fn extension_version_picked(&mut self, request_id: u64, index: Option<usize>) -> bool {
        let Some((req, id, versions, download)) = self.ext_pick.clone() else { return false };
        if req != request_id {
            return false;
        }
        self.ext_pick = None;
        if let Some(v) = index.and_then(|i| versions.get(i)).cloned() {
            if download {
                self.download_extension(&id, Some(v));
            } else {
                self.extension_page_action(&id, ExtPageAction::InstallVersion(v));
            }
        }
        true
    }

    /// A sidebar container from an extension: its views stacked, webview
    /// views as native web views, tree views not yet.
    fn extension_container_view(&mut self, ui: &mut egui::Ui) {
        let containers = self.extensions.visible_containers();
        let Some(c) = containers.into_iter().find(|c| Some(&c.id) == self.ext_container.as_ref()) else {
            sidebar::title_bar(ui, "");
            return;
        };
        sidebar::title_bar(ui, &c.title.to_uppercase());
        let n = c.views.len().max(1);
        // VS Code drops the section header when a container has one view.
        let header_h = if n > 1 { 22.0 } else { 0.0 };
        let avail = ui.available_rect_before_wrap();
        let each = ((avail.height() - header_h * n as f32) / n as f32).max(40.0);
        for v in &c.views {
            if n > 1 {
                let (h, _) = ui.allocate_exact_size(egui::vec2(avail.width(), header_h), egui::Sense::hover());
                ui.painter().text(
                    egui::pos2(h.left() + 20.0, h.center().y),
                    egui::Align2::LEFT_CENTER,
                    v.name.to_uppercase(),
                    crate::ui_font::bold(11.0),
                    Palette::SIDEBAR_SECTION_HEADER_FG,
                );
            }
            let (rect, _) = ui.allocate_exact_size(egui::vec2(avail.width(), each), egui::Sense::hover());
            let note = |ui: &egui::Ui, text: &str| {
                ui.painter().text(
                    egui::pos2(rect.left() + 20.0, rect.top() + 16.0),
                    egui::Align2::LEFT_CENTER,
                    text,
                    egui::FontId::proportional(13.0),
                    Palette::FG_DESCRIPTION,
                );
            };
            if v.kind == "webview" {
                match self.extensions.webview_for_view(&v.id, &v.name) {
                    Some(handle) => {
                        if self.extensions.webviews.get(&handle).is_some_and(|w| w.html.is_empty()) {
                            note(ui, "Loading…");
                        }
                        self.webview_host.place(handle, rect);
                    }
                    None => note(ui, "Loading…"),
                }
            } else {
                note(ui, "This kind of view isn't supported yet.");
            }
        }
    }

    /// Send the open documents and the active editor's selection to the
    /// extension host (only what changed).
    fn sync_extension_documents(&mut self) {
        let docs: Vec<(PathBuf, &'static str, &str, bool)> = self
            .documents
            .iter()
            .filter(|d| d.diff_base.is_none() && d.webview.is_none())
            .map(|d| (d.path.clone(), d.language_id(), d.text.as_str(), d.dirty))
            .collect();
        let active = self.active_doc.and_then(|i| self.documents.get(i)).filter(|d| d.diff_base.is_none() && d.webview.is_none()).map(|d| {
            let line = d.cursor_line.saturating_sub(1);
            let ch = crate::extensions::text::char_col_to_utf16(&d.text, line, d.cursor_col.saturating_sub(1));
            (d.path.clone(), [line, ch, line, ch])
        });
        self.extensions.sync_documents(&docs, active);
        let tabs: Vec<serde_json::Value> = self
            .documents
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let view_type = d.webview.and_then(|h| self.extensions.webviews.get(&h)).and_then(|w| match &w.kind {
                    crate::extensions::WebviewKind::Panel { view_type } => Some(view_type.clone()),
                    _ => None,
                });
                serde_json::json!({
                    "label": d.diff_title.clone().unwrap_or_else(|| d.display_name()),
                    "path": d.path,
                    "kind": if d.webview.is_some() { "webview" } else if d.diff_base.is_some() { "diff" } else { "text" },
                    "viewType": view_type,
                    "active": Some(i) == self.active_doc,
                    "dirty": d.dirty,
                    "pinned": d.pinned,
                })
            })
            .collect();
        self.extensions.sync_tabs(serde_json::Value::Array(tabs));
    }

    /// Apply an extension's edits to a file, opening it first if needed.
    fn apply_extension_edits(&mut self, path: &std::path::Path, edits: &[crate::extensions::WireEdit]) -> bool {
        if !self.documents.iter().any(|d| d.path == path) {
            match crate::editor::Document::open(path.to_path_buf()) {
                Ok(doc) => self.documents.push(doc),
                Err(_) => return false,
            }
        }
        let Some(doc) = self.documents.iter_mut().find(|d| d.path == path) else { return false };
        crate::extensions::text::apply_edits(&mut doc.text, edits);
        doc.check_dirty();
        true
    }

    /// Run a built-in command by its VS Code id (for extensions calling
    /// `commands.executeCommand`).
    fn run_editor_command(&mut self, ctx: &Context, id: &str) {
        let cmd = match id {
            "workbench.action.files.openFolder" => CommandId::OpenFolder,
            "workbench.action.files.openFile" => CommandId::OpenFile,
            "workbench.action.closeFolder" => CommandId::CloseFolder,
            "workbench.action.files.save" => CommandId::Save,
            "workbench.action.files.saveAll" => CommandId::SaveAll,
            "workbench.action.closeActiveEditor" => CommandId::CloseFile,
            "workbench.action.closeAllEditors" => CommandId::CloseAllFiles,
            "workbench.action.toggleSidebarVisibility" => CommandId::ToggleSidebar,
            "workbench.view.explorer" => CommandId::ShowExplorer,
            "workbench.view.search" => CommandId::ShowSearch,
            "workbench.action.quickOpen" => CommandId::QuickOpen,
            "workbench.action.quit" => CommandId::Quit,
            "workbench.action.showCommands" => {
                self.palette.open();
                return;
            }
            "workbench.view.extensions" => {
                self.show_view(ActivityView::Extensions);
                return;
            }
            _ => return,
        };
        self.execute_command(ctx, cmd);
    }

    /// Open the Source Control view on launch (used by the `--scm` CLI flag).
    pub fn bootstrap_scm(&mut self) {
        self.show_view(ActivityView::SourceControl);
        self.sidebar_visible = true;
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
        self.set_status(format!("Opened folder: {}", path.display()));
        self.git_watcher = crate::git::Watcher::new(&path);
        self.git_model = crate::git::Model::discover(&path);
        self.scm_ui = crate::workbench::source_control::ScmUiState::default();
        self.refresh_git();
    }

    /// The repository that owns the active document (longest-prefix match),
    /// else the first repo. Drives the status bar + GRAPH section.
    fn active_repo_root(&self) -> Option<PathBuf> {
        if let Some(idx) = self.active_doc {
            if let Some(doc) = self.documents.get(idx) {
                if let Some(r) = self.git_model.repo_for(&doc.path) {
                    return Some(r.root.clone());
                }
            }
        }
        self.git_model.repos.first().map(|r| r.root.clone())
    }

    /// Re-run status for every repository, rebuild Explorer decorations and
    /// the active repo's commit graph, and recompute the active-doc gutter.
    fn refresh_git(&mut self) {
        self.git_model.refresh();
        self.git_decorations = self.git_model.decorations();
        // Commit graph for the active repository.
        self.git_history = match self.active_repo_root() {
            Some(root) => crate::git::build_graph(&crate::git::log(&root, 200)),
            None => Vec::new(),
        };
        self.refresh_git_line_changes();
    }

    /// Commit `repo_root` with its SCM message box, in `mode`. Commits staged
    /// changes; if nothing is staged, commits all tracked changes (`-a`) —
    /// VS Code prompts here; we commit-all directly to keep the flow simple.
    fn do_commit(&mut self, repo_root: &Path, mode: crate::git::CommitMode) {
        let msg = self
            .scm_ui
            .commit_messages
            .get(repo_root)
            .map(|m| m.trim().to_string())
            .unwrap_or_default();
        if msg.is_empty() {
            self.set_status("Commit message is empty");
            return;
        }
        let Some(repo) = self.git_model.repo_for(repo_root) else {
            return;
        };
        let all = repo.index.is_empty();
        if all && repo.working.is_empty() && repo.merge.is_empty() {
            self.set_status("Nothing to commit");
            return;
        }
        match crate::git::commit_with(repo_root, &msg, all, mode) {
            Ok(()) => {
                self.scm_ui.commit_messages.remove(repo_root);
                self.set_status("Committed");
            }
            Err(e) => {
                self.set_status(format!("Commit failed: {}", e.lines().next().unwrap_or("")));
            }
        }
        self.refresh_git();
    }

    /// Recompute the active document's per-line gutter diff against HEAD, and
    /// cache the HEAD blob so subsequent edits diff live (in-process) without
    /// spawning git. Resolves the file's repository via `repo_for`.
    fn refresh_git_line_changes(&mut self) {
        self.git_line_changes.clear();
        self.git_head_blob = None;
        self.git_diff_snapshot.clear();
        let Some(idx) = self.active_doc else { return };
        let Some(doc) = self.documents.get(idx) else { return };
        if doc.diff_base.is_some() {
            return;
        }
        let Some(repo) = self.git_model.repo_for(&doc.path) else {
            return;
        };
        let rel = doc
            .path
            .strip_prefix(&repo.root)
            .unwrap_or(&doc.path)
            .to_string_lossy()
            .into_owned();
        if let Some(blob) = crate::git::head_blob(&repo.root, &rel) {
            self.git_line_changes = crate::git::line_changes_diff(&blob, &doc.text);
            self.git_diff_snapshot = doc.text.clone();
            self.git_head_blob = Some(blob);
        }
    }

    /// Cheap per-frame check: if the active document's text changed since the
    /// last diff, recompute the gutter strip live from the cached HEAD blob.
    fn poll_live_gutter_diff(&mut self) {
        let Some(idx) = self.active_doc else { return };
        let Some(blob) = &self.git_head_blob else { return };
        let Some(doc) = self.documents.get(idx) else {
            return;
        };
        if doc.diff_base.is_some() || doc.text == self.git_diff_snapshot {
            return;
        }
        self.git_line_changes = crate::git::line_changes_diff(blob, &doc.text);
        self.git_diff_snapshot = doc.text.clone();
    }

    /// Close the current workspace folder (VS Code's "Close Folder"). Clears
    /// the explorer tree and any search results, and returns to the Welcome
    /// view. Open editors are left untouched — matching VS Code, which keeps
    /// already-open files visible after the folder is closed.
    fn close_folder(&mut self) {
        if self.workspace_root.is_none() {
            return;
        }
        self.workspace_root = None;
        self.file_tree = None;
        self.search = SearchState::default();
        self.git_watcher = None;
        self.git_model = crate::git::Model::default();
        self.git_history.clear();
        self.git_decorations.clear();
        self.scm_ui = crate::workbench::source_control::ScmUiState::default();
        if self.documents.is_empty() {
            self.show_welcome = true;
        }
        self.set_status("Closed folder");
    }

    fn open_file(&mut self, path: PathBuf) {
        // Reveal the file in the Explorer tree (expand its ancestor folders),
        // matching VS Code's "reveal active file" behaviour.
        if let Some(root) = self.file_tree.as_mut() {
            root.reveal(&path);
        }
        if let Some(idx) = self.documents.iter().position(|d| d.path == path) {
            self.active_doc = Some(idx);
            self.refresh_git_line_changes();
            self.track_recent_active();
            return;
        }
        match Document::open(path.clone()) {
            Ok(doc) => {
                self.documents.push(doc);
                self.active_doc = Some(self.documents.len() - 1);
                self.set_status(format!("Opened {}", path.display()));
                self.refresh_git_line_changes();
                self.track_recent_active();
            }
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                // Binary file — silently refuse, like VS Code's untrusted/binary
                // editor flow. Don't pollute the status bar with raw UTF-8 errors.
                self.status_message =
                    format!("Cannot show binary file: {}", path.display());
            }
            Err(e) => {
                self.set_status(format!("Failed to open {}: {}", path.display(), e));
            }
        }
    }

    /// Open a read-only diff tab for `path` (working tree vs HEAD), matching
    /// VS Code's behaviour when a changed file is clicked in Source Control.
    /// Falls back to a plain open for files with no HEAD version (new files).
    fn open_diff(&mut self, path: PathBuf) {
        let Some(repo) = self.git_model.repo_for(&path) else {
            self.open_file(path);
            return;
        };
        let root = repo.root.clone();
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        let Some(base) = crate::git::head_blob(&root, &rel) else {
            // Untracked / added file — no base to diff against; open plain.
            self.open_file(path);
            return;
        };
        // Reuse an existing diff tab for this path if open.
        if let Some(idx) = self
            .documents
            .iter()
            .position(|d| d.path == path && d.diff_base.is_some())
        {
            self.active_doc = Some(idx);
            return;
        }
        match Document::open(path.clone()) {
            Ok(mut doc) => {
                doc.diff_base = Some(base);
                self.documents.push(doc);
                self.active_doc = Some(self.documents.len() - 1);
            }
            Err(_) => self.open_file(path),
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
        self.set_status("Closed all editors");
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

    /// Branch-picker popup (status-bar branch click): a context menu of local
    /// branches; selecting one checks it out.
    fn show_branch_picker(&mut self, ctx: &Context) {
        use crate::vscode_widgets::composite::{context_menu, ContextMenuItem, ContextMenuProps};

        let Some((root, pos)) = self.branch_picker.clone() else {
            return;
        };
        let branches = crate::git::branches(&root);
        if branches.is_empty() {
            self.branch_picker = None;
            return;
        }
        let items: Vec<ContextMenuItem> =
            branches.iter().map(|b| ContextMenuItem::new(b)).collect();

        // Anchor the menu so its bottom sits at `pos.y` (it grows upward from
        // the status bar). Approximate height = items * 26 + padding.
        let height = items.len() as f32 * 26.0 + 8.0;
        let mut chosen = None;
        let area = egui::Area::new(egui::Id::new("branch_picker"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(pos.x, pos.y - height))
            .show(ctx, |ui| {
                let resp = context_menu(ui, &ContextMenuProps::default(), &items);
                chosen = resp.selected;
            });

        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if let Some(sel) = chosen {
            if let Some(name) = branches.get(sel) {
                if crate::git::checkout(&root, name) {
                    self.set_status(format!("Switched to branch '{name}'"));
                } else {
                    self.status_message =
                        format!("Could not switch to '{name}' (uncommitted changes?)");
                }
                self.refresh_git();
            }
            self.branch_picker = None;
        } else if escape || area.response.clicked_elsewhere() {
            self.branch_picker = None;
        }
    }

    /// Apply the Source Control view's emitted events to git, then refresh.
    fn dispatch_scm(&mut self, scm: crate::workbench::source_control::ScmOutput) {
        let mut dirty = false;
        if let Some((root, rel, _staged)) = scm.open_diff {
            self.open_diff(root.join(rel));
        }
        if let Some((root, rel)) = scm.stage {
            crate::git::stage(&root, &rel);
            dirty = true;
        }
        if let Some((root, rel)) = scm.unstage {
            crate::git::unstage(&root, &rel);
            dirty = true;
        }
        if let Some((root, rel, untracked)) = scm.discard {
            crate::git::discard(&root, &rel, untracked);
            dirty = true;
        }
        if let Some(root) = scm.stage_all {
            crate::git::stage_all(&root);
            dirty = true;
        }
        if let Some(root) = scm.unstage_all {
            crate::git::unstage_all(&root);
            dirty = true;
        }
        if let Some(root) = scm.refresh {
            let _ = root;
            dirty = true;
        }
        if let Some(root) = scm.sync {
            crate::git::pull(&root);
            crate::git::push(&root);
            dirty = true;
        }
        if let Some((root, mode)) = scm.commit {
            self.do_commit(&root, mode); // refreshes itself
        }
        if let Some((root, anchor)) = scm.commit_menu {
            self.commit_menu = Some((root, anchor));
        }
        if let Some((root, anchor)) = scm.repo_menu {
            // Reuse the branch picker location for now: open a small repo menu.
            let _ = anchor;
            let _ = root;
        }
        if let Some((root, rel, parent, commit)) = scm.open_commit_diff {
            self.open_commit_diff(&root, &rel, &parent, &commit);
        }
        if dirty {
            self.refresh_git();
        }
    }

    /// Open a read-only diff tab for a file at a commit (its first parent on
    /// the left, the commit on the right) — VS Code's behaviour when you click
    /// a file under a commit in the GRAPH.
    fn open_commit_diff(&mut self, root: &Path, rel: &str, parent: &str, commit: &str) {
        let base = if parent.is_empty() {
            String::new()
        } else {
            crate::git::blob_at(root, parent, rel).unwrap_or_default()
        };
        let working = crate::git::blob_at(root, commit, rel).unwrap_or_default();
        let path = root.join(rel);
        let short = |r: &str| r.chars().take(7).collect::<String>();
        let name = rel.rsplit('/').next().unwrap_or(rel);
        let title = format!("{name} ({}) ↔ {name} ({})", short(parent), short(commit));
        // Reuse an existing identical diff tab.
        if let Some(idx) = self
            .documents
            .iter()
            .position(|d| d.diff_title.as_deref() == Some(title.as_str()))
        {
            self.active_doc = Some(idx);
            return;
        }
        let doc = Document::diff(path, base, working, title);
        self.documents.push(doc);
        self.active_doc = Some(self.documents.len() - 1);
    }

    /// The Commit-mode dropdown (Commit / Commit & Push / Commit & Sync).
    fn show_commit_menu(&mut self, ctx: &Context) {
        use crate::vscode_widgets::composite::{context_menu, ContextMenuItem, ContextMenuProps};
        use crate::git::CommitMode;

        let Some((root, pos)) = self.commit_menu.clone() else {
            return;
        };
        let items = [
            ContextMenuItem::new("Commit"),
            ContextMenuItem::new("Commit & Push"),
            ContextMenuItem::new("Commit & Sync"),
        ];
        let mut chosen = None;
        let area = egui::Area::new(egui::Id::new("commit_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                chosen = context_menu(ui, &ContextMenuProps::default(), &items).selected;
            });
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if let Some(sel) = chosen {
            let mode = match sel {
                1 => CommitMode::AndPush,
                2 => CommitMode::AndSync,
                _ => CommitMode::Plain,
            };
            self.commit_menu = None;
            self.do_commit(&root, mode);
        } else if escape || area.response.clicked_elsewhere() {
            self.commit_menu = None;
        }
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
        let mut msg: Option<String> = None;
        let mut saved_path: Option<PathBuf> = None;
        if let Some(idx) = self.active_doc {
            if let Some(doc) = self.documents.get_mut(idx) {
                msg = Some(match doc.save() {
                    Ok(()) => {
                        saved_path = Some(doc.path.clone());
                        format!("Saved {}", doc.path.display())
                    }
                    Err(e) => format!("Save failed: {}", e),
                });
            }
        }
        if let Some(m) = msg {
            self.set_status(m);
        }
        if let Some(p) = saved_path {
            self.extensions.document_saved(&p);
        }
        self.refresh_git();
    }

    fn save_all(&mut self) {
        let mut saved = 0usize;
        let mut errs = 0usize;
        let mut saved_paths = Vec::new();
        for d in self.documents.iter_mut() {
            if d.dirty {
                match d.save() {
                    Ok(()) => {
                        saved += 1;
                        saved_paths.push(d.path.clone());
                    }
                    Err(_) => errs += 1,
                }
            }
        }
        for p in &saved_paths {
            self.extensions.document_saved(p);
        }
        let msg = if errs > 0 {
            format!("Saved {} file(s), {} errors", saved, errs)
        } else {
            format!("Saved {} file(s)", saved)
        };
        self.set_status(msg);
    }

    fn show_view(&mut self, view: ActivityView) {
        self.active_view = view;
        self.sidebar_visible = true;
        if view == ActivityView::Search {
            self.search.focus_input = true;
        }
        if view == ActivityView::SourceControl {
            self.refresh_git();
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
                    self.set_status(format!("Failed to open {}: {}", path.display(), e));
                    return;
                }
            }
        }
        if let Some(idx) = self.active_doc {
            if let Some(doc) = self.documents.get_mut(idx) {
                doc.pending_nav = Some((line, byte_in_line));
            }
        }
        self.refresh_git_line_changes();
    }

    /// Open Quick Open (⌘P) over the current workspace.
    fn open_quick_open(&mut self) {
        let root = self.workspace_root.clone();
        self.palette.open_quick_open(root.as_deref(), &self.recent_files);
    }

    /// ⌘P: open Quick Open, or step the selection down when it is already
    /// showing file results (VS Code's quick-navigate behaviour).
    fn quick_open_shortcut(&mut self) {
        if self.palette.visible && !self.palette.is_command_mode() {
            self.palette.select_next();
        } else {
            self.open_quick_open();
        }
    }

    /// Move the active document to the front of the editor history.
    fn track_recent_active(&mut self) {
        let Some(path) = self.active_doc.and_then(|i| self.documents.get(i)).map(|d| d.path.clone())
        else {
            return;
        };
        if self.recent_tracked.as_ref() == Some(&path) {
            return;
        }
        self.recent_tracked = Some(path.clone());
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(RECENT_FILES_MAX);
    }

    fn dispatch_palette(&mut self, ctx: &Context, action: PaletteAction) {
        match action {
            PaletteAction::Command(cmd) => self.execute_command(ctx, cmd),
            PaletteAction::OpenFile { path, line } => {
                self.show_welcome = false;
                self.open_file(path.clone());
                if let Some(line) = line {
                    self.go_to_line(line);
                }
            }
            PaletteAction::GoToLine(line) => self.go_to_line(line),
            PaletteAction::RemoveRecent(path) => self.recent_files.retain(|p| p != &path),
            PaletteAction::ExtensionCommand(id) => {
                self.extensions.execute(&id, serde_json::json!([]))
            }
            PaletteAction::Picked { request_id, index } if self.extension_version_picked(request_id, index) => {}
            PaletteAction::Picked { request_id, index } => {
                let value = index.map(|i| serde_json::json!(i)).unwrap_or(serde_json::Value::Null);
                self.extensions.answer(request_id, value);
            }
            PaletteAction::Input { request_id, value } => {
                let value = value.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null);
                self.extensions.answer(request_id, value);
            }
        }
    }

    /// Put the cursor at the start of 1-based `line` in the active editor,
    /// clamped to the document length.
    fn go_to_line(&mut self, line: usize) {
        let Some(doc) = self.active_doc.and_then(|i| self.documents.get_mut(i)) else {
            return;
        };
        let last = doc.text.split('\n').count().max(1);
        doc.pending_nav = Some((line.clamp(1, last), 0));
    }

    fn execute_command(&mut self, ctx: &Context, cmd: CommandId) {
        match cmd {
            CommandId::OpenFolder => self.open_folder_dialog(),
            CommandId::OpenFile => self.open_file_dialog(),
            CommandId::CloseFolder => self.close_folder(),
            CommandId::Save => self.save_active(),
            CommandId::SaveAll => self.save_all(),
            CommandId::CloseFile => self.close_active(),
            CommandId::CloseAllFiles => self.close_all(),
            CommandId::ToggleSidebar => self.sidebar_visible = !self.sidebar_visible,
            CommandId::ShowExplorer => self.show_view(ActivityView::Explorer),
            CommandId::ShowSearch => self.show_view(ActivityView::Search),
            CommandId::QuickOpen => self.open_quick_open(),
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
            .show_separator_line(false)
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

                use crate::icons::codicon_font;
                let p = ui.painter().clone();
                let cy = rect.center().y;

                // Left: back / forward navigation arrows (after the traffic lights).
                // Inert for now (no navigation history) — but they hover.
                let nav_x = rect.left() + left_inset + 6.0;
                for (i, glyph) in [crate::icons::ARROW_LEFT, crate::icons::ARROW_RIGHT].into_iter().enumerate() {
                    let r = egui::Rect::from_center_size(
                        egui::pos2(nav_x + 8.0 + i as f32 * 26.0, cy),
                        egui::vec2(24.0, 22.0),
                    );
                    let resp = ui.interact(r, ui.id().with(("tnav", i)), egui::Sense::click());
                    let (bg, fg) = if resp.hovered() {
                        (Palette::COMMAND_CENTER_BG, Palette::FG)
                    } else {
                        (egui::Color32::TRANSPARENT, Palette::TITLE_BAR_INACTIVE_FG)
                    };
                    ui.painter().rect_filled(r, egui::CornerRadius::same(4), bg);
                    ui.painter().text(
                        r.center(), egui::Align2::CENTER_CENTER,
                        glyph.to_string(), codicon_font(16.0), fg,
                    );
                    resp.on_hover_text(if i == 0 { "Go Back" } else { "Go Forward" });
                }

                // Centre: command-center pill (search icon + workspace name).
                // Clicking it opens the command palette, matching VS Code.
                let workspace_name = self
                    .workspace_root
                    .as_ref()
                    .and_then(|r| r.file_name())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "vscode-rust".to_string());
                let pill_w = 360.0_f32.min(rect.width() * 0.4);
                let pill = egui::Rect::from_center_size(
                    egui::pos2(rect.center().x, cy),
                    egui::vec2(pill_w, 22.0),
                );
                let pill_resp =
                    ui.interact(pill, ui.id().with("tcmdcenter"), egui::Sense::click());
                let pill_bg = if pill_resp.hovered() {
                    Palette::LIST_HOVER_BG
                } else {
                    Palette::COMMAND_CENTER_BG
                };
                ui.painter().rect(
                    pill,
                    egui::CornerRadius::same(6),
                    pill_bg,
                    egui::Stroke::new(1.0, Palette::COMMAND_CENTER_BORDER),
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    egui::pos2(pill.left() + 10.0, cy),
                    egui::Align2::LEFT_CENTER,
                    crate::icons::SEARCH.to_string(),
                    codicon_font(13.0),
                    Palette::COMMAND_CENTER_FG,
                );
                ui.painter().text(
                    pill.center(),
                    egui::Align2::CENTER_CENTER,
                    &workspace_name,
                    egui::FontId::proportional(12.5),
                    Palette::COMMAND_CENTER_FG,
                );
                // VS Code's command center opens Quick Open (file search).
                if pill_resp.clicked() {
                    self.open_quick_open();
                }
                pill_resp.on_hover_text(format!("Search {workspace_name} (⌘P)"));

                // Right: layout-control cluster. The leftmost icon
                // (`layout-sidebar-left`) toggles the primary sidebar; the
                // others are placeholders for parity with VS Code.
                let mut rx = rect.right() - 22.0;
                let layout_items: [(char, &str); 4] = [
                    (crate::icons::LAYOUT, "Customize Layout…"),
                    (crate::icons::LAYOUT_SIDEBAR_RIGHT, "Toggle Secondary Side Bar"),
                    (crate::icons::LAYOUT_PANEL, "Toggle Panel"),
                    (crate::icons::LAYOUT_SIDEBAR_LEFT, "Toggle Primary Side Bar (⌘B)"),
                ];
                for (i, (glyph, tip)) in layout_items.into_iter().enumerate() {
                    let r = egui::Rect::from_center_size(egui::pos2(rx, cy), egui::vec2(26.0, 22.0));
                    let resp = ui.interact(r, ui.id().with(("tlayout", i)), egui::Sense::click());
                    let (bg, fg) = if resp.hovered() {
                        (Palette::COMMAND_CENTER_BG, Palette::FG)
                    } else {
                        (egui::Color32::TRANSPARENT, Palette::TITLE_BAR_INACTIVE_FG)
                    };
                    ui.painter().rect_filled(r, egui::CornerRadius::same(4), bg);
                    ui.painter().text(
                        r.center(), egui::Align2::CENTER_CENTER,
                        glyph.to_string(), codicon_font(16.0), fg,
                    );
                    // Only the primary side bar toggle is wired up for now.
                    if i == 3 && resp.clicked() {
                        self.sidebar_visible = !self.sidebar_visible;
                    }
                    resp.on_hover_text(tip);
                    rx -= 30.0;
                }

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
                            self.menu_go(ui);
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

    #[cfg_attr(target_os = "macos", allow(dead_code))]
    fn menu_go(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(menu_label("Go"), |ui| {
            if ui.button("Go to File…       ⌘P").clicked() {
                self.open_quick_open();
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
        self.language_shortcuts(ctx);
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

        // Cmd+P — Quick Open. Checked after Cmd+Shift+P so the Shift chord
        // wins; also active while the popup is open (quick navigate).
        let quick_open = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::P))
        });
        if quick_open {
            self.quick_open_shortcut();
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

        // Cmd+F — in-editor find (only when a document is open). Must come
        // after the Cmd+Shift+F check so the chord with Shift wins.
        let find_in_editor = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::F))
        });
        if find_in_editor && self.active_doc.is_some() {
            self.find.open();
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

        let show_extensions = ctx.input_mut(|i| {
            i.consume_shortcut(&egui::KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::X,
            ))
        });
        if show_extensions {
            self.show_view(ActivityView::Extensions);
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
        self.webview_host.begin_frame();
        self.sync_webview_tabs();
        self.refresh_extension_pages();
        // Drain any native-menu events from the previous frame.
        while let Some(ev) = crate::menubar::poll_event() {
            if let Some((_, id, action)) = self.ext_menu_actions.iter().find(|(m, ..)| m == &ev.id).cloned() {
                self.extension_page_action(&id, action);
                continue;
            }
            self.dispatch_menu_event(ctx, &ev.id);
        }
        if let Some(text) = self.pending_copy.take() {
            ctx.copy_text(text);
        }
        self.viewport_height = ctx.viewport_rect().height();

        // Filesystem watcher → auto-refresh git when the working tree changes.
        if self.git_watcher.as_ref().is_some_and(|w| w.take_dirty()) {
            self.refresh_git();
            ctx.request_repaint();
        }
        // Live (as-you-type) gutter diff for the active editor.
        self.poll_live_gutter_diff();

        self.pump_extensions(ctx);

        self.handle_shortcuts(ctx);

        self.title_bar(ctx);

        TopBottomPanel::bottom("status_bar")
            .exact_height(22.0)
            .show_separator_line(false)
            .frame(
                Frame::default()
                    .fill(Palette::STATUS_BAR_BG)
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                // Webview tabs have no text: no cursor, language or encoding items.
                let active = self.active_doc.and_then(|i| self.documents.get(i)).filter(|d| d.webview.is_none());
                let has_workspace = self.workspace_root.is_some();
                // Status bar reflects the active repository.
                let active_root = self.active_repo_root();
                let repo = active_root
                    .as_ref()
                    .and_then(|r| self.git_model.repo_for(r));
                let branch = repo.and_then(|r| r.branch.as_deref());
                let changes = repo.map(|r| r.total()).unwrap_or(0);
                let ahead_behind = repo.map(|r| (r.ahead, r.behind)).unwrap_or((0, 0));
                let has_upstream = repo.map(|r| r.has_upstream).unwrap_or(false);
                // Status messages are transient (VS Code's setStatusMessage):
                // hide after STATUS_TTL, and keep repainting while one shows
                // so it disappears without further input.
                let status_visible = self
                    .status_message_at
                    .is_some_and(|t| t.elapsed() < STATUS_TTL);
                if status_visible {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
                }
                let message = if status_visible { self.status_message.as_str() } else { "" };
                let extension_items: Vec<_> =
                    self.extensions.status_items.values().cloned().collect();
                let sb = status_bar::show_with_extensions(
                    ui,
                    active,
                    message,
                    has_workspace,
                    branch,
                    changes,
                    ahead_behind,
                    has_upstream,
                    &extension_items,
                    self.extensions.problem_counts(),
                );
                if let Some((command, args)) = sb.extension_command.clone() {
                    self.extensions.execute(&command, args);
                }
                if let Some(rect) = sb.branch_clicked {
                    if let Some(root) = active_root.clone() {
                        self.branch_picker =
                            Some((root, egui::pos2(rect.left(), rect.top() - 4.0)));
                    }
                }
                if sb.sync_clicked {
                    if let Some(root) = active_root {
                        if has_upstream {
                            crate::git::pull(&root);
                            crate::git::push(&root);
                        } else {
                            crate::git::push(&root); // Publish (push)
                        }
                        self.refresh_git();
                    }
                }
                if sb.palette_requested {
                    self.palette.open();
                }
            });

        self.show_branch_picker(ctx);
        self.show_commit_menu(ctx);

        SidePanel::left("activity_bar")
            .exact_width(48.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                Frame::default()
                    .fill(Palette::ACTIVITY_BAR_BG)
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                let prev_view = self.active_view;
                let scm_count = self.git_model.total_changes();
                let containers: Vec<activity_bar::ContainerIcon> = self
                    .extensions
                    .visible_containers()
                    .into_iter()
                    .map(|c| activity_bar::ContainerIcon {
                        icon: (!c.icon.starts_with("$(") && !c.icon.is_empty())
                            .then(|| crate::assets::icon_mask_texture(ui.ctx(), std::path::Path::new(&c.icon), 48))
                            .flatten(),
                        id: c.id,
                        title: c.title,
                    })
                    .collect();
                if activity_bar::show_with_containers(
                    ui,
                    &mut self.active_view,
                    &mut self.sidebar_visible,
                    scm_count,
                    &containers,
                    &mut self.ext_container,
                ) {
                    self.palette.open();
                }
                // Refresh Git when the user just switched to Source Control.
                if self.active_view == ActivityView::SourceControl
                    && prev_view != ActivityView::SourceControl
                {
                    self.refresh_git();
                }
            });

        if self.sidebar_visible {
            // Use a panel-id keyed by the active view so each view has its own width
            // memory; this way switching Explorer↔Search doesn't carry over a stale
            // resize state which previously stretched the sidebar to fill the window.
            let panel_id = match self.active_view {
                ActivityView::Explorer => "sidebar_explorer",
                ActivityView::Search => "sidebar_search",
                ActivityView::SourceControl => "sidebar_scm",
                ActivityView::Extensions => "sidebar_extensions",
                ActivityView::ExtensionContainer => "sidebar_ext_container",
            };
            let sidebar_output: SidebarOutput;
            {
                let resp = SidePanel::left(panel_id)
                    .resizable(true)
                    .show_separator_line(false)
                    .width_range(170.0..=420.0)
                    .default_width(self.sidebar_width)
                    .frame(
                        Frame::default()
                            .fill(Palette::SIDEBAR_BG)
                            .inner_margin(Margin::ZERO),
                    )
                    .show(ctx, |ui| {
                        if self.active_view == ActivityView::ExtensionContainer {
                            let rect = ui.max_rect();
                            ui.painter().rect_filled(rect, 0.0, Palette::SIDEBAR_BG);
                            self.extension_container_view(ui);
                            return SidebarOutput::default();
                        }
                        if self.active_view == ActivityView::Extensions {
                            let rect = ui.max_rect();
                            ui.painter().rect_filled(rect, 0.0, Palette::SIDEBAR_BG);
                            let view = crate::workbench::extensions_view::show(ui, &mut self.extensions);
                            if let Some(ext) = view.install {
                                self.extensions.marketplace.install(&ext);
                            }
                            if let Some(id) = view.uninstall {
                                self.extensions.uninstall(&id);
                            }
                            if view.refresh {
                                self.extensions.marketplace.lists.clear();
                            }
                            if let Some(target) = view.open {
                                self.open_extension_page(target, view.open_pinned);
                            }
                            return SidebarOutput::default();
                        }
                        let graph_root = self.active_repo_root();
                        let active_file: Option<PathBuf> = self
                            .active_doc
                            .and_then(|i| self.documents.get(i))
                            .map(|d| d.path.clone());
                        sidebar::show(
                            ui,
                            self.active_view,
                            &self.workspace_root,
                            &mut self.file_tree,
                            &mut self.search,
                            &self.git_model,
                            &self.git_history,
                            graph_root.as_deref(),
                            &mut self.scm_ui,
                            &self.git_decorations,
                            active_file.as_deref(),
                            &mut self.outline_expanded,
                            &mut self.timeline_expanded,
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
            self.dispatch_scm(sidebar_output.scm);
        }

        let mut editor_output: Option<crate::editor::view::EditorOutput> = None;
        let mut welcome_pending_folder = false;
        let mut find_matches_for_widget: Vec<(usize, usize)> = Vec::new();
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
                            self.refresh_git_line_changes();
                        }
                        if let Some(idx) = tabs_action.close {
                            self.close_doc(idx);
                        }
                        if let Some(idx) = tabs_action.keep {
                            if let Some(doc) = self.documents.get_mut(idx) {
                                doc.preview = false;
                            }
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

                    let webview_tab = self.active_doc.and_then(|i| self.documents.get(i)).and_then(|d| d.webview);
                    if let Some(handle) = webview_tab {
                        // An extension's webview panel fills the editor area.
                        let rect = ui.available_rect_before_wrap();
                        ui.allocate_rect(rect, egui::Sense::hover());
                        if self.extensions.webviews.get(&handle).is_some_and(|w| w.html.is_empty()) {
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Loading…", egui::FontId::proportional(13.0), Palette::FG_DESCRIPTION);
                        }
                        self.webview_host.place(handle, rect);
                    } else if let Some(idx) = self.active_doc {
                        // Breadcrumbs row (path of the active file), like VS Code.
                        if let Some(doc) = self.documents.get(idx) {
                            crate::workbench::breadcrumbs::show(
                                ui,
                                &doc.path,
                                self.workspace_root.as_deref(),
                            );
                        }
                        // Find matches for the active document (Cmd+F widget).
                        let find_matches: Vec<(usize, usize)> = if self.find.open {
                            self.documents
                                .get(idx)
                                .map(|d| {
                                    crate::editor::find::compute_matches(
                                        &d.text,
                                        &self.find.query,
                                    )
                                })
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        };
                        let mut find_hl =
                            crate::editor::find::highlight_for(&self.find, &find_matches);
                        if let Some(doc) = self.documents.get(idx) {
                            let spans = self.extension_decoration_spans(doc);
                            if !spans.is_empty() {
                                find_hl
                                    .get_or_insert_with(|| crate::editor::highlight::FindHighlight {
                                        ranges: Vec::new(),
                                        current: 0,
                                        decorations: Vec::new(),
                                    })
                                    .decorations = spans;
                            }
                        }
                        let squiggles = self
                            .documents
                            .get(idx)
                            .map(|d| self.problem_squiggles(d))
                            .unwrap_or_default();
                        if let Some(doc) = self.documents.get_mut(idx) {
                            let out = crate::editor::view::show_with_squiggles(
                                ui,
                                doc,
                                &self.git_line_changes,
                                find_hl.as_ref(),
                                &squiggles,
                            );
                            editor_output = Some(out);
                        }
                        find_matches_for_widget = find_matches;
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

        // In-editor find widget (drawn over the editor, after all panels).
        if self.find.open && self.active_doc.is_some() {
            let fr = crate::editor::find::show(ctx, &mut self.find, &find_matches_for_widget);
            if let Some(byte) = fr.goto {
                if let Some(doc) = self.active_doc.and_then(|i| self.documents.get_mut(i)) {
                    // Byte offset → (1-based line, byte offset within that line).
                    let upto = &doc.text[..byte.min(doc.text.len())];
                    let line = upto.matches('\n').count() + 1;
                    let line_start = upto.rfind('\n').map(|p| p + 1).unwrap_or(0);
                    doc.pending_nav = Some((line, byte - line_start));
                }
            }
        }

        self.track_recent_active();
        if self.palette.visible {
            self.palette.set_extension_commands(self.extensions.palette_commands());
        }
        if let Some(action) = command_palette::show(ctx, &mut self.palette) {
            self.dispatch_palette(ctx, action);
        }

        if let Some(out) = editor_output {
            self.after_editor(ctx, out);
        }
        // Native web views sit above everything egui draws; hide them while a
        // popup would overlap them.
        self.webview_host.suppressed = self.palette.visible
            || self.tab_menu.is_some()
            || self.branch_picker.is_some()
            || self.commit_menu.is_some();
        self.language_widgets(ctx);

        for d in crate::workbench::notifications::show(ctx, &self.extensions.notifications) {
            self.extensions.close_notification(d.id, d.action);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // UI is registered via panels in `update`, not via the root Ui.
    }

    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        self.render(ctx);
        self.webview_host.forward_page_messages(&mut self.extensions);
        for (handle, msg) in self.webview_host.take_internal_messages() {
            self.extension_page_message(handle, msg, Some(frame));
        }
        self.webview_host.sync(frame, &mut self.extensions);
        if !self.extensions.webviews.is_empty() || !self.ext_pages.is_empty() {
            // Page messages arrive between frames.
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
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

            // Left column: Start + Recent (stacked); right column: Walkthroughs.
            let left_w = (content_w * 0.52).min(content_w - 260.0).max(220.0);
            let right_w = content_w - left_w - 40.0;
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(left_w, 320.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.label(
                            egui::RichText::new("Start").size(20.0).color(Palette::FG).strong(),
                        );
                        ui.add_space(8.0);
                        let _ = welcome_link_icon(ui, Some(crate::icons::NEW_FILE), "New File…");
                        if welcome_link_icon(ui, Some(crate::icons::FILE), "Open File…").clicked() {
                            action.open_file = true;
                        }
                        if welcome_link_icon(ui, Some(crate::icons::FOLDER_OPENED), "Open Folder…")
                            .clicked()
                        {
                            action.open_folder = true;
                        }
                        let _ = welcome_link_icon(ui, Some(crate::icons::REPO), "Clone Git Repository…");
                        if welcome_link_icon(ui, Some(crate::icons::SEARCH), "Show All Commands  ⇧⌘P")
                            .clicked()
                        {
                            action.open_palette = true;
                        }

                        ui.add_space(28.0);
                        ui.label(
                            egui::RichText::new("Recent").size(20.0).color(Palette::FG).strong(),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("No recent folders")
                                .color(Palette::FG_DESCRIPTION),
                        );
                    },
                );

                ui.add_space(40.0);

                ui.allocate_ui_with_layout(
                    egui::vec2(right_w, 320.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.label(
                            egui::RichText::new("Walkthroughs")
                                .size(20.0)
                                .color(Palette::FG)
                                .strong(),
                        );
                        ui.add_space(10.0);
                        walkthrough_card(
                            ui,
                            "Get Started with VS Code",
                            "Customize your editor, learn the basics, and start coding",
                        );
                        ui.add_space(8.0);
                        walkthrough_card(ui, "Learn the Fundamentals", "");
                    },
                );
            });

            ui.add_space(40.0);
            let mut show_on_startup = true;
            ui.checkbox(
                &mut show_on_startup,
                egui::RichText::new("Show welcome page on startup")
                    .size(13.0)
                    .color(Palette::FG_DESCRIPTION),
            );
        });
    });

    action
}

/// A Walkthroughs card: codicon tile + title + optional subtitle.
fn walkthrough_card(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    let h = if subtitle.is_empty() { 40.0 } else { 56.0 };
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
    let bg = if resp.hovered() {
        Palette::LIST_HOVER_BG
    } else {
        Palette::SIDEBAR_SECTION_HEADER_BG
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(6),
        bg,
        egui::Stroke::new(1.0, Palette::BORDER),
        egui::StrokeKind::Inside,
    );
    let icon_c = egui::pos2(rect.left() + 22.0, rect.center().y);
    ui.painter().text(
        icon_c,
        egui::Align2::CENTER_CENTER,
        crate::icons::VSCODE.to_string(),
        crate::icons::codicon_font(20.0),
        Palette::ACCENT,
    );
    let tx = rect.left() + 44.0;
    if subtitle.is_empty() {
        ui.painter().text(
            egui::pos2(tx, rect.center().y),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(13.5),
            Palette::FG,
        );
    } else {
        ui.painter().text(
            egui::pos2(tx, rect.top() + 16.0),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(13.5),
            Palette::FG,
        );
        ui.painter().text(
            egui::pos2(tx, rect.top() + 36.0),
            egui::Align2::LEFT_CENTER,
            subtitle,
            egui::FontId::proportional(12.0),
            Palette::FG_DESCRIPTION,
        );
    }
}

fn welcome_link(ui: &mut egui::Ui, text: &str) -> egui::Response {
    welcome_link_icon(ui, None, text)
}

fn welcome_link_icon(ui: &mut egui::Ui, icon: Option<char>, text: &str) -> egui::Response {
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
    let mut tx = rect.left();
    if let Some(g) = icon {
        painter.text(
            egui::pos2(rect.left() + 8.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            g.to_string(),
            crate::icons::codicon_font(15.0),
            color,
        );
        tx += 24.0;
    }
    painter.text(
        egui::pos2(tx, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        color,
    );
    if !shortcut.is_empty() {
        painter.text(
            egui::pos2(tx + label.len() as f32 * 7.5 + 16.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            shortcut,
            egui::FontId::proportional(12.0),
            Palette::FG_DESCRIPTION,
        );
    }
    resp
}

/// Byte range of the word around `byte` (for hover cards without a range).
fn word_bytes(text: &str, byte: usize) -> (usize, usize) {
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let byte = byte.min(text.len());
    let start = text[..byte]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map(|(b, _)| b)
        .unwrap_or(byte);
    let end = text[byte..].find(|c: char| !is_word(c)).map(|e| byte + e).unwrap_or(text.len());
    (start, end)
}
