//! The extension host process: Node.js running `assets/ext-host/host.js`,
//! one JSON object per line on stdin (requests) and stdout (events).
//!
//! VS Code analogue: `src/vs/workbench/services/extensions/electron-browser/
//! localProcessExtensionHost.ts` (spawns the host and wires its pipes).

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};

use serde_json::{json, Value};

const HOST_SCRIPT: &str = include_str!("../../assets/ext-host/host.js");
/// Names of the published API (generated from `vscode.d.ts`), spliced into
/// the script.
const API_NAMES: &str = include_str!("../../assets/ext-host/api-names.json");
/// Read by extensions from `env.appRoot` (e.g. to check the product name).
const PRODUCT_JSON: &str = r#"{"nameShort":"Code","nameLong":"Visual Studio Code","applicationName":"code","version":"1.119.0","quality":"stable"}"#;

/// Something the host process said, or that it went away.
#[derive(Debug, Clone)]
pub enum HostEvent {
    /// A protocol message (`type` plus fields).
    Message(Value),
    /// A line on the host's stderr (Node warnings, crashes).
    Stderr(String),
    /// The process ended.
    Exited,
}

pub struct Host {
    child: Child,
    stdin: ChildStdin,
    events: Receiver<HostEvent>,
}

impl Host {
    /// Start the host with `node`. `wake` runs after every event so a UI can
    /// schedule a repaint.
    pub fn spawn(node: &Path, wake: impl Fn() + Send + Sync + 'static) -> std::io::Result<Self> {
        let script = write_script()?;
        let mut child = Command::new(node)
            .arg(&script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("ELECTRON_RUN_AS_NODE", "1")
            .spawn()?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");

        let (tx, rx) = channel();
        let wake = std::sync::Arc::new(wake);
        {
            let tx: Sender<HostEvent> = tx.clone();
            let wake = wake.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if let Ok(v) = serde_json::from_str::<Value>(&line) {
                        if v.get("type").is_some() && tx.send(HostEvent::Message(v)).is_err() {
                            return;
                        }
                        wake();
                    }
                }
                let _ = tx.send(HostEvent::Exited);
                wake();
            });
        }
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                if tx.send(HostEvent::Stderr(line)).is_err() {
                    return;
                }
                wake();
            }
        });

        Ok(Self { child, stdin, events: rx })
    }

    pub fn send(&mut self, msg: &Value) {
        let mut line = msg.to_string();
        line.push('\n');
        let _ = self.stdin.write_all(line.as_bytes());
        let _ = self.stdin.flush();
    }

    /// Next event, if one has arrived.
    pub fn try_event(&self) -> Option<HostEvent> {
        self.events.try_recv().ok()
    }

    /// Wait up to `timeout` for the next event.
    pub fn wait_event(&self, timeout: std::time::Duration) -> Option<HostEvent> {
        self.events.recv_timeout(timeout).ok()
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        // Let extensions run `deactivate`, then make sure the process goes.
        self.send(&json!({ "type": "shutdown" }));
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
        while std::time::Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The host script, written once per content version to the temp folder so
/// the binary works from anywhere.
fn write_script() -> std::io::Result<PathBuf> {
    use std::hash::{Hash, Hasher};
    let script = HOST_SCRIPT.replacen("/*API_NAMES*/null", API_NAMES, 1);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    script.hash(&mut h);
    let dir = std::env::temp_dir().join("vscode-rust");
    std::fs::create_dir_all(&dir)?;
    let product = dir.join("product.json");
    if !product.exists() {
        let _ = std::fs::write(&product, PRODUCT_JSON);
    }
    let path = dir.join(format!("ext-host-{:016x}.js", h.finish()));
    if !path.exists() {
        // Unique temp name: several hosts may start at once.
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = path.with_extension(format!("{}-{seq}.tmp", std::process::id()));
        std::fs::write(&tmp, &script)?;
        if std::fs::rename(&tmp, &path).is_err() {
            let _ = std::fs::remove_file(&tmp);
            if !path.exists() {
                return Err(std::io::Error::other("couldn't write the extension host script"));
            }
        }
    }
    Ok(path)
}

/// Find a Node.js binary: `VSCODE_RUST_NODE`, then `PATH`, then the usual
/// install locations (apps started from the Finder don't get the shell's
/// `PATH`).
pub fn find_node() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("VSCODE_RUST_NODE").map(PathBuf::from) {
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let p = dir.join("node");
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let mut candidates = vec![
        PathBuf::from("/opt/homebrew/bin/node"),
        PathBuf::from("/usr/local/bin/node"),
        PathBuf::from("/usr/bin/node"),
        home.join(".local/bin/node"),
        home.join(".volta/bin/node"),
    ];
    // nvm: newest installed version.
    if let Ok(entries) = std::fs::read_dir(home.join(".nvm/versions/node")) {
        let mut versions: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        versions.sort();
        candidates.extend(versions.into_iter().rev().map(|v| v.join("bin/node")));
    }
    candidates.into_iter().find(|p| p.is_file())
}
