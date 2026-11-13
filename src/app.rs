use std::path::{Path, PathBuf};

use eframe::CreationContext;
use egui::{Context, Frame, Key, Margin, Modifiers, SidePanel, TopBottomPanel};

use crate::editor::Document;
use crate::fs::FileNode;
use crate::menubar::MenuIds;
use crate::search::SearchState;
use crate::theme::{self, Palette};
use crate::workbench::command_palette::{self, CommandId, CommandPaletteState, PaletteAction};
use crate::workbench::{cards, 
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
    /// Recently opened folders, most recent first (the Welcome page's
    /// Recent list), kept in `<user data>/User/globalStorage`.
    recent_folders: Vec<PathBuf>,
    /// `--user-data-dir`; `None` in tests (nothing is written).
    user_data_dir: Option<PathBuf>,
    /// User settings (`settings.json`); in memory until
    /// `bootstrap_user_data` points at a folder.
    settings: crate::settings::Settings,
    /// The Color Theme picker while it's open.
    theme_pick: Option<ThemePick>,
    /// The Panel (Problems, Output, Debug Console, Terminal, Ports).
    panel: crate::workbench::panel::PanelState,
    /// "Do you trust the authors…?" before something that runs code
    /// (a terminal, debugging) in Restricted Mode.
    trust_request: Option<TrustRequest>,
    /// The "terminal failed to launch" toast (its Open Help button).
    terminal_failed_toast: Option<u64>,
    /// Trusted folders and dismissed banners (Workspace Trust).
    trust: crate::workspace_trust::TrustStore,
    /// The Workspace Trust editor while it's open, in its modal.
    trust_editor: Option<(crate::workbench::workspace_trust_editor::TrustEditorState, crate::workbench::modal::ModalState)>,
    /// The Settings editor (⌘,) while it's open, in its modal.
    settings_editor: Option<(crate::workbench::settings_editor::SettingsEditorState, crate::workbench::modal::ModalState)>,
    /// The open folder's `.vscode/settings.json`.
    workspace_settings: Option<crate::settings::Settings>,
    /// A Settings editor menu asked for during the frame.
    pending_settings_menu: Option<SettingsMenu>,
    keybindings_editor: Option<(crate::workbench::keybindings_editor::KeybindingsEditorState, crate::workbench::modal::ModalState)>,
    /// keybindings.json, parsed.
    user_keybindings: Vec<crate::workbench::keybindings_editor::model::UserBinding>,
    settings_menu_actions: Vec<(muda::MenuId, SettingsMenuAction)>,
    /// `theme::generation()` the egui visuals were last built for.
    visuals_generation: u32,
    /// ⌘K was pressed: the second key of a chord (⌘K ⌘T) may follow.
    chord_k: Option<std::time::Instant>,
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
    /// Open Commit-mode dropdown: `(repo_root, anchor)`.
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
    /// A menu asked for during the frame: (extension, which menu, where).
    pending_ext_menu: Option<(String, String, egui::Pos2)>,
    /// A Source Control menu asked for during the frame.
    pending_scm_menu: Option<(crate::workbench::source_control::ScmMenu, egui::Pos2)>,
    /// What the items of the open Source Control menu do.
    scm_menu_actions: Vec<(muda::MenuId, ScmAction)>,
    /// The editor title "…" menu asked for this frame, and its items.
    pending_editor_menu: Option<egui::Pos2>,
    editor_menu_actions: Vec<(muda::MenuId, EditorMenuAction)>,
    /// A Quick Access pick or input the Source Control view opened.
    git_prompt: Option<(u64, GitPrompt)>,
    /// `git.enableSmartCommit` ("Always" in the no-staged-changes prompt).
    smart_commit: bool,
}

use crate::workbench::source_control::ScmAction;

/// A Settings editor menu waiting for the window (native menus need it).
enum SettingsMenu {
    /// The gear of a setting row.
    Gear { key: String, pos: egui::Pos2, configured: bool, target: crate::workbench::settings_editor::Target },
    /// Filter Settings.
    Filter { pos: egui::Pos2 },
    /// A row of the Keyboard Shortcuts editor.
    Keybinding(crate::workbench::keybindings_editor::RowMenu),
}

