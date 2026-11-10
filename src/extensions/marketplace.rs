//! The extension marketplace: search, popular and recommended lists, icons,
//! install and uninstall.
//!
//! The catalogue is [Open VSX](https://open-vsx.org), the open registry
//! used by open-source builds of VS Code (the Visual Studio Marketplace's
//! terms limit it to Microsoft's own products). `VSCODE_RUST_GALLERY`
//! points it at another Open VSX-compatible server.
//!
//! Network and archive work run on background threads through the system's
//! `curl` and `unzip`; results arrive through [`Marketplace::poll`].
//!
//! VS Code analogues:
//! - `src/vs/platform/extensionManagement/common/extensionGalleryService.ts`
//! - `src/vs/platform/extensionManagement/node/extensionManagementService.ts`
//!   (a `.vsix` is a zip whose `extension/` folder becomes
//!   `<publisher>.<name>-<version>` in the extensions folder)
//! - `src/vs/workbench/contrib/extensions/browser/extensionsViews.ts`

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;

const DEFAULT_GALLERY: &str = "https://open-vsx.org";
/// Results per list.
const PAGE: usize = 30;
/// Wait this long after the last keystroke before searching.
pub const SEARCH_DEBOUNCE: Duration = Duration::from_millis(350);

/// An extension as the catalogue describes it.
#[derive(Clone, Debug, PartialEq)]
pub struct GalleryExtension {
    pub namespace: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub downloads: u64,
    pub rating: Option<f32>,
    pub verified: bool,
    pub icon_url: Option<String>,
}

impl GalleryExtension {
    /// `publisher.name`, the extension id.
    pub fn id(&self) -> String {
        format!("{}.{}", self.namespace, self.name)
    }

    fn from_json(v: &Value) -> Option<Self> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let namespace = s("namespace");
        let name = s("name");
        if namespace.is_empty() || name.is_empty() {
            return None;
        }
        let display_name = {
            let d = s("displayName");
            if d.is_empty() { name.clone() } else { d }
        };
        Some(Self {
            display_name,
            description: s("description"),
            version: s("version"),
            downloads: v.get("downloadCount").and_then(Value::as_u64).unwrap_or(0),
            rating: v.get("averageRating").and_then(Value::as_f64).map(|r| r as f32),
            verified: v.get("verified").and_then(Value::as_bool).unwrap_or(false),
            icon_url: v.pointer("/files/icon").and_then(Value::as_str).map(str::to_string),
            namespace,
            name,
        })
    }
}

/// Parse an Open VSX `/api/-/search` answer.
pub fn parse_search(json: &str) -> Result<Vec<GalleryExtension>, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("Unexpected answer: {e}"))?;
    if let Some(err) = v.get("error").and_then(Value::as_str) {
        return Err(err.to_string());
    }
    Ok(v.get("extensions")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(GalleryExtension::from_json).collect())
        .unwrap_or_default())
}

/// "244.6M", "12.4K", "950" — VS Code's download count format.
pub fn format_count(n: u64) -> String {
    let f = n as f64;
    let (v, unit) = if f >= 1e9 {
        (f / 1e9, "B")
    } else if f >= 1e6 {
        (f / 1e6, "M")
    } else if f >= 1e3 {
        (f / 1e3, "K")
    } else {
        return n.to_string();
    };
    let s = format!("{v:.1}");
    format!("{}{unit}", s.trim_end_matches(".0"))
}

/// Which list a fetch fills.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum List {
    Popular,
    Search(String),
    Recommended,
}

/// What the extension page shows for a catalogue extension.
#[derive(Clone, Debug, Default)]
pub struct GalleryDetails {
    /// `/api/{namespace}/{name}` answer.
    pub json: Value,
    pub readme: Option<String>,
    pub changelog: Option<String>,
    /// Folder of the README file, for its relative links and images.
    pub base_url: Option<String>,
    /// The extension's `package.json` (for its commands and settings).
    pub manifest: Option<Value>,
}

enum Job {
    Details(String, Result<GalleryDetails, String>),
    List(List, Result<Vec<GalleryExtension>, String>),
    Icon(String, Option<Vec<u8>>),
    Installed(String, Result<PathBuf, String>),
}

