//! VS Code extension support: finds installed extensions, runs their code
//! in a Node.js extension host and turns what they ask for (commands,
//! notifications, status bar items, messages) into editor state.
//!
//! VS Code analogues:
//! - `src/vs/workbench/services/extensions/common/abstractExtensionService.ts`
//! - `src/vs/workbench/api/browser/mainThreadCommands.ts`,
//!   `mainThreadMessageService.ts`, `mainThreadStatusBar.ts`

pub mod host;
pub mod manifest;
pub mod marketplace;
pub mod text;
pub mod when;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{json, Value};

pub use manifest::{ContributedCommand, Extension};

/// Where an extension stands in this session.
#[derive(Clone, Debug, PartialEq)]
pub enum Activation {
    /// Waiting for one of its activation events.
    Inactive,
    Active,
    Failed(String),
    /// Has code but no Node.js was found to run it.
    NeedsNode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// A toast in the bottom-right corner (`window.show*Message`).
#[derive(Clone, Debug)]
pub struct Notification {
    pub id: u64,
    pub severity: Severity,
    pub text: String,
    /// Button titles.
    pub actions: Vec<String>,
    /// The extension's display name ("Source: …").
    pub source: Option<String>,
    /// Host request to answer with the chosen button (or nothing).
    request_id: Option<u64>,
    pub created: Instant,
}

/// A status bar entry from `window.createStatusBarItem`.
#[derive(Clone, Debug)]
pub struct StatusItem {
    pub handle: u64,
    pub right: bool,
    pub priority: f64,
    /// Text with `$(icon)` references.
    pub text: String,
    pub tooltip: Option<String>,
    pub command: Option<String>,
    pub command_args: Value,
}

/// A text edit from an extension: replace `range` (start line, start
/// character, end line, end character; characters in UTF-16 code units, as
/// in VS Code) with `text`.
#[derive(Clone, Debug, PartialEq)]
pub struct WireEdit {
    pub range: [usize; 4],
    pub text: String,
}

/// One entry of `window.showQuickPick`.
#[derive(Clone, Debug, PartialEq)]
pub struct PickItem {
    pub label: String,
    pub description: Option<String>,
    pub detail: Option<String>,
    /// `QuickPickItemKind.Separator`: a group label, not pickable.
    pub separator: bool,
}

/// How an extension decorates text (`createTextEditorDecorationType`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DecorationStyle {
    pub background: Option<egui::Color32>,
    pub color: Option<egui::Color32>,
    pub border: Option<egui::Color32>,
    pub bold: bool,
    pub whole_line: bool,
}

/// One decorated range in a document.
#[derive(Clone, Debug, PartialEq)]
pub struct DecoratedRange {
    pub range: [usize; 4],
    pub hover: Option<String>,
    /// `renderOptions.after.contentText` (inline hints after the range).
    pub after: Option<String>,
}

/// A problem an extension reported (`DiagnosticCollection.set`).
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub range: [usize; 4],
    pub message: String,
    /// `DiagnosticSeverity`: 0 error, 1 warning, 2 information, 3 hint.
    pub severity: u8,
    pub source: Option<String>,
    pub code: Option<String>,
}

/// One completion suggestion.
#[derive(Clone, Debug, PartialEq)]
pub struct CompletionEntry {
    pub label: String,
    /// `CompletionItemKind` name ("Function", "Variable"…).
    pub kind: String,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: String,
    /// What accepting it replaces.
    pub range: [usize; 4],
    pub sort_text: String,
    pub filter_text: String,
    pub preselect: bool,
}

/// A place in a file (`Location`, `LocationLink`).
#[derive(Clone, Debug, PartialEq)]
pub struct SourceLocation {
    pub path: PathBuf,
    pub range: [usize; 4],
}

/// The answer to a language feature request.
#[derive(Clone, Debug, PartialEq)]
pub enum ProviderResult {
    Completion { items: Vec<CompletionEntry>, incomplete: bool },
    /// Markdown blocks, and the range they describe.
    Hover { contents: Vec<String>, range: Option<[usize; 4]> },
    Definition(Vec<SourceLocation>),
}

/// Where a webview lives.
#[derive(Clone, Debug, PartialEq)]
pub enum WebviewKind {
    /// `window.createWebviewPanel`: an editor tab.
    Panel { view_type: String },
    /// A contributed `"type": "webview"` view in a sidebar container.
    View { view_id: String },
}

/// An extension's webview: its page and the messages for it.
#[derive(Clone, Debug)]
pub struct WebviewState {
    pub handle: u64,
    pub kind: WebviewKind,
    pub title: String,
    pub html: String,
    /// Bumped on every `html` change, so the page reloads.
    pub html_version: u64,
    /// Folders the page may load files from (the extension's own folder,
    /// `localResourceRoots`, the workspace).
    pub roots: Vec<PathBuf>,
    /// Messages for the page (JSON), in order.
    pub to_page: Vec<String>,
    /// The extension asked to show it (`reveal`, `show`).
    pub reveal: bool,
}

/// Something the editor itself has to do for an extension.
#[derive(Clone, Debug, PartialEq)]
pub enum EditorRequest {
    /// Apply edits to a file (open it if needed), then `respond(id, true)`.
    ApplyEdits { request_id: u64, path: PathBuf, edits: Vec<WireEdit> },
    /// `window.showTextDocument`: open (or focus) the file, then respond.
    OpenEditor { request_id: u64, path: PathBuf },
    /// `TextDocument.save()`.
    SaveDocument { request_id: u64, path: PathBuf },
    /// `TextEditor.revealRange`: scroll to a 0-based line.
    Reveal { path: PathBuf, line: usize },
    /// `TextEditor.selection = …`: move the caret (0-based line, UTF-16 column).
    SetSelection { path: PathBuf, line: usize, character: usize },
    /// `window.showQuickPick`: answer with the index (or indices), or null.
    QuickPick { request_id: u64, items: Vec<PickItem>, placeholder: Option<String>, can_pick_many: bool },
    /// `vscode.diff`: open a diff tab (base vs text), then respond.
    OpenDiff { request_id: u64, path: PathBuf, base: String, text: String, title: String },
    /// `window.tabGroups.close`: close these tabs, then respond.
    CloseTabs { request_id: u64, paths: Vec<PathBuf> },
    /// `window.showInputBox`: answer with the text, or null.
    InputBox { request_id: u64, prompt: Option<String>, placeholder: Option<String>, value: String, password: bool },
    /// Run a built-in command (`commands.executeCommand` on an id no
    /// extension registered).
    Command(String),
    /// `window.setStatusBarMessage` (empty clears).
    StatusMessage(String),
    /// `env.clipboard.writeText`.
    CopyText(String),
}

/// Last lines of the extension host log kept for the Extensions view.
const LOG_LINES: usize = 500;

/// How an extension was activated (VS Code's `ActivationTimes`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActivationTime {
    /// Loading its code (`codeLoadingTime`).
    pub load_ms: u64,
    /// Its `activate()` call (`activateCallTime`).
    pub activate_ms: u64,
    /// The event that activated it (`*`, `onLanguage:python`…).
    pub reason: String,
}

