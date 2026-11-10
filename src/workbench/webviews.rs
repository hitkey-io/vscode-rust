//! Extension webviews on screen: one native web view (WKWebView on macOS,
//! through `wry`) per webview, laid over the rectangle the workbench gives
//! it (an editor tab or a sidebar view).
//!
//! VS Code analogues:
//! - `src/vs/workbench/contrib/webview/browser/webviewElement.ts`
//! - `src/vs/workbench/contrib/webview/browser/pre/index.html` (the host
//!   page: `acquireVsCodeApi`, message passing, theme variables, default
//!   styles — `assets/ext-host/webview-theme.css`)
//!
//! Pages load from `vscode-resource://w<handle>/index.html`; any other path
//! on that origin is a local file (`webview.asWebviewUri`), served only from
//! the webview's resource roots.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::extensions::ExtensionService;

const SCHEME: &str = "vscode-resource";
const THEME_CSS: &str = include_str!("../../assets/ext-host/webview-theme.css");

/// What the resource handler serves for one webview.
#[derive(Clone, Default)]
struct Page {
    html: String,
    roots: Vec<PathBuf>,
}

struct Live {
    view: wry::WebView,
    version: u64,
    rect: egui::Rect,
    visible: bool,
    /// `theme::generation()` the page's colours were last sent for.
    theme: u32,
}

/// The native web views and where the workbench wants them this frame.
pub struct WebviewHost {
    live: HashMap<u64, Live>,
    pages: Arc<Mutex<HashMap<u64, Page>>>,
    ipc_tx: Sender<(u64, String)>,
    ipc_rx: Receiver<(u64, String)>,
    /// Filled while rendering: handle → screen rect (points).
    slots: Vec<(u64, egui::Rect)>,
    /// Hide every web view (a popup is over them: they'd cover it).
    pub suppressed: bool,
    /// The editor's own pages (extension details…): handle → (html,
    /// resource roots, version).
    internal: HashMap<u64, (String, Vec<PathBuf>, u64)>,
    /// Messages from the editor's own pages, for the app.
    internal_messages: Vec<(u64, serde_json::Value)>,
}

/// Handles at or above this are the editor's own pages; below are
/// extension webviews.
pub const INTERNAL_HANDLE_BASE: u64 = 1 << 40;

impl Default for WebviewHost {
    fn default() -> Self {
        let (ipc_tx, ipc_rx) = channel();
        Self {
            live: HashMap::new(),
            pages: Arc::new(Mutex::new(HashMap::new())),
            ipc_tx,
            ipc_rx,
            slots: Vec::new(),
            suppressed: false,
            internal: HashMap::new(),
            internal_messages: Vec::new(),
        }
    }
}

impl WebviewHost {
    /// Start a frame: no webview has a place yet.
    pub fn begin_frame(&mut self) {
        self.slots.clear();
        self.suppressed = false;
    }

    /// Show webview `handle` over `rect` this frame.
    pub fn place(&mut self, handle: u64, rect: egui::Rect) {
        self.slots.push((handle, rect));
    }

    /// Whether `handle` has a place this frame.
    /// Where `handle` is placed this frame (window points).
    pub fn placed_rect(&self, handle: u64) -> Option<egui::Rect> {
        self.slots.iter().find(|(h, _)| *h == handle).map(|(_, r)| *r)
    }

    pub fn is_placed(&self, handle: u64) -> bool {
        self.slots.iter().any(|(h, _)| *h == handle)
    }

    /// Show (or update) one of the editor's own pages under `handle`.
    pub fn set_internal_page(&mut self, handle: u64, html: String, roots: Vec<PathBuf>) {
        let entry = self.internal.entry(handle).or_insert_with(|| (String::new(), Vec::new(), 0));
        if entry.0 != html || entry.1 != roots {
            entry.0 = html;
            entry.1 = roots;
            entry.2 += 1;
        }
    }

    /// Forget one of the editor's own pages (its tab closed).
    pub fn remove_internal_page(&mut self, handle: u64) {
        self.internal.remove(&handle);
        self.live.remove(&handle);
    }

    /// Messages the editor's own pages posted since the last call.
    pub fn take_internal_messages(&mut self) -> Vec<(u64, serde_json::Value)> {
        std::mem::take(&mut self.internal_messages)
    }

