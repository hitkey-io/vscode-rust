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

/// Something the editor itself has to do for an extension.
#[derive(Clone, Debug, PartialEq)]
pub enum EditorRequest {
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
            self.poll();
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
                        let mut ignored = Vec::new();
                        self.handle(m, &mut ignored);
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
            "showQuickPick" | "showInputBox" => {
                // Not built yet: answer "cancelled" so the extension goes on.
                self.push_log(format!("{kind} isn't supported yet; answered as cancelled."));
                if let Some(r) = request_id {
                    self.respond(r, Value::Null);
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