/// Whether an installed extension runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Enablement {
    Enabled,
    DisabledGlobally,
    DisabledWorkspace,
    /// Enabled, but the workspace isn't trusted and the extension doesn't
    /// support that (`EnablementState.DisabledByTrustRequirement`).
    DisabledByTrust,
}

/// Choices per extension, kept in `<install dir>/.extension-state.json`.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExtensionState {
    /// Ids (lower case) disabled everywhere.
    pub disabled: BTreeSet<String>,
    /// Per workspace folder: ids disabled or enabled there.
    pub workspace: BTreeMap<String, WorkspaceEnablement>,
    /// Ids that don't update by themselves.
    pub no_auto_update: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct WorkspaceEnablement {
    pub disabled: BTreeSet<String>,
    pub enabled: BTreeSet<String>,
}

impl ExtensionState {
    fn read(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    fn write(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }

    /// A workspace choice wins over the global one, as in VS Code.
    pub fn enablement(&self, id: &str, workspace: Option<&Path>) -> Enablement {
        let key = id.to_lowercase();
        if let Some(w) = workspace.and_then(|w| self.workspace.get(&*w.to_string_lossy())) {
            if w.disabled.contains(&key) {
                return Enablement::DisabledWorkspace;
            }
            if w.enabled.contains(&key) {
                return Enablement::Enabled;
            }
        }
        if self.disabled.contains(&key) { Enablement::DisabledGlobally } else { Enablement::Enabled }
    }
}

#[derive(Default)]
pub struct ExtensionService {
    /// Installed and enabled: these run.
    pub extensions: Vec<Extension>,
    /// Installed but disabled (globally or for this workspace).
    pub disabled: Vec<Extension>,
    /// Enablement and auto-update choices, kept beside the installs.
    pub state: ExtensionState,
    /// The workspace is in Restricted Mode: extensions that don't support
    /// untrusted workspaces don't run.
    pub restricted: bool,
    /// `id@version` updates already started by auto-update.
    auto_updated: BTreeSet<String>,
    /// How long each running extension took to activate.
    pub activation_times: BTreeMap<String, ActivationTime>,
    pub activation: BTreeMap<String, Activation>,
    host: Option<host::Host>,
    /// Commands an extension registered at runtime.
    registered: BTreeSet<String>,
    pub status_items: BTreeMap<u64, StatusItem>,
    pub notifications: Vec<Notification>,
    next_notification: u64,
    /// Activation events already sent (`onLanguage:…`).
    fired_events: BTreeSet<String>,
    /// Host log: extension console output, output channels, failures.
    pub log: Vec<String>,
    /// Output channels extensions created (`window.createOutputChannel`),
    /// by name: their text so far.
    pub channels: BTreeMap<String, String>,
    /// A channel an extension asked to show (`channel.show()`); the app
    /// takes it and reveals the Output view.
    pub show_channel: Option<String>,
    /// Commands run before the host was ready.
    queued: Vec<Value>,
    ready: bool,
    next_request: u64,
    /// Folders scanned by the last `load`.
    dirs: Vec<PathBuf>,
    /// Workspace and wake callback of the last `start`, for `reload`.
    workspace: Option<PathBuf>,
    wake: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    /// Search, install and uninstall.
    pub marketplace: marketplace::Marketplace,
    /// Documents the host knows about: path → (version, text sent).
    synced: BTreeMap<PathBuf, (u64, String)>,
    /// Last active editor sent: (path, selection).
    synced_active: Option<(Option<PathBuf>, [usize; 4])>,
    /// Decoration types by key.
    pub decoration_types: BTreeMap<String, DecorationStyle>,
    /// Decorations per document: path → type key → ranges.
    pub decorations: BTreeMap<PathBuf, BTreeMap<String, Vec<DecoratedRange>>>,
    /// Editor requests that arrived during `wait_until` (tests take them
    /// with `take_requests`).
    waited_requests: Vec<EditorRequest>,
    /// Last `apiCoverage` answer from the host.
    pub api_coverage: Option<Value>,
    /// Problems per file and per reporting collection.
    pub diagnostics: BTreeMap<PathBuf, BTreeMap<String, Vec<Diagnostic>>>,
    /// Answers to language feature requests, by request id.
    provider_results: BTreeMap<u64, ProviderResult>,
    /// Kinds of language providers registered ("completion", "hover"…).
    pub provider_kinds: BTreeSet<String>,
    /// Characters that open completions (`registerCompletionItemProvider`).
    pub completion_triggers: BTreeSet<String>,
    /// Context keys set with `setContext` (for `when` clauses).
    pub context: BTreeMap<String, Value>,
    /// Open webviews by handle.
    pub webviews: BTreeMap<u64, WebviewState>,
    /// Webview views already asked for (`resolveWebviewView` sent).
    requested_views: BTreeSet<String>,
    /// Webview visibility last told to the host.
    webview_visible: BTreeMap<u64, bool>,
    /// Panels the extensions created / disposed since the editor last
    /// looked (it opens / closes their tabs).
    pub opened_panels: Vec<u64>,
    pub closed_panels: Vec<u64>,
    /// Tabs last sent to the host (`window.tabGroups`).
    synced_tabs: Option<Value>,
}

impl ExtensionService {
    /// Read the extensions in `dirs`. Nothing runs yet.
    pub fn load(&mut self, dirs: &[PathBuf]) {
        self.dirs = dirs.to_vec();
        self.state = ExtensionState::read(&self.state_file());
        let mut all = manifest::scan(dirs);
        all.append(&mut self.disabled);
        self.split_enabled(all);
    }

    /// Put each installed extension in `extensions` or `disabled`.
    fn split_enabled(&mut self, all: Vec<Extension>) {
        let workspace = self.workspace.clone();
        let restricted = self.restricted;
        let (enabled, disabled): (Vec<_>, Vec<_>) = all.into_iter().partition(|e| {
            self.state.enablement(&e.id, workspace.as_deref()) == Enablement::Enabled
                && !(restricted && e.untrusted_support() == manifest::UntrustedSupport::No)
        });
        self.extensions = enabled;
        self.disabled = disabled;
        self.activation = self
            .extensions
            .iter()
            .map(|e| (e.id.clone(), Activation::Inactive))
            .collect();
    }

    fn state_file(&self) -> PathBuf {
        self.marketplace.install_dir.join(".extension-state.json")
    }

    /// Every installed extension, enabled ones first.
    pub fn installed(&self) -> impl Iterator<Item = &Extension> {
        self.extensions.iter().chain(self.disabled.iter())
    }

    /// Whether `id` runs, or why not.
    pub fn enablement(&self, id: &str) -> Enablement {
        let state = self.state.enablement(id, self.workspace.as_deref());
        if state == Enablement::Enabled && self.restricted && self.installed().any(|e| e.id == id && e.untrusted_support() == manifest::UntrustedSupport::No) {
            return Enablement::DisabledByTrust;
        }
        state
    }