    /// Messages the pages posted, forwarded to their extensions (or kept
    /// for the app, for its own pages).
    pub fn forward_page_messages(&mut self, service: &mut ExtensionService) {
        while let Ok((handle, raw)) = self.ipc_rx.try_recv() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else { continue };
            if v.get("kind").and_then(|k| k.as_str()) != Some("message") {
                continue;
            }
            let data = v.get("data").cloned().unwrap_or(serde_json::Value::Null);
            if handle >= INTERNAL_HANDLE_BASE {
                self.internal_messages.push((handle, data));
            } else {
                service.webview_message(handle, data);
            }
        }
    }

    /// Create, move, show, hide, reload and drop the native views to match
    /// the slots and the service, and deliver messages to the pages.
    /// `window` is the editor window (the views are its children).
    pub fn sync(&mut self, window: &impl wry::raw_window_handle::HasWindowHandle, service: &mut ExtensionService) {
        // Pages for the resource handler.
        {
            let mut pages = self.pages.lock().unwrap();
            pages.retain(|h, _| service.webviews.contains_key(h) || self.internal.contains_key(h));
            for (h, w) in &service.webviews {
                let page = pages.entry(*h).or_default();
                page.html.clone_from(&w.html);
                page.roots.clone_from(&w.roots);
            }
            for (h, (html, roots, _)) in &self.internal {
                let page = pages.entry(*h).or_default();
                page.html.clone_from(html);
                page.roots.clone_from(roots);
            }
        }
        // Gone webviews.
        let internal = &self.internal;
        self.live.retain(|h, _| service.webviews.contains_key(h) || internal.contains_key(h));

        let slots = self.slots.clone();
        for (handle, rect) in &slots {
            let version = match (service.webviews.get(handle), self.internal.get(handle)) {
                (Some(state), _) => state.html_version,
                (None, Some((_, _, v))) => *v,
                (None, None) => continue,
            };
            if !self.live.contains_key(handle) {
                match self.create(window, *handle, *rect) {
                    Ok(view) => {
                        self.live.insert(
                            *handle,
                            Live { view, version: u64::MAX, rect: *rect, visible: true, theme: crate::theme::generation() },
                        );
                    }
                    Err(e) => {
                        eprintln!("webview {handle}: {e}");
                        continue;
                    }
                }
            }
            let live = self.live.get_mut(handle).unwrap();
            if live.theme != crate::theme::generation() {
                live.theme = crate::theme::generation();
                let _ = live.view.evaluate_script(&format!(
                    "window.__vscodeApplyTheme && window.__vscodeApplyTheme({});",
                    theme_json()
                ));
            }
            if live.version != version {
                live.version = version;
                let _ = live.view.load_url(&format!("{SCHEME}://w{handle}/index.html?v={version}"));
            }
            if live.rect != *rect {
                live.rect = *rect;
                let _ = live.view.set_bounds(bounds(*rect));
            }
            let show = !self.suppressed;
            if live.visible != show {
                live.visible = show;
                let _ = live.view.set_visible(show);
                if !show {
                    // A hidden page keeps the keyboard otherwise: hand it back
                    // to the editor (the popup that covered it needs it).
                    let _ = live.view.focus_parent();
                }
            }
            if *handle >= INTERNAL_HANDLE_BASE {
                continue;
            }
            for msg in service.take_page_messages(*handle) {
                let _ = live
                    .view
                    .evaluate_script(&format!("window.__vscodeDeliver && window.__vscodeDeliver({msg});"));
            }
            service.set_webview_visible(*handle, true);
        }
        // Webviews without a place this frame are hidden.
        for (handle, live) in self.live.iter_mut() {
            if !slots.iter().any(|(h, _)| h == handle) {
                if live.visible {
                    live.visible = false;
                    let _ = live.view.set_visible(false);
                    let _ = live.view.focus_parent();
                }
                if *handle < INTERNAL_HANDLE_BASE {
                    service.set_webview_visible(*handle, false);
                }
            }
        }
    }

    fn create(
        &self,
        window: &impl wry::raw_window_handle::HasWindowHandle,
        handle: u64,
        rect: egui::Rect,
    ) -> wry::Result<wry::WebView> {
        let pages = self.pages.clone();
        let ipc = self.ipc_tx.clone();
        wry::WebViewBuilder::new()
            .with_bounds(bounds(rect))
            .with_background_color({
                let [r, g, b, _] = crate::theme::Palette::EDITOR_BG().to_array();
                (r, g, b, 0xFF)
            })
            .with_initialization_script(bootstrap_script())
            .with_custom_protocol(SCHEME.to_string(), move |_id, request| serve(&pages, request))
            .with_ipc_handler(move |request| {
                let _ = ipc.send((handle, request.body().clone()));
            })
            .with_navigation_handler(|url| {
                // Links out of the page open in the browser, as in VS Code.
                if url.starts_with("http://") || url.starts_with("https://") {
                    let _ = std::process::Command::new("open").arg(&url).spawn();
                    return false;
                }
                url.starts_with(SCHEME) || url == "about:blank"
            })
            .with_url(format!("{SCHEME}://w{handle}/index.html?v=0"))
            .build_as_child(window)
    }
}

fn bounds(rect: egui::Rect) -> wry::Rect {
    wry::Rect {
        position: wry::dpi::LogicalPosition::new(rect.left() as f64, rect.top() as f64).into(),
        size: wry::dpi::LogicalSize::new(rect.width().max(1.0) as f64, rect.height().max(1.0) as f64).into(),
    }
}