/// State of one list in the view.
#[derive(Clone, Debug, Default)]
pub struct ListState {
    pub loading: bool,
    pub items: Vec<GalleryExtension>,
    pub error: Option<String>,
}

/// An extension icon: still downloading, ready, or unavailable.
pub enum Icon {
    Loading,
    Ready(egui::TextureHandle),
    Missing,
}

pub struct Marketplace {
    gallery: String,
    /// Where installs go.
    pub install_dir: PathBuf,
    /// Turned off in tests and when there's no network wanted.
    pub enabled: bool,
    pub query: String,
    query_changed: Option<Instant>,
    /// The query the search list holds (or is fetching).
    searched: String,
    pub lists: BTreeMap<List, ListState>,
    /// Workspace recommendations (`.vscode/extensions.json`).
    pub recommendations: Vec<String>,
    pub icons: std::collections::HashMap<String, Icon>,
    /// Ids being installed.
    pub installing: BTreeSet<String>,
    /// Ids installed this session: the caller reloads extensions.
    pub just_installed: Vec<String>,
    /// Install and uninstall failures, for a notification.
    pub errors: Vec<String>,
    /// Extension pages' data by id, once fetched (or the error).
    pub details: std::collections::HashMap<String, Result<GalleryDetails, String>>,
    details_loading: BTreeSet<String>,
    tx: Sender<Job>,
    rx: Receiver<Job>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Default for Marketplace {
    fn default() -> Self {
        let (tx, rx) = channel();
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        Self {
            gallery: std::env::var("VSCODE_RUST_GALLERY")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_GALLERY.to_string())
                .trim_end_matches('/')
                .to_string(),
            install_dir: home.join(".vscode-rust/extensions"),
            enabled: false,
            query: String::new(),
            query_changed: None,
            searched: String::new(),
            lists: BTreeMap::new(),
            recommendations: Vec::new(),
            icons: Default::default(),
            installing: BTreeSet::new(),
            just_installed: Vec::new(),
            errors: Vec::new(),
            details: Default::default(),
            details_loading: BTreeSet::new(),
            tx,
            rx,
            wake: Arc::new(|| {}),
        }
    }
}

impl Marketplace {
    /// Allow network access; `wake` schedules a repaint when results come.
    pub fn enable(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        self.enabled = true;
        self.wake = Arc::new(wake);
    }