    /// The workspace's trust changed: extensions that need trust stop in
    /// Restricted Mode and start again once it is trusted (the host restarts).
    pub fn set_restricted(&mut self, restricted: bool) {
        if self.restricted == restricted {
            return;
        }
        self.restricted = restricted;
        if self.wake.is_some() {
            self.reload();
        } else {
            let mut all = std::mem::take(&mut self.extensions);
            all.append(&mut self.disabled);
            self.split_enabled(all);
        }
    }

    /// Enable or disable `id` everywhere (`workspace: false`) or only in
    /// this folder, then restart the extensions ("Enable", "Disable
    /// (Workspace)"…).
    pub fn set_enabled(&mut self, id: &str, enabled: bool, workspace: bool) {
        let key = id.to_lowercase();
        let folder = self.workspace.as_ref().map(|w| w.to_string_lossy().into_owned());
        if workspace {
            let Some(folder) = folder else { return };
            let entry = self.state.workspace.entry(folder).or_default();
            entry.disabled.remove(&key);
            entry.enabled.remove(&key);
            if enabled { entry.enabled.insert(key) } else { entry.disabled.insert(key) };
        } else {
            if enabled {
                self.state.disabled.remove(&key);
                // Enabling everywhere also drops a workspace "disabled".
                if let Some(f) = folder.and_then(|f| self.state.workspace.get_mut(&f)) {
                    f.disabled.remove(&key);
                }
            } else {
                self.state.disabled.insert(key);
            }
        }
        self.state.write(&self.state_file());
        self.reload();
    }

    /// Whether `id` updates by itself when the catalogue has a newer
    /// version ("Auto Update", on by default as in VS Code).
    pub fn auto_update(&self, id: &str) -> bool {
        !self.state.no_auto_update.contains(&id.to_lowercase())
    }

    pub fn set_auto_update(&mut self, id: &str, on: bool) {
        let key = id.to_lowercase();
        if on { self.state.no_auto_update.remove(&key) } else { self.state.no_auto_update.insert(key) };
        self.state.write(&self.state_file());
    }