#[derive(Clone, Debug)]
enum SettingsMenuAction {
    Reset(crate::workbench::settings_editor::Target, String),
    CopyId(String),
    CopyJson(String),
    Filter(&'static str),
    Keybinding(usize, crate::workbench::keybindings_editor::MenuItem),
}

/// An entry of a native menu built for the Source Control view.
/// The editor title's "More Actions..." menu (MenuId.EditorTitle, the
/// entries that work here).
#[derive(Clone, Copy, Debug)]
enum EditorMenuAction {
    CloseAll,
    CloseSaved,
    TogglePreview,
}

enum MenuEntry {
    Item(String, ScmAction),
    Check(String, ScmAction, bool),
    Separator,
    Sub(String, Vec<MenuEntry>),
}

/// Where a picked checkout target goes.
#[derive(Clone, Debug)]
enum GitTarget {
    NewBranch,
    NewBranchFrom,
    Detached,
    /// `refs/heads/x`, `refs/remotes/o/x` or `refs/tags/t`.
    Ref(String),
}

/// A Source Control pick or input box waiting for its answer.
#[derive(Clone, Debug)]
enum GitPrompt {
    Checkout(PathBuf, Vec<GitTarget>),
    Detached(PathBuf, Vec<String>),
    NewBranch(PathBuf, Option<String>),
    BranchFrom(PathBuf, Vec<String>),
    RenameBranch(PathBuf),
    DeleteBranch(PathBuf, Vec<String>),
    Merge(PathBuf, Vec<String>),
    Rebase(PathBuf, Vec<String>),
    TagName(PathBuf, Option<String>),
    TagMessage(PathBuf, String, Option<String>),
    DeleteTag(PathBuf, Vec<String>),
    StashMessage(PathBuf, bool, bool),
    /// `apply` / `pop` / `drop` and the stash ids offered.
    Stash(PathBuf, &'static str, Vec<String>),
    RemoteUrl(PathBuf),
    RemoteName(PathBuf, String),
    RemoveRemote(PathBuf, Vec<String>),
    PublishTo(PathBuf, String, Vec<String>),
    CommitMessage(PathBuf, crate::git::CommitMode, crate::workbench::source_control::CommitScope, bool),
    GraphFilter(Vec<crate::workbench::source_control::GraphFilter>),
    CloneUrl,
}

/// Request ids of the Source Control view's picks and inputs.
const GIT_PROMPT_BASE: u64 = 1 << 52;

/// Request id of the Color Theme picker.
const THEME_PICK_ID: u64 = 1 << 53;
/// Request id of Run and Debug's "Select debugger" pick.
const DEBUGGER_PICK_ID: u64 = 1 << 54;

/// What asked for workspace trust (`requestWorkspaceTrust`'s message), and
/// what runs once it's granted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrustRequest {
    Terminal,
    Debug,
}

/// The open Color Theme picker: the theme of each entry (none for "Browse
/// Additional Color Themes...") and the theme to go back to on cancel.
struct ThemePick {
    entries: Vec<Option<&'static str>>,
    original: String,
}

/// `refs/heads/main` → `main`, `refs/remotes/origin/x` → `origin/x`.
fn short_ref(r: &str) -> String {
    r.trim_start_matches("refs/heads/").trim_start_matches("refs/remotes/").trim_start_matches("refs/tags/").to_string()
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
    ToggleAutoUpdate,
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
            // layout.ts: `Math.min(300, width / 4)`.
            sidebar_width: 300.0,
            status_message: String::new(),
            palette: CommandPaletteState::default(),
            settings: crate::settings::Settings::in_memory(),
            recent_folders: Vec::new(),
            user_data_dir: None,
            theme_pick: None,
            visuals_generation: 0,
            chord_k: None,
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
            pending_ext_menu: None,
            pending_scm_menu: None,
            scm_menu_actions: Vec::new(),
            pending_editor_menu: None,
            editor_menu_actions: Vec::new(),
            settings_editor: None,
            trust: crate::workspace_trust::TrustStore::in_memory(),
            panel: Default::default(),
            trust_request: None,
            terminal_failed_toast: None,
            trust_editor: None,
            workspace_settings: None,
            pending_settings_menu: None,
            keybindings_editor: None,
            user_keybindings: Vec::new(),
            settings_menu_actions: Vec::new(),
            git_prompt: None,
            smart_commit: false,
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
        let cmd = if id == &ids.new_file {
            self.new_untitled_file();
            None
        } else if id == &ids.open_folder {
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
        } else if id == &ids.color_theme {
            Some(CommandId::SelectColorTheme)
        } else if id == &ids.open_settings {
            Some(CommandId::OpenSettings)
        } else if id == &ids.open_keybindings {
            Some(CommandId::OpenKeybindings)
        } else if id == &ids.toggle_panel {
            self.toggle_panel(ctx);
            None
        } else if id == &ids.show_problems {
            self.show_panel_view(ctx, crate::workbench::panel::PanelView::Problems);
            None
        } else if id == &ids.show_output {
            self.show_panel_view(ctx, crate::workbench::panel::PanelView::Output);
            None
        } else if id == &ids.show_debug_console {
            self.show_panel_view(ctx, crate::workbench::panel::PanelView::DebugConsole);
            None
        } else if id == &ids.show_terminal {
            self.show_panel_view(ctx, crate::workbench::panel::PanelView::Terminal);
            None
        } else if id == &ids.new_terminal {
            self.panel.visible = true;
            self.panel.view = crate::workbench::panel::PanelView::Terminal;
            self.create_terminal(ctx);
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
            if !self.extension_version_picked(id, None) && !self.git_prompt_answered(id, None, None) {
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
                    0 => Palette::VSCE_EDITOR_ERROR_FG(),
                    1 => Palette::VSCE_EDITOR_WARNING_FG(),
                    _ => Palette::VSCE_EDITOR_INFO_FG(),
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
        let replace = self.documents.iter().position(|d| d.preview && !d.dirty);
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
                        activation: self
                            .extensions
                            .activation_times
                            .get(&e.id)
                            .map(|t| (t.reason.clone(), t.activate_ms)),
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
                // The page sits at the tab's rect; menus open under the button.
                let origin = self.webview_host.placed_rect(handle).map(|r| r.min).unwrap_or_default();
                self.pending_ext_menu = Some((id.clone(), menu, egui::pos2(origin.x + x, origin.y + y + 2.0)));
                if let Some(frame) = frame {
                    self.show_pending_extension_menu(frame);
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
    fn extension_menu_items(&self, id: &str, menu: &str) -> Vec<Option<(String, ExtPageAction, Option<bool>)>> {
        let installed = self.extensions.is_installed(id);
        let workspace = self.workspace_root.is_some();
        let item = |label: &str, a: ExtPageAction| Some((label.to_string(), a, None));
        match menu {
            // `ManageExtensionAction`: the ⚙ of a row in the Extensions view.
            "list" => {
                let mut v = Vec::new();
                if self.extensions.enablement(id) == crate::extensions::Enablement::Enabled {
                    v.push(item("Disable", ExtPageAction::Disable(false)));
                    if workspace {
                        v.push(item("Disable (Workspace)", ExtPageAction::Disable(true)));
                    }
                } else {
                    v.push(item("Enable", ExtPageAction::Enable(false)));
                    if workspace {
                        v.push(item("Enable (Workspace)", ExtPageAction::Enable(true)));
                    }
                }
                let ours = self.extensions.can_uninstall(id);
                if ours {
                    v.push(None);
                    v.push(item("Uninstall", ExtPageAction::Uninstall));
                    v.push(None);
                    v.push(item("Install Specific Version...", ExtPageAction::PickVersion));
                    v.push(Some(("Auto Update".to_string(), ExtPageAction::ToggleAutoUpdate, Some(self.extensions.auto_update(id)))));
                }
                v.push(None);
                v.extend(self.extension_menu_items(id, "manage").into_iter().filter(|e| {
                    !matches!(e, Some((_, ExtPageAction::PickVersion, _)))
                }));
                // No leading separator twice.
                v.dedup_by(|a, b| a.is_none() && b.is_none());
                v
            }
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

    /// Show the menu a page or a row asked for (needs the window).
    fn show_pending_extension_menu(&mut self, frame: &eframe::Frame) {
        if let Some((id, menu, pos)) = self.pending_ext_menu.take() {
            if let Some((ns, name)) = id.split_once('.') {
                // "Install Specific Version..." lists the catalogue's versions.
                if self.marketplace_allowed && !self.extensions.marketplace.enabled {
                    self.extensions.marketplace.enable(|| {});
                }
                self.extensions.marketplace.request_details(ns, name);
            }
            let items = self.extension_menu_items(&id, &menu);
            self.show_extension_menu(frame, &id, items, pos.x, pos.y);
        }
    }

    /// Show a native menu at `x, y` (window points) and remember what its
    /// items do; the choice comes back as a menu event.
    fn show_extension_menu(&mut self, frame: &eframe::Frame, id: &str, items: Vec<Option<(String, ExtPageAction, Option<bool>)>>, x: f32, y: f32) {
        use muda::ContextMenu;
        use wry::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let menu = muda::Menu::new();
        let mut actions = Vec::new();
        for entry in items {
            match entry {
                Some((label, action, Some(checked))) => {
                    let item = muda::CheckMenuItem::new(label, true, checked, None);
                    actions.push((item.id().clone(), id.to_string(), action));
                    let _ = menu.append(&item);
                }
                Some((label, action, None)) => {
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
            ExtPageAction::ToggleAutoUpdate => {
                let on = self.extensions.auto_update(id);
                self.extensions.set_auto_update(id, !on);
            }
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
                if e.is_none() {
                    if let Some(x) = self.extensions.installed().find(|x| x.id.eq_ignore_ascii_case(id)) {
                        self.pending_copy = Some(format!(
                            "Name: {}\nId: {}\nDescription: {}\nVersion: {}\nPublisher: {}\nMarketplace Link: {}",
                            x.display_name, x.id, x.description, x.version, x.publisher, link
                        ));
                    }
                }
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
                    Palette::SIDEBAR_SECTION_HEADER_FG(),
                );
            }
            let (rect, _) = ui.allocate_exact_size(egui::vec2(avail.width(), each), egui::Sense::hover());
            let note = |ui: &egui::Ui, text: &str| {
                ui.painter().text(
                    egui::pos2(rect.left() + 20.0, rect.top() + 16.0),
                    egui::Align2::LEFT_CENTER,
                    text,
                    egui::FontId::proportional(13.0),
                    Palette::FG_DESCRIPTION(),
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
            .filter(|d| d.diff_base.is_none() && d.webview.is_none() && !d.welcome)
            .map(|d| (d.path.clone(), d.language_id(), d.text.as_str(), d.dirty))
            .collect();
        let active = self.active_doc.and_then(|i| self.documents.get(i)).filter(|d| d.diff_base.is_none() && d.webview.is_none() && !d.welcome).map(|d| {
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
    /// Opens the Settings editor, optionally with a search query (like
    /// `workbench.action.openSettings` with a query).
    pub fn bootstrap_settings(&mut self, query: Option<&str>) {
        self.open_settings_editor(query);
    }

    /// Opens the Keyboard Shortcuts editor (⌘K ⌘S), optionally searching.
    pub fn bootstrap_keybindings(&mut self, query: Option<&str>) {
        self.open_keybindings_editor(query);
    }

    /// keybindings.json as the editor last wrote it.
    pub fn user_keybindings_json(&self) -> String {
        crate::workbench::keybindings_editor::to_json(&self.user_keybindings)
    }

    /// Shows the Panel on `view` ("problems", "output", "debug", "terminal",
    /// "ports"), as View ▸ Problems and friends do.
    pub fn bootstrap_panel(&mut self, ctx: &Context, view: &str) {
        use crate::workbench::panel::PanelView;
        let view = match view {
            "problems" => PanelView::Problems,
            "output" => PanelView::Output,
            "debug" => PanelView::DebugConsole,
            "ports" => PanelView::Ports,
            _ => PanelView::Terminal,
        };
        self.show_panel_view(ctx, view);
    }

    /// A value from user settings (`None` when it isn't set).
    pub fn user_setting(&self, key: &str) -> Option<serde_json::Value> {
        self.settings.get(key).cloned()
    }

    pub fn bootstrap_search(&mut self, query: String) {
        self.show_view(ActivityView::Search);
        self.search.query = query;
        if let Some(root) = &self.workspace_root {
            let q = self.search.engine_query(None);
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
        self.add_recent_folder(path.clone());
        self.workspace_root = Some(path.clone());
        self.workspace_settings = Some(crate::settings::Settings::load_file(path.join(".vscode").join("settings.json")));
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
        // Git is an extension that doesn't run in Restricted Mode: no
        // repositories, decorations, graph or gutter markers until trusted.
        if !self.workspace_trusted() {
            self.git_model = crate::git::Model::default();
            self.git_decorations.clear();
            self.git_history.clear();
            self.git_line_changes.clear();
            self.git_head_blob = None;
            self.scm_ui.folder_open = self.workspace_root.is_some();
            self.scm_ui.parent_repos.clear();
            return;
        }
        if self.git_model.repos.is_empty() {
            if let Some(root) = self.workspace_root.clone() {
                self.git_model = crate::git::Model::discover(&root);
            }
        }
        self.git_model.refresh();
        self.scm_ui.folder_open = self.workspace_root.is_some();
        // A repository in a parent folder isn't opened by itself
        // (`git.openRepositoryInParentFolders: prompt`): the view offers it.
        self.scm_ui.parent_repos = match &self.workspace_root {
            Some(w) if self.git_model.repos.is_empty() => {
                crate::git::repo_root(w).filter(|r| r != w).into_iter().collect()
            }
            _ => Vec::new(),
        };
        self.git_decorations = self.git_model.decorations();
        // Commit graph for the active repository.
        self.git_history = match self.active_repo_root() {
            Some(root) => {
                use crate::workbench::source_control::GraphFilter;
                let filter = match &self.scm_ui.graph_filter {
                    GraphFilter::Auto => None,
                    GraphFilter::All => Some("--all".to_string()),
                    GraphFilter::Ref(r) => Some(r.clone()),
                };
                crate::git::graph(&root, filter.as_deref(), 200)
            }
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
        if !self.workspace_trusted() {
            return;
        }
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
        self.open_file_with(path, true);
    }

    /// Opens `path` in an editor: `pinned` keeps it, otherwise it takes the
    /// preview slot (a single click in the Explorer).
    fn open_file_with(&mut self, path: PathBuf, pinned: bool) {
        // Reveal the file in the Explorer tree (expand its ancestor folders),
        // matching VS Code's "reveal active file" behaviour.
        if let Some(root) = self.file_tree.as_mut() {
            root.reveal(&path);
        }
        if let Some(idx) = self.documents.iter().position(|d| d.path == path && d.diff_base.is_none() && !d.read_only && d.webview.is_none()) {
            if pinned {
                self.documents[idx].preview = false;
            }
            self.active_doc = Some(idx);
            self.refresh_git_line_changes();
            self.track_recent_active();
            return;
        }
        match Document::open(path.clone()) {
            Ok(doc) => {
                self.open_tab(doc, pinned);
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

    /// Apply the Source Control view's events.
    fn dispatch_scm(&mut self, scm: crate::workbench::source_control::ScmOutput) {
        for a in scm.actions {
            self.run_scm_action(a);
        }
        if let Some(m) = scm.menu {
            self.pending_scm_menu = Some(m);
        }
    }

    /// Run a git command in `root`; failures become an error toast (as the
    /// git extension shows them). Returns its output when it succeeded.
    fn git(&mut self, root: &Path, args: &[&str]) -> Option<String> {
        let out = std::process::Command::new("git").arg("-C").arg(root).args(args).output();
        match out {
            Ok(o) if o.status.success() => Some(String::from_utf8_lossy(&o.stdout).into_owned()),
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr);
                let first = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("git failed").trim().to_string();
                self.extensions.notify_error(format!("Git: {first}"));
                None
            }
            Err(e) => {
                self.extensions.notify_error(format!("Git: {e}"));
                None
            }
        }
    }

    /// A modal warning with one action button (`showWarningMessage(…,
    /// { modal: true }, yes)`); `true` when that button was chosen.
    fn confirm(message: &str, detail: &str, yes: &str) -> bool {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title(message)
            .set_description(detail)
            .set_buttons(rfd::MessageButtons::OkCancelCustom(yes.to_string(), "Cancel".to_string()))
            .show()
            == rfd::MessageDialogResult::Custom(yes.to_string())
    }

    /// Open a Quick Access pick for the Source Control view.
    fn git_pick(&mut self, prompt: GitPrompt, items: Vec<(String, Option<String>)>, placeholder: &str) {
        let request = GIT_PROMPT_BASE + self.git_prompt.as_ref().map(|p| p.0 - GIT_PROMPT_BASE + 1).unwrap_or(0);
        let items = items.into_iter().map(|(l, d)| (l, d, None, false)).collect();
        self.git_prompt = Some((request, prompt));
        self.palette.open_pick(request, items, Some(placeholder.to_string()));
    }

    /// Open a Quick Access input box for the Source Control view.
    fn git_input(&mut self, prompt: GitPrompt, text: &str, placeholder: &str, value: &str) {
        let request = GIT_PROMPT_BASE + self.git_prompt.as_ref().map(|p| p.0 - GIT_PROMPT_BASE + 1).unwrap_or(0);
        self.git_prompt = Some((request, prompt));
        self.palette.open_input(request, Some(text.to_string()), Some(placeholder.to_string()), value.to_string(), false);
    }

    /// Branches (`refs/heads`), remote branches and tags of `root`, with
    /// their short commit, for pickers.
    fn git_refs(&mut self, root: &Path, kinds: &[&str]) -> Vec<(String, String)> {
        let mut args = vec!["for-each-ref", "--sort=-committerdate", "--format=%(refname)%00%(objectname:short)"];
        args.extend_from_slice(kinds);
        self.git(root, &args)
            .map(|s| {
                s.lines()
                    .filter_map(|l| l.split_once('\0'))
                    .filter(|(r, _)| !r.ends_with("/HEAD"))
                    .map(|(r, h)| (r.to_string(), h.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Do what the Source Control view, its menus or the status bar ask.
    fn run_scm_action(&mut self, action: ScmAction) {
        use crate::workbench::source_control::CommitScope;
        let mut dirty = true;
        match action {
            ScmAction::OpenChange { root, rel, staged, kind, pinned } => {
                dirty = false;
                self.open_scm_change(&root, &rel, staged, kind, pinned);
            }
            ScmAction::OpenFile { root, rel } => {
                dirty = false;
                self.open_file(root.join(rel));
            }
            ScmAction::OpenHeadFile { root, rel } => {
                dirty = false;
                let text = crate::git::head_blob(&root, &rel).unwrap_or_default();
                let name = rel.rsplit('/').next().unwrap_or(&rel).to_string();
                let doc = Document::diff(root.join(&rel), text.clone(), text, format!("{name} (HEAD)"));
                self.documents.push(doc);
                self.active_doc = Some(self.documents.len() - 1);
            }
            ScmAction::OpenGroupChanges { root, kind } => {
                dirty = false;
                if let Some(repo) = self.git_model.repo_for(&root).cloned() {
                    let files = match kind {
                        crate::git::ResourceGroupKind::Index => repo.index.clone(),
                        crate::git::ResourceGroupKind::Merge => repo.merge.clone(),
                        _ => repo.working.clone(),
                    };
                    for f in files {
                        self.open_diff(root.join(&f.rel));
                    }
                }
            }
            ScmAction::Stage { root, rel } => {
                self.git(&root, &["add", "-A", "--", &rel]);
            }
            ScmAction::Unstage { root, rel } => {
                crate::git::unstage(&root, &rel);
            }
            ScmAction::Discard { root, rel, untracked } => {
                let name = rel.rsplit('/').next().unwrap_or(&rel).to_string();
                let ok = if untracked {
                    Self::confirm(
                        &format!("Are you sure you want to DELETE the following untracked file: '{name}'?"),
                        "This is IRREVERSIBLE!\nThis file will be FOREVER LOST if you proceed.",
                        "Delete File",
                    )
                } else {
                    Self::confirm(&format!("Are you sure you want to discard changes in '{name}'?"), "", "Discard File")
                };
                if ok {
                    crate::git::discard(&root, &rel, untracked);
                }
            }
            ScmAction::StageAll(root) => {
                self.git(&root, &["add", "-A"]);
            }
            ScmAction::UnstageAll(root) => {
                crate::git::unstage_all(&root);
            }
            ScmAction::DiscardAll(root) => {
                let Some(repo) = self.git_model.repo_for(&root).cloned() else { return };
                let n = repo.working.len();
                if n == 0 {
                    return;
                }
                let ok = Self::confirm(
                    &format!("Are you sure you want to discard ALL changes in {n} files?"),
                    "This is IRREVERSIBLE!\nYour current working set will be FOREVER LOST if you proceed.",
                    &format!("Discard All {n} Files"),
                );
                if ok {
                    for f in &repo.working {
                        crate::git::discard(&root, &f.rel, matches!(f.kind, crate::git::ChangeKind::Untracked));
                    }
                }
            }
            ScmAction::Ignore { root, rel } => {
                let path = root.join(".gitignore");
                let mut text = std::fs::read_to_string(&path).unwrap_or_default();
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(&rel);
                text.push('\n');
                if let Err(e) = std::fs::write(&path, text) {
                    self.extensions.notify_error(format!("Couldn't update .gitignore: {e}"));
                }
            }
            ScmAction::RevealInFinder(path) => {
                dirty = false;
                let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
            }
            ScmAction::RevealInExplorer(path) => {
                dirty = false;
                self.show_view(ActivityView::Explorer);
                self.open_file(path);
            }
            ScmAction::Commit { root, mode, scope, amend, signoff } => {
                dirty = false;
                self.scm_commit(&root, mode, scope, amend, signoff, None);
            }
            ScmAction::UndoCommit(root) => {
                let merge = self
                    .git(&root, &["rev-list", "--parents", "-n", "1", "HEAD"])
                    .is_some_and(|l| l.split_whitespace().count() > 2);
                if !merge || Self::confirm("The last commit was a merge commit. Are you sure you want to undo it?", "", "Undo merge commit") {
                    if let Some(msg) = self.git(&root, &["log", "-1", "--format=%B"]) {
                        if self.git(&root, &["reset", "--soft", "HEAD~1"]).is_some() {
                            self.scm_ui.commit_messages.insert(root.clone(), msg.trim_end().to_string());
                        }
                    }
                }
            }
            ScmAction::Refresh(_) | ScmAction::GraphRefresh(_) => {}
            ScmAction::Pull { root, rebase } => {
                self.git(&root, if rebase { &["pull", "--rebase"] } else { &["pull"] });
            }
            ScmAction::Push { root } => {
                self.git(&root, &["push"]);
            }
            ScmAction::Sync { root, rebase } => {
                if self.git(&root, if rebase { &["pull", "--rebase"] } else { &["pull"] }).is_some() {
                    self.git(&root, &["push"]);
                }
            }
            ScmAction::Fetch { root, prune, all } => {
                let mut args = vec!["fetch"];
                if all {
                    args.push("--all");
                }
                if prune {
                    args.push("--prune");
                }
                self.git(&root, &args);
            }
            ScmAction::Publish(root) => {
                let remotes: Vec<String> = self.git(&root, &["remote"]).map(|s| s.lines().map(str::to_string).collect()).unwrap_or_default();
                let branch = self.git_model.repo_for(&root).and_then(|r| r.branch.clone());
                match (remotes.as_slice(), branch) {
                    (_, None) => {}
                    ([], _) => self.extensions.notify_info("Your repository has no remotes configured to publish to.".into()),
                    ([one], Some(b)) => {
                        let one = one.clone();
                        self.git(&root, &["push", "-u", &one, &b]);
                    }
                    (many, Some(b)) => {
                        let items = many.iter().map(|r| (r.clone(), None)).collect();
                        dirty = false;
                        self.git_pick(GitPrompt::PublishTo(root, b, many.to_vec()), items, "Pick a remote to publish the branch to:");
                    }
                }
            }
            ScmAction::Checkout(root) => {
                dirty = false;
                let refs = self.git_refs(&root, &["refs/heads", "refs/remotes", "refs/tags"]);
                let mut items = vec![
                    ("+ Create new branch...".to_string(), None),
                    ("+ Create new branch from...".to_string(), None),
                    ("Checkout detached...".to_string(), None),
                ];
                let mut targets = vec![GitTarget::NewBranch, GitTarget::NewBranchFrom, GitTarget::Detached];
                for (r, h) in refs {
                    let (label, kind) = if let Some(n) = r.strip_prefix("refs/heads/") {
                        (n.to_string(), "")
                    } else if let Some(n) = r.strip_prefix("refs/remotes/") {
                        (n.to_string(), "Remote branch at ")
                    } else {
                        (r.trim_start_matches("refs/tags/").to_string(), "Tag at ")
                    };
                    items.push((label.clone(), Some(format!("{kind}{h}"))));
                    targets.push(GitTarget::Ref(r));
                }
                self.git_pick(GitPrompt::Checkout(root, targets), items, "Select a branch or tag to checkout");
            }
            ScmAction::CreateBranch { root, from } => {
                dirty = false;
                self.git_input(GitPrompt::NewBranch(root, from), "Please provide a new branch name", "Branch name", "");
            }
            ScmAction::CreateBranchFromPick(root) => {
                dirty = false;
                let refs = self.git_refs(&root, &["refs/heads", "refs/remotes", "refs/tags"]);
                let items = refs.iter().map(|(r, h)| (short_ref(r), Some(h.clone()))).collect();
                self.git_pick(GitPrompt::BranchFrom(root, refs.into_iter().map(|r| r.0).collect()), items, "Select a ref to create the branch from");
            }
            ScmAction::RenameBranch(root) => {
                dirty = false;
                let current = self.git_model.repo_for(&root).and_then(|r| r.branch.clone()).unwrap_or_default();
                self.git_input(GitPrompt::RenameBranch(root), "Please provide a new branch name", "Branch name", &current);
            }
            ScmAction::DeleteBranch(root) => {
                dirty = false;
                let current = self.git_model.repo_for(&root).and_then(|r| r.branch.clone());
                let refs: Vec<String> = self
                    .git_refs(&root, &["refs/heads"])
                    .into_iter()
                    .map(|(r, _)| r.trim_start_matches("refs/heads/").to_string())
                    .filter(|b| Some(b) != current.as_ref())
                    .collect();
                let items = refs.iter().map(|b| (b.clone(), None)).collect();
                self.git_pick(GitPrompt::DeleteBranch(root, refs), items, "Select a branch to delete");
            }
            ScmAction::Merge(root) => {
                dirty = false;
                let refs = self.git_refs(&root, &["refs/heads", "refs/remotes", "refs/tags"]);
                let items = refs.iter().map(|(r, h)| (short_ref(r), Some(h.clone()))).collect();
                self.git_pick(GitPrompt::Merge(root, refs.into_iter().map(|r| r.0).collect()), items, "Select a branch or tag to merge from");
            }
            ScmAction::Rebase(root) => {
                dirty = false;
                let refs = self.git_refs(&root, &["refs/heads", "refs/remotes"]);
                let items = refs.iter().map(|(r, h)| (short_ref(r), Some(h.clone()))).collect();
                self.git_pick(GitPrompt::Rebase(root, refs.into_iter().map(|r| r.0).collect()), items, "Select a branch to rebase onto");
            }
            ScmAction::CreateTag { root, rev } => {
                dirty = false;
                self.git_input(GitPrompt::TagName(root, rev), "Please provide a tag name", "Tag name", "");
            }
            ScmAction::DeleteTag(root) => {
                dirty = false;
                let tags: Vec<String> = self
                    .git_refs(&root, &["refs/tags"])
                    .into_iter()
                    .map(|(r, _)| r.trim_start_matches("refs/tags/").to_string())
                    .collect();
                let items = tags.iter().map(|t| (t.clone(), None)).collect();
                self.git_pick(GitPrompt::DeleteTag(root, tags), items, "Select a tag to delete");
            }
            ScmAction::PushTags(root) => {
                self.git(&root, &["push", "--tags"]);
            }
            ScmAction::Stash { root, untracked, staged } => {
                dirty = false;
                self.git_input(GitPrompt::StashMessage(root, untracked, staged), "Optionally provide a stash message", "Stash message", "");
            }
            ScmAction::StashApplyLatest(root) => {
                self.git(&root, &["stash", "apply"]);
            }
            ScmAction::StashPopLatest(root) => {
                self.git(&root, &["stash", "pop"]);
            }
            ScmAction::StashApply(root) => {
                dirty = false;
                self.pick_stash(root, "apply");
            }
            ScmAction::StashPop(root) => {
                dirty = false;
                self.pick_stash(root, "pop");
            }
            ScmAction::StashDrop(root) => {
                dirty = false;
                self.pick_stash(root, "drop");
            }
            ScmAction::StashDropAll(root) => {
                let n = self.git(&root, &["stash", "list"]).map(|s| s.lines().count()).unwrap_or(0);
                if n > 0
                    && Self::confirm(
                        &format!(
                            "Are you sure you want to drop ALL stashes? There {} that will be subject to pruning, and MAY BE IMPOSSIBLE TO RECOVER.",
                            if n == 1 { "is 1 stash".to_string() } else { format!("are {n} stashes") }
                        ),
                        "",
                        "Drop All Stashes",
                    )
                {
                    self.git(&root, &["stash", "clear"]);
                }
            }
            ScmAction::AddRemote(root) => {
                dirty = false;
                self.git_input(GitPrompt::RemoteUrl(root), "Please provide a repository URL", "Repository URL", "");
            }
            ScmAction::RemoveRemote(root) => {
                dirty = false;
                let remotes: Vec<String> = self.git(&root, &["remote"]).map(|s| s.lines().map(str::to_string).collect()).unwrap_or_default();
                let items = remotes.iter().map(|r| (r.clone(), None)).collect();
                self.git_pick(GitPrompt::RemoveRemote(root, remotes), items, "Pick a remote to remove");
            }
            ScmAction::CheckoutDetached { root, id } => {
                self.git(&root, &["checkout", "--detach", &id]);
            }
            ScmAction::CherryPick { root, id } => {
                self.git(&root, &["cherry-pick", &id]);
            }
            ScmAction::CopyText(t) => {
                dirty = false;
                self.pending_copy = Some(t);
            }
            ScmAction::OpenCommitDiff { root, rel, parent, commit, pinned } => {
                dirty = false;
                self.open_commit_diff(&root, &rel, &parent, &commit, pinned);
            }
            ScmAction::GraphFilterPick(root) => {
                dirty = false;
                let refs = self.git_refs(&root, &["refs/heads", "refs/remotes", "refs/tags"]);
                let mut items = vec![
                    ("Auto".to_string(), Some("Current history item reference(s)".to_string())),
                    ("All".to_string(), Some("All history item references".to_string())),
                ];
                let mut filters = vec![
                    crate::workbench::source_control::GraphFilter::Auto,
                    crate::workbench::source_control::GraphFilter::All,
                ];
                for (r, h) in refs {
                    items.push((short_ref(&r), Some(h)));
                    filters.push(crate::workbench::source_control::GraphFilter::Ref(r));
                }
                self.git_pick(GitPrompt::GraphFilter(filters), items, "Select one/more history item references to view, press Enter to confirm");
            }
            ScmAction::TogglePane(changes) => {
                dirty = false;
                let ui = &mut self.scm_ui;
                if changes {
                    ui.changes_visible = !ui.changes_visible || !ui.graph_visible;
                } else {
                    ui.graph_visible = !ui.graph_visible || !ui.changes_visible;
                }
            }
            ScmAction::GraphViewTree(tree) => {
                dirty = false;
                self.scm_ui.graph_tree = tree;
            }
            ScmAction::ManageWorkspaceTrust => self.open_trust_editor(),
            ScmAction::InitRepository => {
                if let Some(root) = self.workspace_root.clone() {
                    if self.git(&root, &["init"]).is_some() {
                        self.git_model = crate::git::Model::discover(&root);
                    }
                }
            }
            ScmAction::OpenParentRepositories => {
                let repos = std::mem::take(&mut self.scm_ui.parent_repos);
                for r in repos {
                    if !self.git_model.repos.iter().any(|x| x.root == r) {
                        self.git_model.repos.push(crate::git::status_v2(&r));
                    }
                }
            }
            ScmAction::OpenFolder => {
                dirty = false;
                self.open_folder_dialog();
            }
            ScmAction::CloneRepository => {
                dirty = false;
                self.git_input(GitPrompt::CloneUrl, "Provide repository URL", "Repository URL", "");
            }
        }
        if dirty {
            self.refresh_git();
        }
    }

    /// "Apply / Pop / Drop Stash...": pick one of the stashes.
    fn pick_stash(&mut self, root: PathBuf, verb: &'static str) {
        let stashes: Vec<String> = self.git(&root, &["stash", "list"]).map(|s| s.lines().map(str::to_string).collect()).unwrap_or_default();
        if stashes.is_empty() {
            self.extensions.notify_info("There are no stashes in the repository.".into());
            return;
        }
        let items = stashes
            .iter()
            .map(|l| {
                let (id, msg) = l.split_once(": ").unwrap_or((l.as_str(), ""));
                (msg.to_string(), Some(id.to_string()))
            })
            .collect();
        let ids = stashes.iter().map(|l| l.split(':').next().unwrap_or("").to_string()).collect();
        self.git_pick(GitPrompt::Stash(root, verb, ids), items, "Pick a stash");
    }

    /// `git.commit` and friends: the message from the box (or asked for),
    /// the smart-commit question when nothing is staged.
    fn scm_commit(
        &mut self,
        root: &Path,
        mode: crate::git::CommitMode,
        scope: crate::workbench::source_control::CommitScope,
        amend: bool,
        signoff: bool,
        message: Option<String>,
    ) {
        use crate::workbench::source_control::CommitScope;
        let Some(repo) = self.git_model.repo_for(root).cloned() else { return };
        let mut all = scope == CommitScope::All;
        if scope == CommitScope::Default && repo.index.is_empty() && !amend {
            if repo.working.is_empty() && repo.merge.is_empty() {
                self.extensions.notify_info("There are no changes to commit.".into());
                return;
            }
            if !self.smart_commit {
                let pick = rfd::MessageDialog::new()
                    .set_level(rfd::MessageLevel::Warning)
                    .set_title("There are no staged changes to commit.")
                    .set_description("Would you like to stage all your changes and commit them directly?")
                    .set_buttons(rfd::MessageButtons::YesNoCancelCustom("Yes".into(), "Always".into(), "Never".into()))
                    .show();
                match pick {
                    rfd::MessageDialogResult::Custom(s) if s == "Always" => self.smart_commit = true,
                    rfd::MessageDialogResult::Custom(s) if s == "Yes" => {}
                    _ => return,
                }
            }
            all = true;
        }
        let msg = match message {
            Some(m) => m,
            None => self.scm_ui.commit_messages.get(root).map(|m| m.trim().to_string()).unwrap_or_default(),
        };
        if msg.is_empty() && !amend {
            self.git_input(GitPrompt::CommitMessage(root.to_path_buf(), mode, scope, signoff), "Please provide a commit message", "Commit message", "");
            return;
        }
        if all {
            self.git(root, &["add", "-A"]);
        }
        let mut args: Vec<&str> = vec!["commit", "--quiet"];
        if amend {
            args.push("--amend");
            if msg.is_empty() {
                args.push("--no-edit");
            }
        }
        if signoff {
            args.push("--signoff");
        }
        if !msg.is_empty() {
            args.push("-m");
            args.push(&msg);
        }
        if self.git(root, &args).is_some() {
            self.scm_ui.commit_messages.remove(root);
            match mode {
                crate::git::CommitMode::Plain => {}
                crate::git::CommitMode::AndPush => {
                    self.git(root, &["push"]);
                }
                crate::git::CommitMode::AndSync => {
                    if self.git(root, &["pull"]).is_some() {
                        self.git(root, &["push"]);
                    }
                }
            }
        }
        self.refresh_git();
    }

    /// The answer to a Source Control pick or input. `true` when the request
    /// was one of ours.
    fn git_prompt_answered(&mut self, request_id: u64, index: Option<usize>, text: Option<String>) -> bool {
        let Some((req, _)) = &self.git_prompt else { return false };
        if *req != request_id {
            return false;
        }
        let (_, prompt) = self.git_prompt.take().unwrap();
        // `None` = dismissed; `Some("")` = submitted empty.
        let submitted = text.is_some();
        let text = text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        match prompt {
            GitPrompt::Checkout(root, targets) => match index.and_then(|i| targets.get(i)).cloned() {
                Some(GitTarget::NewBranch) => self.run_scm_action(ScmAction::CreateBranch { root, from: None }),
                Some(GitTarget::NewBranchFrom) => self.run_scm_action(ScmAction::CreateBranchFromPick(root)),
                Some(GitTarget::Detached) => {
                    let refs = self.git_refs(&root, &["refs/heads", "refs/remotes", "refs/tags"]);
                    let items = refs.iter().map(|(r, h)| (short_ref(r), Some(h.clone()))).collect();
                    self.git_pick(GitPrompt::Detached(root, refs.into_iter().map(|r| r.0).collect()), items, "Select a branch to checkout in detached mode");
                }
                Some(GitTarget::Ref(r)) => {
                    if let Some(local) = r.strip_prefix("refs/heads/") {
                        self.git(&root, &["checkout", local]);
                    } else if let Some(remote) = r.strip_prefix("refs/remotes/") {
                        // A remote branch: check out a local tracking branch.
                        let local = remote.split_once('/').map(|(_, b)| b).unwrap_or(remote).to_string();
                        if self.git(&root, &["checkout", "-q", &local]).is_none() {
                            self.git(&root, &["checkout", "--track", remote]);
                        }
                    } else {
                        self.git(&root, &["checkout", "--detach", &r]);
                    }
                    self.refresh_git();
                }
                None => {}
            },
            GitPrompt::Detached(root, refs) => {
                if let Some(r) = index.and_then(|i| refs.get(i)) {
                    self.git(&root, &["checkout", "--detach", r]);
                    self.refresh_git();
                }
            }
            GitPrompt::NewBranch(root, from) => {
                if let Some(name) = text {
                    let name = name.replace(char::is_whitespace, "-");
                    let mut args = vec!["checkout", "-q", "-b", name.as_str()];
                    if let Some(f) = &from {
                        args.push(f);
                    }
                    self.git(&root, &args);
                    self.refresh_git();
                }
            }
            GitPrompt::BranchFrom(root, refs) => {
                if let Some(r) = index.and_then(|i| refs.get(i)).cloned() {
                    self.run_scm_action(ScmAction::CreateBranch { root, from: Some(r) });
                }
            }
            GitPrompt::RenameBranch(root) => {
                if let Some(name) = text {
                    self.git(&root, &["branch", "-m", &name]);
                    self.refresh_git();
                }
            }
            GitPrompt::DeleteBranch(root, branches) => {
                if let Some(b) = index.and_then(|i| branches.get(i)).cloned() {
                    if self.git(&root, &["branch", "-d", &b]).is_none()
                        && Self::confirm(&format!("The branch '{b}' is not fully merged. Delete anyway?"), "", "Delete Branch")
                    {
                        self.git(&root, &["branch", "-D", &b]);
                    }
                    self.refresh_git();
                }
            }
            GitPrompt::Merge(root, refs) => {
                if let Some(r) = index.and_then(|i| refs.get(i)).cloned() {
                    self.git(&root, &["merge", &short_ref(&r)]);
                    self.refresh_git();
                }
            }
            GitPrompt::Rebase(root, refs) => {
                if let Some(r) = index.and_then(|i| refs.get(i)).cloned() {
                    self.git(&root, &["rebase", &short_ref(&r)]);
                    self.refresh_git();
                }
            }
            GitPrompt::TagName(root, rev) => {
                if let Some(name) = text {
                    self.git_input(GitPrompt::TagMessage(root, name, rev), "Please provide a message to annotate the tag", "Message", "");
                }
            }
            GitPrompt::TagMessage(root, name, rev) => {
                let mut args = vec!["tag"];
                let msg = text.unwrap_or_default();
                if !msg.is_empty() {
                    args.extend_from_slice(&["-a", name.as_str(), "-m", msg.as_str()]);
                } else {
                    args.push(&name);
                }
                if let Some(r) = &rev {
                    args.push(r);
                }
                self.git(&root, &args);
                self.refresh_git();
            }
            GitPrompt::DeleteTag(root, tags) => {
                if let Some(t) = index.and_then(|i| tags.get(i)).cloned() {
                    self.git(&root, &["tag", "-d", &t]);
                    self.refresh_git();
                }
            }
            GitPrompt::StashMessage(root, untracked, staged) => {
                // Dismissing the box cancels; an empty message is fine.
                if !submitted {
                    return true;
                }
                let mut args = vec!["stash", "push"];
                if untracked {
                    args.push("--include-untracked");
                }
                if staged {
                    args.push("--staged");
                }
                let msg = text.unwrap_or_default();
                if !msg.is_empty() {
                    args.extend_from_slice(&["-m", msg.as_str()]);
                }
                self.git(&root, &args);
                self.refresh_git();
            }
            GitPrompt::Stash(root, verb, ids) => {
                if let Some(id) = index.and_then(|i| ids.get(i)).cloned() {
                    self.git(&root, &["stash", verb, &id]);
                    self.refresh_git();
                }
            }
            GitPrompt::RemoteUrl(root) => {
                if let Some(url) = text {
                    self.git_input(GitPrompt::RemoteName(root, url), "Please provide a remote name", "Remote name", "origin");
                }
            }
            GitPrompt::RemoteName(root, url) => {
                if let Some(name) = text {
                    self.git(&root, &["remote", "add", &name, &url]);
                    self.refresh_git();
                }
            }
            GitPrompt::RemoveRemote(root, remotes) => {
                if let Some(r) = index.and_then(|i| remotes.get(i)).cloned() {
                    self.git(&root, &["remote", "remove", &r]);
                    self.refresh_git();
                }
            }
            GitPrompt::PublishTo(root, branch, remotes) => {
                if let Some(r) = index.and_then(|i| remotes.get(i)).cloned() {
                    self.git(&root, &["push", "-u", &r, &branch]);
                    self.refresh_git();
                }
            }
            GitPrompt::CommitMessage(root, mode, scope, signoff) => {
                if let Some(msg) = text {
                    self.scm_commit(&root, mode, scope, false, signoff, Some(msg));
                }
            }
            GitPrompt::GraphFilter(filters) => {
                if let Some(f) = index.and_then(|i| filters.get(i)).cloned() {
                    self.scm_ui.graph_filter = f;
                    self.refresh_git();
                }
            }
            GitPrompt::CloneUrl => {
                if let Some(url) = text {
                    if let Some(parent) = rfd::FileDialog::new().set_title("Choose a folder to clone into").pick_folder() {
                        let name = url.trim_end_matches('/').rsplit('/').next().unwrap_or("repo").trim_end_matches(".git").to_string();
                        if self.git(&parent, &["clone", &url, &name]).is_some() {
                            self.set_workspace(parent.join(name));
                        }
                    }
                }
            }
        }
        true
    }

    /// The items of a Source Control menu (the git extension's menus).
    fn scm_menu_entries(&self, menu: &crate::workbench::source_control::ScmMenu) -> Vec<MenuEntry> {
        use crate::git::{CommitMode, ResourceGroupKind as K};
        use crate::workbench::source_control::{CommitScope, ScmMenu};
        let item = |l: &str, a: ScmAction| MenuEntry::Item(l.to_string(), a);
        let commit = |root: &PathBuf, scope: CommitScope, amend: bool, signoff: bool| ScmAction::Commit {
            root: root.clone(),
            mode: CommitMode::Plain,
            scope,
            amend,
            signoff,
        };
        match menu {
            ScmMenu::ChangesTitle(r) => vec![
                item("Pull", ScmAction::Pull { root: r.clone(), rebase: false }),
                item("Push", ScmAction::Push { root: r.clone() }),
                item("Clone", ScmAction::CloneRepository),
                item("Checkout to...", ScmAction::Checkout(r.clone())),
                item("Fetch", ScmAction::Fetch { root: r.clone(), prune: false, all: false }),
                MenuEntry::Separator,
                MenuEntry::Sub(
                    "Commit".into(),
                    vec![
                        item("Commit", commit(r, CommitScope::Default, false, false)),
                        item("Commit Staged", commit(r, CommitScope::Staged, false, false)),
                        item("Commit All", commit(r, CommitScope::All, false, false)),
                        item("Undo Last Commit", ScmAction::UndoCommit(r.clone())),
                        MenuEntry::Separator,
                        item("Commit (Amend)", commit(r, CommitScope::Default, true, false)),
                        item("Commit Staged (Amend)", commit(r, CommitScope::Staged, true, false)),
                        item("Commit All (Amend)", commit(r, CommitScope::All, true, false)),
                        MenuEntry::Separator,
                        item("Commit (Signed Off)", commit(r, CommitScope::Default, false, true)),
                        item("Commit Staged (Signed Off)", commit(r, CommitScope::Staged, false, true)),
                        item("Commit All (Signed Off)", commit(r, CommitScope::All, false, true)),
                    ],
                ),
                MenuEntry::Sub(
                    "Changes".into(),
                    vec![
                        item("Stage All Changes", ScmAction::StageAll(r.clone())),
                        item("Unstage All Changes", ScmAction::UnstageAll(r.clone())),
                        item("Discard All Changes", ScmAction::DiscardAll(r.clone())),
                    ],
                ),
                MenuEntry::Sub(
                    "Pull, Push".into(),
                    vec![
                        item("Sync", ScmAction::Sync { root: r.clone(), rebase: false }),
                        item("Sync (Rebase)", ScmAction::Sync { root: r.clone(), rebase: true }),
                        MenuEntry::Separator,
                        item("Pull", ScmAction::Pull { root: r.clone(), rebase: false }),
                        item("Pull (Rebase)", ScmAction::Pull { root: r.clone(), rebase: true }),
                        MenuEntry::Separator,
                        item("Push", ScmAction::Push { root: r.clone() }),
                        MenuEntry::Separator,
                        item("Fetch", ScmAction::Fetch { root: r.clone(), prune: false, all: false }),
                        item("Fetch (Prune)", ScmAction::Fetch { root: r.clone(), prune: true, all: false }),
                        item("Fetch From All Remotes", ScmAction::Fetch { root: r.clone(), prune: false, all: true }),
                    ],
                ),
                MenuEntry::Sub(
                    "Branch".into(),
                    vec![
                        item("Merge...", ScmAction::Merge(r.clone())),
                        item("Rebase Branch...", ScmAction::Rebase(r.clone())),
                        MenuEntry::Separator,
                        item("Create Branch...", ScmAction::CreateBranch { root: r.clone(), from: None }),
                        item("Create Branch From...", ScmAction::CreateBranchFromPick(r.clone())),
                        MenuEntry::Separator,
                        item("Rename Branch...", ScmAction::RenameBranch(r.clone())),
                        item("Delete Branch...", ScmAction::DeleteBranch(r.clone())),
                        MenuEntry::Separator,
                        item("Publish Branch...", ScmAction::Publish(r.clone())),
                    ],
                ),
                MenuEntry::Sub(
                    "Remote".into(),
                    vec![item("Add Remote...", ScmAction::AddRemote(r.clone())), item("Remove Remote", ScmAction::RemoveRemote(r.clone()))],
                ),
                MenuEntry::Sub(
                    "Stash".into(),
                    vec![
                        item("Stash", ScmAction::Stash { root: r.clone(), untracked: false, staged: false }),
                        item("Stash (Include Untracked)", ScmAction::Stash { root: r.clone(), untracked: true, staged: false }),
                        item("Stash Staged", ScmAction::Stash { root: r.clone(), untracked: false, staged: true }),
                        MenuEntry::Separator,
                        item("Apply Latest Stash", ScmAction::StashApplyLatest(r.clone())),
                        item("Apply Stash...", ScmAction::StashApply(r.clone())),
                        MenuEntry::Separator,
                        item("Pop Latest Stash", ScmAction::StashPopLatest(r.clone())),
                        item("Pop Stash...", ScmAction::StashPop(r.clone())),
                        MenuEntry::Separator,
                        item("Drop Stash...", ScmAction::StashDrop(r.clone())),
                        item("Drop All Stashes...", ScmAction::StashDropAll(r.clone())),
                    ],
                ),
                MenuEntry::Sub(
                    "Tags".into(),
                    vec![
                        item("Create Tag...", ScmAction::CreateTag { root: r.clone(), rev: None }),
                        item("Delete Tag...", ScmAction::DeleteTag(r.clone())),
                        MenuEntry::Separator,
                        item("Push Tags", ScmAction::PushTags(r.clone())),
                    ],
                ),
            ],
            ScmMenu::CommitDropdown(r) => vec![
                item("Commit", commit(r, CommitScope::Default, false, false)),
                item("Commit (Amend)", commit(r, CommitScope::Default, true, false)),
                item("Commit & Push", ScmAction::Commit { root: r.clone(), mode: CommitMode::AndPush, scope: CommitScope::Default, amend: false, signoff: false }),
                item("Commit & Sync", ScmAction::Commit { root: r.clone(), mode: CommitMode::AndSync, scope: CommitScope::Default, amend: false, signoff: false }),
            ],
            ScmMenu::Group { root, kind } => match kind {
                K::Index => vec![item("Unstage All Changes", ScmAction::UnstageAll(root.clone()))],
                K::Merge => vec![item("Stage All Merge Changes", ScmAction::StageAll(root.clone()))],
                _ => vec![
                    item("Discard All Changes", ScmAction::DiscardAll(root.clone())),
                    item("Stage All Changes", ScmAction::StageAll(root.clone())),
                ],
            },
            ScmMenu::File { root, rel, kind, untracked } => {
                let path = root.join(rel);
                let mut v = vec![
                    item("Open Changes", ScmAction::OpenChange {
                        root: root.clone(),
                        rel: rel.clone(),
                        staged: *kind == K::Index,
                        kind: self
                            .git_model
                            .repo_for(root)
                            .and_then(|r| {
                                let list = if *kind == K::Index { &r.index } else { &r.working };
                                list.iter().find(|f| &f.rel == rel).map(|f| f.kind)
                            })
                            .unwrap_or(crate::git::ChangeKind::Modified),
                        pinned: true,
                    }),
                    item("Open File", ScmAction::OpenFile { root: root.clone(), rel: rel.clone() }),
                ];
                if *kind != K::Merge {
                    v.push(item("Open File (HEAD)", ScmAction::OpenHeadFile { root: root.clone(), rel: rel.clone() }));
                }
                v.push(MenuEntry::Separator);
                match kind {
                    K::Index => v.push(item("Unstage Changes", ScmAction::Unstage { root: root.clone(), rel: rel.clone() })),
                    K::Merge => v.push(item("Stage Changes", ScmAction::Stage { root: root.clone(), rel: rel.clone() })),
                    _ => {
                        v.push(item("Stage Changes", ScmAction::Stage { root: root.clone(), rel: rel.clone() }));
                        v.push(item("Discard Changes", ScmAction::Discard { root: root.clone(), rel: rel.clone(), untracked: *untracked }));
                        v.push(item("Add to .gitignore", ScmAction::Ignore { root: root.clone(), rel: rel.clone() }));
                    }
                }
                v.push(MenuEntry::Separator);
                v.push(item("Reveal in Finder", ScmAction::RevealInFinder(path.clone())));
                v.push(item("Reveal in Explorer View", ScmAction::RevealInExplorer(path)));
                v
            }
            ScmMenu::Commit { root, id, summary, .. } => vec![
                item("Checkout (Detached)", ScmAction::CheckoutDetached { root: root.clone(), id: id.clone() }),
                MenuEntry::Separator,
                item("Create Branch...", ScmAction::CreateBranch { root: root.clone(), from: Some(id.clone()) }),
                MenuEntry::Separator,
                item("Create Tag...", ScmAction::CreateTag { root: root.clone(), rev: Some(id.clone()) }),
                MenuEntry::Separator,
                item("Cherry Pick", ScmAction::CherryPick { root: root.clone(), id: id.clone() }),
                MenuEntry::Separator,
                item("Copy Commit Hash", ScmAction::CopyText(id.clone())),
                item("Copy Commit Message", ScmAction::CopyText(summary.clone())),
            ],
            ScmMenu::Views => vec![
                MenuEntry::Check("Changes".into(), ScmAction::TogglePane(true), self.scm_ui.changes_visible),
                MenuEntry::Check("Graph".into(), ScmAction::TogglePane(false), self.scm_ui.graph_visible),
            ],
            ScmMenu::GraphTitle(r) => vec![
                MenuEntry::Check("View as List".into(), ScmAction::GraphViewTree(false), !self.scm_ui.graph_tree),
                MenuEntry::Check("View as Tree".into(), ScmAction::GraphViewTree(true), self.scm_ui.graph_tree),
                MenuEntry::Separator,
                item("Refresh", ScmAction::GraphRefresh(r.clone())),
            ],
        }
    }

    /// Open a Source Control menu natively, under the pointer or button.
    /// `workbench.editor.enablePreview` (default true).
    fn preview_enabled(&self) -> bool {
        self.settings.get_bool("workbench.editor.enablePreview").unwrap_or(true)
    }

    fn show_pending_editor_menu(&mut self, frame: &eframe::Frame) {
        use muda::ContextMenu;
        use wry::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Some(pos) = self.pending_editor_menu.take() else { return };
        let root = muda::Menu::new();
        let mut actions = Vec::new();
        let close_all = muda::MenuItem::new("Close All", true, None);
        let close_saved = muda::MenuItem::new("Close Saved", true, None);
        let preview = muda::CheckMenuItem::new("Enable Preview Editors", true, self.preview_enabled(), None);
        actions.push((close_all.id().clone(), EditorMenuAction::CloseAll));
        actions.push((close_saved.id().clone(), EditorMenuAction::CloseSaved));
        actions.push((preview.id().clone(), EditorMenuAction::TogglePreview));
        let _ = root.append_items(&[&close_all, &close_saved, &muda::PredefinedMenuItem::separator(), &preview]);
        self.editor_menu_actions = actions;
        let Ok(handle) = frame.window_handle() else { return };
        if let RawWindowHandle::AppKit(appkit) = handle.as_raw() {
            let y = self.viewport_height - pos.y;
            // SAFETY: the view belongs to the live editor window.
            unsafe {
                root.show_context_menu_for_nsview(
                    appkit.ns_view.as_ptr() as *const std::ffi::c_void,
                    Some(muda::dpi::Position::Logical(muda::dpi::LogicalPosition::new(pos.x as f64, y as f64))),
                );
            }
        }
    }

    fn editor_menu_action(&mut self, action: EditorMenuAction) {
        match action {
            EditorMenuAction::CloseAll => {
                self.close_all();
                self.show_welcome = false;
            }
            EditorMenuAction::CloseSaved => {
                while let Some(i) = self.documents.iter().position(|d| !d.dirty) {
                    self.close_doc(i);
                }
                self.show_welcome = false;
            }
            EditorMenuAction::TogglePreview => {
                let value = if self.preview_enabled() { Some(serde_json::json!(false)) } else { None };
                let _ = self.settings.update("workbench.editor.enablePreview", value);
            }
        }
    }

    fn show_pending_scm_menu(&mut self, frame: &eframe::Frame) {
        use muda::ContextMenu;
        use wry::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Some((menu, pos)) = self.pending_scm_menu.take() else { return };
        let entries = self.scm_menu_entries(&menu);
        let root = muda::Menu::new();
        let mut actions = Vec::new();
        fn add(entries: Vec<MenuEntry>, actions: &mut Vec<(muda::MenuId, ScmAction)>, append: &mut dyn FnMut(&dyn muda::IsMenuItem)) {
            for e in entries {
                match e {
                    MenuEntry::Item(label, a) => {
                        let it = muda::MenuItem::new(label, true, None);
                        actions.push((it.id().clone(), a));
                        append(&it);
                    }
                    MenuEntry::Check(label, a, checked) => {
                        let it = muda::CheckMenuItem::new(label, true, checked, None);
                        actions.push((it.id().clone(), a));
                        append(&it);
                    }
                    MenuEntry::Separator => append(&muda::PredefinedMenuItem::separator()),
                    MenuEntry::Sub(label, children) => {
                        let sub = muda::Submenu::new(label, true);
                        add(children, actions, &mut |i| {
                            let _ = sub.append(i);
                        });
                        append(&sub);
                    }
                }
            }
        }
        add(entries, &mut actions, &mut |i| {
            let _ = root.append(i);
        });
        self.scm_menu_actions = actions;
        let Ok(handle) = frame.window_handle() else { return };
        if let RawWindowHandle::AppKit(appkit) = handle.as_raw() {
            let y = self.viewport_height - pos.y;
            // SAFETY: the view belongs to the live editor window.
            unsafe {
                root.show_context_menu_for_nsview(
                    appkit.ns_view.as_ptr() as *const std::ffi::c_void,
                    Some(muda::dpi::Position::Logical(muda::dpi::LogicalPosition::new(pos.x as f64, y as f64))),
                );
            }
        }
    }

    /// Open a read-only diff tab for a file at a commit (its first parent on
    /// the left, the commit on the right) — VS Code's behaviour when you click
    /// a file under a commit in the GRAPH.
    fn open_commit_diff(&mut self, root: &Path, rel: &str, parent: &str, commit: &str, pinned: bool) {
        let short = |r: &str| r.chars().take(7).collect::<String>();
        let name = rel.rsplit('/').next().unwrap_or(rel).to_string();
        let path = root.join(rel);
        let before = if parent.is_empty() { None } else { crate::git::blob_at(root, parent, rel) };
        let after = crate::git::blob_at(root, commit, rel);
        // As the git extension: an added or deleted file shows the one side
        // that exists, read-only, titled with its commit.
        let doc = match (before, after) {
            (Some(b), Some(a)) => Document::diff(path, b, a, format!("{name} ({}) ↔ {name} ({})", short(parent), short(commit))),
            (None, Some(a)) => Document::read_only(path, a, format!("{name} ({})", short(commit))),
            (Some(b), None) => Document::read_only(path, b, format!("{name} ({})", short(parent))),
            (None, None) => return,
        };
        self.open_tab(doc, pinned);
    }

    /// Open a tab as VS Code's preview editor (`pinned: false`: italic,
    /// replaced by the next preview) or keep it. A tab with the same title
    /// and file is reused (and refreshed).
    fn open_tab(&mut self, mut doc: Document, pinned: bool) {
        self.show_welcome = false;
        self.keep_welcome();
        let pinned = pinned || !self.preview_enabled();
        if let Some(i) = self.documents.iter().position(|d| {
            d.path == doc.path && d.diff_title == doc.diff_title && d.diff_base.is_some() == doc.diff_base.is_some() && d.webview.is_none()
        }) {
            let d = &mut self.documents[i];
            if d.read_only {
                d.text = doc.text;
                d.saved_text = d.text.clone();
                d.diff_base = doc.diff_base;
            }
            if pinned {
                d.preview = false;
            }
            self.active_doc = Some(i);
            return;
        }
        doc.preview = !pinned;
        match self.documents.iter().position(|d| d.preview && !d.dirty) {
            Some(i) => {
                if let Some(h) = self.documents[i].webview {
                    self.webview_host.remove_internal_page(h);
                }
                self.documents[i] = doc;
                self.active_doc = Some(i);
            }
            None => {
                self.documents.push(doc);
                self.active_doc = Some(self.documents.len() - 1);
            }
        }
        self.refresh_git_line_changes();
    }

    /// Open a changed file from Source Control the way the git extension's
    /// `ResourceCommandResolver` does: the diff of the right two versions
    /// with "(Working Tree)" / "(Index)", or the one version that exists
    /// ("(Deleted)", "(Untracked)", an added file's index copy).
    fn open_scm_change(&mut self, root: &Path, rel: &str, staged: bool, kind: crate::git::ChangeKind, pinned: bool) {
        use crate::git::ChangeKind as C;
        let name = rel.rsplit('/').next().unwrap_or(rel).to_string();
        let path = root.join(rel);
        let head = || crate::git::head_blob(root, rel).unwrap_or_default();
        let index = || crate::git::index_blob(root, rel).unwrap_or_default();
        let doc = match (staged, kind) {
            (true, C::Added | C::Copied) => Document::read_only(path, index(), format!("{name} (Index)")),
            (true, C::Deleted) | (false, C::Deleted) => Document::read_only(path, head(), format!("{name} (Deleted)")),
            (true, _) => Document::diff(path, head(), index(), format!("{name} (Index)")),
            (false, C::Untracked | C::Added | C::Conflicted | C::Ignored) => {
                // The file itself, editable, labelled with its state.
                let label = match kind {
                    C::Untracked | C::Ignored => format!("{name} (Untracked)"),
                    C::Added => format!("{name} (Intent to add)"),
                    _ => name.clone(),
                };
                match Document::open(path.clone()) {
                    Ok(mut d) => {
                        d.diff_title = Some(label);
                        d
                    }
                    Err(_) => return,
                }
            }
            (false, _) => {
                // Index ↔ working tree; a file that isn't staged compares with HEAD.
                let left = crate::git::index_blob(root, rel).unwrap_or_else(head);
                let right = std::fs::read_to_string(&path).unwrap_or_default();
                Document::diff(path, left, right, format!("{name} (Working Tree)"))
            }
        };
        self.open_tab(doc, pinned);
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
        if let Some(idx) = self.active_doc.filter(|&i| self.documents.get(i).is_some_and(|d| d.untitled)) {
            if self.save_untitled(idx) {
                let path = self.documents[idx].path.clone();
                self.set_status(format!("Saved {}", path.display()));
                self.extensions.document_saved(&path);
            }
            return;
        }
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
            self.settings_file_saved(&p);
        }
        self.refresh_git();
    }

    /// `settings.json` was saved from an editor: read it again.
    fn settings_file_saved(&mut self, path: &std::path::Path) {
        if self.keybindings_file().as_deref() == Some(path) {
            self.load_user_keybindings();
        }
        if self.settings.path() == Some(path) {
            self.settings.reload();
            self.apply_theme_settings();
        }
        if let Some(ws) = self.workspace_settings.as_mut().filter(|w| w.path() == Some(path)) {
            ws.reload();
        }
        if let Some((st, _)) = self.settings_editor.as_mut() {
            st.reset_inputs();
        }
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
            self.settings_file_saved(p);
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
            PaletteAction::ConfigureKeybinding(id) => self.open_keybindings_editor(Some(&format!("@command:{id}"))),
            PaletteAction::ExtensionCommand(id) => {
                self.extensions.execute(&id, serde_json::json!([]))
            }
            PaletteAction::PickActive { request_id, index } => self.theme_pick_active(request_id, index),
            PaletteAction::PickItemButton { request_id, .. } if request_id == THEME_PICK_ID => {
                self.theme_pick_closed(None);
                self.show_view(ActivityView::Extensions);
                self.extensions.marketplace.set_query("@id:vscode.theme-defaults".into());
            }
            PaletteAction::PickItemButton { .. } => {}
            PaletteAction::PickInputButton { request_id } => {
                if request_id == THEME_PICK_ID {
                    self.theme_pick_closed(None);
                    self.open_user_settings();
                }
            }
            PaletteAction::Picked { request_id, index } if request_id == THEME_PICK_ID => self.theme_pick_closed(index),
            PaletteAction::Picked { request_id, index } if request_id == DEBUGGER_PICK_ID => {
                if index.is_some() {
                    // `debug.installAdditionalDebuggers`.
                    self.show_view(ActivityView::Extensions);
                    self.extensions.marketplace.set_query("@category:debuggers".into());
                }
            }
            PaletteAction::Picked { request_id, index } if self.extension_version_picked(request_id, index) => {}
            PaletteAction::Picked { request_id, index } if self.git_prompt_answered(request_id, index, None) => {}
            PaletteAction::Input { request_id, value } if self.git_prompt_answered(request_id, None, value.clone()) => {}
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

    /// Points settings at `<dir>/User/settings.json` and applies them
    /// (`--user-data-dir`, like VS Code).
    pub fn bootstrap_user_data(&mut self, dir: PathBuf) {
        self.settings = crate::settings::Settings::load(&dir);
        self.trust = crate::workspace_trust::TrustStore::load(&dir);
        self.recent_folders = std::fs::read_to_string(Self::recent_folders_file(&dir))
            .ok()
            .and_then(|t| serde_json::from_str::<Vec<PathBuf>>(&t).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.is_dir())
            .collect();
        self.user_data_dir = Some(dir);
        self.apply_theme_settings();
        // `workbench.startupEditor`: "none" skips the Welcome page.
        if self.settings.get_str("workbench.startupEditor") == Some("none") {
            self.show_welcome = false;
        }
    }

    fn recent_folders_file(dir: &std::path::Path) -> PathBuf {
        dir.join("User").join("globalStorage").join("recent-folders.json")
    }

    /// Moves `folder` to the front of the recent folders and saves them.
    fn add_recent_folder(&mut self, folder: PathBuf) {
        self.recent_folders.retain(|p| p != &folder);
        self.recent_folders.insert(0, folder);
        self.recent_folders.truncate(50);
        self.save_recent_folders();
    }

    fn save_recent_folders(&self) {
        if let Some(dir) = &self.user_data_dir {
            let file = Self::recent_folders_file(dir);
            if let Some(parent) = file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(&self.recent_folders) {
                let _ = std::fs::write(file, json);
            }
        }
    }

    /// Shows the Welcome tab (opening it if it isn't open).
    fn open_welcome(&mut self) {
        if let Some(i) = self.documents.iter().position(|d| d.welcome) {
            self.active_doc = Some(i);
            return;
        }
        self.documents.push(crate::editor::Document::welcome());
        self.active_doc = Some(self.documents.len() - 1);
    }

    /// Another editor is opening: the Welcome tab stops being a preview
    /// (it stays open beside the new editor, as in VS Code).
    fn keep_welcome(&mut self) {
        for d in self.documents.iter_mut().filter(|d| d.welcome) {
            d.preview = false;
        }
    }

    /// `workbench.action.files.newUntitledFile` (⌘N, "New File..." with
    /// only the built-in Text File entry): an empty "Untitled-<n>" editor.
    fn new_untitled_file(&mut self) {
        let n = (1..)
            .find(|n| !self.documents.iter().any(|d| d.untitled && d.path == std::path::Path::new(&format!("Untitled-{n}"))))
            .unwrap_or(1);
        self.show_welcome = false;
        self.open_tab(crate::editor::Document::untitled(n), true);
    }

    /// Save As for an untitled editor: the file chosen in the save dialog.
    fn save_untitled(&mut self, idx: usize) -> bool {
        let Some(doc) = self.documents.get(idx) else { return false };
        let name = doc.display_name();
        let mut dialog = rfd::FileDialog::new().set_file_name(&name);
        if let Some(root) = &self.workspace_root {
            dialog = dialog.set_directory(root);
        }
        let Some(path) = dialog.save_file() else { return false };
        let doc = &mut self.documents[idx];
        doc.path = path;
        doc.untitled = false;
        doc.language = crate::editor::buffer::detect_language(&doc.path);
        doc.save().is_ok()
    }

    /// `workbench.colorTheme` + `workbench.colorCustomizations`.
    fn apply_theme_settings(&mut self) {
        let id = self.settings.get_str("workbench.colorTheme").unwrap_or(theme::themes::DEFAULT_THEME).to_string();
        self.set_color_theme(&id);
    }

    /// Makes `id` the active colour theme (falls back to the default for an
    /// unknown id, as VS Code does), with the user's colour customizations.
    fn set_color_theme(&mut self, id: &str) {
        let info = theme::themes::find(id)
            .or_else(|| theme::themes::find(theme::themes::DEFAULT_THEME))
            .expect("default theme");
        let mut t = theme::ColorTheme::builtin(info);
        if let Some(serde_json::Value::Object(custom)) = self.settings.get("workbench.colorCustomizations") {
            // Global entries, then `[Theme Name]` sections for this theme.
            let mut merged = serde_json::Map::new();
            for (k, v) in custom {
                if !k.starts_with('[') {
                    merged.insert(k.clone(), v.clone());
                }
            }
            for (k, v) in custom {
                if let (Some(names), serde_json::Value::Object(section)) = (k.strip_prefix('[').and_then(|k| k.strip_suffix(']')), v) {
                    if names.split("][").any(|n| n == info.id || n == info.label) {
                        merged.extend(section.clone());
                    }
                }
            }
            t.set_customizations(&merged);
        }
        theme::set_theme(t);
    }

    /// Preferences: Color Theme (`workbench.action.selectTheme`,
    /// themes.contribution.ts): light, dark and high contrast groups, the
    /// default themes pinned first, the rest by label; moving through the
    /// list previews the theme.
    fn open_color_theme_picker(&mut self) {
        use theme::registry::ThemeKind;
        let current = theme::current().id.clone();
        let mut items = vec![("$(plus) Browse Additional Color Themes...".to_string(), None, None, false)];
        let mut entries: Vec<Option<&'static str>> = vec![None];
        let groups: [(&str, fn(ThemeKind) -> bool); 3] = [
            ("light themes", |k| k == ThemeKind::Light),
            ("dark themes", |k| k == ThemeKind::Dark),
            ("high contrast themes", |k| k.is_high_contrast()),
        ];
        for (label, filter) in groups {
            let mut themes: Vec<&theme::themes::ThemeInfo> =
                theme::themes::BUILTIN.iter().filter(|t| filter(t.kind)).collect();
            let pinned = |t: &theme::themes::ThemeInfo| {
                t.id == theme::themes::DEFAULT_THEME || t.id == theme::themes::DEFAULT_LIGHT_THEME
            };
            themes.sort_by(|a, b| pinned(b).cmp(&pinned(a)).then_with(|| locale_compare(a.label, b.label)));
            if themes.is_empty() {
                continue;
            }
            items.push((label.to_string(), None, None, true));
            entries.push(None);
            for t in themes {
                let description = match t.id {
                    theme::themes::DEFAULT_THEME => Some("Default Dark".to_string()),
                    theme::themes::DEFAULT_LIGHT_THEME => Some("Default Light".to_string()),
                    id if id != t.label => Some(id.to_string()),
                    _ => None,
                };
                items.push((t.label.to_string(), description, None, false));
                entries.push(Some(t.id));
            }
        }
        let active = entries.iter().position(|e| *e == Some(current.as_str()));
        self.theme_pick = Some(ThemePick { entries, original: current });
        self.palette.open_pick_with(
            THEME_PICK_ID,
            items,
            Some("Select Color Theme (detect system color mode disabled)".into()),
            crate::workbench::command_palette::PickOptions {
                active,
                report_active: true,
                item_button: crate::icons::codicon_by_name("gear").map(|g| (g, "Manage Extension")),
                input_button: Some((
                    crate::icons::codicon_by_name("color-mode").unwrap_or('?'),
                    "Detect system color mode disabled. Click to configure.",
                )),
            },
        );
    }

    /// The picker's focus moved: preview that theme.
    fn theme_pick_active(&mut self, request_id: u64, index: usize) {
        if request_id != THEME_PICK_ID {
            return;
        }
        if let Some(Some(id)) = self.theme_pick.as_ref().and_then(|p| p.entries.get(index)).copied() {
            if theme::current().id != id {
                self.set_color_theme(id);
            }
        }
    }

    /// The picker closed: keep the chosen theme (and write
    /// `workbench.colorTheme`), or go back to the one it opened with.
    fn theme_pick_closed(&mut self, index: Option<usize>) {
        let Some(pick) = self.theme_pick.take() else { return };
        match index.and_then(|i| pick.entries.get(i).copied()) {
            Some(Some(id)) => {
                self.set_color_theme(id);
                // themeConfiguration.ts writeConfiguration (USER target): the
                // default value is removed rather than written.
                let user = self.settings.get_str("workbench.colorTheme").map(str::to_string);
                let result = if id == theme::themes::DEFAULT_THEME {
                    if user.is_some() { self.settings.update("workbench.colorTheme", None) } else { Ok(()) }
                } else if user.as_deref() != Some(id) {
                    self.settings.update("workbench.colorTheme", Some(serde_json::json!(id)))
                } else {
                    Ok(())
                };
                if let Err(e) = result {
                    self.set_status(format!("Unable to write into user settings: {e}"));
                }
            }
            Some(None) => {
                // "Browse Additional Color Themes...": the catalogue's themes.
                self.set_color_theme(&pick.original);
                self.show_view(ActivityView::Extensions);
                self.extensions.marketplace.set_query("@category:themes".into());
            }
            None => self.set_color_theme(&pick.original),
        }
    }

    /// Opens the user `settings.json` in an editor (created empty when
    /// missing).
    fn open_user_settings(&mut self) {
        let Some(path) = self.settings.path().map(PathBuf::from) else { return };
        if !path.exists() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&path, "{}");
        }
        self.show_welcome = false;
        self.open_file(path);
    }

    /// Search view requests: replace in files, open the search editor.
    fn dispatch_search(&mut self, out: crate::search::ui::SearchOutput) {
        if out.replace_all {
            self.search_replace(None);
        }
        if let Some((path, hit)) = out.replace {
            self.search_replace(Some((path, hit)));
        }
        if out.open_search_editor {
            self.open_search_editor();
        }
    }

    /// Replaces results with the Replace text: all of them, one file's
    /// (`hit` None) or one match. Open editors are edited in place (and
    /// left dirty, as VS Code does); other files are written.
    fn search_replace(&mut self, target: Option<(PathBuf, Option<(usize, usize)>)>) {
        let Some(outcome) = self.search.outcome.take() else { return };
        let replacement = self.search.replace.clone();
        let regex = self.search.regex;
        let pattern = if regex {
            regex::RegexBuilder::new(&self.search.query).case_insensitive(!self.search.match_case).build().ok()
        } else {
            None
        };
        let mut replaced = 0usize;
        for file in &outcome.results {
            if target.as_ref().is_some_and(|(p, _)| p != &file.path) {
                continue;
            }
            let hits: Vec<_> = file
                .hits
                .iter()
                .filter(|h| !self.search.dismissed.contains(&(file.path.clone(), h.line, h.byte_start)))
                .filter(|h| match &target {
                    Some((_, Some((line, byte)))) => h.line == *line && h.byte_start == *byte,
                    _ => true,
                })
                .collect();
            if hits.is_empty() {
                continue;
            }
            let open = self.documents.iter().position(|d| d.path == file.path && !d.read_only);
            let text = match open {
                Some(i) => self.documents[i].text.clone(),
                None => match std::fs::read_to_string(&file.path) {
                    Ok(t) => t,
                    Err(_) => continue,
                },
            };
            // Line start byte offsets.
            let mut starts = vec![0usize];
            for (i, b) in text.bytes().enumerate() {
                if b == b'\n' {
                    starts.push(i + 1);
                }
            }
            let mut edits: Vec<(usize, usize, String)> = hits
                .iter()
                .filter_map(|h| {
                    let s = starts.get(h.line - 1)? + h.byte_start;
                    let e = s + h.match_len;
                    let matched = text.get(s..e)?;
                    let new = match &pattern {
                        Some(re) => re.replace(matched, replacement.as_str()).into_owned(),
                        None if self.search.preserve_case => preserve_case(matched, &replacement),
                        None => replacement.clone(),
                    };
                    Some((s, e, new))
                })
                .collect();
            edits.sort_by(|a, b| b.0.cmp(&a.0));
            let mut new_text = text.clone();
            for (s, e, r) in &edits {
                new_text.replace_range(*s..*e, r);
            }
            replaced += edits.len();
            match open {
                Some(i) => {
                    self.documents[i].text = new_text;
                    self.documents[i].check_dirty();
                }
                None => {
                    let _ = std::fs::write(&file.path, new_text);
                }
            }
        }
        self.set_status(format!("Replaced {replaced} occurrence{}", if replaced == 1 { "" } else { "s" }));
        // Search again: the results now show what's left.
        if let Some(root) = self.workspace_root.clone() {
            self.search.outcome = Some(crate::search::engine::run(&root, &self.search.engine_query(None)));
        }
    }

    /// "Open in editor" / New Search Editor: the results as a search editor
    /// (`# Query:` header, then each file and its matching lines).
    fn open_search_editor(&mut self) {
        let mut text = format!("# Query: {}\n", self.search.query);
        if self.search.match_case || self.search.regex || self.search.whole_word {
            let mut flags = Vec::new();
            if self.search.regex { flags.push("RegExp"); }
            if self.search.match_case { flags.push("CaseSensitive"); }
            if self.search.whole_word { flags.push("WordMatch"); }
            text.push_str(&format!("# Flags: {}\n", flags.join(" ")));
        }
        if !self.search.include.is_empty() {
            text.push_str(&format!("# Including: {}\n", self.search.include));
        }
        if !self.search.exclude.is_empty() {
            text.push_str(&format!("# Excluding: {}\n", self.search.exclude));
        }
        text.push('\n');
        if let (Some(o), Some(root)) = (&self.search.outcome, &self.workspace_root) {
            text.push_str(&format!("{} result{} - {} file{}\n", o.total_hits, if o.total_hits == 1 { "" } else { "s" }, o.results.len(), if o.results.len() == 1 { "" } else { "s" }));
            for f in &o.results {
                let rel = f.path.strip_prefix(root).unwrap_or(&f.path).display();
                text.push_str(&format!("\n{rel}:\n"));
                let mut seen = std::collections::BTreeSet::new();
                for h in &f.hits {
                    if seen.insert(h.line) {
                        let line = std::fs::read_to_string(&f.path).ok().and_then(|t| t.lines().nth(h.line - 1).map(str::to_string)).unwrap_or_default();
                        text.push_str(&format!("  {}: {}\n", h.line, line));
                    }
                }
            }
        }
        let title = if self.search.query.is_empty() { "Search".to_string() } else { format!("Search: {}", self.search.query) };
        let mut doc = Document::read_only(PathBuf::from(format!("{title}.code-search")), text, title);
        doc.read_only = false;
        self.open_tab(doc, true);
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
            CommandId::SelectColorTheme => self.open_color_theme_picker(),
            CommandId::OpenSettings => self.open_settings_editor(None),
            CommandId::OpenKeybindings => self.open_keybindings_editor(None),
            CommandId::OpenKeybindingsJson => self.open_keybindings_json(),
            CommandId::ManageWorkspaceTrust => self.open_trust_editor(),
            CommandId::OpenUserSettingsJson => self.open_settings_json(crate::workbench::settings_editor::Target::User, None),
            CommandId::OpenWorkspaceSettings => {
                self.open_settings_editor(None);
                if self.workspace_root.is_some() {
                    if let Some((st, _)) = self.settings_editor.as_mut() {
                        st.target = crate::workbench::settings_editor::Target::Workspace;
                    }
                }
            }
        }
    }

    fn title_bar(&mut self, ctx: &Context) {
        let bar_height = 35.0;
        let mut toggle_panel_clicked = false;
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
                    .fill(Palette::TITLE_BAR_BG())
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                // Modern UI: no border; the cards start right below.

                use crate::icons::codicon_font;
                // The title bar's items sit 2px lower in Modern UI
                // (titlebar.css `padding-top: 2px`).
                let cy = rect.top() + (bar_height + 2.0) / 2.0;
                let icon = |name: &str| crate::icons::codicon_by_name(name).unwrap_or('?');

                // Centre: the command center in compact mode, as VS Code 1.145
                // draws it with AI features off (commandCenterControl.ts):
                // 38vw (≤ 600) × 22 + border, label left-aligned 8px in, no
                // search icon; Back / Forward to its left. The group is centred.
                let workspace_name = self
                    .workspace_root
                    .as_ref()
                    .and_then(|r| r.file_name())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "vscode-rust".to_string());
                let pill_w = (rect.width() * 0.38).min(600.0).floor() + 2.0;
                let pill_left = (rect.center().x - pill_w / 2.0 + 26.0).round();
                let pill = egui::Rect::from_min_size(egui::pos2(pill_left, cy - 12.0), egui::vec2(pill_w, 24.0));

                // Back / Forward, centred 52px and 16px left of the box.
                for (i, (name, tip, dx)) in
                    [("arrow-left", "Go Back (⌃-)", 52.0), ("arrow-right", "Go Forward (⌃⇧-)", 16.0)].into_iter().enumerate()
                {
                    let r = egui::Rect::from_center_size(egui::pos2(pill.left() - dx, cy), egui::vec2(22.0, 22.0));
                    let resp = ui.interact(r, ui.id().with(("tnav", i)), egui::Sense::click());
                    // No navigation history yet: both stay disabled.
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, icon(name).to_string(), codicon_font(16.0), Palette::VSCE_DISABLED_FG());
                    resp.on_hover_text(tip);
                }

                let pill_resp =
                    ui.interact(pill, ui.id().with("tcmdcenter"), egui::Sense::click());
                let (pill_bg, pill_border, pill_fg) = if pill_resp.hovered() {
                    (Palette::COMMAND_CENTER_ACTIVE_BG(), Palette::COMMAND_CENTER_ACTIVE_BORDER(), Palette::COMMAND_CENTER_ACTIVE_FG())
                } else {
                    (Palette::COMMAND_CENTER_BG(), Palette::COMMAND_CENTER_BORDER(), Palette::COMMAND_CENTER_FG())
                };
                ui.painter().rect(
                    pill,
                    egui::CornerRadius::same(6),
                    pill_bg,
                    egui::Stroke::new(1.0, pill_border),
                    egui::StrokeKind::Inside,
                );
                ui.painter().text(
                    egui::pos2(pill.left() + 10.0, cy),
                    egui::Align2::LEFT_CENTER,
                    &workspace_name,
                    egui::FontId::proportional(12.0),
                    pill_fg,
                );
                // VS Code's command center opens Quick Open (file search).
                if pill_resp.clicked() {
                    self.open_quick_open();
                }
                pill_resp.on_hover_text(format!("Search {workspace_name} (⌘P)"));

                // Right: layout controls, 26px apart, the last centred 23px
                // from the right edge; the toggles show their part's state.
                let layout_items = [
                    ("layout", "Customize Layout...", true),
                    (if self.sidebar_visible { "layout-sidebar-left" } else { "layout-sidebar-left-off" }, "Toggle Primary Side Bar (⌘B)", true),
                    (if self.panel.visible { "layout-panel" } else { "layout-panel-off" }, "Toggle Panel (⌘J)", true),
                    ("layout-sidebar-right-off", "Toggle Secondary Side Bar (⌥⌘B)", false),
                ];
                let first = rect.right() - 23.0 - 26.0 * (layout_items.len() as f32 - 1.0);
                for (i, (name, tip, _)) in layout_items.into_iter().enumerate() {
                    let r = egui::Rect::from_center_size(egui::pos2(first + 26.0 * i as f32, cy), egui::vec2(22.0, 22.0));
                    let resp = ui.interact(r, ui.id().with(("tlayout", i)), egui::Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(r, egui::CornerRadius::same(4), Palette::VSCE_TOOLBAR_HOVER_BG());
                    }
                    ui.painter().text(
                        r.center(), egui::Align2::CENTER_CENTER,
                        icon(name).to_string(), codicon_font(16.0), Palette::TITLE_BAR_FG(),
                    );
                    if i == 1 && resp.clicked() {
                        self.sidebar_visible = !self.sidebar_visible;
                    }
                    if i == 2 && resp.clicked() {
                        toggle_panel_clicked = true;
                    }
                    let label = tip.to_string();
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
                    resp.on_hover_text(tip);
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
                            v.hovered.weak_bg_fill = Palette::LIST_HOVER_BG();
                            v.hovered.bg_fill = Palette::LIST_HOVER_BG();
                            v.active.bg_stroke = egui::Stroke::NONE;
                            v.open.bg_stroke = egui::Stroke::NONE;
                            v.open.weak_bg_fill = Palette::LIST_HOVER_BG();
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
        if toggle_panel_clicked {
            self.toggle_panel(ctx);
        }
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
        // Keys being recorded in the Keyboard Shortcuts editor aren't commands.
        if self.keybindings_editor.as_ref().is_some_and(|(st, _)| st.captures_keys()) {
            return;
        }
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

        // ⌘K chords: ⌘K ⌘T = Preferences: Color Theme.
        if ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::K))) {
            self.chord_k = Some(std::time::Instant::now());
            self.set_status("(⌘K) was pressed. Waiting for second key of chord...");
            return;
        }
        if let Some(at) = self.chord_k {
            let second = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Key { key, pressed: true, modifiers, .. } if *key != Key::K || !modifiers.command => {
                        Some((*key, *modifiers))
                    }
                    _ => None,
                })
            });
            if let Some((key, modifiers)) = second {
                self.chord_k = None;
                self.status_message.clear();
                if key == Key::T && modifiers.command {
                    ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::T));
                    self.open_color_theme_picker();
                    return;
                }
                if key == Key::S && modifiers.command {
                    ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S));
                    self.open_keybindings_editor(None);
                    return;
                }
            } else if at.elapsed() > std::time::Duration::from_secs(10) {
                self.chord_k = None;
            }
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

        // ⇧⌘D: Run and Debug.
        let show_run = ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::D)));
        if show_run {
            self.show_view(ActivityView::RunAndDebug);
        }
    }
}

impl App {
    /// Frame-agnostic entry point used both by `eframe::App::update` and by
    /// in-process test harnesses (egui_kittest) that don't have a real
    /// `eframe::Frame` instance.
    pub fn render(&mut self, ctx: &Context) {
        // A new colour theme: rebuild egui's visuals from it.
        if theme::generation() != self.visuals_generation {
            theme::apply(ctx);
            self.visuals_generation = theme::generation();
        }
        self.render_impl(ctx);
    }

    fn render_impl(&mut self, ctx: &Context) {
        self.webview_host.begin_frame();
        self.scm_ui.active_path = self.active_doc.and_then(|i| self.documents.get(i)).filter(|d| d.webview.is_none()).map(|d| d.path.clone());
        self.sync_webview_tabs();
        self.refresh_extension_pages();
        // Drain any native-menu events from the previous frame.
        while let Some(ev) = crate::menubar::poll_event() {
            if let Some((_, id, action)) = self.ext_menu_actions.iter().find(|(m, ..)| m == &ev.id).cloned() {
                self.extension_page_action(&id, action);
                continue;
            }
            if let Some((_, action)) = self.settings_menu_actions.iter().find(|(m, _)| m == &ev.id).cloned() {
                self.settings_menu_action(ctx, action);
                continue;
            }
            if let Some((_, action)) = self.editor_menu_actions.iter().find(|(m, _)| m == &ev.id).map(|(m, a)| (m.clone(), *a)) {
                self.editor_menu_action(action);
                continue;
            }
            if let Some((_, action)) = self.scm_menu_actions.iter().find(|(m, _)| m == &ev.id).cloned() {
                self.run_scm_action(action);
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

        // The Welcome page asked for (startup, Help ▸ Welcome) opens as a tab.
        if std::mem::take(&mut self.show_welcome) {
            self.open_welcome();
        }

        // The shell behind the Modern UI cards, under every panel (gaps
        // between panels would otherwise show the clear colour).
        ctx.layer_painter(egui::LayerId::background())
            .rect_filled(ctx.content_rect(), 0.0, Palette::SHELL_BG());

        self.title_bar(ctx);
        self.restricted_mode_banner(ctx);
        self.scm_ui.restricted = !self.workspace_trusted();

        // Modern UI status bar: 24px items with 2px / 12px padding
        // (floatingPanels.css `.part.statusbar`).
        TopBottomPanel::bottom("status_bar")
            .exact_height(28.0)
            .show_separator_line(false)
            .frame(
                Frame::default()
                    .fill(Palette::STATUS_BAR_BG())
                    .inner_margin(Margin { left: 12, right: 12, top: 2, bottom: 2 }),
            )
            .show(ctx, |ui| {
                // Webview tabs have no text: no cursor, language or encoding items.
                let active = self.active_doc.and_then(|i| self.documents.get(i)).filter(|d| d.webview.is_none());
                let has_workspace = self.workspace_root.is_some();
                // Status bar reflects the active repository.
                let active_root = self.active_repo_root();
                // In Restricted Mode git doesn't run: no branch item.
                let trusted = self.workspace_trusted();
                let repo = active_root
                    .as_ref()
                    .and_then(|r| self.git_model.repo_for(r))
                    .filter(|_| trusted);
                // The git extension's branch item: `main*+` (working tree
                // changes, staged changes, merge changes).
                let branch_label = repo.and_then(|r| {
                    let b = r.branch.clone()?;
                    Some(format!(
                        "{b}{}{}{}",
                        if r.working.is_empty() { "" } else { "*" },
                        if r.index.is_empty() { "" } else { "+" },
                        if r.merge.is_empty() { "" } else { "!" }
                    ))
                });
                let branch = branch_label.as_deref();
                let changes = 0;
                let branch_icon = match repo {
                    Some(r) if !r.merge.is_empty() => "git-branch-conflicts",
                    Some(r) if !r.index.is_empty() => "git-branch-staged-changes",
                    Some(r) if !r.working.is_empty() => "git-branch-changes",
                    _ => "git-branch",
                };
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
                    branch_icon,
                    changes,
                    ahead_behind,
                    has_upstream,
                    &extension_items,
                    self.extensions.problem_counts(),
                    !self.workspace_trusted(),
                );
                if sb.manage_trust {
                    self.open_trust_editor();
                }
                if let Some((command, args)) = sb.extension_command.clone() {
                    self.extensions.execute(&command, args);
                }
                if sb.branch_clicked.is_some() {
                    if let Some(root) = active_root.clone() {
                        self.run_scm_action(ScmAction::Checkout(root));
                    }
                }
                if sb.sync_clicked {
                    if let Some(root) = active_root {
                        self.run_scm_action(if has_upstream {
                            ScmAction::Sync { root, rebase: false }
                        } else {
                            ScmAction::Publish(root)
                        });
                    }
                }
                if sb.palette_requested {
                    self.palette.open();
                }
            });


        // The activity bar card: 4px outer margin + 44px card.
        SidePanel::left("activity_bar")
            .exact_width(48.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                Frame::default()
                    .fill(Palette::SHELL_BG())
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
            // One width for every view, as in VS Code; views lay out within
            // it (nothing may widen the panel).
            let panel_id = "sidebar";
            let sidebar_output: SidebarOutput;
            {
                // The side bar card: 1px outline inside (its left edge is a
                // seam with the activity bar), 4px above the status bar.
                let resp = SidePanel::left(panel_id)
                    .resizable(true)
                    .show_separator_line(false)
                    .width_range(170.0..=420.0)
                    .default_width(self.sidebar_width)
                    .frame(
                        Frame::default()
                            .fill(Palette::SHELL_BG())
                            .inner_margin(Margin { left: 1, right: 1, top: 1, bottom: 1 + cards::MARGIN as i8 }),
                    )
                    .show(ctx, |ui| {
                        if self.active_view == ActivityView::ExtensionContainer {
                            let rect = ui.max_rect();
                            ui.painter().rect_filled(rect, 0.0, Palette::SIDEBAR_BG());
                            self.extension_container_view(ui);
                            return SidebarOutput::default();
                        }
                        if self.active_view == ActivityView::Extensions {
                            let rect = ui.max_rect();
                            ui.painter().rect_filled(rect, 0.0, Palette::SIDEBAR_BG());
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
                            if let Some((id, pos)) = view.manage {
                                self.pending_ext_menu = Some((id, "list".into(), pos));
                            }
                            return SidebarOutput::default();
                        }
                        let graph_root = self.active_repo_root();
                        let search_scope = crate::search::ui::SearchScope {
                            open_editors: self.documents.iter().filter(|d| !d.read_only && !d.untitled && d.webview.is_none()).map(|d| d.path.clone()).collect(),
                            changed_files: self.git_decorations.keys().cloned().collect(),
                        };
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
                            &search_scope,
                        )
                    });
                sidebar_output = resp.inner;
                self.sidebar_width = resp.response.rect.width().clamp(170.0, 420.0);
                let r = resp.response.rect;
                let card = egui::Rect::from_min_max(r.min, egui::pos2(r.right(), r.bottom() - cards::MARGIN));
                let radius = egui::CornerRadius { nw: 0, sw: 0, ne: cards::RADIUS as u8, se: cards::RADIUS as u8 };
                cards::finish(
                    &ctx.layer_painter(egui::LayerId::background()),
                    card,
                    radius,
                    Palette::SURFACE_BORDER(),
                    Palette::SHELL_BG(),
                    true,
                );
            }

            if sidebar_output.open_folder_requested {
                self.open_folder_dialog();
            }
            if let Some(path) = sidebar_output.file_to_open {
                // Explorer: one click = preview editor, double click keeps it.
                self.open_file_with(path, sidebar_output.keep_open);
            }
            if let Some((path, line, col)) = sidebar_output.navigate_to {
                self.navigate_to(path, line, col);
            }
            if let Some(action) = sidebar_output.run_debug {
                self.run_debug_action(action);
            }
            self.dispatch_scm(sidebar_output.scm);
            self.dispatch_search(sidebar_output.search);
        }

        let mut editor_output: Option<crate::editor::view::EditorOutput> = None;
        let mut welcome_pending_folder = false;
        let mut find_matches_for_widget: Vec<(usize, usize)> = Vec::new();
        let mut welcome_pending_file = false;
        let mut welcome_action: Option<(crate::workbench::welcome::WelcomeAction, bool)> = None;

        // The editor card: 4px from its neighbours and the window, 1px
        // outline inside, 8px corners (editorBorder.css).
        let editor_margin = cards::MARGIN as i8;
        self.panel_part(ctx);

        let editor_panel = egui::CentralPanel::default()
            .frame(
                Frame::default()
                    .fill(Palette::SHELL_BG())
                    .inner_margin(Margin { left: editor_margin + 1, right: editor_margin + 1, top: 1, bottom: editor_margin + 1 }),
            )
            .show(ctx, |ui| {
                ui.painter().rect_filled(ui.max_rect().expand(1.0), 0.0, Palette::EDITOR_BG());
                ui.vertical(|ui| {
                    // The editor (or page) starts right under the tab strip.
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let has_any_tab = !self.documents.is_empty();

                    if has_any_tab {
                        let tabs_action = ui
                            .allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), 32.0),
                                egui::Layout::left_to_right(egui::Align::Min),
                                |ui| {
                                    tabs::show(ui, &self.documents, self.active_doc, false)
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
                        if let Some(pos) = tabs_action.more_actions {
                            self.pending_editor_menu = Some(pos);
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
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Loading…", egui::FontId::proportional(13.0), Palette::FG_DESCRIPTION());
                        }
                        self.webview_host.place(handle, rect);
                    } else if self.active_doc.and_then(|i| self.documents.get(i)).is_some_and(|d| d.welcome) {
                        let show_on_startup = self.settings.get_str("workbench.startupEditor").unwrap_or("welcomePage") == "welcomePage";
                        let model = crate::workbench::welcome::WelcomeModel {
                            git_available: crate::git::available(),
                            recent: &self.recent_folders,
                            show_on_startup,
                        };
                        let act = crate::workbench::welcome::show(ui, &model);
                        welcome_action = Some((act, show_on_startup));
                    } else if let Some(idx) = self.active_doc {
                        // Breadcrumbs row (path of the active file); an
                        // untitled editor has none.
                        if let Some(doc) = self.documents.get(idx).filter(|d| !d.untitled) {
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
                    } else {
                        empty_editor_hints(ui);
                    }
                });
            });
        {
            let r = editor_panel.response.rect;
            let card = egui::Rect::from_min_max(
                egui::pos2(r.left() + cards::MARGIN, r.top()),
                egui::pos2(r.right() - cards::MARGIN, r.bottom() - cards::MARGIN),
            );
            cards::finish(
                &ctx.layer_painter(egui::LayerId::background()),
                card,
                egui::CornerRadius::same(cards::RADIUS as u8),
                Palette::EDITOR_CARD_BORDER(),
                Palette::SHELL_BG(),
                false,
            );
        }

        if let Some((act, show_on_startup)) = welcome_action {
            use crate::workbench::welcome::StartEntry;
            match act.start {
                Some(StartEntry::NewFile) => self.new_untitled_file(),
                Some(StartEntry::Open) => welcome_pending_folder = true,
                Some(StartEntry::CloneRepository) => self.run_scm_action(ScmAction::CloneRepository),
                Some(StartEntry::ConnectTo) => self.set_status("Remote development is not available"),
                None => {}
            }
            if let Some(folder) = act.open_recent {
                self.set_workspace(folder);
            }
            if let Some(folder) = act.remove_recent {
                self.recent_folders.retain(|p| p != &folder);
                self.save_recent_folders();
            }
            if act.toggle_show_on_startup {
                // Unchecking writes "none"; checking restores the default.
                let value = if show_on_startup { Some(serde_json::json!("none")) } else { None };
                let _ = self.settings.update("workbench.startupEditor", value);
            }
        }
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

        self.settings_modal(ctx);
        self.keybindings_modal(ctx);
        self.trust_modal(ctx);
        self.terminal_trust_dialog(ctx);

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
            || self.settings_editor.is_some()
            || self.keybindings_editor.is_some()
            || self.trust_editor.is_some();
        self.language_widgets(ctx);

        for d in crate::workbench::notifications::show(ctx, &self.extensions.notifications) {
            if Some(d.id) == self.terminal_failed_toast && d.action == Some(0) {
                let _ = std::process::Command::new("open").arg("https://code.visualstudio.com/docs/editor/workspace-trust").spawn();
            }
            self.extensions.close_notification(d.id, d.action);
        }
    }
}

impl App {
    /// View: Toggle Panel Visibility (⌘J). Showing the Terminal view with
    /// no terminal starts one.
    fn toggle_panel(&mut self, ctx: &Context) {
        self.panel.visible = !self.panel.visible;
        if self.panel.visible && self.panel.view == crate::workbench::panel::PanelView::Terminal && self.panel.terminals.is_empty() {
            self.create_terminal(ctx);
        }
    }

    /// View: Problems / Output / Debug Console / Terminal.
    fn show_panel_view(&mut self, ctx: &Context, view: crate::workbench::panel::PanelView) {
        if self.panel.visible && self.panel.view == view {
            // The shortcut again hides it (`toggle` commands).
            self.panel.visible = false;
            return;
        }
        self.panel.visible = true;
        self.panel.view = view;
        if view == crate::workbench::panel::PanelView::Terminal && self.panel.terminals.is_empty() {
            self.create_terminal(ctx);
        }
    }

    /// Terminal: Create New Terminal. In Restricted Mode it asks for trust
    /// first (`terminalProcessManager`'s trust request).
    fn create_terminal(&mut self, ctx: &Context) {
        if !self.workspace_trusted() {
            self.trust_request = Some(TrustRequest::Terminal);
            return;
        }
        let wake = ctx.clone();
        match crate::terminal::Terminal::spawn(self.workspace_root.as_deref(), 24, 80, move || wake.request_repaint()) {
            Ok(t) => {
                self.panel.terminals.push(t);
                self.panel.active_terminal = self.panel.terminals.len() - 1;
                self.panel.view = crate::workbench::panel::PanelView::Terminal;
                self.panel.visible = true;
                self.panel.terminal_focused = true;
            }
            Err(e) => {
                self.extensions.notify_error(format!("The terminal process failed to launch: {e}"));
            }
        }
    }

    fn panel_part(&mut self, ctx: &Context) {
        use crate::workbench::panel;
        // Exited shells close their terminal; the last one hides the panel.
        let before = self.panel.terminals.len();
        self.panel.terminals.retain_mut(|t| !t.has_exited());
        if self.panel.terminals.len() < before {
            self.panel.active_terminal = self.panel.active_terminal.min(self.panel.terminals.len().saturating_sub(1));
            if self.panel.terminals.is_empty() && self.panel.view == panel::PanelView::Terminal {
                self.panel.visible = false;
            }
        }
        if !self.panel.visible {
            return;
        }
        let window_h = ctx.content_rect().height();
        let card_h = if self.panel.maximized {
            window_h - self.title_bar_offset() - 28.0 - 4.0 - 4.0
        } else {
            self.panel.height.unwrap_or_else(|| panel::default_height(window_h))
        };
        let problems = { let (e, w) = self.extensions.problem_counts(); e + w };
        let out = TopBottomPanel::bottom("panel")
            .exact_height(card_h + cards::MARGIN)
            .show_separator_line(false)
            .frame(Frame::NONE)
            .show(ctx, |ui| {
                let r = ui.max_rect();
                let card = egui::Rect::from_min_max(
                    egui::pos2(r.left() + cards::MARGIN, r.top()),
                    egui::pos2(r.right() - cards::MARGIN, r.bottom() - cards::MARGIN),
                );
                panel::show(ui, card, &mut self.panel, problems, "")
            })
            .inner;
        if out.close {
            self.panel.visible = false;
        }
        if out.new_terminal {
            self.create_terminal(ctx);
        }
        if out.kill_terminal && !self.panel.terminals.is_empty() {
            let i = self.panel.active_terminal.min(self.panel.terminals.len() - 1);
            self.panel.terminals.remove(i);
            self.panel.active_terminal = i.saturating_sub(1);
            if self.panel.terminals.is_empty() {
                self.panel.visible = false;
            }
        }
        if out.forward_port {
            self.set_status("Port forwarding needs a remote tunnel, which this build doesn't have");
        }
    }

    /// A link or button of the Run and Debug view.
    fn run_debug_action(&mut self, action: crate::workbench::run_debug::RunDebugAction) {
        use crate::workbench::run_debug::RunDebugAction;
        match action {
            RunDebugAction::OpenFile => self.open_file_dialog(),
            RunDebugAction::OpenFolder => self.open_folder_dialog(),
            RunDebugAction::Start => {
                // `startDebugging` asks for trust before anything runs.
                if self.workspace_trusted() {
                    self.open_debugger_pick();
                } else {
                    self.trust_request = Some(TrustRequest::Debug);
                }
            }
            RunDebugAction::CreateLaunchJson => {
                let Some(root) = self.workspace_root.clone() else { return };
                let path = root.join(".vscode").join("launch.json");
                if !path.exists() {
                    let _ = std::fs::create_dir_all(path.parent().unwrap_or(&root));
                    if let Err(e) = std::fs::write(&path, crate::workbench::run_debug::LAUNCH_JSON) {
                        self.extensions.notify_error(format!("Unable to create 'launch.json' file: {e}"));
                        return;
                    }
                    if let Some(tree) = self.file_tree.as_mut() {
                        tree.refresh();
                    }
                }
                self.open_file(path);
            }
        }
    }

    /// "Select debugger" (`guessDebugger`): this build has no debug
    /// adapters, so only "Install extension..." is offered.
    fn open_debugger_pick(&mut self) {
        let language = self
            .active_doc
            .and_then(|i| self.documents.get(i))
            .filter(|d| !d.welcome && d.webview.is_none())
            .map(|d| d.language_label().to_string());
        let label = match language {
            Some(l) if l != "Plain Text" => format!("Install an extension for {l}..."),
            _ => "Install extension...".to_string(),
        };
        self.palette.open_pick_with(DEBUGGER_PICK_ID, vec![(label, None, None, false)], Some("Select debugger".into()), Default::default());
    }

    /// "Do you trust the authors of the files in this folder?" before a
    /// terminal starts in Restricted Mode (workspace.contribution.ts
    /// `onDidInitiateWorkspaceTrustRequest`).
    fn terminal_trust_dialog(&mut self, ctx: &Context) {
        use crate::workbench::dialog::{self, Dialog, DialogResult};
        if let Some(id) = self.terminal_failed_toast {
            if !self.extensions.notifications.iter().any(|n| n.id == id) {
                self.terminal_failed_toast = None;
            }
        }
        let Some(request) = self.trust_request else { return };
        let detail = match request {
            TrustRequest::Terminal => "Creating a terminal process requires executing code",
            TrustRequest::Debug => "Debugging executes build tasks and program code from your workspace.",
        };
        let d = Dialog {
            icon: "shield",
            message: "Do you trust the authors of the files in this folder?",
            details: &[
                detail,
                "If you don't trust the authors of these files, we do not recommend continuing as the files may be malicious. See [our docs](https://aka.ms/vscode-workspace-trust) to learn more.",
            ],
            buttons: &["Manage", "Cancel", "Trust Folder & Continue"],
        };
        match dialog::show(ctx, egui::Id::new("terminal-trust-dialog"), &d) {
            Some(DialogResult::Button(2)) => {
                self.trust_request = None;
                if let Some(folder) = self.workspace_root.clone() {
                    self.trust.trust(&folder);
                    self.refresh_git();
                }
                match request {
                    TrustRequest::Terminal => self.create_terminal(ctx),
                    TrustRequest::Debug => self.open_debugger_pick(),
                }
            }
            Some(DialogResult::Button(0)) => {
                self.trust_request = None;
                self.open_trust_editor();
            }
            Some(DialogResult::Link(url)) => {
                let _ = std::process::Command::new("open").arg(&url).spawn();
            }
            Some(_) if request == TrustRequest::Debug => self.trust_request = None,
            Some(_) => {
                // Cancel: the terminal can't start, the panel goes away.
                self.trust_request = None;
                if self.panel.terminals.is_empty() && self.panel.view == crate::workbench::panel::PanelView::Terminal {
                    self.panel.visible = false;
                }
                self.terminal_failed_toast = Some(self.extensions.notify_error_with_actions(
                    "The terminal process failed to launch: Cannot launch a terminal process in an untrusted workspace.".into(),
                    vec!["Open Help".into()],
                ));
            }
            None => {}
        }
    }
}

impl App {
    /// Whether the window is trusted (`security.workspace.trust.*`).
    fn workspace_trusted(&self) -> bool {
        let enabled = self.settings.get_bool("security.workspace.trust.enabled").unwrap_or(true);
        let empty = self.settings.get_bool("security.workspace.trust.emptyWindow").unwrap_or(true);
        crate::workspace_trust::is_trusted(&self.trust, self.workspace_root.as_deref(), enabled, empty)
    }

    /// Whether the Restricted Mode banner is up (`security.workspace.trust.banner`).
    fn banner_visible(&self) -> bool {
        if self.workspace_trusted() {
            return false;
        }
        match self.settings.get_str("security.workspace.trust.banner").unwrap_or("untilDismissed") {
            "never" => false,
            "always" => true,
            _ => !self.workspace_root.as_deref().is_some_and(|f| self.trust.banner_dismissed(f)),
        }
    }

    /// Where the modal editor part may start: under the title bar and the
    /// banner (`mainContainerOffset.top`).
    fn title_bar_offset(&self) -> f32 {
        35.0 + if self.banner_visible() { crate::workbench::banner::HEIGHT } else { 0.0 }
    }

    fn restricted_mode_banner(&mut self, ctx: &Context) {
        if !self.banner_visible() {
            return;
        }
        let message = crate::workbench::banner::restricted_message(self.workspace_root.is_some());
        let out = TopBottomPanel::top("banner")
            .exact_height(crate::workbench::banner::HEIGHT)
            .show_separator_line(false)
            .frame(Frame::NONE)
            .show(ctx, |ui| crate::workbench::banner::show(ui, message))
            .inner;
        if out.manage {
            self.open_trust_editor();
        }
        if out.learn_more {
            let _ = std::process::Command::new("open").arg("https://aka.ms/vscode-workspace-trust").spawn();
        }
        if out.close {
            if let Some(folder) = self.workspace_root.clone() {
                self.trust.dismiss_banner(&folder);
            }
        }
    }

    /// Workspaces: Manage Workspace Trust.
    fn open_trust_editor(&mut self) {
        if self.trust_editor.is_none() {
            self.trust_editor = Some((Default::default(), Default::default()));
        }
        self.settings_editor = None;
        self.palette.visible = false;
    }

    /// Extensions with code that don't declare full support for untrusted
    /// workspaces (`getExtensionUntrustedWorkspaceSupportType` ≠ true).
    fn untrusted_unsupported_extensions(&self) -> usize {
        self.extensions
            .installed()
            .filter(|e| e.main.is_some())
            .filter(|e| {
                let manifest = std::fs::read_to_string(e.path.join("package.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
                let supported = manifest.as_ref().and_then(|m| m.pointer("/capabilities/untrustedWorkspaces/supported")).cloned();
                supported != Some(serde_json::Value::Bool(true))
            })
            .count()
    }

    fn trust_modal(&mut self, ctx: &Context) {
        use crate::workbench::{modal, workspace_trust_editor as editor};
        let Some((mut st, mut modal_state)) = self.trust_editor.take() else { return };
        let header = modal::ModalHeader {
            icon: crate::icons::codicon_by_name("shield").unwrap_or('!'),
            title: "Workspace Trust",
            actions: &[],
        };
        let unsupported = self.untrusted_unsupported_extensions();
        let trusted = self.workspace_trusted();
        let mut out = editor::TrustEditorOutput::default();
        let action = {
            let view = editor::TrustView {
                trusted,
                folder: self.workspace_root.as_deref(),
                trusted_folders: self.trust.trusted_folders(),
                restricted_settings: crate::workbench::settings_editor::model::restricted_workspace_setting_count(),
                unsupported_extensions: unsupported,
            };
            modal::show(ctx, egui::Id::new("trust-modal"), &modal_state, &header, self.title_bar_offset(), |ui| {
                let rect = ui.max_rect();
                out = editor::show(ui, rect, &mut st, &view);
            })
        };
        let mut keep = true;
        match action {
            Some(modal::ModalAction::Close) => keep = false,
            Some(modal::ModalAction::ToggleMaximized) | Some(modal::ModalAction::OpenInMainWindow) => modal_state.maximized = !modal_state.maximized,
            _ => {}
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            keep = false;
        }
        if out.trust {
            if let Some(folder) = self.workspace_root.clone() {
                self.trust.trust(&folder);
                self.refresh_git();
            }
        }
        if out.dont_trust {
            if let Some(folder) = self.workspace_root.clone() {
                self.trust.untrust(&folder);
                self.refresh_git();
            }
        }
        if let Some(entry) = out.remove {
            self.trust.remove(&entry);
        }
        if out.add_folder {
            if let Some(path) = rfd::FileDialog::new().set_title("Select Folder To Trust").pick_folder() {
                self.trust.trust(&path);
            }
        }
        if let Some(url) = out.open_url {
            let _ = std::process::Command::new("open").arg(&url).spawn();
        }
        if out.configure_settings {
            self.trust_editor = None;
            self.open_settings_editor(Some("@tag:workspaceTrust"));
            return;
        }
        if out.show_untrusted_settings {
            self.trust_editor = None;
            self.open_settings_editor(Some("@tag:requireTrustedWorkspace"));
            return;
        }
        if out.show_unsupported_extensions {
            self.trust_editor = None;
            self.show_view(ActivityView::Extensions);
            return;
        }
        if keep {
            self.trust_editor = Some((st, modal_state));
        }
    }
}

impl App {
    /// Preferences: Open Settings (UI) (⌘,): the Settings editor in its
    /// modal, the search box focused.
    fn open_settings_editor(&mut self, query: Option<&str>) {
        use crate::workbench::{modal::ModalState, settings_editor::SettingsEditorState};
        if self.workspace_settings.is_none() {
            if let Some(root) = &self.workspace_root {
                self.workspace_settings = Some(crate::settings::Settings::load_file(root.join(".vscode").join("settings.json")));
            }
        }
        match self.settings_editor.as_mut() {
            Some((st, _)) => {
                st.focus_search = true;
                if let Some(q) = query {
                    st.query = q.to_string();
                }
            }
            None => {
                let st = query.map(SettingsEditorState::with_query).unwrap_or_else(SettingsEditorState::new);
                self.settings_editor = Some((st, ModalState::default()));
            }
        }
        self.palette.visible = false;
    }

    /// The settings file of a target, created as `{}` when missing (as VS
    /// Code does before opening it).
    fn settings_file(&mut self, target: crate::workbench::settings_editor::Target) -> Option<PathBuf> {
        use crate::workbench::settings_editor::Target;
        let path = match target {
            Target::User => self.settings.path().map(PathBuf::from),
            Target::Workspace => self.workspace_settings.as_ref().and_then(|w| w.path()).map(PathBuf::from),
        }?;
        if !path.exists() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&path, "{\n}\n");
        }
        Some(path)
    }

    /// Writes (or with `None` resets) a setting in a target. Choosing the
    /// default in User settings removes it (`updateChangedSetting`).
    fn write_setting(&mut self, target: crate::workbench::settings_editor::Target, key: &str, value: Option<serde_json::Value>) {
        use crate::workbench::settings_editor::{model, Target};
        let value = match (target, value) {
            (Target::User, Some(v)) if !v.is_object() && !v.is_array() && model::find(key).is_some_and(|i| model::settings()[i].default == v) => None,
            (_, v) => v,
        };
        let result = match target {
            Target::User => self.settings.update(key, value),
            Target::Workspace => match self.workspace_settings.as_mut() {
                Some(ws) => ws.update(key, value),
                None => Ok(()),
            },
        };
        if let Err(e) = result {
            let which = if target == Target::User { "user" } else { "workspace" };
            self.set_status(format!("Unable to write into {which} settings: {e}"));
        }
        if key.starts_with("workbench.color") {
            self.apply_theme_settings();
        }
    }

    /// "Edit in settings.json" / Open User Settings (JSON): the file in an
    /// editor, with the setting added at its current value and the cursor
    /// on it.
    fn open_settings_json(&mut self, target: crate::workbench::settings_editor::Target, key: Option<&str>) {
        use crate::workbench::settings_editor::{model, Target};
        let Some(path) = self.settings_file(target) else {
            self.set_status("Settings are not saved in this window");
            return;
        };
        if let Some(key) = key {
            let set = match target {
                Target::User => self.settings.get(key).is_some(),
                Target::Workspace => self.workspace_settings.as_ref().is_some_and(|w| w.get(key).is_some()),
            };
            if !set {
                if let Some(i) = model::find(key) {
                    let default = model::settings()[i].default.clone();
                    let _ = match target {
                        Target::User => self.settings.update(key, Some(default)),
                        Target::Workspace => self.workspace_settings.as_mut().map(|w| w.update(key, Some(default))).unwrap_or(Ok(())),
                    };
                }
            }
        }
        self.settings_editor = None;
        // Re-read an open editor of the file so it shows the new text.
        if let Some(i) = self.documents.iter().position(|d| d.path == path && !d.dirty) {
            if let Ok(text) = std::fs::read_to_string(&path) {
                self.documents[i].text = text;
            }
        }
        self.open_file(path);
        if let (Some(key), Some(doc)) = (key, self.active_doc.and_then(|i| self.documents.get_mut(i))) {
            let needle = format!("\"{key}\"");
            if let Some(byte) = doc.text.find(&needle) {
                let line = doc.text[..byte].matches('\n').count() + 1;
                let line_start = doc.text[..byte].rfind('\n').map(|p| p + 1).unwrap_or(0);
                doc.pending_nav = Some((line, byte - line_start));
            }
        }
    }

    /// Draws the Settings editor modal and acts on what was done in it.
    fn settings_modal(&mut self, ctx: &Context) {
        use crate::workbench::{modal, settings_editor};
        let Some((mut st, mut modal_state)) = self.settings_editor.take() else { return };
        let header = modal::ModalHeader {
            icon: crate::icons::codicon_by_name("settings").unwrap_or('*'),
            title: "Settings",
            actions: &[("go-to-file", "Open Settings (JSON)")],
        };
        let mut out = settings_editor::SettingsEditorOutput::default();
        let action = {
            let values = settings_editor::Values {
                user: self.settings.values(),
                workspace: self.workspace_settings.as_ref().map(|w| w.values()),
            };
            modal::show(ctx, egui::Id::new("settings-modal"), &modal_state, &header, self.title_bar_offset(), |ui| {
                let rect = ui.max_rect();
                out = settings_editor::show(ui, rect, &mut st, &values);
            })
        };
        let target = st.target;
        let mut keep = true;
        match action {
            Some(modal::ModalAction::Close) => keep = false,
            Some(modal::ModalAction::ToggleMaximized) => modal_state.maximized = !modal_state.maximized,
            Some(modal::ModalAction::OpenInMainWindow) => {
                // The Settings editor only lives in its modal here.
                modal_state.maximized = true;
            }
            Some(modal::ModalAction::Editor(_)) => {
                self.settings_editor = Some((st, modal_state));
                self.open_settings_json(target, None);
                return;
            }
            None => {}
        }
        if out.escape {
            keep = false;
        }
        if let Some((t, key, value)) = out.set.take() {
            self.write_setting(t, &key, value);
        }
        if let Some(url) = out.open_url.take() {
            let _ = std::process::Command::new("open").arg(&url).spawn();
        }
        if let Some((key, pos, configured)) = out.gear_menu.take() {
            self.pending_settings_menu = Some(SettingsMenu::Gear { key, pos, configured, target });
        }
        if let Some(pos) = out.filter_menu.take() {
            self.pending_settings_menu = Some(SettingsMenu::Filter { pos });
        }
        if out.backup_and_sync {
            self.set_status("Settings Sync is not available");
        }
        if let Some((t, key)) = out.edit_in_json.take() {
            self.settings_editor = Some((st, modal_state));
            self.open_settings_json(t, Some(&key));
            return;
        }
        if keep {
            self.settings_editor = Some((st, modal_state));
        }
    }

    fn show_pending_settings_menu(&mut self, frame: &eframe::Frame) {
        use wry::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use muda::ContextMenu;
        let Some(menu) = self.pending_settings_menu.take() else { return };
        let root = muda::Menu::new();
        let mut actions = Vec::new();
        let pos = match menu {
            SettingsMenu::Gear { key, pos, configured, target } => {
                let reset = muda::MenuItem::new("Reset Setting", configured, None);
                let copy_id = muda::MenuItem::new("Copy Setting ID", true, None);
                let copy_json = muda::MenuItem::new("Copy Setting as JSON", true, None);
                actions.push((reset.id().clone(), SettingsMenuAction::Reset(target, key.clone())));
                actions.push((copy_id.id().clone(), SettingsMenuAction::CopyId(key.clone())));
                actions.push((copy_json.id().clone(), SettingsMenuAction::CopyJson(key)));
                let _ = root.append_items(&[&reset, &muda::PredefinedMenuItem::separator(), &copy_id, &copy_json]);
                pos
            }
            SettingsMenu::Keybinding(m) => {
                use crate::workbench::keybindings_editor::MenuItem as K;
                let mut add = |label: &str, enabled: bool, what: K, actions: &mut Vec<(muda::MenuId, SettingsMenuAction)>| -> muda::MenuItem {
                    let item = muda::MenuItem::new(label, enabled, None);
                    actions.push((item.id().clone(), SettingsMenuAction::Keybinding(m.item, what)));
                    item
                };
                let copy = add("Copy", true, K::Copy, &mut actions);
                let copy_id = add("Copy Command ID", true, K::CopyCommandId, &mut actions);
                let copy_title = add("Copy Command Title", true, K::CopyCommandTitle, &mut actions);
                let define = add(if m.has_key { "Change Keybinding..." } else { "Add Keybinding..." }, true, K::Define, &mut actions);
                let sep = muda::PredefinedMenuItem::separator();
                let _ = root.append_items(&[&copy, &copy_id, &copy_title, &sep, &define]);
                if m.has_key {
                    let add_kb = add("Add Keybinding...", true, K::Add, &mut actions);
                    let when = add("Change When Expression", true, K::DefineWhen, &mut actions);
                    let _ = root.append_items(&[&add_kb, &when]);
                    let remove = add("Remove Keybinding", true, K::Remove, &mut actions);
                    let reset = add("Reset Keybinding", m.user, K::Reset, &mut actions);
                    let _ = root.append_items(&[&muda::PredefinedMenuItem::separator(), &remove, &reset]);
                    let same = add("Show Same Keybindings", true, K::ShowSame, &mut actions);
                    let _ = root.append_items(&[&muda::PredefinedMenuItem::separator(), &same]);
                }
                m.pos
            }
            SettingsMenu::Filter { pos } => {
                for f in crate::workbench::settings_editor::FILTERS {
                    let item = muda::MenuItem::new(*f, true, None);
                    actions.push((item.id().clone(), SettingsMenuAction::Filter(f)));
                    let _ = root.append(&item);
                }
                pos
            }
        };
        self.settings_menu_actions = actions;
        let Ok(handle) = frame.window_handle() else { return };
        if let RawWindowHandle::AppKit(appkit) = handle.as_raw() {
            let y = self.viewport_height - pos.y;
            // SAFETY: the view belongs to the live editor window.
            unsafe {
                root.show_context_menu_for_nsview(
                    appkit.ns_view.as_ptr() as *const std::ffi::c_void,
                    Some(muda::dpi::Position::Logical(muda::dpi::LogicalPosition::new(pos.x as f64, y as f64))),
                );
            }
        }
    }

    fn settings_menu_action(&mut self, ctx: &Context, action: SettingsMenuAction) {
        use crate::workbench::settings_editor::model;
        match action {
            SettingsMenuAction::Reset(target, key) => {
                self.write_setting(target, &key, None);
                if let Some((st, _)) = self.settings_editor.as_mut() {
                    st.reset_inputs();
                }
            }
            SettingsMenuAction::CopyId(key) => ctx.copy_text(key),
            SettingsMenuAction::CopyJson(key) => {
                let value = self
                    .settings
                    .get(&key)
                    .cloned()
                    .or_else(|| model::find(&key).map(|i| model::settings()[i].default.clone()))
                    .unwrap_or(serde_json::Value::Null);
                let text = serde_json::to_string_pretty(&value).unwrap_or_default().replace("  ", "\t");
                ctx.copy_text(format!("\"{key}\": {text}"));
            }
            SettingsMenuAction::Filter(f) => {
                if let Some((st, _)) = self.settings_editor.as_mut() {
                    st.add_filter(f);
                }
            }
            SettingsMenuAction::Keybinding(item, what) => self.keybinding_menu_action(ctx, item, what),
        }
    }

    // ── Keyboard Shortcuts editor ──────────────────────────────────────

    fn keybindings_file(&self) -> Option<PathBuf> {
        self.settings.path().and_then(|p| p.parent()).map(|d| d.join("keybindings.json"))
    }

    fn load_user_keybindings(&mut self) {
        self.user_keybindings = self
            .keybindings_file()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| crate::workbench::keybindings_editor::model::parse_user(&t))
            .unwrap_or_default();
    }

    fn open_keybindings_editor(&mut self, query: Option<&str>) {
        use crate::workbench::{keybindings_editor::KeybindingsEditorState, modal::ModalState};
        self.load_user_keybindings();
        match self.keybindings_editor.as_mut() {
            Some((st, _)) => {
                st.focus_search = true;
                if let Some(q) = query {
                    st.query = q.to_string();
                }
            }
            None => {
                let st = query.map(KeybindingsEditorState::with_query).unwrap_or_default();
                self.keybindings_editor = Some((st, ModalState::default()));
            }
        }
        self.settings_editor = None;
        self.palette.visible = false;
    }

    /// Opens keybindings.json, created with VS Code's header when missing.
    fn open_keybindings_json(&mut self) {
        let Some(path) = self.keybindings_file() else {
            self.set_status("Keyboard shortcuts are not saved in this window");
            return;
        };
        if !path.exists() {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&path, crate::workbench::keybindings_editor::EMPTY_JSON);
        }
        self.keybindings_editor = None;
        if let Some(i) = self.documents.iter().position(|d| d.path == path && !d.dirty) {
            if let Ok(text) = std::fs::read_to_string(&path) {
                self.documents[i].text = text;
            }
        }
        self.open_file(path);
    }

    fn edit_keybindings(&mut self, edit: crate::workbench::keybindings_editor::Edit) {
        use crate::workbench::keybindings_editor as kb;
        kb::apply_edit(&mut self.user_keybindings, &edit);
        let Some(path) = self.keybindings_file() else {
            // In-memory settings (tests): the edit stays in this window.
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&path, kb::to_json(&self.user_keybindings)) {
            self.set_status(&format!("Error '{e}' while editing the keybinding. Please open 'keybindings.json' file and check for errors."));
        }
        if let Some(i) = self.documents.iter().position(|d| d.path == path && !d.dirty) {
            if let Ok(text) = std::fs::read_to_string(&path) {
                self.documents[i].text = text;
            }
        }
    }

    fn keybinding_menu_action(&mut self, ctx: &Context, index: usize, what: crate::workbench::keybindings_editor::MenuItem) {
        use crate::workbench::keybindings_editor::{copy_text, Edit, MenuItem as K, model::Source};
        let Some((st, _)) = self.keybindings_editor.as_mut() else { return };
        let Some(item) = st.item(index).cloned() else { return };
        let mut edit = None;
        match what {
            K::Copy => ctx.copy_text(copy_text(&item)),
            K::CopyCommandId => ctx.copy_text(item.command.clone()),
            K::CopyCommandTitle => ctx.copy_text(item.label.clone()),
            K::Define => st.define(index, false),
            K::Add => st.define(index, true),
            K::DefineWhen => st.define_when(index),
            K::Remove => {
                if let Some(k) = &item.key {
                    edit = Some(Edit::Remove { command: item.command.clone(), key: k.raw.clone(), when: item.when.clone(), user: item.source == Source::User });
                }
            }
            K::Reset => edit = Some(Edit::Reset { command: item.command.clone() }),
            K::ShowSame => st.show_same(index),
        }
        if let Some(e) = edit {
            self.edit_keybindings(e);
        }
    }

    fn keybindings_modal(&mut self, ctx: &Context) {
        use crate::workbench::{keybindings_editor, modal};
        let Some((mut st, mut modal_state)) = self.keybindings_editor.take() else { return };
        let header = modal::ModalHeader {
            icon: crate::icons::codicon_by_name("keyboard").unwrap_or('*'),
            title: "Keyboard Shortcuts",
            actions: &[("go-to-file", "Open Keyboard Shortcuts (JSON)")],
        };
        let mut out = keybindings_editor::KeybindingsEditorOutput::default();
        let user = self.user_keybindings.clone();
        let action = modal::show(ctx, egui::Id::new("keybindings-modal"), &modal_state, &header, self.title_bar_offset(), |ui| {
            let rect = ui.max_rect();
            out = keybindings_editor::show(ui, rect, &mut st, &user);
        });
        let mut keep = true;
        match action {
            Some(modal::ModalAction::Close) => keep = false,
            Some(modal::ModalAction::ToggleMaximized) => modal_state.maximized = !modal_state.maximized,
            Some(modal::ModalAction::OpenInMainWindow) => modal_state.maximized = true,
            Some(modal::ModalAction::Editor(_)) => {
                self.open_keybindings_json();
                return;
            }
            None => {}
        }
        if out.escape {
            keep = false;
        }
        if let Some(text) = out.copy.take() {
            ctx.copy_text(text);
        }
        if let Some(m) = out.menu.take() {
            self.pending_settings_menu = Some(SettingsMenu::Keybinding(m));
        }
        if let Some(name) = out.open_extension.take() {
            self.set_status(&format!("{name} is built into Visual Studio Code"));
        }
        if keep {
            self.keybindings_editor = Some((st, modal_state));
        }
        if let Some(e) = out.edit.take() {
            self.edit_keybindings(e);
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
        self.show_pending_extension_menu(frame);
        self.show_pending_scm_menu(frame);
        self.show_pending_editor_menu(frame);
        self.show_pending_settings_menu(frame);
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
        .color(Palette::TITLE_BAR_FG())
}

fn title_icon(ui: &mut egui::Ui, glyph: char, tip: &str) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 4.0, Palette::LIST_HOVER_BG());
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph.to_string(),
        crate::icons::codicon_font(15.0),
        Palette::TITLE_BAR_FG(),
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
            Palette::FG_DESCRIPTION(),
        );
        painter.text(
            egui::pos2(cx - total_w / 2.0 + label_w + 40.0, y),
            egui::Align2::LEFT_CENTER,
            *shortcut,
            egui::FontId::proportional(13.0),
            Palette::FG_DESCRIPTION(),
        );
    }
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