    /// Read `.vscode/extensions.json` recommendations of `workspace`.
    pub fn load_recommendations(&mut self, workspace: Option<&Path>) {
        self.recommendations = workspace
            .and_then(|w| std::fs::read_to_string(w.join(".vscode/extensions.json")).ok())
            .and_then(|s| serde_json::from_str::<Value>(&strip_jsonc_comments(&s)).ok())
            .and_then(|v| v.get("recommendations").cloned())
            .and_then(|v| v.as_array().cloned())
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_lowercase).collect())
            .unwrap_or_default();
        self.lists.remove(&List::Recommended);
    }

    /// The search box changed.
    pub fn set_query(&mut self, query: String) {
        if query != self.query {
            self.query = query;
            self.query_changed = Some(Instant::now());
        }
    }

    pub fn list(&self, list: &List) -> Option<&ListState> {
        self.lists.get(list)
    }

    /// The list matching the current query (search results or popular).
    pub fn current_search(&self) -> List {
        List::Search(self.searched.clone())
    }

    /// Start whatever the view needs and isn't loaded yet; pick up finished
    /// work. Returns `true` while something is still loading.
    pub fn tick(&mut self, ctx: &egui::Context) -> bool {
        self.poll(ctx);
        if !self.enabled {
            return false;
        }
        if !self.lists.contains_key(&List::Popular) {
            self.fetch_list(List::Popular);
        }
        if !self.recommendations.is_empty() && !self.lists.contains_key(&List::Recommended) {
            self.fetch_list(List::Recommended);
        }
        let q = self.query.trim().to_string();
        if q != self.searched {
            match self.query_changed {
                Some(t) if t.elapsed() < SEARCH_DEBOUNCE => {
                    ctx.request_repaint_after(SEARCH_DEBOUNCE - t.elapsed());
                }
                _ => {
                    self.searched = q.clone();
                    if !q.is_empty() && !self.lists.contains_key(&List::Search(q.clone())) {
                        self.fetch_list(List::Search(q));
                    }
                }
            }
        }
        self.lists.values().any(|l| l.loading) || !self.installing.is_empty() || !self.details_loading.is_empty()
    }

    /// Start downloading `url` as an icon (once).
    pub fn request_icon(&mut self, url: &str) {
        if !self.enabled || self.icons.contains_key(url) {
            return;
        }
        self.icons.insert(url.to_string(), Icon::Loading);
        let (tx, wake, url) = (self.tx.clone(), self.wake.clone(), url.to_string());
        std::thread::spawn(move || {
            let bytes = cached_download(&url);
            let _ = tx.send(Job::Icon(url, bytes));
            wake();
        });
    }

    /// Fetch what the extension page shows (README, changelog, metadata)
    /// for `namespace.name`, once.
    pub fn request_details(&mut self, namespace: &str, name: &str) {
        let id = format!("{namespace}.{name}");
        if !self.enabled || self.details.contains_key(&id) || !self.details_loading.insert(id.clone()) {
            return;
        }
        let (tx, wake, gallery) = (self.tx.clone(), self.wake.clone(), self.gallery.clone());
        let (ns, nm) = (namespace.to_string(), name.to_string());
        std::thread::spawn(move || {
            let result = (|| {
                let body = http_get(&format!("{gallery}/api/{ns}/{nm}"))?;
                let json: Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
                if let Some(e) = json.get("error").and_then(Value::as_str) {
                    return Err(e.to_string());
                }
                let text = |key: &str| {
                    json.pointer(&format!("/files/{key}"))
                        .and_then(Value::as_str)
                        .and_then(|u| http_get(u).ok())
                        .map(|b| String::from_utf8_lossy(&b).into_owned())
                };
                let base_url = json
                    .pointer("/files/readme")
                    .and_then(Value::as_str)
                    .and_then(|u| u.rfind('/').map(|i| u[..=i].to_string()));
                let manifest = text("manifest").and_then(|m| serde_json::from_str(&m).ok());
                Ok(GalleryDetails { readme: text("readme"), changelog: text("changelog"), base_url, manifest, json })
            })();
            let _ = tx.send(Job::Details(id, result));
            wake();
        });
    }

    /// Download and install the newest version of `ext` for this platform.
    pub fn install(&mut self, ext: &GalleryExtension) {
        let id = ext.id();
        if !self.enabled || !self.installing.insert(id.clone()) {
            return;
        }
        let (tx, wake) = (self.tx.clone(), self.wake.clone());
        let (gallery, dir) = (self.gallery.clone(), self.install_dir.clone());
        let (ns, name) = (ext.namespace.clone(), ext.name.clone());
        std::thread::spawn(move || {
            let result = install_from_gallery(&gallery, &ns, &name, &dir);
            let _ = tx.send(Job::Installed(id, result));
            wake();
        });
    }

    fn fetch_list(&mut self, list: List) {
        self.lists.insert(list.clone(), ListState { loading: true, ..Default::default() });
        let (tx, wake, gallery) = (self.tx.clone(), self.wake.clone(), self.gallery.clone());
        let recommendations = self.recommendations.clone();
        std::thread::spawn(move || {
            let result = match &list {
                List::Popular => search(&gallery, "", "downloadCount"),
                List::Search(q) => search(&gallery, q, "relevance"),
                List::Recommended => Ok(recommendations
                    .iter()
                    .filter_map(|id| {
                        let (ns, name) = id.split_once('.')?;
                        let json = http_get(&format!("{gallery}/api/{ns}/{name}")).ok()?;
                        GalleryExtension::from_json(&serde_json::from_slice(&json).ok()?)
                    })
                    .collect()),
            };
            let _ = tx.send(Job::List(list, result));
            wake();
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(job) = self.rx.try_recv() {
            match job {
                Job::Details(id, result) => {
                    self.details_loading.remove(&id);
                    self.details.insert(id, result);
                }
                Job::List(list, result) => {
                    let state = match result {
                        Ok(items) => ListState { loading: false, items, error: None },
                        Err(e) => ListState { loading: false, items: Vec::new(), error: Some(e) },
                    };
                    self.lists.insert(list, state);
                }
                Job::Icon(url, bytes) => {
                    let icon = bytes
                        .and_then(|b| decode_icon(&b))
                        .map(|img| Icon::Ready(ctx.load_texture(&url, img, egui::TextureOptions::LINEAR)))
                        .unwrap_or(Icon::Missing);
                    self.icons.insert(url, icon);
                }
                Job::Installed(id, result) => {
                    self.installing.remove(&id);
                    match result {
                        Ok(_) => self.just_installed.push(id),
                        Err(e) => self.errors.push(format!("Couldn't install '{id}': {e}")),
                    }
                }
            }
        }
    }
}

/// Remove an extension this editor installed (only inside `install_dir`).
pub fn uninstall(path: &Path, install_dir: &Path) -> Result<(), String> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let dir = install_dir.canonicalize().map_err(|e| e.to_string())?;
    if path.parent() != Some(dir.as_path()) {
        return Err("only extensions installed here can be uninstalled".into());
    }
    std::fs::remove_dir_all(&path).map_err(|e| e.to_string())
}