    /// Start the extension host for the extensions that have code, with
    /// `workspace` as the open folder. `wake` is called whenever the host
    /// has something new (to schedule a repaint).
    pub fn start(
        &mut self,
        workspace: Option<&Path>,
        wake: impl Fn() + Send + Sync + 'static,
    ) {
        let wake: std::sync::Arc<dyn Fn() + Send + Sync> = std::sync::Arc::new(wake);
        if self.workspace.as_deref() != workspace {
            self.workspace = workspace.map(Path::to_path_buf);
            // "Disable (Workspace)" depends on the folder.
            let mut all = std::mem::take(&mut self.extensions);
            all.append(&mut self.disabled);
            self.split_enabled(all);
        }
        self.wake = Some(wake.clone());
        if self.host.is_some() || !self.extensions.iter().any(|e| e.main.is_some()) {
            return;
        }
        let Some(node) = host::find_node() else {
            for e in self.extensions.iter().filter(|e| e.main.is_some()) {
                self.activation.insert(e.id.clone(), Activation::NeedsNode);
            }
            self.push_log("Node.js was not found; extensions with code can't run.".into());
            return;
        };
        let mut host = match host::Host::spawn(&node, move || wake()) {
            Ok(h) => h,
            Err(e) => {
                self.push_log(format!("Couldn't start the extension host: {e}"));
                return;
            }
        };

        let mut config_defaults = serde_json::Map::new();
        for e in &self.extensions {
            for (k, v) in &e.config_defaults {
                config_defaults.insert(k.clone(), v.clone());
            }
        }
        let storage_root = std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|h| h.join(".vscode-rust/extension-storage"));
        let descriptions: Vec<Value> = self
            .extensions
            .iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "path": e.path,
                    "main": e.main,
                    "activationEvents": e.activation_events,
                    "commands": e.commands.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
                    "manifest": e.manifest,
                })
            })
            .collect();
        host.send(&json!({
            "type": "init",
            "extensions": descriptions,
            "workspaceFolders": workspace.map(|w| vec![w.to_path_buf()]).unwrap_or_default(),
            "configDefaults": config_defaults,
            "storageRoot": storage_root,
        }));
        self.host = Some(host);
    }

    /// Stop the host, scan the folders again and start over (after an
    /// install or uninstall).
    pub fn reload(&mut self) {
        self.host = None;
        self.ready = false;
        self.queued.clear();
        self.registered.clear();
        self.activation_times.clear();
        self.status_items.clear();
        self.fired_events.clear();
        self.synced.clear();
        self.synced_active = None;
        self.synced_tabs = None;
        self.decoration_types.clear();
        self.decorations.clear();
        self.diagnostics.clear();
        self.provider_results.clear();
        self.provider_kinds.clear();
        self.completion_triggers.clear();
        self.context.clear();
        self.closed_panels.extend(self.webviews.keys().copied());
        self.webviews.clear();
        self.requested_views.clear();
        self.webview_visible.clear();
        let dirs = self.dirs.clone();
        self.disabled.clear();
        self.load(&dirs);
        let workspace = self.workspace.clone();
        if let Some(wake) = self.wake.clone() {
            self.start(workspace.as_deref(), move || wake());
        }
    }

    /// Uninstall an extension this editor installed, then reload.
    pub fn uninstall(&mut self, id: &str) {
        let Some(ext) = self.installed().find(|e| e.id == id) else { return };
        let path = ext.path.clone();
        match marketplace::uninstall(&path, &self.marketplace.install_dir) {
            Ok(()) => self.reload(),
            Err(e) => self.notify(Severity::Error, format!("Couldn't uninstall '{id}': {e}"), Vec::new(), None, None),
        }
    }

    /// Whether `id` can be uninstalled here (installed through this editor,
    /// not one of VS Code's).
    pub fn can_uninstall(&self, id: &str) -> bool {
        let dir = &self.marketplace.install_dir;
        self.installed()
            .find(|e| e.id == id)
            .is_some_and(|e| e.path.parent() == Some(dir.as_path()))
    }

    /// Whether an extension with this id (any case) is installed.
    pub fn is_installed(&self, id: &str) -> bool {
        self.installed().any(|e| e.id.eq_ignore_ascii_case(id))
    }

    /// Marketplace housekeeping each frame: results, finished installs
    /// (reload so they run), failures (as notifications).
    pub fn tick_marketplace(&mut self, ctx: &egui::Context) -> bool {
        let busy = self.marketplace.tick(ctx);
        if !self.marketplace.just_installed.is_empty() {
            let ids = std::mem::take(&mut self.marketplace.just_installed);
            self.reload();
            for id in ids {
                let name = self.display_name(&id).unwrap_or(id);
                self.notify(Severity::Info, format!("Installed {name}."), Vec::new(), None, None);
            }
        }
        for e in std::mem::take(&mut self.marketplace.errors) {
            self.notify(Severity::Error, e, Vec::new(), None, None);
        }
        for (id, result) in std::mem::take(&mut self.marketplace.downloaded) {
            match result {
                Ok(path) => self.notify(Severity::Info, format!("Downloaded {id} to {}.", path.display()), Vec::new(), None, None),
                Err(e) => self.notify(Severity::Error, format!("Couldn't download '{id}': {e}"), Vec::new(), None, None),
            }
        }
        self.auto_update_extensions();
        busy
    }

    /// A newer version of `id` in the catalogue, if any.
    pub fn update_available(&self, id: &str) -> Option<String> {
        let ext = self.installed().find(|e| e.id.eq_ignore_ascii_case(id))?;
        let latest = self.marketplace.latest.get(&id.to_lowercase())?;
        (marketplace::compare_versions(latest, &ext.version) == std::cmp::Ordering::Greater).then(|| latest.clone())
    }

    /// Ask the catalogue for newer versions of what this editor installed,
    /// and install them where auto-update is on (VS Code's default).
    fn auto_update_extensions(&mut self) {
        if !self.marketplace.enabled {
            return;
        }
        let ours: Vec<String> = self.installed().filter(|e| self.can_uninstall(&e.id)).map(|e| e.id.clone()).collect();
        self.marketplace.check_updates(&ours);
        for id in ours {
            if !self.auto_update(&id) || self.marketplace.installing.contains(&id) {
                continue;
            }
            if let (Some(v), Some((ns, name))) = (self.update_available(&id), id.split_once('.')) {
                // Once per version: a failed install isn't retried every frame.
                if self.auto_updated.insert(format!("{id}@{v}")) {
                    self.marketplace.install_version(ns, name, Some(v));
                }
            }
        }
    }

    /// An information toast from the editor itself.
    pub fn notify_info(&mut self, text: String) {
        self.notify(Severity::Info, text, Vec::new(), None, None);
    }

    /// An error toast from the editor itself.
    pub fn notify_error(&mut self, text: String) {
        self.notify(Severity::Error, text, Vec::new(), None, None);
    }

    /// An error toast with buttons; returns its id (the action comes back
    /// through `close_notification`).
    pub fn notify_error_with_actions(&mut self, text: String, actions: Vec<String>) -> u64 {
        self.notify(Severity::Error, text, actions, None, None);
        self.next_notification
    }

    pub fn is_running(&self) -> bool {
        self.host.is_some()
    }

    /// Commands for the command palette: every contributed command plus
    /// ones registered at runtime that have a contributed title.
    pub fn palette_commands(&self) -> Vec<(String, String, String)> {
        let mut out = Vec::new();
        for e in &self.extensions {
            for c in e.palette_commands() {
                out.push((c.id.clone(), c.category.clone(), c.title.clone()));
            }
        }
        out
    }

    /// An installed extension contributes the command `id`.
    pub fn contributes_command(&self, id: &str) -> bool {
        self.extensions.iter().any(|e| e.commands.iter().any(|c| c.id == id))
    }

    /// Run an extension command (activating its extension first).
    pub fn execute(&mut self, id: &str, args: Value) {
        let msg = json!({ "type": "executeCommand", "id": id, "args": args, "requestId": self.request_id() });
        match self.host.as_mut() {
            Some(h) if self.ready => h.send(&msg),
            Some(_) => self.queued.push(msg),
            None => {
                let text = if self.extensions.iter().any(|e| e.commands.iter().any(|c| c.id == id)) {
                    format!("Command '{id}' can't run: the extension host isn't running (Node.js is required).")
                } else {
                    format!("command '{id}' not found")
                };
                self.notify(Severity::Error, text, Vec::new(), None, None);
            }
        }
    }

    /// Tell the host about the open documents and the active editor: newly
    /// opened, changed (full text), closed. `active` is the active file and
    /// its selection (anchor line, anchor character, active line, active
    /// character; UTF-16 characters).
    pub fn sync_documents(
        &mut self,
        documents: &[(PathBuf, &'static str, &str, bool)],
        active: Option<(PathBuf, [usize; 4])>,
    ) {
        if !self.ready {
            return;
        }
        let mut msgs = Vec::new();
        let open: BTreeSet<&PathBuf> = documents.iter().map(|d| &d.0).collect();
        let closed: Vec<PathBuf> = self.synced.keys().filter(|p| !open.contains(p)).cloned().collect();
        for path in closed {
            self.synced.remove(&path);
            self.decorations.remove(&path);
            msgs.push(json!({ "type": "documentClosed", "path": path }));
        }
        for (path, language, text, dirty) in documents {
            match self.synced.get_mut(path) {
                None => {
                    msgs.push(json!({ "type": "documentOpened", "path": path, "languageId": language, "version": 1, "text": text }));
                    self.synced.insert(path.clone(), (1, text.to_string()));
                }
                Some((version, sent)) if sent.as_str() != *text => {
                    *version += 1;
                    *sent = text.to_string();
                    msgs.push(json!({ "type": "documentChanged", "path": path, "version": *version, "text": text, "dirty": dirty }));
                }
                Some(_) => {}
            }
        }
        let active_key = active.as_ref().map(|(p, s)| (Some(p.clone()), *s)).unwrap_or((None, [0; 4]));
        if self.synced_active.as_ref() != Some(&active_key) {
            let same_file = self.synced_active.as_ref().is_some_and(|(p, _)| *p == active_key.0);
            let sel = active_key.1;
            if same_file {
                if let Some(path) = &active_key.0 {
                    msgs.push(json!({ "type": "selectionChanged", "path": path, "selections": [sel] }));
                }
            } else {
                msgs.push(json!({ "type": "activeEditor", "path": active_key.0, "selections": [sel] }));
            }
            self.synced_active = Some(active_key);
        }
        if let Some(h) = self.host.as_mut() {
            for m in &msgs {
                h.send(m);
            }
        }
    }

    /// Tell the host about the editor's tabs (`window.tabGroups`), when
    /// they changed. Each tab: label, path, kind (`text`, `diff`,
    /// `webview`), viewType, active, dirty, pinned.
    pub fn sync_tabs(&mut self, tabs: Value) {
        if !self.ready || self.synced_tabs.as_ref() == Some(&tabs) {
            return;
        }
        if let Some(h) = self.host.as_mut() {
            h.send(&json!({ "type": "tabs", "tabs": tabs }));
        }
        self.synced_tabs = Some(tabs);
    }

    /// A file was saved.
    pub fn document_saved(&mut self, path: &Path) {
        if let Some(h) = self.host.as_mut() {
            h.send(&json!({ "type": "documentSaved", "path": path }));
        }
    }

    /// Editor requests collected while waiting (for tests).
    pub fn take_requests(&mut self) -> Vec<EditorRequest> {
        std::mem::take(&mut self.waited_requests)
    }

    /// Ask the extensions for completions / hover / definitions at a
    /// position (0-based line, UTF-16 character). `None` when no extension
    /// provides that feature. The answer arrives via `take_result`.
    pub fn request_language_feature(
        &mut self,
        kind: LanguageFeature,
        path: &Path,
        line: usize,
        character: usize,
        trigger: Option<&str>,
    ) -> Option<u64> {
        let (msg_type, provider) = match kind {
            LanguageFeature::Completion => ("provideCompletion", "completion"),
            LanguageFeature::Hover => ("provideHover", "hover"),
            LanguageFeature::Definition => ("provideDefinition", "definition"),
        };
        let available = self.provider_kinds.contains(provider)
            || (kind == LanguageFeature::Definition && self.provider_kinds.contains("declaration"));
        if !self.ready || !available {
            return None;
        }
        let id = self.request_id();
        let host = self.host.as_mut()?;
        host.send(&json!({
            "type": msg_type, "requestId": id, "path": path,
            "position": [line, character], "trigger": trigger,
        }));
        Some(id)
    }

    /// The answer to a language feature request, once it has arrived.
    pub fn take_result(&mut self, request_id: u64) -> Option<ProviderResult> {
        self.provider_results.remove(&request_id)
    }

    /// Problems in a file, all collections together.
    pub fn diagnostics_for(&self, path: &Path) -> Vec<&Diagnostic> {
        self.diagnostics.get(path).map(|m| m.values().flatten().collect()).unwrap_or_default()
    }

    /// (errors, warnings) across every file.
    pub fn problem_counts(&self) -> (usize, usize) {
        let mut counts = (0, 0);
        for d in self.diagnostics.values().flat_map(|m| m.values()).flatten() {
            match d.severity {
                0 => counts.0 += 1,
                1 => counts.1 += 1,
                _ => {}
            }
        }
        counts
    }

    /// Sidebar containers the extensions contribute that their `when`
    /// clauses allow now, with the views allowed in them. Containers meant
    /// for VS Code's secondary sidebar join the activity bar here.
    pub fn visible_containers(&self) -> Vec<manifest::ViewContainer> {
        let ok = |w: &Option<String>| w.as_deref().is_none_or(|w| when::eval(w, &self.context));
        self.extensions
            .iter()
            .flat_map(|e| e.view_containers.iter())
            .filter(|c| c.location != "panel" && ok(&c.when))
            .map(|c| {
                let mut c = c.clone();
                c.views.retain(|v| ok(&v.when));
                c
            })
            .filter(|c| !c.views.is_empty())
            .collect()
    }

    /// The webview showing contributed view `view_id`, asking its provider
    /// to fill one the first time.
    pub fn webview_for_view(&mut self, view_id: &str, title: &str) -> Option<u64> {
        if let Some((h, _)) = self
            .webviews
            .iter()
            .find(|(_, w)| w.kind == WebviewKind::View { view_id: view_id.to_string() })
        {
            return Some(*h);
        }
        if self.ready && self.requested_views.insert(view_id.to_string()) {
            if let Some(h) = self.host.as_mut() {
                h.send(&json!({ "type": "resolveWebviewView", "viewId": view_id, "title": title }));
            }
        }
        None
    }

    /// A message from a webview page (`acquireVsCodeApi().postMessage`).
    pub fn webview_message(&mut self, handle: u64, message: Value) {
        if let Some(h) = self.host.as_mut() {
            h.send(&json!({ "type": "webviewMessage", "handle": handle, "message": message }));
        }
    }

    /// The user closed a webview panel's tab.
    pub fn close_webview(&mut self, handle: u64) {
        if self.webviews.remove(&handle).is_some() {
            if let Some(h) = self.host.as_mut() {
                h.send(&json!({ "type": "webviewClosed", "handle": handle }));
            }
        }
        self.webview_visible.remove(&handle);
    }

    /// Tell the extension whether its webview is on screen (only changes).
    pub fn set_webview_visible(&mut self, handle: u64, visible: bool) {
        if self.webview_visible.get(&handle) == Some(&visible) {
            return;
        }
        self.webview_visible.insert(handle, visible);
        if let Some(h) = self.host.as_mut() {
            h.send(&json!({ "type": "webviewVisible", "handle": handle, "visible": visible }));
        }
    }

    /// Messages waiting for a webview page.
    pub fn take_page_messages(&mut self, handle: u64) -> Vec<String> {
        self.webviews.get_mut(&handle).map(|w| std::mem::take(&mut w.to_page)).unwrap_or_default()
    }

    /// Ask the host how much of the published API is implemented; the
    /// answer lands in `api_coverage`.
    pub fn request_api_coverage(&mut self) {
        if let Some(h) = self.host.as_mut() {
            h.send(&json!({ "type": "apiCoverage" }));
        }
    }

    /// Whether the host has started and answered `init`.
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Answer a host request (quick pick, input box, edits, open editor).
    pub fn answer(&mut self, request_id: u64, value: Value) {
        self.respond(request_id, value);
    }

    /// Whether `id` belongs to an extension (contributed or registered).
    pub fn owns_command(&self, id: &str) -> bool {
        self.registered.contains(id)
            || self.extensions.iter().any(|e| e.commands.iter().any(|c| c.id == id))
    }

    /// Fire an activation event such as `onLanguage:rust`, once.
    pub fn fire_event(&mut self, event: &str) {
        if !self.fired_events.insert(event.to_string()) {
            return;
        }
        let wanted = self
            .extensions
            .iter()
            .any(|e| e.activation_events.iter().any(|a| a == event));
        if !wanted {
            return;
        }
        let msg = json!({ "type": "activateByEvent", "event": event });
        match self.host.as_mut() {
            Some(h) if self.ready => h.send(&msg),
            Some(_) => self.queued.push(msg),
            None => {}
        }
    }

    /// Dismiss a toast; `action` is the clicked button, if any.
    pub fn close_notification(&mut self, id: u64, action: Option<usize>) {
        let Some(pos) = self.notifications.iter().position(|n| n.id == id) else { return };
        let n = self.notifications.remove(pos);
        if let Some(request_id) = n.request_id {
            let value = action.map(|a| json!(a)).unwrap_or(Value::Null);
            self.respond(request_id, value);
        }
    }

    /// Handle everything the host sent since the last call.
    pub fn poll(&mut self) -> Vec<EditorRequest> {
        let mut out = Vec::new();
        loop {
            let Some(event) = self.host.as_ref().and_then(|h| h.try_event()) else { break };
            match event {
                host::HostEvent::Message(msg) => self.handle(msg, &mut out),
                host::HostEvent::Stderr(line) => self.push_log(line),
                host::HostEvent::Exited => {
                    self.push_log("The extension host stopped.".into());
                    self.host = None;
                    self.ready = false;
                    for state in self.activation.values_mut() {
                        if *state == Activation::Active {
                            *state = Activation::Failed("The extension host stopped.".into());
                        }
                    }
                    break;
                }
            }
        }
        out
    }

    /// Block until the host has handled everything and `until` holds, or
    /// `timeout` passes (for tests). Returns whether `until` held.
    pub fn wait_until(
        &mut self,
        timeout: std::time::Duration,
        mut until: impl FnMut(&Self) -> bool,
    ) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            let polled = self.poll();
            self.waited_requests.extend(polled);
            if until(self) {
                return true;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            let Some(host) = self.host.as_ref() else { return until(self) };
            let wait = left.min(std::time::Duration::from_millis(50));
            if let Some(ev) = host.wait_event(wait) {
                match ev {
                    host::HostEvent::Message(m) => {
                        let mut requests = Vec::new();
                        self.handle(m, &mut requests);
                        self.waited_requests.extend(requests);
                    }
                    host::HostEvent::Stderr(l) => self.push_log(l),
                    host::HostEvent::Exited => {
                        self.host = None;
                        return until(self);
                    }
                }
            }
        }
    }

    fn handle(&mut self, msg: Value, out: &mut Vec<EditorRequest>) {
        let kind = msg.get("type").and_then(Value::as_str).unwrap_or_default();
        let s = |k: &str| msg.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let request_id = msg.get("requestId").and_then(Value::as_u64);
        match kind {
            "ready" => {
                self.ready = true;
                let queued = std::mem::take(&mut self.queued);
                if let Some(h) = self.host.as_mut() {
                    for m in &queued {
                        h.send(m);
                    }
                }
            }
            "activated" => {
                let ms = |k: &str| msg.get(k).and_then(Value::as_u64).unwrap_or(0);
                self.activation_times.insert(
                    s("id"),
                    ActivationTime { load_ms: ms("loadMs"), activate_ms: ms("activateMs"), reason: s("reason") },
                );
                self.activation.insert(s("id"), Activation::Active);
            }
            "activationFailed" => {
                let id = s("id");
                let error = s("error");
                self.push_log(format!("Activating extension '{id}' failed: {error}"));
                let first_line = error.lines().next().unwrap_or_default().to_string();
                self.activation.insert(id.clone(), Activation::Failed(first_line.clone()));
                self.notify(
                    Severity::Error,
                    format!("Activating extension '{id}' failed: {first_line}"),
                    Vec::new(),
                    None,
                    None,
                );
            }
            "registerCommand" => {
                self.registered.insert(s("id"));
            }
            "unregisterCommand" => {
                self.registered.remove(&s("id"));
            }
            "showMessage" => {
                let severity = match s("level").as_str() {
                    "error" => Severity::Error,
                    "warning" => Severity::Warning,
                    _ => Severity::Info,
                };
                let actions = msg
                    .get("items")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                    .unwrap_or_default();
                let source = self.display_name(&s("extension"));
                self.notify(severity, s("text"), actions, source, request_id);
            }
            "statusBarItem" => {
                let handle = msg.get("handle").and_then(Value::as_u64).unwrap_or_default();
                let visible = msg.get("visible").and_then(Value::as_bool).unwrap_or(false);
                if visible {
                    self.status_items.insert(
                        handle,
                        StatusItem {
                            handle,
                            right: s("alignment") == "right",
                            priority: msg.get("priority").and_then(Value::as_f64).unwrap_or(0.0),
                            text: s("text"),
                            tooltip: msg.get("tooltip").and_then(Value::as_str).map(str::to_string),
                            command: msg.get("command").and_then(Value::as_str).map(str::to_string),
                            command_args: msg.get("commandArgs").cloned().unwrap_or(Value::Null),
                        },
                    );
                } else {
                    self.status_items.remove(&handle);
                }
            }
            "statusMessage" => out.push(EditorRequest::StatusMessage(s("text"))),
            "commandResult" => {
                if msg.get("ok").and_then(Value::as_bool) == Some(false) {
                    let text = format!(
                        "Running the contributed command: '{}' failed. {}",
                        s("id"),
                        s("error")
                    );
                    self.notify(Severity::Error, text, Vec::new(), None, None);
                }
            }
            "executeEditorCommand" => {
                out.push(EditorRequest::Command(s("id")));
                if let Some(r) = request_id {
                    self.respond(r, Value::Null);
                }
            }
            "showQuickPick" => {
                let items = msg
                    .get("items")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .map(|i| PickItem {
                                label: i.get("label").and_then(Value::as_str).unwrap_or_default().to_string(),
                                description: i.get("description").and_then(Value::as_str).map(str::to_string),
                                detail: i.get("detail").and_then(Value::as_str).map(str::to_string),
                                separator: i.get("separator").and_then(Value::as_bool).unwrap_or(false),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(request_id) = request_id {
                    out.push(EditorRequest::QuickPick {
                        request_id,
                        items,
                        placeholder: msg.get("placeholder").and_then(Value::as_str).map(str::to_string),
                        can_pick_many: msg.get("canPickMany").and_then(Value::as_bool).unwrap_or(false),
                    });
                }
            }
            "showInputBox" => {
                if let Some(request_id) = request_id {
                    out.push(EditorRequest::InputBox {
                        request_id,
                        prompt: msg.get("prompt").and_then(Value::as_str).map(str::to_string),
                        placeholder: msg.get("placeholder").and_then(Value::as_str).map(str::to_string),
                        value: s("value"),
                        password: msg.get("password").and_then(Value::as_bool).unwrap_or(false),
                    });
                }
            }
            "applyEdits" => {
                let edits = msg
                    .get("edits")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|e| {
                                let r = e.get("range")?.as_array()?;
                                let n = |i: usize| r.get(i).and_then(Value::as_u64).unwrap_or(0) as usize;
                                Some(WireEdit {
                                    range: [n(0), n(1), n(2), n(3)],
                                    text: e.get("text").and_then(Value::as_str).unwrap_or_default().to_string(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(request_id) = request_id {
                    out.push(EditorRequest::ApplyEdits { request_id, path: PathBuf::from(s("path")), edits });
                }
            }
            "openDiff" => {
                if let Some(request_id) = request_id {
                    out.push(EditorRequest::OpenDiff {
                        request_id,
                        path: PathBuf::from(s("path")),
                        base: s("base"),
                        text: s("text"),
                        title: s("title"),
                    });
                }
            }
            "closeTabs" => {
                if let Some(request_id) = request_id {
                    let paths = msg
                        .get("paths")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().filter_map(Value::as_str).map(PathBuf::from).collect())
                        .unwrap_or_default();
                    out.push(EditorRequest::CloseTabs { request_id, paths });
                }
            }
            "openEditor" => {
                if let Some(request_id) = request_id {
                    out.push(EditorRequest::OpenEditor { request_id, path: PathBuf::from(s("path")) });
                }
            }
            "saveDocument" => {
                if let Some(request_id) = request_id {
                    out.push(EditorRequest::SaveDocument { request_id, path: PathBuf::from(s("path")) });
                }
            }
            "reveal" => {
                let line = msg.get("line").and_then(Value::as_u64).unwrap_or(0) as usize;
                out.push(EditorRequest::Reveal { path: PathBuf::from(s("path")), line });
            }
            "setSelection" => {
                let sel = msg.get("selection").and_then(Value::as_array);
                let n = |i: usize| sel.and_then(|a| a.get(i)).and_then(Value::as_u64).unwrap_or(0) as usize;
                out.push(EditorRequest::SetSelection { path: PathBuf::from(s("path")), line: n(2), character: n(3) });
            }
            "setContext" => {
                let value = msg.get("value").cloned().unwrap_or(Value::Null);
                self.context.insert(s("key"), value);
            }
            "webviewCreated" => {
                let Some(handle) = msg.get("handle").and_then(Value::as_u64) else { return };
                let kind = if s("kind") == "view" {
                    WebviewKind::View { view_id: s("viewId") }
                } else {
                    WebviewKind::Panel { view_type: s("viewType") }
                };
                let mut roots: Vec<PathBuf> = msg
                    .get("localResourceRoots")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).map(PathBuf::from).collect())
                    .unwrap_or_default();
                if let Some(p) = msg.get("extensionPath").and_then(Value::as_str) {
                    roots.push(PathBuf::from(p));
                }
                if let Some(w) = &self.workspace {
                    roots.push(w.clone());
                }
                let is_panel = matches!(kind, WebviewKind::Panel { .. });
                self.webviews.insert(
                    handle,
                    WebviewState {
                        handle,
                        kind,
                        title: s("title"),
                        html: String::new(),
                        html_version: 0,
                        roots,
                        to_page: Vec::new(),
                        reveal: true,
                    },
                );
                if is_panel {
                    self.opened_panels.push(handle);
                }
            }
            "webviewHtml" => {
                if let Some(w) = msg.get("handle").and_then(Value::as_u64).and_then(|h| self.webviews.get_mut(&h)) {
                    w.html = s("html");
                    w.html_version += 1;
                }
            }
            "webviewOptions" => {
                if let Some(w) = msg.get("handle").and_then(Value::as_u64).and_then(|h| self.webviews.get_mut(&h)) {
                    for r in msg.get("localResourceRoots").and_then(Value::as_array).into_iter().flatten() {
                        if let Some(p) = r.as_str() {
                            w.roots.push(PathBuf::from(p));
                        }
                    }
                }
            }
            "webviewPostMessage" => {
                if let Some(w) = msg.get("handle").and_then(Value::as_u64).and_then(|h| self.webviews.get_mut(&h)) {
                    w.to_page.push(msg.get("message").cloned().unwrap_or(Value::Null).to_string());
                }
            }
            "webviewTitle" => {
                if let Some(w) = msg.get("handle").and_then(Value::as_u64).and_then(|h| self.webviews.get_mut(&h)) {
                    w.title = s("title");
                }
            }
            "webviewReveal" => {
                if let Some(w) = msg.get("handle").and_then(Value::as_u64).and_then(|h| self.webviews.get_mut(&h)) {
                    w.reveal = true;
                }
            }
            "webviewDisposed" => {
                if let Some(handle) = msg.get("handle").and_then(Value::as_u64) {
                    if self.webviews.remove(&handle).is_some() {
                        self.closed_panels.push(handle);
                    }
                }
            }
            "webviewViewProvider" => {}
            "providerRegistered" => {
                self.provider_kinds.insert(s("kind"));
                if let Some(chars) = msg.get("triggerCharacters").and_then(Value::as_array) {
                    self.completion_triggers.extend(chars.iter().filter_map(Value::as_str).map(str::to_string));
                }
            }
            "diagnostics" => {
                let items: Vec<Diagnostic> = msg
                    .get("items")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|d| {
                                Some(Diagnostic {
                                    range: wire_range(d.get("range")?)?,
                                    message: d.get("message").and_then(Value::as_str).unwrap_or_default().to_string(),
                                    severity: d.get("severity").and_then(Value::as_u64).unwrap_or(0).min(3) as u8,
                                    source: d.get("source").and_then(Value::as_str).map(str::to_string),
                                    code: d.get("code").and_then(Value::as_str).map(str::to_string),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let path = PathBuf::from(s("path"));
                let per_file = self.diagnostics.entry(path.clone()).or_default();
                if items.is_empty() {
                    per_file.remove(&s("owner"));
                    if per_file.is_empty() {
                        self.diagnostics.remove(&path);
                    }
                } else {
                    per_file.insert(s("owner"), items);
                }
            }
            "providerResult" => {
                if let Some(id) = request_id {
                    let result = msg.get("result").cloned().unwrap_or(Value::Null);
                    let parsed = match s("kind").as_str() {
                        "provideCompletion" => ProviderResult::Completion {
                            incomplete: result.get("isIncomplete").and_then(Value::as_bool).unwrap_or(false),
                            items: result
                                .get("items")
                                .and_then(Value::as_array)
                                .map(|a| a.iter().filter_map(completion_entry).collect())
                                .unwrap_or_default(),
                        },
                        "provideHover" => ProviderResult::Hover {
                            contents: result
                                .get("contents")
                                .and_then(Value::as_array)
                                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                                .unwrap_or_default(),
                            range: result.get("range").and_then(wire_range),
                        },
                        _ => ProviderResult::Definition(
                            result
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|l| {
                                            Some(SourceLocation {
                                                path: PathBuf::from(l.get("path")?.as_str()?),
                                                range: wire_range(l.get("range")?)?,
                                            })
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                        ),
                    };
                    self.provider_results.insert(id, parsed);
                }
            }
            "apiCoverage" => {
                self.api_coverage = Some(msg.clone());
            }
            "decorationType" => {
                let style = msg.get("options").map(decoration_style).unwrap_or_default();
                self.decoration_types.insert(s("key"), style);
            }
            "decorationTypeDisposed" => {
                let key = s("key");
                self.decoration_types.remove(&key);
                for per_doc in self.decorations.values_mut() {
                    per_doc.remove(&key);
                }
            }
            "setDecorations" => {
                let ranges = msg
                    .get("ranges")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|d| {
                                let r = d.get("range")?.as_array()?;
                                let n = |i: usize| r.get(i).and_then(Value::as_u64).unwrap_or(0) as usize;
                                Some(DecoratedRange {
                                    range: [n(0), n(1), n(2), n(3)],
                                    hover: d.get("hover").and_then(Value::as_str).map(str::to_string),
                                    after: d.get("after").and_then(Value::as_str).map(str::to_string),
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let per_doc = self.decorations.entry(PathBuf::from(s("path"))).or_default();
                if ranges.is_empty() {
                    per_doc.remove(&s("key"));
                } else {
                    per_doc.insert(s("key"), ranges);
                }
            }
            "clipboardRead" => {
                if let Some(r) = request_id {
                    self.respond(r, json!(""));
                }
            }
            "clipboardWrite" => out.push(EditorRequest::CopyText(s("text"))),
            "openExternal" => {
                let url = s("url");
                if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("mailto:") {
                    let _ = std::process::Command::new("open").arg(&url).spawn();
                }
            }
            "outputCreate" => {
                self.channels.entry(s("channel")).or_default();
            }
            "outputDispose" => {
                self.channels.remove(&s("channel"));
            }
            "outputShow" => {
                let channel = s("channel");
                self.channels.entry(channel.clone()).or_default();
                self.show_channel = Some(channel);
            }
            "output" => {
                let channel = s("channel");
                let text = s("text");
                let entry = self.channels.entry(channel.clone()).or_default();
                if msg.get("replace").and_then(Value::as_bool).unwrap_or(false) {
                    entry.clear();
                }
                entry.push_str(&text);
                // Output channels keep the last ~1 MiB, as the output service does.
                if entry.len() > 1 << 20 {
                    let cut = entry.len() - (1 << 20);
                    let at = entry[cut..].find('\n').map(|n| cut + n + 1).unwrap_or(cut);
                    entry.drain(..at);
                }
                for line in text.lines() {
                    self.push_log(format!("[{channel}] {line}"));
                }
            }
            "log" => {
                let level = s("level");
                self.push_log(format!("[{level}] {}", s("text")));
            }
            _ => {}
        }
    }

    fn display_name(&self, id: &str) -> Option<String> {
        self.extensions.iter().find(|e| e.id == id).map(|e| e.display_name.clone())
    }

    fn notify(
        &mut self,
        severity: Severity,
        text: String,
        actions: Vec<String>,
        source: Option<String>,
        request_id: Option<u64>,
    ) {
        self.next_notification += 1;
        self.notifications.push(Notification {
            id: self.next_notification,
            severity,
            text,
            actions,
            source,
            request_id,
            created: Instant::now(),
        });
    }

    fn respond(&mut self, request_id: u64, value: Value) {
        if let Some(h) = self.host.as_mut() {
            h.send(&json!({ "type": "response", "requestId": request_id, "value": value }));
        }
    }

    fn request_id(&mut self) -> u64 {
        self.next_request += 1;
        self.next_request
    }

    fn push_log(&mut self, line: String) {
        self.log.push(line);
        if self.log.len() > LOG_LINES {
            let extra = self.log.len() - LOG_LINES;
            self.log.drain(..extra);
        }
    }
}

/// Turn `DecorationRenderOptions` into what the editor can draw.
fn decoration_style(options: &Value) -> DecorationStyle {
    let color = |k: &str| options.get(k).and_then(css_color);
    let border = options
        .get("borderColor")
        .and_then(css_color)
        .or_else(|| options.get("border").and_then(Value::as_str).and_then(border_color));
    DecorationStyle {
        background: color("backgroundColor"),
        color: color("color"),
        border,
        bold: options
            .get("fontWeight")
            .and_then(Value::as_str)
            .is_some_and(|w| w == "bold" || w.parse::<u32>().is_ok_and(|n| n >= 600)),
        whole_line: options.get("isWholeLine").and_then(Value::as_bool).unwrap_or(false),
    }
}

/// The colour in a CSS `border` shorthand (`"1px solid red"`).
fn border_color(border: &str) -> Option<egui::Color32> {
    border.split_whitespace().find_map(|part| css_color(&Value::String(part.to_string())))
}

/// A CSS colour (`#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, a few
/// names) or a theme colour reference.
pub fn css_color(v: &Value) -> Option<egui::Color32> {
    if let Some(id) = v.get("themeColor").and_then(Value::as_str) {
        return theme_color(id);
    }
    let s = v.as_str()?.trim().to_lowercase();
    if let Some(hex) = s.strip_prefix('#') {
        let h = |i: usize, len: usize| u8::from_str_radix(&hex[i..i + len], 16).ok();
        let dup = |x: u8| x * 17;
        return match hex.len() {
            3 => Some(egui::Color32::from_rgb(dup(h(0, 1)?), dup(h(1, 1)?), dup(h(2, 1)?))),
            4 => Some(egui::Color32::from_rgba_unmultiplied(dup(h(0, 1)?), dup(h(1, 1)?), dup(h(2, 1)?), dup(h(3, 1)?))),
            6 => Some(egui::Color32::from_rgb(h(0, 2)?, h(2, 2)?, h(4, 2)?)),
            8 => Some(egui::Color32::from_rgba_unmultiplied(h(0, 2)?, h(2, 2)?, h(4, 2)?, h(6, 2)?)),
            _ => None,
        };
    }
    if let Some(args) = s.strip_prefix("rgba(").or_else(|| s.strip_prefix("rgb(")) {
        let parts: Vec<f32> = args
            .trim_end_matches(')')
            .split(',')
            .filter_map(|p| p.trim().trim_end_matches('%').parse().ok())
            .collect();
        if parts.len() >= 3 {
            let a = parts.get(3).map(|a| if *a <= 1.0 { a * 255.0 } else { *a }).unwrap_or(255.0);
            return Some(egui::Color32::from_rgba_unmultiplied(
                parts[0] as u8,
                parts[1] as u8,
                parts[2] as u8,
                a.clamp(0.0, 255.0) as u8,
            ));
        }
        return None;
    }
    let named = match s.as_str() {
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "blue" => (0, 0, 255),
        "yellow" => (255, 255, 0),
        "orange" => (255, 165, 0),
        "white" => (255, 255, 255),
        "black" => (0, 0, 0),
        "gray" | "grey" => (128, 128, 128),
        "purple" => (128, 0, 128),
        "pink" => (255, 192, 203),
        "cyan" => (0, 255, 255),
        "magenta" => (255, 0, 255),
        "transparent" => return Some(egui::Color32::TRANSPARENT),
        _ => return None,
    };
    Some(egui::Color32::from_rgb(named.0, named.1, named.2))
}

/// Theme colours extensions commonly decorate with (2026-dark.json values).
fn theme_color(id: &str) -> Option<egui::Color32> {
    use crate::theme::Palette;
    Some(match id {
        "editor.findMatchHighlightBackground" => Palette::FIND_MATCH_HL_BG(),
        "editor.findMatchBackground" => Palette::FIND_MATCH_BG(),
        "editorError.foreground" | "errorForeground" => Palette::VSCE_NOTIFICATIONS_ERROR_ICON(),
        "editorWarning.foreground" => Palette::VSCE_NOTIFICATIONS_WARNING_ICON(),
        "editorInfo.foreground" => Palette::VSCE_NOTIFICATIONS_INFO_ICON(),
        "descriptionForeground" | "editorCodeLens.foreground" | "editorInlayHint.foreground" => Palette::FG_DESCRIPTION(),
        "foreground" | "editor.foreground" => Palette::FG(),
        "textLink.foreground" => Palette::VSCE_EXTENSION_VERIFIED_FG(),
        _ => return None,
    })
}

/// Which language feature to ask the extensions for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LanguageFeature {
    Completion,
    Hover,
    Definition,
}

fn wire_range(v: &Value) -> Option<[usize; 4]> {
    let a = v.as_array()?;
    let n = |i: usize| a.get(i).and_then(Value::as_u64).map(|x| x as usize);
    Some([n(0)?, n(1)?, n(2)?, n(3)?])
}

fn completion_entry(v: &Value) -> Option<CompletionEntry> {
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let label = s("label")?;
    Some(CompletionEntry {
        kind: s("kind").unwrap_or_else(|| "Text".into()),
        detail: s("detail"),
        documentation: s("documentation"),
        insert_text: s("insertText").unwrap_or_else(|| label.clone()),
        range: v.get("range").and_then(wire_range)?,
        sort_text: s("sortText").unwrap_or_else(|| label.clone()),
        filter_text: s("filterText").unwrap_or_else(|| label.clone()),
        preselect: v.get("preselect").and_then(Value::as_bool).unwrap_or(false),
        label,
    })
}