/// Runs in every page before its own scripts: the `acquireVsCodeApi`
/// bridge, message delivery and the theme (variables, default styles,
/// `vscode-dark` body class).
fn bootstrap_script() -> String {
    format!(
        r#"(function () {{
  let state = undefined;
  let acquired = false;
  const post = (kind, data) => window.ipc.postMessage(JSON.stringify({{ kind, data }}));
  window.acquireVsCodeApi = function () {{
    if (acquired) {{ throw new Error('An instance of the VS Code API has already been acquired'); }}
    acquired = true;
    return Object.freeze({{
      postMessage: (message) => post('message', message),
      getState: () => state,
      setState: (s) => {{ state = s; post('state', s); return s; }},
    }});
  }};
  window.__vscodeDeliver = function (data) {{
    window.dispatchEvent(new MessageEvent('message', {{ data }}));
  }};
  const theme = document.createElement('style');
  theme.id = '_defaultStyles';
  let current = {theme};
  const apply = () => {{
    theme.textContent = current.css;
    (document.head || document.documentElement).prepend(theme);
    const body = document.body;
    if (body) {{
      body.classList.remove('vscode-light', 'vscode-dark', 'vscode-high-contrast', 'vscode-high-contrast-light');
      body.classList.add(current.kind);
      if (current.kind === 'vscode-high-contrast-light') {{ body.classList.add('vscode-high-contrast'); }}
      body.dataset.vscodeThemeKind = current.kind;
      body.dataset.vscodeThemeName = current.name;
      body.dataset.vscodeThemeId = current.id;
    }}
    document.documentElement.style.setProperty('color-scheme', current.kind.includes('light') ? 'light' : 'dark');
  }};
  window.__vscodeApplyTheme = (t) => {{ current = t; apply(); }};
  if (document.readyState === 'loading') {{
    document.addEventListener('DOMContentLoaded', apply);
  }} else {{
    apply();
  }}
}})();"#,
        theme = theme_json()
    )
}

/// The active theme for a page: CSS (colour variables + default styles),
/// body class, name and id.
pub fn theme_json() -> String {
    let t = crate::theme::current();
    serde_json::json!({
        "css": format!("{}{}", crate::theme::webview_css(), THEME_CSS),
        "kind": crate::theme::webview_theme_class(),
        "name": t.label,
        "id": t.id,
    })
    .to_string()
}

/// Answer `vscode-resource://w<handle>/…`: the page itself, or a file
/// inside the webview's resource roots.
/// Where every page finds the codicon font (`vscode-resource://w<n>/…`).
pub const CODICON_PATH: &str = "/__codicon.ttf";
const CODICON_TTF: &[u8] = include_bytes!("../../assets/codicon.ttf");

fn serve(
    pages: &Arc<Mutex<HashMap<u64, Page>>>,
    request: wry::http::Request<Vec<u8>>,
) -> wry::http::Response<Cow<'static, [u8]>> {
    let uri = request.uri();
    let handle: Option<u64> = uri.host().and_then(|h| h.strip_prefix('w')).and_then(|h| h.parse().ok());
    let page = handle.and_then(|h| pages.lock().unwrap().get(&h).cloned());
    let Some(page) = page else { return respond(404, "text/plain", b"no such webview".to_vec()) };
    let path = percent_decode(uri.path());
    if path == "/index.html" {
        return respond(200, "text/html; charset=utf-8", page.html.into_bytes());
    }
    if path == CODICON_PATH {
        // The editor's own pages draw codicons, as the workbench does.
        return respond(200, "font/ttf", CODICON_TTF.to_vec());
    }
    let file = PathBuf::from(&path);
    let allowed = page.roots.iter().any(|root| file.starts_with(root)) && !path.contains("/../");
    if !allowed {
        return respond(403, "text/plain", b"outside the webview's resource roots".to_vec());
    }
    match std::fs::read(&file) {
        Ok(bytes) => respond(200, mime(&file), bytes),
        Err(_) => respond(404, "text/plain", b"not found".to_vec()),
    }
}

fn respond(status: u16, mime: &str, body: Vec<u8>) -> wry::http::Response<Cow<'static, [u8]>> {
    wry::http::Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Access-Control-Allow-Origin", "*")
        .body(Cow::Owned(body))
        .unwrap()
}

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" | "cjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_the_page_and_only_files_in_its_roots() {
        let pages = Arc::new(Mutex::new(HashMap::new()));
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/extensions/sample.hello-world-0.0.1");
        pages.lock().unwrap().insert(7, Page { html: "<p>hi</p>".into(), roots: vec![root.clone()] });
        let get = |url: &str| serve(&pages, wry::http::Request::builder().uri(url).body(Vec::new()).unwrap());
        let r = get("vscode-resource://w7/index.html?v=1");
        assert_eq!(r.status(), 200);
        assert_eq!(r.body().as_ref(), b"<p>hi</p>");
        let file = format!("vscode-resource://w7{}", root.join("package.json").display());
        assert_eq!(get(&file).status(), 200);
        assert_eq!(get("vscode-resource://w7/etc/hosts").status(), 403);
        assert_eq!(get("vscode-resource://w8/index.html").status(), 404);
        assert_eq!(percent_decode("/a%20b"), "/a b");
    }
}