fn search(gallery: &str, query: &str, sort: &str) -> Result<Vec<GalleryExtension>, String> {
    let mut url = format!("{gallery}/api/-/search?size={PAGE}&sortBy={sort}&sortOrder=desc");
    if !query.is_empty() {
        url.push_str("&query=");
        url.push_str(&percent_encode(query));
    }
    let body = http_get(&url)?;
    parse_search(&String::from_utf8_lossy(&body))
}

/// The platform name Open VSX uses for builds of native extensions.
fn target_platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-arm64",
        ("macos", _) => "darwin-x64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "arm") => "linux-armhf",
        ("linux", _) => "linux-x64",
        ("windows", "aarch64") => "win32-arm64",
        ("windows", _) => "win32-x64",
        _ => "universal",
    }
}

fn install_from_gallery(gallery: &str, ns: &str, name: &str, dir: &Path) -> Result<PathBuf, String> {
    // A build for this platform if there is one, else the universal one.
    let meta = http_get(&format!("{gallery}/api/{ns}/{name}/{}/latest", target_platform()))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .filter(|v| v.get("error").is_none())
        .map(Ok)
        .unwrap_or_else(|| {
            let b = http_get(&format!("{gallery}/api/{ns}/{name}/latest"))?;
            serde_json::from_slice::<Value>(&b).map_err(|e| e.to_string())
        })?;
    if let Some(err) = meta.get("error").and_then(Value::as_str) {
        return Err(err.to_string());
    }
    let url = meta
        .pointer("/files/download")
        .and_then(Value::as_str)
        .ok_or("the catalogue has no download for it")?;
    let tmp = std::env::temp_dir().join(format!("vscode-rust-{}-{ns}.{name}.vsix", std::process::id()));
    let status = Command::new("curl")
        .args(["-fsSL", "--max-time", "300", "-A", "vscode-rust", "-o"])
        .arg(&tmp)
        .arg(url)
        .status()
        .map_err(|e| format!("curl: {e}"))?;
    if !status.success() {
        return Err("the download failed".into());
    }
    let result = install_vsix(&tmp, dir);
    let _ = std::fs::remove_file(&tmp);
    result
}

