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

#[derive(Default)]
pub struct ExtensionService {
    pub extensions: Vec<Extension>,
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
}

impl ExtensionService {
    /// Read the extensions in `dirs`. Nothing runs yet.
    pub fn load(&mut self, dirs: &[PathBuf]) {
        self.dirs = dirs.to_vec();
        self.extensions = manifest::scan(dirs);
        self.activation = self
            .extensions
            .iter()
            .map(|e| (e.id.clone(), Activation::Inactive))
            .collect();
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
        self.workspace = workspace.map(Path::to_path_buf);
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
        self.status_items.clear();
        self.fired_events.clear();
        self.synced.clear();
        self.synced_active = None;
        self.decoration_types.clear();
        self.decorations.clear();
        let dirs = self.dirs.clone();
        self.load(&dirs);
        let workspace = self.workspace.clone();
        if let Some(wake) = self.wake.clone() {
            self.start(workspace.as_deref(), move || wake());
        }
    }

    /// Uninstall an extension this editor installed, then reload.
    pub fn uninstall(&mut self, id: &str) {
        let Some(ext) = self.extensions.iter().find(|e| e.id == id) else { return };
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
        self.extensions
            .iter()
            .find(|e| e.id == id)
            .is_some_and(|e| e.path.parent() == Some(dir.as_path()))
    }

    /// Whether an extension with this id (any case) is installed.
    pub fn is_installed(&self, id: &str) -> bool {
        self.extensions.iter().any(|e| e.id.eq_ignore_ascii_case(id))
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
        busy
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
            "output" => {
                let channel = s("channel");
                for line in s("text").lines() {
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
        "editor.findMatchHighlightBackground" => Palette::FIND_MATCH_HL_BG,
        "editor.findMatchBackground" => Palette::FIND_MATCH_BG,
        "editorError.foreground" | "errorForeground" => Palette::VSCE_NOTIFICATIONS_ERROR_ICON,
        "editorWarning.foreground" => Palette::VSCE_NOTIFICATIONS_WARNING_ICON,
        "editorInfo.foreground" => Palette::VSCE_NOTIFICATIONS_INFO_ICON,
        "descriptionForeground" | "editorCodeLens.foreground" | "editorInlayHint.foreground" => Palette::FG_DESCRIPTION,
        "foreground" | "editor.foreground" => Palette::FG,
        "textLink.foreground" => Palette::VSCE_EXTENSION_VERIFIED_FG,
        _ => return None,
    })
}