/// `String.prototype.localeCompare` as the theme picker sorts with it
/// (ICU root collation, simplified): spaces, then punctuation, then
/// digits, then letters, case-insensitive first.
fn locale_compare(a: &str, b: &str) -> std::cmp::Ordering {
    let key = |s: &str| -> Vec<(u8, char)> {
        s.chars()
            .map(|c| {
                let class = if c.is_whitespace() {
                    0
                } else if c.is_ascii_punctuation() {
                    1
                } else if c.is_numeric() {
                    2
                } else {
                    3
                };
                (class, c.to_lowercase().next().unwrap_or(c))
            })
            .collect()
    };
    key(a).cmp(&key(b)).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::locale_compare;

    #[test]
    fn theme_labels_sort_like_vs_code() {
        // Order of the Color Theme picker in VS Code 1.145.2.
        let mut light = vec!["Solarized Light", "Light+", "Quiet Light", "Light Modern", "Light (Visual Studio)"];
        light.sort_by(|a, b| locale_compare(a, b));
        assert_eq!(light, ["Light (Visual Studio)", "Light Modern", "Light+", "Quiet Light", "Solarized Light"]);
        let mut dark = vec!["Dark+", "Abyss", "Dark Modern", "Dark (Visual Studio)", "Kimbie Dark"];
        dark.sort_by(|a, b| locale_compare(a, b));
        assert_eq!(dark, ["Abyss", "Dark (Visual Studio)", "Dark Modern", "Dark+", "Kimbie Dark"]);
    }
}

/// Search's Preserve Case: the replacement takes the matched text's case
/// shape (all upper, all lower, capitalised).
fn preserve_case(matched: &str, replacement: &str) -> String {
    if matched.chars().all(|c| !c.is_lowercase()) && matched.chars().any(|c| c.is_uppercase()) {
        replacement.to_uppercase()
    } else if matched.chars().all(|c| !c.is_uppercase()) {
        replacement.to_lowercase()
    } else if matched.chars().next().is_some_and(|c| c.is_uppercase()) {
        let mut cs = replacement.chars();
        cs.next().map(|f| f.to_uppercase().collect::<String>() + cs.as_str()).unwrap_or_default()
    } else {
        replacement.to_string()
    }
}