/// Unpack a `.vsix` into `dir` as `<publisher>.<name>-<version>`, replacing
/// an older copy of the same extension. Returns the new folder.
pub fn install_vsix(vsix: &Path, dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let staging = dir.join(format!(".installing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    let status = Command::new("unzip")
        .args(["-q", "-o"])
        .arg(vsix)
        .arg("extension/*")
        .arg("-d")
        .arg(&staging)
        .status()
        .map_err(|e| format!("unzip: {e}"))?;
    let unpacked = staging.join("extension");
    if !status.success() || !unpacked.join("package.json").is_file() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("the package is damaged or isn't an extension".into());
    }
    let Some(ext) = super::manifest::Extension::read(&unpacked) else {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("its package.json can't be read".into());
    };
    let id = ext.id.to_lowercase();
    // Older versions of the same extension go.
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let folder = e.file_name().to_string_lossy().to_lowercase();
            if folder.starts_with(&format!("{id}-")) {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    let target = dir.join(format!("{id}-{}", ext.version));
    std::fs::rename(&unpacked, &target).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(target)
}

fn http_get(url: &str) -> Result<Vec<u8>, String> {
    let out = Command::new("curl")
        .args(["-fsSL", "--max-time", "30", "-A", "vscode-rust", "-H", "Accept: application/json"])
        .arg(url)
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err("the marketplace can't be reached".into());
    }
    Ok(out.stdout)
}

/// Icons are kept in `~/.vscode-rust/cache/icons` between sessions.
fn cached_download(url: &str) -> Option<Vec<u8>> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    let dir = std::env::var_os("HOME").map(PathBuf::from)?.join(".vscode-rust/cache/icons");
    let file = dir.join(format!("{:016x}", h.finish()));
    if let Ok(bytes) = std::fs::read(&file) {
        return Some(bytes);
    }
    let bytes = http_get(url).ok()?;
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&file, &bytes);
    Some(bytes)
}

fn decode_icon(bytes: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    let img = img.resize(128, 128, image::imageops::FilterType::Triangle).to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// `extensions.json` allows comments (JSONC).
fn strip_jsonc_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut prev = ' ';
                for n in chars.by_ref() {
                    if prev == '*' && n == '/' {
                        break;
                    }
                    prev = n;
                }
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_vsx_search() {
        let json = r#"{"totalSize":1,"extensions":[{"namespace":"redhat","name":"vscode-yaml",
            "displayName":"YAML","description":"YAML Language Support","version":"1.0.0",
            "downloadCount":9196620,"averageRating":4.5,"verified":true,
            "files":{"icon":"https://example.org/icon.png"}}]}"#;
        let list = parse_search(json).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id(), "redhat.vscode-yaml");
        assert_eq!(list[0].rating, Some(4.5));
        assert!(list[0].verified);
        assert_eq!(parse_search(r#"{"error":"nope"}"#), Err("nope".into()));
    }

    #[test]
    fn counts_like_vs_code() {
        assert_eq!(format_count(244_600_000), "244.6M");
        assert_eq!(format_count(12_000), "12K");
        assert_eq!(format_count(950), "950");
    }

    #[test]
    fn jsonc_comments_are_ignored() {
        let s = "{\n // c\n \"recommendations\": [\"a.b\" /* x */, \"c//d\"]\n}";
        let v: Value = serde_json::from_str(&strip_jsonc_comments(s)).unwrap();
        assert_eq!(v["recommendations"][1], "c//d");
    }

    #[test]
    fn installs_a_vsix_and_replaces_older_versions() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/extensions/sample.hello-world-0.0.1");
        let work = std::env::temp_dir().join(format!("vscode-rust-vsix-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let pack = work.join("pack/extension");
        std::fs::create_dir_all(&pack).unwrap();
        for f in ["package.json", "package.nls.json", "extension.js"] {
            std::fs::copy(fixture.join(f), pack.join(f)).unwrap();
        }
        let vsix = work.join("hello.vsix");
        let ok = Command::new("zip")
            .current_dir(work.join("pack"))
            .args(["-qr"])
            .arg(&vsix)
            .arg("extension")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            return; // No zip tool on this machine.
        }
        let dir = work.join("extensions");
        std::fs::create_dir_all(dir.join("sample.hello-world-0.0.0")).unwrap();
        let installed = install_vsix(&vsix, &dir).unwrap();
        assert_eq!(installed.file_name().unwrap(), "sample.hello-world-0.0.1");
        assert!(installed.join("extension.js").is_file());
        assert!(!dir.join("sample.hello-world-0.0.0").exists());
        uninstall(&installed, &dir).unwrap();
        assert!(!installed.exists());
        let _ = std::fs::remove_dir_all(&work);
    }
}
