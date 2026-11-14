//! The extension editor: the page that opens when an extension is clicked in
//! the Extensions view. Header (icon, name, publisher, install count,
//! rating, description, actions, status), the DETAILS (README), FEATURES
//! (contributions by kind), CHANGELOG, DEPENDENCIES and EXTENSION PACK tabs,
//! and the Installation / Marketplace / Categories / Resources column.
//! Rendered as an HTML page in a native web view; its buttons post messages
//! the app turns into actions and native menus.
//!
//! VS Code analogues:
//! - `src/vs/workbench/contrib/extensions/browser/extensionEditor.ts`
//!   (header widgets, navbar tabs, `AdditionalDetailsWidget`)
//! - `src/vs/workbench/contrib/extensions/browser/extensionFeaturesTab.ts`
//!   (feature list + view) and every `registerExtensionFeature` renderer
//!   (commands, settings, activation events, languages, views…)
//! - `src/vs/workbench/contrib/extensions/browser/media/extensionEditor.css`,
//!   `extensionActions.css`, `keybindingLabel.css`
//! - `src/vs/workbench/contrib/markdown/browser/markdownDocumentRenderer.ts`
//!   (`DEFAULT_MARKDOWN_STYLES`, in `assets/ext-host/markdown.css`)

use serde_json::Value;

/// Which extension's page to open.
#[derive(Clone, Debug)]
pub enum OpenTarget {
    /// Installed: its id.
    Installed(String),
    /// From the catalogue.
    Gallery(crate::extensions::marketplace::GalleryExtension),
}

const MARKDOWN_CSS: &str = include_str!("../../assets/ext-host/markdown.css");

/// Where the extension stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InstallState {
    #[default]
    NotInstalled,
    Installing,
    Installed,
}

/// Enabled, or disabled where.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Enabled {
    #[default]
    Yes,
    DisabledGlobally,
    DisabledWorkspace,
    /// The workspace isn't trusted and the extension doesn't support that.
    DisabledByTrust,
}

/// Another extension shown on the page (a dependency, a pack member).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtRef {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub description: String,
    pub icon: Option<String>,
}

/// "Installation" details of an installed extension.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstallInfo {
    pub version: String,
    /// When it was installed (Unix ms).
    pub last_updated: Option<i64>,
    /// Size of its folder, and where it is.
    pub size: Option<(u64, String)>,
    /// Size of its global storage, and where it is.
    pub cache: Option<(u64, String)>,
}

/// "Marketplace" details from the catalogue.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarketInfo {
    pub version: String,
    /// First version's date (Unix ms).
    pub published: Option<i64>,
    /// Newest version's date (Unix ms).
    pub last_released: Option<i64>,
}

/// "Runtime Status" of a running extension.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Runtime {
    pub activated: bool,
    /// What activated it and how long its `activate()` took (ms).
    pub activation: Option<(String, u64)>,
    pub error: Option<String>,
}

/// Everything the page shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DetailsData {
    /// The page's web view handle (its resources' origin).
    pub handle: u64,
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub verified: bool,
    pub description: String,
    /// Image URL (`https://…` or `vscode-resource://…`).
    pub icon: Option<String>,
    pub preview: bool,
    pub pre_release: bool,
    pub deprecated: bool,
    pub install_count: Option<u64>,
    pub rating: Option<f32>,
    pub rating_count: Option<u64>,
    pub sponsor_url: Option<String>,
    /// The extension's page in the catalogue.
    pub url: Option<String>,
    pub publisher_url: Option<String>,
    pub state: InstallState,
    pub enabled: Enabled,
    pub can_uninstall: bool,
    pub auto_update: bool,
    /// A newer version in the catalogue.
    pub update_to: Option<String>,
    pub has_workspace: bool,
    /// Lines under the actions: (`error` / `warning` / `info`, text).
    pub status: Vec<(&'static str, String)>,
    pub runtime: Option<Runtime>,
    pub readme: Option<String>,
    pub changelog: Option<String>,
    /// Where relative links and images in the README point.
    pub base_url: Option<String>,
    /// Still fetching from the marketplace.
    pub loading: bool,
    /// `package.json`, for the FEATURES tab.
    pub manifest: Option<Value>,
    pub dependencies: Vec<ExtRef>,
    pub pack: Vec<ExtRef>,
    pub install_info: Option<InstallInfo>,
    pub market_info: Option<MarketInfo>,
    pub categories: Vec<String>,
    pub repository: Option<String>,
    pub issues: Option<String>,
    pub license_url: Option<String>,
    /// Open tab (`details`, `features`, `changelog`, `dependencies`,
    /// `extensionPack`) and feature (its id).
    pub tab: String,
    pub feature: Option<String>,
}

impl DetailsData {
    /// Categories, repository and issues from an extension's `package.json`
    /// (the catalogue's values win where both exist).
    pub fn read_manifest(&mut self, manifest: &Value) {
        if self.categories.is_empty() {
            if let Some(cats) = manifest.get("categories").and_then(|c| c.as_array()) {
                self.categories = cats.iter().filter_map(|c| c.as_str()).map(str::to_string).collect();
            }
        }
        if self.repository.is_none() {
            self.repository = manifest
                .get("repository")
                .and_then(|r| r.as_str().map(str::to_string).or_else(|| r.get("url").and_then(|u| u.as_str()).map(str::to_string)))
                .map(|u| clean_repo_url(&u));
        }
        if self.issues.is_none() {
            self.issues = manifest
                .get("bugs")
                .and_then(|b| b.as_str().map(str::to_string).or_else(|| b.get("url").and_then(|u| u.as_str()).map(str::to_string)));
        }
        self.preview |= manifest.get("preview").and_then(Value::as_bool).unwrap_or(false);
        self.manifest = Some(manifest.clone());
    }
}

/// `git+https://github.com/x/y.git` → `https://github.com/x/y`.
pub fn clean_repo_url(url: &str) -> String {
    url.trim_start_matches("git+").trim_end_matches(".git").to_string()
}

/// ISO 8601 (`2026-05-01T04:48:48.461Z`) to Unix milliseconds.
pub fn iso_to_ms(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (h, mi, se) = (n(11..13)?, n(14..16)?, n(17..19)?);
    // Days from the civil date (Howard Hinnant's algorithm).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (mo + if mo > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(((days * 86400) + h * 3600 + mi * 60 + se) * 1000)
}

/// A `%key%` placeholder whose translation isn't at hand (catalogue
/// manifests come without `package.nls.json`) shown as words from its key:
/// `%python.command.python.createNewFile.title%` → "Create New File".
pub fn readable(s: &str) -> String {
    let Some(key) = s.strip_prefix('%').and_then(|k| k.strip_suffix('%')) else { return s.to_string() };
    let parts: Vec<&str> = key.split('.').filter(|p| !matches!(*p, "title" | "description" | "markdownDescription" | "label")).collect();
    let last = parts.last().copied().unwrap_or(key);
    let mut out = String::new();
    for (i, ch) in last.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            out.push(' ');
        }
        if i == 0 {
            out.extend(ch.to_uppercase());
        } else {
            out.push(ch);
        }
    }
    out.replace(['-', '_'], " ")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A string field, or a `{ value }` / `{ original }` localized one.
fn text_of(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => readable(s),
        Some(Value::Object(o)) => o.get("value").and_then(|x| x.as_str()).map(readable).unwrap_or_default(),
        _ => String::new(),
    }
}

/// Markdown to HTML (GitHub flavour: tables, strikethrough, task lists),
/// without scripts.
pub fn markdown_html(md: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    let mut out = String::new();
    html::push_html(&mut out, Parser::new_ext(md, opts));
    strip_scripts(&out)
}

/// Drop `<script>` elements and inline event handlers from README HTML.
fn strip_scripts(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        let lower = rest.to_lowercase();
        match lower.find("<script") {
            Some(i) => {
                out.push_str(&rest[..i]);
                let end = lower[i..].find("</script>").map(|e| i + e + 9).unwrap_or(rest.len());
                rest = &rest[end..];
            }
            None => {
                out.push_str(rest);
                break;
            }
        }
    }
    // `onclick=` and friends.
    let mut cleaned = String::with_capacity(out.len());
    let bytes = out.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b' ' && bytes.get(i + 1..i + 3).is_some_and(|b| b.eq_ignore_ascii_case(b"on")) {
            let name_end = out[i + 1..].find(|c: char| !c.is_ascii_alphabetic()).map(|e| i + 1 + e).unwrap_or(out.len());
            if out[name_end..].starts_with('=') && name_end > i + 3 {
                // Skip the attribute value.
                let after = &out[name_end + 1..];
                let skip = match after.chars().next() {
                    Some(q @ ('"' | '\'')) => after[1..].find(q).map(|e| e + 2).unwrap_or(after.len()),
                    _ => after.find([' ', '>']).unwrap_or(after.len()),
                };
                i = name_end + 1 + skip;
                continue;
            }
        }
        let ch = out[i..].chars().next().unwrap();
        cleaned.push(ch);
        i += ch.len_utf8();
    }
    cleaned
}

/// Limit a stylesheet to elements under `scope` (`body` becomes the scope
/// itself), so the README's styles don't reach the rest of the page — in
/// VS Code the README is a document of its own.
fn scope_css(css: &str, scope: &str) -> String {
    let mut out = String::new();
    let mut rest = css;
    while let Some(open) = rest.find('{') {
        let head = rest[..open].trim();
        let head = strip_comments(head);
        if head.starts_with('@') {
            // A block of rules: scope its contents.
            let mut depth = 0;
            let mut end = rest.len();
            for (i, c) in rest[open..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = open + i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            out.push_str(&format!("{head} {{\n{}\n}}\n", scope_css(&rest[open + 1..end], scope)));
            rest = &rest[(end + 1).min(rest.len())..];
            continue;
        }
        let close = rest[open..].find('}').map(|c| open + c).unwrap_or(rest.len());
        let selectors: Vec<String> = head
            .split(',')
            .map(|sel| {
                let sel = sel.trim();
                if let Some(after) = sel.strip_prefix("body") {
                    format!("{scope}{after}")
                } else if sel.starts_with(".vscode-") {
                    // Theme class on <body>: keep it in front.
                    match sel.split_once(' ') {
                        Some((theme, inner)) => format!("{theme} {scope} {inner}"),
                        None => format!("{sel} {scope}"),
                    }
                } else {
                    format!("{scope} {sel}")
                }
            })
            .collect();
        out.push_str(&format!("{} {}\n", selectors.join(", "), &rest[open..=close.min(rest.len() - 1)]));
        rest = &rest[(close + 1).min(rest.len())..];
    }
    out
}

fn strip_comments(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        rest = rest[i..].find("*/").map(|e| &rest[i + e + 2..]).unwrap_or("");
    }
    out.push_str(rest);
    out.trim().to_string()
}

/// `150138697` → `150,138,697` (`toLocaleString` in en-US).
pub fn group_digits(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `ByteSize.formatSize`: `40.29MB`, `512B`.
pub fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes}B")
    } else if b < KB * KB {
        format!("{:.2}KB", b / KB)
    } else if b < KB * KB * KB {
        format!("{:.2}MB", b / KB / KB)
    } else {
        format!("{:.2}GB", b / KB / KB / KB)
    }
}

/// A codicon by name, as the workbench draws them.
fn codicon(name: &str) -> String {
    let cp = crate::icons::codicon_by_name(name).map(|c| format!("&#x{:x};", c as u32)).unwrap_or_default();
    format!(r#"<span class="codicon codicon-{name}">{cp}</span>"#)
}

// ---------------------------------------------------------------------------
// FEATURES

/// One entry in the FEATURES list.
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    pub id: &'static str,
    pub label: &'static str,
    pub html: String,
}

fn table(headers: &[&str], rows: Vec<Vec<String>>) -> String {
    let mut out = String::from("<table><tr>");
    for h in headers {
        out.push_str(&format!("<th>{}</th>", esc(h)));
    }
    out.push_str("</tr>");
    for row in rows {
        out.push_str("<tr>");
        for cell in row {
            out.push_str(&format!("<td>{cell}</td>"));
        }
        out.push_str("</tr>");
    }
    out.push_str("</table>");
    out
}

/// A plain-text table cell.
fn cell(s: &str) -> String {
    format!("<p>{}</p>", esc(s))
}

/// A markdown table cell.
fn md_cell(s: &str) -> String {
    format!(r#"<div class="markdown">{}</div>"#, markdown_html(s))
}

fn code_cell(s: &str) -> String {
    format!("<div><p><code>{}</code></p></div>", esc(s))
}

fn list_md(items: &[String]) -> String {
    let mut out = String::from(r#"<div class="markdown"><ul>"#);
    for i in items {
        out.push_str(&format!("<li><code>{}</code></li>", esc(i)));
    }
    out.push_str("</ul></div>");
    out
}

fn arr<'a>(v: Option<&'a Value>) -> Vec<&'a Value> {
    match v {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(o @ Value::Object(_)) => vec![o],
        _ => Vec::new(),
    }
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// A keybinding as `keybindingLabel` draws it on macOS: one box per key.
pub fn keybinding_html(raw: &str) -> String {
    let mut out = String::from(r#"<div class="monaco-keybinding">"#);
    for (i, chord) in raw.split_whitespace().enumerate() {
        if i > 0 {
            out.push_str(r#"<span class="monaco-keybinding-key-chord-separator"> </span>"#);
        }
        let parts: Vec<String> = chord.split('+').map(|p| p.trim().to_lowercase()).collect();
        let has = |m: &[&str]| parts.iter().any(|p| m.contains(&p.as_str()));
        let mut keys = Vec::new();
        if has(&["ctrl"]) {
            keys.push("⌃".to_string());
        }
        if has(&["shift"]) {
            keys.push("⇧".to_string());
        }
        if has(&["alt", "option"]) {
            keys.push("⌥".to_string());
        }
        if has(&["cmd", "meta", "win"]) {
            keys.push("⌘".to_string());
        }
        if let Some(key) = parts.iter().rev().find(|p| !["ctrl", "shift", "alt", "option", "cmd", "meta", "win"].contains(&p.as_str())) {
            keys.push(key_label(key));
        }
        for k in keys {
            out.push_str(&format!(r#"<span class="monaco-keybinding-key">{}</span>"#, esc(&k)));
        }
    }
    out.push_str("</div>");
    out
}

fn key_label(key: &str) -> String {
    match key {
        "left" => "←".into(),
        "right" => "→".into(),
        "up" => "↑".into(),
        "down" => "↓".into(),
        "enter" => "Enter".into(),
        "escape" => "Escape".into(),
        "tab" => "Tab".into(),
        "space" => "Space".into(),
        "backspace" => "Backspace".into(),
        "delete" => "Delete".into(),
        "pageup" => "PageUp".into(),
        "pagedown" => "PageDown".into(),
        "home" => "Home".into(),
        "end" => "End".into(),
        k if k.chars().count() == 1 => k.to_uppercase(),
        k => {
            let mut c = k.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
        }
    }
}

/// Every feature the manifest contributes, as VS Code's FEATURES tab lists
/// them: sorted by label, with Runtime Status first for a running
/// extension.
pub fn features(manifest: &Value, runtime: Option<&Runtime>) -> Vec<Feature> {
    let c = manifest.get("contributes").cloned().unwrap_or(Value::Null);
    let has = |k: &str| c.get(k).is_some_and(|v| !v.is_null());
    let mut out: Vec<Feature> = Vec::new();

    if let Some(events) = manifest.get("activationEvents").and_then(Value::as_array) {
        let items: Vec<String> = events.iter().filter_map(Value::as_str).map(str::to_string).collect();
        out.push(Feature { id: "activationEvents", label: "Activation Events", html: list_md(&items) });
    }
    let proposals: Vec<String> = manifest
        .get("enabledApiProposals")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    if !proposals.is_empty() {
        out.push(Feature { id: "enabledApiProposals", label: "API Proposals", html: list_md(&proposals) });
    }
    if has("authentication") {
        let mut items = arr(c.get("authentication"));
        items.sort_by(|a, b| text_of(a.get("label")).cmp(&text_of(b.get("label"))));
        let rows = items
            .iter()
            .map(|a| {
                let globs: Vec<&str> = a.get("authorizationServerGlobs").and_then(Value::as_array).map(|g| g.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                vec![cell(&text_of(a.get("label"))), cell(s(a, "id")), cell(&globs.join(",\n"))]
            })
            .collect();
        out.push(Feature { id: "authentication", label: "Authentication", html: table(&["Label", "ID", "MCP Authorization Servers"], rows) });
    }
    if has("chatParticipants") {
        let rows: Vec<Vec<String>> = arr(c.get("chatParticipants"))
            .into_iter()
            .filter(|p| !p.get("isDefault").and_then(Value::as_bool).unwrap_or(false))
            .map(|p| {
                let cmds: Vec<String> = arr(p.get("commands")).iter().map(|c| format!("- /{}", s(c, "name"))).collect();
                vec![
                    cell(&format!("@{}", s(p, "name"))),
                    cell(&text_of(p.get("fullName"))),
                    cell(&{
                        let d = text_of(p.get("description"));
                        if d.is_empty() { "-".into() } else { d }
                    }),
                    if cmds.is_empty() { cell("-") } else { md_cell(&cmds.join("\n")) },
                ]
            })
            .collect();
        if !rows.is_empty() {
            out.push(Feature { id: "chatParticipants", label: "Chat Participants", html: table(&["Name", "Full Name", "Description", "Commands"], rows) });
        }
    }
    if has("colors") {
        let mut items = arr(c.get("colors"));
        items.sort_by(|a, b| s(a, "id").cmp(s(b, "id")));
        let color = |v: &str| -> String {
            if v.starts_with('#') {
                format!(r#"<span class="colorBox" style="background-color: {}"></span><code>{}</code>"#, esc(v), esc(v))
            } else {
                format!("<code>{}</code>", esc(v))
            }
        };
        let rows = items
            .iter()
            .map(|col| {
                let d = col.get("defaults").cloned().unwrap_or(Value::Null);
                vec![
                    code_cell(s(col, "id")),
                    cell(&text_of(col.get("description"))),
                    color(s(&d, "dark")),
                    color(s(&d, "light")),
                    color(s(&d, "highContrast")),
                ]
            })
            .collect();
        out.push(Feature {
            id: "colors",
            label: "Colors",
            html: table(&["ID", "Description", "Dark Default", "Light Default", "High Contrast Default"], rows),
        });
    }
    if has("commands") || has("keybindings") || has("menus") {
        if let Some(html) = commands_table(&c) {
            out.push(Feature { id: "commands", label: "Commands", html });
        }
    }
    if has("customEditors") {
        let rows = arr(c.get("customEditors"))
            .iter()
            .map(|e| {
                let pats: Vec<&str> = arr(e.get("selector")).iter().filter_map(|x| x.get("filenamePattern").and_then(Value::as_str)).collect();
                vec![cell(s(e, "viewType")), cell(s(e, "priority")), cell(&pats.join(", "))]
            })
            .collect();
        out.push(Feature { id: "customEditors", label: "Custom Editors", html: table(&["View Type", "Priority", "Filename Pattern"], rows) });
    }
    if has("debuggers") {
        let rows = arr(c.get("debuggers")).iter().map(|d| vec![cell(&text_of(d.get("label"))), cell(s(d, "type"))]).collect();
        out.push(Feature { id: "debuggers", label: "Debuggers", html: table(&["Name", "Type"], rows) });
    }
    if has("jsonValidation") {
        let rows = arr(c.get("jsonValidation"))
            .iter()
            .map(|v| {
                let m = match v.get("fileMatch") {
                    Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "),
                    Some(Value::String(s)) => s.clone(),
                    _ => String::new(),
                };
                vec![code_cell(&m), cell(s(v, "url"))]
            })
            .collect();
        out.push(Feature { id: "jsonValidation", label: "JSON Validation", html: table(&["File Match", "Schema"], rows) });
    }
    if has("languageModelTools") {
        let rows = arr(c.get("languageModelTools"))
            .iter()
            .map(|t| {
                let d = text_of(t.get("userDescription"));
                let d = if d.is_empty() { text_of(t.get("modelDescription")) } else { d };
                vec![code_cell(s(t, "name")), cell(&text_of(t.get("displayName"))), cell(&d)]
            })
            .collect();
        out.push(Feature { id: "languageModelTools", label: "Language Model Tools", html: table(&["Name", "Display Name", "Description"], rows) });
    }
    if has("languageModelToolSets") {
        let rows = arr(c.get("languageModelToolSets"))
            .iter()
            .map(|t| {
                let tools: Vec<&str> = t.get("tools").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                let reference = s(t, "referenceName");
                vec![
                    code_cell(s(t, "name")),
                    if reference.is_empty() { cell("none") } else { code_cell(&format!("#{reference}")) },
                    cell(&tools.join(", ")),
                    cell(&text_of(t.get("description"))),
                ]
            })
            .collect();
        out.push(Feature {
            id: "languageModelToolSets",
            label: "Language Model Tool Sets",
            html: table(&["Name", "Reference Name", "Tools", "Description"], rows),
        });
    }
    if has("localizations") {
        let mut items = arr(c.get("localizations"));
        items.sort_by(|a, b| s(a, "languageId").cmp(s(b, "languageId")));
        let rows = items
            .iter()
            .map(|l| vec![cell(s(l, "languageId")), cell(s(l, "languageName")), cell(s(l, "localizedLanguageName"))])
            .collect();
        out.push(Feature {
            id: "localizations",
            label: "Language Packs",
            html: table(&["Language ID", "Language Name", "Language Name (Localized)"], rows),
        });
    }
    let mcp = arr(c.get("mcpServerDefinitionProviders"));
    if !mcp.is_empty() {
        let rows = mcp.iter().map(|m| vec![code_cell(s(m, "id")), cell(&text_of(m.get("label")))]).collect();
        out.push(Feature { id: "mcp", label: "MCP Servers", html: table(&["ID", "Name"], rows) });
    }
    if has("notebookRenderer") {
        let mut items = arr(c.get("notebookRenderer"));
        items.sort_by(|a, b| text_of(a.get("displayName")).cmp(&text_of(b.get("displayName"))));
        let rows = items
            .iter()
            .map(|r| {
                let mimes: Vec<&str> = r.get("mimeTypes").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
                vec![cell(&text_of(r.get("displayName"))), cell(&mimes.join(","))]
            })
            .collect();
        out.push(Feature { id: "notebookRenderer", label: "Notebook Renderers", html: table(&["Name", "Mimetypes"], rows) });
    }
    if has("notebooks") {
        let mut items = arr(c.get("notebooks"));
        items.sort_by(|a, b| s(a, "type").cmp(s(b, "type")));
        let rows = items.iter().map(|n| vec![cell(s(n, "type")), cell(&text_of(n.get("displayName")))]).collect();
        out.push(Feature { id: "notebooks", label: "Notebooks", html: table(&["ID", "Name"], rows) });
    }
    if has("languages") || has("grammars") || has("snippets") {
        if let Some(html) = languages_table(&c) {
            out.push(Feature { id: "languages", label: "Programming Languages", html });
        }
    }
    if has("configuration") {
        out.push(Feature { id: "configuration", label: "Settings", html: settings_table(&c) });
    }
    if has("configurationDefaults") {
        let mut rows = Vec::new();
        if let Some(obj) = c.get("configurationDefaults").and_then(Value::as_object) {
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort();
            for key in keys {
                let value = &obj[key];
                let json = |v: &Value| format!("<pre><code>{}</code></pre>", esc(&serde_json::to_string_pretty(v).unwrap_or_default()));
                if key.starts_with('[') && key.ends_with(']') {
                    let langs: Vec<&str> = key.trim_matches(['[', ']']).split("][").collect();
                    if let Some(inner) = value.as_object() {
                        let mut ik: Vec<&String> = inner.keys().collect();
                        ik.sort();
                        for k in ik {
                            rows.push(vec![cell(&langs.join(", ")), code_cell(k), json(&inner[k])]);
                        }
                    }
                } else {
                    rows.push(vec![cell(""), code_cell(key), json(value)]);
                }
            }
        }
        out.push(Feature {
            id: "configurationDefaults",
            label: "Settings Default Overrides",
            html: table(&["Languages", "Setting", "Override Value"], rows),
        });
    }
    if has("themes") || has("iconThemes") || has("productIconThemes") {
        let mut md = String::new();
        for (key, title) in [("themes", "Color Themes"), ("iconThemes", "File Icon Themes"), ("productIconThemes", "Product Icon Themes")] {
            if has(key) {
                md.push_str(&format!("### {title}\n\n"));
                for t in arr(c.get(key)) {
                    let label = text_of(t.get("label"));
                    let label = if label.is_empty() { s(t, "id").to_string() } else { label };
                    md.push_str(&format!("- {label}\n"));
                }
                md.push('\n');
            }
        }
        out.push(Feature { id: "themes", label: "Themes", html: format!(r#"<div class="markdown">{}</div>"#, markdown_html(&md)) });
    }
    if has("viewsContainers") {
        let mut rows: Vec<(String, String, String)> = Vec::new();
        if let Some(obj) = c.get("viewsContainers").and_then(Value::as_object) {
            for (loc, list) in obj {
                for vc in arr(Some(list)) {
                    rows.push((s(vc, "id").to_string(), text_of(vc.get("title")), loc.clone()));
                }
            }
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        let rows = rows.into_iter().map(|(a, b, c)| vec![cell(&a), cell(&b), cell(&c)]).collect();
        out.push(Feature { id: "viewsContainers", label: "View Containers", html: table(&["ID", "Title", "Where"], rows) });
    }
    if has("views") {
        let mut rows: Vec<(String, String, String)> = Vec::new();
        if let Some(obj) = c.get("views").and_then(Value::as_object) {
            for (loc, list) in obj {
                for v in arr(Some(list)) {
                    rows.push((s(v, "id").to_string(), text_of(v.get("name")), loc.clone()));
                }
            }
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        let rows = rows.into_iter().map(|(a, b, c)| vec![cell(&a), cell(&b), cell(&c)]).collect();
        out.push(Feature { id: "views", label: "Views", html: table(&["ID", "Name", "Where"], rows) });
    }

    out.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    if let Some(rt) = runtime {
        let mut md = String::from("### Activation\n\n");
        if rt.activated {
            // RuntimeStatusMarkdownRenderer's wording.
            match &rt.activation {
                Some((reason, ms)) if reason == "*" => md.push_str(&format!("Activated on Startup: `{ms}ms`\n\n")),
                Some((reason, ms)) => md.push_str(&format!("Activated by `{reason}` event: `{ms}ms`\n\n")),
                None => md.push_str("Activated\n\n"),
            }
        } else {
            md.push_str("Not yet activated\n\n");
        }
        let mut html = markdown_html(&md);
        if let Some(e) = &rt.error {
            html.push_str(&markdown_html("\n ### Uncaught Errors (1)\n"));
            html.push_str(&format!(r#"<div class="message-entry">{}<span>{}</span></div>"#, codicon("error"), esc(e)));
        }
        out.insert(0, Feature { id: "runtimeStatus", label: "Runtime Status", html: format!(r#"<div class="runtime-status markdown">{html}</div>"#) });
    }
    out
}

/// `CommandsTableRenderer`: contributed commands plus the ones only named
/// in menus or keybindings, with their shortcuts and menus.
fn commands_table(c: &Value) -> Option<String> {
    struct Cmd {
        id: String,
        title: String,
        keys: Vec<String>,
        menus: Vec<String>,
    }
    let mut cmds: Vec<Cmd> = arr(c.get("commands"))
        .iter()
        .map(|x| Cmd { id: s(x, "command").to_string(), title: text_of(x.get("title")), keys: Vec::new(), menus: Vec::new() })
        .collect();
    let menus = c.get("menus").and_then(Value::as_object).cloned().unwrap_or_default();
    // Commands not placed in the palette explicitly are there implicitly.
    let in_palette: Vec<String> = menus
        .get("commandPalette")
        .map(|m| arr(Some(m)).iter().map(|x| s(x, "command").to_string()).collect())
        .unwrap_or_default();
    let mut all_menus: Vec<(String, Vec<(String, Option<String>)>)> = menus
        .iter()
        .map(|(k, v)| (k.clone(), arr(Some(v)).iter().map(|x| (s(x, "command").to_string(), x.get("when").and_then(Value::as_str).map(str::to_string))).collect()))
        .collect();
    let implicit: Vec<(String, Option<String>)> =
        cmds.iter().filter(|c| !in_palette.contains(&c.id)).map(|c| (c.id.clone(), None)).collect();
    if !implicit.is_empty() {
        match all_menus.iter_mut().find(|(k, _)| k == "commandPalette") {
            Some((_, items)) => items.extend(implicit),
            None => all_menus.push(("commandPalette".into(), implicit)),
        }
    }
    for (context, items) in &all_menus {
        for (command, when) in items {
            if when.as_deref() == Some("false") || command.is_empty() {
                continue;
            }
            match cmds.iter_mut().find(|c| &c.id == command) {
                Some(cmd) => {
                    if !cmd.menus.contains(context) {
                        cmd.menus.push(context.clone());
                    }
                }
                None => cmds.push(Cmd { id: command.clone(), title: String::new(), keys: Vec::new(), menus: vec![context.clone()] }),
            }
        }
    }
    for kb in arr(c.get("keybindings")) {
        let key = kb.get("mac").and_then(Value::as_str).or_else(|| kb.get("key").and_then(Value::as_str));
        let Some(key) = key else { continue };
        let id = s(kb, "command").to_string();
        match cmds.iter_mut().find(|c| c.id == id) {
            Some(cmd) => cmd.keys.push(key.to_string()),
            None => cmds.push(Cmd { id, title: String::new(), keys: vec![key.to_string()], menus: Vec::new() }),
        }
    }
    if cmds.is_empty() {
        return None;
    }
    cmds.sort_by(|a, b| a.id.cmp(&b.id));
    let rows = cmds
        .iter()
        .map(|cmd| {
            let mut menus = cmd.menus.clone();
            menus.sort();
            vec![
                code_cell(&cmd.id),
                cell(&cmd.title),
                cmd.keys.iter().map(|k| keybinding_html(k)).collect::<String>(),
                format!(
                    "<div><p>{}</p></div>",
                    menus.iter().map(|m| format!("<code>{}</code>", esc(m))).collect::<Vec<_>>().join("&nbsp;")
                ),
            ]
        })
        .collect();
    Some(table(&["ID", "Title", "Keyboard Shortcuts", "Menu Contexts"], rows))
}

/// `SettingsTableRenderer`.
fn settings_table(c: &Value) -> String {
    let mut props: Vec<(String, Value)> = Vec::new();
    for section in arr(c.get("configuration")) {
        if let Some(p) = section.get("properties").and_then(Value::as_object) {
            props.extend(p.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
    }
    props.sort_by(|a, b| a.0.cmp(&b.0));
    let rows = props
        .iter()
        .map(|(key, schema)| {
            let desc = match schema.get("markdownDescription").and_then(Value::as_str) {
                Some(md) => md_cell(&readable(md)),
                None => cell(&text_of(schema.get("description"))),
            };
            let default = schema.get("default").cloned().unwrap_or_else(|| crate::extensions::manifest::default_for_type(schema.get("type")));
            let json = serde_json::to_string_pretty(&default).unwrap_or_default();
            vec![
                code_cell(key),
                desc,
                format!(r#"<div class="markdown"><pre><code class="language-json">{}</code></pre></div>"#, esc(&json)),
            ]
        })
        .collect();
    table(&["ID", "Description", "Default"], rows)
}

/// `LanguageTableRenderer`.
fn languages_table(c: &Value) -> Option<String> {
    struct Lang {
        id: String,
        name: String,
        exts: Vec<String>,
        grammar: bool,
        snippets: bool,
    }
    let mut langs: Vec<Lang> = arr(c.get("languages"))
        .iter()
        .filter(|l| l.get("id").and_then(Value::as_str).is_some())
        .map(|l| Lang {
            id: s(l, "id").to_string(),
            name: l
                .get("aliases")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .unwrap_or(s(l, "id"))
                .to_string(),
            exts: l.get("extensions").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default(),
            grammar: false,
            snippets: false,
        })
        .collect();
    for (key, is_grammar) in [("grammars", true), ("snippets", false)] {
        for g in arr(c.get(key)) {
            let Some(lang) = g.get("language").and_then(Value::as_str) else { continue };
            let entry = match langs.iter().position(|l| l.id == lang) {
                Some(i) => &mut langs[i],
                None => {
                    langs.push(Lang { id: lang.into(), name: lang.into(), exts: Vec::new(), grammar: false, snippets: false });
                    langs.last_mut().unwrap()
                }
            };
            if is_grammar { entry.grammar = true } else { entry.snippets = true }
        }
    }
    if langs.is_empty() {
        return None;
    }
    langs.sort_by(|a, b| a.id.cmp(&b.id));
    let rows = langs
        .iter()
        .map(|l| {
            vec![
                cell(&l.id),
                cell(&l.name),
                format!("<div><p>{}</p></div>", l.exts.iter().map(|e| format!("<code>{}</code>", esc(e))).collect::<Vec<_>>().join("&nbsp;")),
                cell(if l.grammar { "✔︎" } else { "\u{2014}" }),
                cell(if l.snippets { "✔︎" } else { "\u{2014}" }),
            ]
        })
        .collect();
    Some(table(&["ID", "Name", "File Extensions", "Grammar", "Snippets"], rows))
}

// ---------------------------------------------------------------------------
// The page

/// Navbar tabs the extension has, in VS Code's order: (id, label, tooltip).
pub fn tabs(d: &DetailsData) -> Vec<(&'static str, &'static str, &'static str)> {
    let mut t = vec![("details", "Details", "Extension details, rendered from the extension's 'README.md' file")];
    if d.manifest.is_some() {
        t.push(("features", "Features", "Lists features contributed by this extension"));
    }
    if d.changelog.is_some() {
        t.push(("changelog", "Changelog", "Extension update history, rendered from the extension's 'CHANGELOG.md' file"));
    }
    if !d.dependencies.is_empty() {
        t.push(("dependencies", "Dependencies", "Lists extensions this extension depends on"));
    }
    if !d.pack.is_empty() && !is_pack_category(d) {
        t.push(("extensionPack", "Extension Pack", "Lists extensions those will be installed together with this extension"));
    }
    t
}

/// An "Extension Packs" extension shows its pack above the README.
fn is_pack_category(d: &DetailsData) -> bool {
    d.categories.iter().any(|c| c.eq_ignore_ascii_case("extension packs"))
}

fn ext_icon(icon: &Option<String>, class: &str) -> String {
    match icon {
        Some(src) => format!(r#"<img class="{class}" src="{}" alt="">"#, esc(src)),
        None => format!(r#"<img class="{class}" src="data:image/svg+xml;utf8,{}" alt="">"#, DEFAULT_ICON),
    }
}

/// VS Code's default extension icon (`defaultIcon.png` as an outline).
const DEFAULT_ICON: &str = "%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 128 128'%3E%3Crect width='128' height='128' rx='8' fill='%23383838'/%3E%3Cpath d='M38 40h22v22H38zM68 40h22v22H68zM38 70h22v22H38zM68 70h22v22H68z' fill='%23888'/%3E%3C/svg%3E";

/// A dependency row (`ExtensionsTree`).
fn dependency_row(e: &ExtRef) -> String {
    format!(
        r#"<div class="dep-row" data-open="{id}" tabindex="0"><div class="extension">{icon}<div class="details"><div class="header"><span class="name">{name}</span><span class="identifier">{id}</span></div><div class="footer"><div class="publisher">{publisher}</div></div></div></div></div>"#,
        id = esc(&e.id),
        icon = ext_icon(&e.icon, "icon"),
        name = esc(&e.name),
        publisher = esc(&e.publisher),
    )
}

/// A pack member in the grid (`ExtensionsGridView`).
fn grid_item(e: &ExtRef) -> String {
    format!(
        r#"<div class="extension-container" data-open="{id}" tabindex="0"><div class="extension-list-item">{icon}<div class="details"><div class="header"><span class="name">{name}</span></div><div class="description">{desc}</div><div class="footer"><span class="publisher-name">{publisher}</span></div></div></div></div>"#,
        id = esc(&e.id),
        icon = ext_icon(&e.icon, "icon"),
        name = esc(&e.name),
        desc = esc(&e.description),
        publisher = esc(&e.publisher),
    )
}

fn markdown_or(md: &Option<String>, loading: bool, none: &str) -> String {
    match md {
        Some(md) => format!(r#"<div class="md">{}</div>"#, markdown_html(md)),
        None if loading => r#"<div class="md"><p class="nocontent">Loading…</p></div>"#.to_string(),
        None => format!(r#"<div class="md"><p>{none}</p></div>"#),
    }
}

/// The header's buttons: Update, Install / Installing, Enable / Disable,
/// Uninstall, Auto Update, Manage.
fn actions_html(d: &DetailsData) -> String {
    let mut a = String::new();
    let button = |label: &str, cmd: &str, prominent: bool| {
        format!(
            r#"<div class="action-item"><a class="extension-action label{}" role="button" tabindex="0" data-cmd="{cmd}">{}</a></div>"#,
            if prominent { " prominent" } else { "" },
            esc(label)
        )
    };
    let split = |label: &str, cmd: &str, menu: &str, prominent: bool| {
        format!(
            r#"<div class="action-item action-dropdown-item"><a class="extension-action label{p}" role="button" tabindex="0" data-cmd="{cmd}">{label}</a><div class="action-dropdown-item-separator{p}"><div></div></div><a class="extension-action label dropdown{p}" role="button" tabindex="0" title="More Actions..." data-menu="{menu}">{chev}</a></div>"#,
            p = if prominent { " prominent" } else { "" },
            label = esc(label),
            chev = codicon("chevron-down"),
        )
    };
    if let Some(v) = &d.update_to {
        a.push_str(&button(&format!("Update to v{v}"), "update", true));
    }
    let auto_update = format!(
        r#"<div class="action-item checkbox-action-item"><a class="extension-action checkbox{}" role="checkbox" aria-checked="{}" tabindex="0" data-cmd="autoUpdate">{}</a><span class="checkbox-label" data-cmd="autoUpdate">Auto Update</span></div>"#,
        if d.auto_update { " checked" } else { "" },
        d.auto_update,
        codicon("check")
    );
    match d.state {
        InstallState::NotInstalled => {
            a.push_str(&button("Install", "install", true));
            a.push_str(&auto_update);
        }
        InstallState::Installing => {
            a.push_str(r#"<div class="action-item disabled"><a class="extension-action label prominent installing">Installing</a></div>"#)
        }
        InstallState::Installed => {
            match d.enabled {
                Enabled::Yes | Enabled::DisabledByTrust => a.push_str(&split("Disable", "disable", "disable", false)),
                _ => a.push_str(&split("Enable", "enable", "enable", false)),
            }
            if d.can_uninstall {
                a.push_str(&split("Uninstall", "uninstall", "uninstall", false));
                a.push_str(&auto_update);
            }
        }
    }
    a.push_str(&format!(
        r#"<div class="action-item"><a class="extension-action icon manage" role="button" tabindex="0" title="Manage" data-menu="manage">{}</a></div>"#,
        codicon("gear")
    ));
    a
}

/// The whole page.
pub fn page_html(d: &DetailsData) -> String {
    let icon = ext_icon(&d.icon, "icon");

    // Title row.
    let mut title = format!(
        r#"<span class="name{}{}" role="heading" title="Extension name"{}>{}</span>"#,
        if d.url.is_some() { " clickable" } else { "" },
        if d.deprecated { " deprecated" } else { "" },
        d.url.as_ref().map(|u| format!(r#" data-href="{}""#, esc(u))).unwrap_or_default(),
        esc(&d.name)
    );
    if d.pre_release {
        title.push_str(r#"<code class="version" title="Extension Version">pre-release</code>"#);
    }
    if d.preview {
        title.push_str(r#"<span class="preview" title="Preview">Preview</span>"#);
    }

    // Subtitle entries; the last non-empty one has no separator.
    let mut entries: Vec<String> = Vec::new();
    {
        let verified = if d.verified {
            format!(r#"<div class="verified-publisher">{}</div>"#, codicon("verified-filled"))
        } else {
            String::new()
        };
        entries.push(format!(
            r#"<div class="publisher{}" role="button" title="Publisher ({name})" data-search="publisher:&quot;{name}&quot;"><div class="publisher-name">{name}</div>{verified}</div>"#,
            if d.url.is_some() { " clickable" } else { "" },
            name = esc(&d.publisher),
        ));
    }
    if let Some(n) = d.install_count.filter(|n| *n > 0) {
        entries.push(format!(
            r#"<span class="install" title="Install count">{}<span class="count">{}</span></span>"#,
            codicon("cloud-download"),
            group_digits(n)
        ));
    }
    if let (Some(r), Some(_)) = (d.rating, &d.url) {
        let r = (r * 2.0).round() / 2.0;
        let mut stars = String::new();
        for i in 1..=5 {
            let i = i as f32;
            stars.push_str(&codicon(if r >= i { "star-full" } else if r >= i - 0.5 { "star-half" } else { "star-empty" }));
        }
        let count = d.rating_count.filter(|c| *c > 0).map(|c| format!(r#"<span style="padding-left:1px"> ({c})</span>"#)).unwrap_or_default();
        let reviews = d.url.as_ref().map(|u| format!("{u}/reviews")).unwrap_or_default();
        entries.push(format!(
            r#"<span class="rating clickable" title="Average rating: {r} out of 5" data-href="{}">{stars}{count}</span>"#,
            esc(&reviews)
        ));
    }
    if let Some(url) = &d.sponsor_url {
        entries.push(format!(r#"<span class="sponsor clickable" data-href="{}" title="{}">{}Sponsor</span>"#, esc(url), esc(url), codicon("heart-filled")));
    }
    let n = entries.len();
    let subtitle: String = entries
        .into_iter()
        .enumerate()
        .map(|(i, e)| format!(r#"<div class="subtitle-entry{}">{e}</div>"#, if i + 1 == n { " last-non-empty" } else { "" }))
        .collect();

    let status: String = d
        .status
        .iter()
        .map(|(sev, text)| {
            let icon = if sev.is_empty() { String::new() } else { codicon(sev) };
            format!(r#"<div class="status-line">{icon}<span>{}</span></div>"#, esc(text))
        })
        .collect();

    // Tabs.
    let tabs = tabs(d);
    let current = if tabs.iter().any(|(id, ..)| *id == d.tab) { d.tab.as_str() } else { "details" };
    let navbar: String = tabs
        .iter()
        .map(|(id, label, tip)| {
            format!(
                r#"<div class="action-item"><a class="action-label{}" role="tab" tabindex="0" title="{}" data-tab="{id}">{}</a></div>"#,
                if *id == current { " checked" } else { "" },
                esc(tip),
                esc(label)
            )
        })
        .collect();

    let details_body = {
        let readme = markdown_or(&d.readme, d.loading, "No README available.");
        if is_pack_category(d) && !d.pack.is_empty() {
            format!(
                r#"<div class="extension-pack-readme"><div class="extension-pack"><div class="header">Extension Pack ({})</div><div class="extension-pack-content"><div class="extensions-grid-view">{}</div></div></div><div class="readme-content">{readme}</div></div>"#,
                d.pack.len(),
                d.pack.iter().map(grid_item).collect::<String>()
            )
        } else {
            readme
        }
    };
    let changelog_body = markdown_or(&d.changelog, d.loading, "No Changelog available.");

    let feats = d.manifest.as_ref().map(|m| features(m, d.runtime.as_ref())).unwrap_or_default();
    let features_body = if feats.is_empty() {
        r#"<div class="subcontent feature-contributions"><div class="no-features">No features contributed.</div></div>"#.to_string()
    } else {
        let selected = d.feature.as_deref().filter(|f| feats.iter().any(|x| x.id == *f)).unwrap_or(feats[0].id);
        let list: String = feats
            .iter()
            .map(|f| {
                format!(
                    r#"<div class="monaco-list-row extension-feature-list-item{}" tabindex="0" data-feature="{}"><div class="extension-feature-label">{}</div></div>"#,
                    if f.id == selected { " selected" } else { "" },
                    f.id,
                    esc(f.label)
                )
            })
            .collect();
        let views: String = feats
            .iter()
            .map(|f| {
                format!(
                    r#"<div class="extension-feature-content{}" data-feature-view="{}"><div class="feature-header"><div class="feature-title">{}</div></div><div class="feature-body"><div class="feature-body-content"><div class="feature-content">{}</div></div></div></div>"#,
                    if f.id == selected { " active" } else { "" },
                    f.id,
                    esc(f.label),
                    f.html
                )
            })
            .collect();
        format!(
            r#"<div class="subcontent feature-contributions"><div class="features-list-container"><div class="features-list-wrapper">{list}</div></div><div class="sash-line"></div><div class="feature-view-container">{views}</div></div>"#
        )
    };
    let dependencies_body = format!(r#"<div class="subcontent dependencies">{}</div>"#, d.dependencies.iter().map(dependency_row).collect::<String>());
    let pack_body = format!(
        r#"<div class="subcontent"><div class="extensions-grid-view">{}</div></div>"#,
        d.pack.iter().map(grid_item).collect::<String>()
    );

    // The column on the right.
    let entry = |name: &str, value: String| format!(r#"<div class="more-info-entry"><div class="more-info-entry-name">{name}</div>{value}</div>"#);
    let mut additional = String::new();
    if let Some(info) = &d.install_info {
        let mut rows = entry("Identifier", format!("<code>{}</code>", esc(&d.id.to_lowercase())));
        rows.push_str(&entry("Version", format!("<code>{}</code>", esc(&info.version))));
        if let Some(t) = info.last_updated {
            rows.push_str(&entry("Last Updated", format!(r#"<div data-time="{t}"></div>"#)));
        }
        if let Some((size, path)) = &info.size {
            rows.push_str(&format!(
                r#"<div class="more-info-entry"><div class="more-info-entry-name" title="Size when installed">Size</div><div class="link" role="link" tabindex="0" title="{p}" data-reveal="{p}">{}</div></div>"#,
                format_size(*size),
                p = esc(path)
            ));
        }
        if let Some((size, path)) = &info.cache {
            rows.push_str(&format!(
                r#"<div class="more-info-entry"><div class="more-info-entry-name" title="Cache size">Cache</div><div class="link" role="link" tabindex="0" title="{p}" data-reveal="{p}">{}</div></div>"#,
                format_size(*size),
                p = esc(path)
            ));
        }
        additional.push_str(&format!(
            r#"<div class="more-info-container additional-details-element"><div class="additional-details-title">Installation</div><div class="more-info">{rows}</div></div>"#
        ));
    }
    if let Some(m) = &d.market_info {
        let mut rows = String::new();
        if d.install_info.is_none() {
            rows.push_str(&entry("Identifier", format!("<code>{}</code>", esc(&d.id.to_lowercase()))));
            rows.push_str(&entry("Version", format!("<code>{}</code>", esc(&m.version))));
        }
        if let Some(t) = m.published {
            rows.push_str(&entry("Published", format!(r#"<div data-time="{t}"></div>"#)));
        }
        if let Some(t) = m.last_released {
            rows.push_str(&entry("Last Released", format!(r#"<div data-time="{t}"></div>"#)));
        }
        additional.push_str(&format!(
            r#"<div class="more-info-container additional-details-element"><div class="additional-details-title">Marketplace</div><div class="more-info">{rows}</div></div>"#
        ));
    }
    if !d.categories.is_empty() {
        let cats: String = d
            .categories
            .iter()
            .map(|c| format!(r#"<span class="category clickable" tabindex="0" data-category="{0}">{0}</span>"#, esc(c)))
            .collect();
        additional.push_str(&format!(
            r#"<div class="categories-container additional-details-element"><div class="additional-details-title">Categories</div><div class="categories">{cats}</div></div>"#
        ));
    }
    let mut resources: Vec<(String, &str, &String)> = Vec::new();
    if let Some(u) = &d.repository {
        resources.push(("Repository".into(), "repo", u));
    }
    if let Some(u) = &d.issues {
        resources.push(("Issues".into(), "issues", u));
    }
    if let Some(u) = &d.license_url {
        resources.push(("License".into(), "link-external", u));
    }
    if let Some(u) = &d.publisher_url {
        resources.push((d.publisher.clone(), "link-external", u));
    }
    if let Some(u) = &d.url {
        resources.push(("Marketplace".into(), "link-external", u));
    }
    if !resources.is_empty() {
        let list: String = resources
            .iter()
            .map(|(label, icon, url)| {
                format!(r#"<div class="resource" title="{u}" data-href="{u}">{}<a tabindex="0">{}</a></div>"#, codicon(icon), esc(label), u = esc(url))
            })
            .collect();
        additional.push_str(&format!(
            r#"<div class="resources-container additional-details-element"><div class="additional-details-title">Resources</div><div class="resources">{list}</div></div>"#
        ));
    }

    let content_for = |id: &str| -> String {
        match id {
            "features" => features_body.clone(),
            "changelog" => changelog_body.clone(),
            "dependencies" => dependencies_body.clone(),
            "extensionPack" => pack_body.clone(),
            _ => details_body.clone(),
        }
    };
    let panes: String = tabs
        .iter()
        .map(|(id, ..)| {
            format!(
                r#"<div class="details tab-pane{}" data-pane="{id}"><div class="content-container{}">{}</div><div class="additional-details-container"><div class="additional-details-content">{additional}</div></div></div>"#,
                if *id == current { " active" } else { "" },
                if *id == "features" { " features" } else { "" },
                content_for(id),
            )
        })
        .collect();

    let base = d.base_url.as_ref().map(|b| format!(r#"<base href="{}">"#, esc(b))).unwrap_or_default();
    let markdown_css = scope_css(MARKDOWN_CSS, ".md");
    let codicon_url = crate::workbench::webviews::CODICON_PATH;
    let actions = actions_html(d);

    format!(
        r##"<!DOCTYPE html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src https: http: data: vscode-resource:; style-src 'unsafe-inline'; script-src 'unsafe-inline'; font-src https: data: vscode-resource:;">
{base}
<style>
@font-face {{ font-family: codicon; src: url("vscode-resource://w{handle}{codicon_url}") format("truetype"); }}
:root {{
  --ext-button-bg: var(--vscode-extensionButton-background, var(--vscode-button-secondaryBackground, transparent));
  --ext-button-fg: var(--vscode-extensionButton-foreground, var(--vscode-button-secondaryForeground, #CCCCCC));
  --ext-button-hover: var(--vscode-extensionButton-hoverBackground, var(--vscode-button-secondaryHoverBackground, #FFFFFF10));
  --ext-button-border: var(--vscode-extensionButton-border, var(--vscode-button-border, #333536FF));
  --ext-button-separator: var(--vscode-extensionButton-separator, #FFFFFF66);
  --section-border: var(--vscode-panelSection-border, var(--vscode-panel-border, #2A2B2C));
  --star: var(--vscode-extensionIcon-starForeground, #FF8E00);
  --verified: var(--vscode-extensionIcon-verifiedForeground, var(--vscode-textLink-foreground));
  --sponsor: var(--vscode-extensionIcon-sponsorForeground, #D758B3);
  --kb-bg: var(--vscode-keybindingLabel-background, rgba(128,128,128,.17));
  --kb-fg: var(--vscode-keybindingLabel-foreground, #CCCCCC);
  --kb-border: var(--vscode-keybindingLabel-border, rgba(51,51,51,.6));
  --kb-bottom: var(--vscode-keybindingLabel-bottomBorder, rgba(68,68,68,.6));
}}
{markdown_css}
html, body {{ height: 100%; overflow: hidden; }}
body {{ padding: 0; margin: 0; color: var(--vscode-foreground); background: var(--vscode-editor-background); font-family: var(--vscode-font-family); font-size: 13px; }}
.codicon {{ font: normal normal normal 16px/1 codicon; display: inline-block; text-decoration: none; text-rendering: auto; text-align: center; -webkit-font-smoothing: antialiased; user-select: none; -webkit-user-select: none; }}
.clickable {{ cursor: pointer; }}
.extension-editor {{ height: 100%; overflow: hidden; display: flex; flex-direction: column; margin: 0 auto; max-width: 95%; }}
.extension-editor > .header {{ display: flex; padding: 20px 0 12px 20px; overflow: hidden; font-size: 14px; flex-shrink: 0; }}
.header > .icon-container > .icon {{ height: 128px; width: 128px; object-fit: contain; }}
.header > .details {{ padding-left: 12px; overflow: hidden; user-select: text; -webkit-user-select: text; }}
.header .title {{ display: flex; align-items: center; }}
.header .title > .name {{ flex: 0; font-size: 26px; line-height: 30px; font-weight: 600; white-space: nowrap; }}
.header .title > .name.deprecated {{ text-decoration: line-through; }}
.header .title > .version {{ margin-left: 10px; white-space: nowrap; }}
.header .title > .preview {{ background: rgb(214, 63, 38); font-size: 10px; font-style: italic; margin-left: 10px; padding: 0 4px; border-radius: 4px; user-select: none; }}
.header .subtitle {{ padding-top: 6px; line-height: 20px; display: flex; align-items: center; flex-wrap: wrap; }}
.subtitle .publisher, .subtitle .install, .subtitle .rating, .subtitle .sponsor {{ display: flex; align-items: center; }}
.subtitle .publisher .verified-publisher {{ margin-left: 4px; display: flex; align-items: center; }}
.subtitle .verified-publisher .codicon {{ color: var(--verified); }}
.subtitle .install > .count {{ margin-left: 6px; }}
.subtitle .rating .codicon {{ color: var(--star); }}
.subtitle .rating .codicon-star-empty {{ opacity: .6; }}
.subtitle .sponsor .codicon {{ color: var(--sponsor); padding-right: 3px; }}
.subtitle > .subtitle-entry:not(:empty):not(.last-non-empty) {{ border-right: 1px solid rgba(128,128,128,.7); margin-right: 14px; padding-right: 14px; }}
.header .description {{ margin-top: 10px; }}
.actions-status-container {{ margin-top: 10px; display: flex; }}
.actions-status-container.list-layout {{ display: block; }}
.actions-container {{ display: flex; flex-wrap: wrap; align-items: center; }}
.action-item {{ display: flex; align-items: center; flex-shrink: 0; margin-right: 6px; overflow: hidden; }}
.extension-action.label {{ display: inline-block; font-size: 13px; line-height: 22px; padding: 0 5px; margin: 2px 0 2px 2px; border-radius: 4px; background-color: var(--ext-button-bg); color: var(--ext-button-fg); border: 1px solid var(--ext-button-border); cursor: pointer; white-space: nowrap; user-select: none; }}
.extension-action.label:hover {{ background-color: var(--ext-button-hover); }}
.extension-action.label.prominent {{ background-color: var(--vscode-extensionButton-prominentBackground); color: var(--vscode-extensionButton-prominentForeground); }}
.extension-action.label.prominent:hover {{ background-color: var(--vscode-extensionButton-prominentHoverBackground); }}
.action-item.disabled .extension-action.label {{ cursor: default; opacity: .9; }}
.action-dropdown-item > .extension-action.label:first-child {{ border-radius: 4px 0 0 4px; border-right-width: 0; }}
.action-dropdown-item > .action-dropdown-item-separator {{ height: 22px; margin: 2px 0; display: flex; align-items: center; background-color: var(--ext-button-bg); border: 1px solid var(--ext-button-border); border-left-width: 0; border-right-width: 0; }}
.action-dropdown-item > .action-dropdown-item-separator.prominent {{ background-color: var(--vscode-extensionButton-prominentBackground); }}
.action-dropdown-item > .action-dropdown-item-separator > div {{ width: 1px; height: 16px; background-color: var(--ext-button-separator); }}
.action-dropdown-item > .extension-action.label.dropdown {{ border-left-width: 0; border-radius: 0 4px 4px 0; padding: 0 2px; margin-left: 0; margin-right: 2px; display: flex; align-items: center; height: 22px; }}
.checkbox-action-item {{ cursor: pointer; }}
.extension-action.checkbox {{ height: 18px; width: 18px; margin: 2px 0 2px 2px; box-sizing: border-box; border: 1px solid var(--vscode-checkbox-border); background: var(--vscode-checkbox-background); color: var(--vscode-checkbox-foreground); border-radius: 3px; display: flex; align-items: center; justify-content: center; }}
.extension-action.checkbox .codicon {{ visibility: hidden; font-size: 16px; }}
.extension-action.checkbox.checked .codicon {{ visibility: visible; }}
.checkbox-label {{ padding-left: 5px; font-size: 13px; line-height: 22px; user-select: none; }}
.extension-action.icon {{ display: flex; align-items: center; justify-content: center; width: 16px; height: 16px; padding: 3px; margin-top: 2px; border-radius: 5px; cursor: pointer; }}
.extension-action.icon:hover {{ background-color: var(--vscode-toolbar-hoverBackground, rgba(90,93,94,.31)); }}
.actions-status-container > .status {{ line-height: 22px; font-size: 90%; margin-top: 3px; }}
.actions-status-container.list-layout > .status {{ margin-top: 5px; }}
.status-line .codicon {{ vertical-align: text-bottom; margin-right: 4px; }}
.status-line .codicon-error {{ color: var(--vscode-editorError-foreground); }}
.status-line .codicon-warning {{ color: var(--vscode-editorWarning-foreground); }}
.status-line .codicon-info {{ color: var(--vscode-editorInfo-foreground); }}
.extension-editor > .body {{ flex: 1; overflow: hidden; display: flex; flex-direction: column; }}
.navbar {{ height: 36px; font-weight: bold; font-size: 14px; line-height: 36px; padding-left: 20px; border-bottom: 1px solid var(--section-border); flex-shrink: 0; }}
.navbar .actions-container {{ height: 100%; }}
.navbar .action-item {{ height: 100%; margin-right: 0; overflow: visible; }}
.navbar .action-label {{ padding: 0 10px; font-size: 11px; font-weight: normal; color: var(--vscode-panelTitle-inactiveForeground); text-transform: uppercase; cursor: pointer; height: 100%; box-sizing: border-box; display: block; }}
.navbar .action-label.checked {{ border-bottom: 1px solid var(--vscode-panelTitle-activeBorder); color: var(--vscode-panelTitle-activeForeground); }}
.navbar .action-label:hover {{ color: var(--vscode-panelTitle-activeForeground); }}
.extension-editor > .body > .content {{ flex: 1; position: relative; overflow: hidden; user-select: text; -webkit-user-select: text; min-height: 0; }}
.tab-pane {{ display: none; height: 100%; }}
.tab-pane.active {{ display: flex; }}
.content-container {{ margin: 0 auto; max-width: 75%; height: 100%; flex: 1; overflow-y: auto; min-width: 0; }}
.content-container.features {{ overflow: hidden; }}
.details.narrow > .content-container {{ margin: inherit; max-width: inherit; }}
.additional-details-container {{ width: 25%; min-width: 175px; height: 100%; }}
.details.narrow > .additional-details-container {{ display: none; }}
.additional-details-content {{ height: 100%; overflow-y: auto; padding: 12px; box-sizing: border-box; }}
.additional-details-element {{ padding-bottom: 16px; }}
.additional-details-element:not(:first-child) {{ padding-top: 16px; }}
.additional-details-element:not(:last-child) {{ border-bottom: 1px solid rgba(128,128,128,.22); }}
.additional-details-title {{ font-size: 120%; padding-bottom: 12px; }}
.category {{ display: inline-block; border: 1px solid rgba(136,136,136,.45); padding: 2px 4px; border-radius: 2px; font-size: 90%; margin: 0 6px 3px 0; }}
.resource {{ display: flex; align-items: center; cursor: pointer; padding: 2px 4px; }}
.resource a {{ padding-left: 4px; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; color: inherit; }}
.extension-editor a {{ text-decoration: none; }}
.extension-editor a.extension-action {{ color: inherit; }}
.extension-action.icon {{ color: var(--vscode-icon-foreground, var(--vscode-foreground)); }}
[tabindex]:focus {{ outline: none; }}
[tabindex]:focus-visible {{ outline: 1px solid var(--vscode-focusBorder); outline-offset: -1px; }}
.resource:hover a {{ text-decoration: underline; }}
.more-info-entry {{ font-size: 90%; display: grid; grid-template-columns: 40% 60%; gap: 6px; padding: 2px 4px; }}
.more-info-entry:nth-child(odd) {{ background-color: rgba(130,130,130,.04); }}
.more-info-entry > .more-info-entry-name {{ overflow: hidden; text-overflow: ellipsis; }}
.more-info-entry code {{ background-color: transparent !important; padding: 0 !important; overflow-wrap: anywhere; }}
.more-info-entry .link {{ cursor: pointer; color: var(--vscode-textLink-foreground); }}
.more-info-entry .link:hover {{ text-decoration: underline; color: var(--vscode-textLink-activeForeground); }}
.extension-editor code:not(:empty) {{ font-family: var(--monaco-monospace-font, var(--vscode-editor-font-family)); font-size: 90%; background-color: rgba(128,128,128,.17); border-radius: 4px; padding: 1px 4px; color: inherit; }}
.subcontent {{ height: 100%; padding: 20px; overflow-y: auto; box-sizing: border-box; }}
.content table {{ width: 100%; border-spacing: 0; border-collapse: separate; }}
.content table tr:nth-child(odd) {{ background-color: rgba(130,130,130,.04); }}
.content table tr:not(:first-child):hover {{ background-color: rgba(128,128,128,.15); }}
.content table th, .content table td {{ padding: 2px 16px 2px 4px; }}
.content table td {{ vertical-align: top; }}
.content table th:last-child, .content table td:last-child {{ padding: 2px 4px; }}
.content table th {{ text-align: left; word-break: keep-all; }}
.content table td > div, .content table td > div > p, .content table td > p {{ padding: 0; margin: 0; }}
.content table .markdown pre {{ margin: 0; white-space: pre-wrap; }}
.content table .markdown pre code {{ background: none; padding: 0; }}
.content table .colorBox {{ box-sizing: border-box; width: .8em; height: .8em; display: inline-block; border: .1em solid rgb(238,238,238); margin: 0 .2em; vertical-align: middle; }}
.feature-contributions {{ margin-top: 14px; padding: 0 !important; height: calc(100% - 14px) !important; display: flex; overflow: hidden !important; }}
.features-list-container {{ width: 200px; flex-shrink: 0; height: 100%; overflow-y: auto; }}
.features-list-wrapper {{ padding-left: 24px; }}
.sash-line {{ width: 1px; background: var(--section-border); flex-shrink: 0; }}
.extension-feature-list-item {{ height: 22px; line-height: 22px; padding: 0 10px; display: flex; align-items: center; cursor: pointer; }}
.extension-feature-list-item:hover {{ background: var(--vscode-list-hoverBackground); }}
.extension-feature-list-item.selected {{ background: var(--vscode-list-inactiveSelectionBackground); color: var(--vscode-list-inactiveSelectionForeground); }}
.features-list-wrapper:focus-within .extension-feature-list-item.selected {{ background: var(--vscode-list-activeSelectionBackground); color: var(--vscode-list-activeSelectionForeground); }}
.extension-feature-label {{ flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
.feature-view-container {{ flex: 1; min-width: 0; height: 100%; }}
.extension-feature-content {{ display: none; padding-left: 24px; height: 100%; box-sizing: border-box; flex-direction: column; }}
.extension-feature-content.active {{ display: flex; }}
.feature-header {{ margin: 0 10px 10px 0; display: flex; line-height: 20px; align-items: center; }}
.feature-title {{ font-size: 26px; font-weight: 600; padding: 10px 10px 10px 0; flex: 1; overflow: hidden; text-overflow: ellipsis; }}
.feature-body {{ flex: 1; min-height: 0; }}
.feature-body-content {{ height: 100%; overflow: auto; box-sizing: border-box; padding-right: 10px; }}
.feature-content.markdown .codicon, .runtime-status .codicon {{ vertical-align: sub; }}
.runtime-status .message-entry {{ display: flex; align-items: center; margin: 5px; }}
.runtime-status .message-entry .codicon {{ padding-right: 2px; color: var(--vscode-editorError-foreground); }}
.markdown ul {{ padding-left: 20px; }}
.markdown li {{ line-height: 22px; }}
.no-features {{ padding: 0 20px; }}
.monaco-keybinding {{ display: flex; align-items: center; line-height: 10px; }}
.monaco-keybinding-key {{ display: inline-block; border-style: solid; border-width: 1px; border-radius: 3px; vertical-align: middle; font-size: 11px; padding: 3px 5px; margin: 0 2px; background-color: var(--kb-bg); color: var(--kb-fg); border-color: var(--kb-border); border-bottom-color: var(--kb-bottom); box-shadow: inset 0 -1px 0 var(--kb-bottom); }}
.monaco-keybinding-key:first-child {{ margin-left: 0; }}
.dep-row {{ height: 62px; display: flex; align-items: center; cursor: pointer; padding-left: 8px; }}
.dep-row:hover {{ background: var(--vscode-list-hoverBackground); }}
.dep-row .extension {{ display: flex; align-items: center; width: 100%; }}
.dep-row .icon {{ height: 40px; width: 40px; object-fit: contain; }}
.dep-row .details {{ flex: 1; overflow: hidden; padding-left: 10px; }}
.dep-row .header {{ display: flex; align-items: center; line-height: 19px; overflow: hidden; }}
.dep-row .name {{ font-weight: bold; font-size: 16px; }}
.dep-row:hover .name {{ text-decoration: underline; }}
.dep-row .identifier {{ font-size: 90%; opacity: .6; margin-left: 10px; background: rgba(173,173,173,.31); padding: 0 4px; border-radius: 4px; }}
.dep-row .footer {{ display: flex; line-height: 19px; overflow: hidden; padding-top: 5px; }}
.dep-row .footer .publisher {{ font-size: 90%; font-weight: 600; opacity: .6; }}
.extensions-grid-view {{ display: flex; flex-wrap: wrap; }}
.extension-container {{ width: 350px; margin: 5px 10px; cursor: pointer; }}
.extension-list-item {{ display: flex; height: 72px; align-items: center; }}
.extension-list-item .icon {{ width: 42px; height: 42px; padding: 10px 14px 10px 0; object-fit: contain; flex-shrink: 0; }}
.extension-list-item .details {{ flex: 1; overflow: hidden; }}
.extension-list-item .name {{ font-weight: bold; }}
.extension-container:hover .name {{ text-decoration: underline; }}
.extension-list-item .description, .extension-list-item .footer {{ white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
.extension-list-item .publisher-name {{ font-size: 90%; color: var(--vscode-descriptionForeground); font-weight: 600; }}
.extension-pack-readme {{ height: 100%; display: flex; flex-direction: column; max-width: 882px; margin: 0 auto; }}
.extension-pack {{ padding-left: 20px; flex-shrink: 0; }}
.extension-pack > .header {{ margin-right: 30px; font-weight: bold; font-size: 120%; padding: 6px; line-height: 22px; min-width: 150px; }}
.extension-pack-content {{ border-top: 1px solid rgba(128,128,128,.22); border-bottom: 1px solid rgba(128,128,128,.22); max-height: 282px; overflow-y: auto; }}
.readme-content {{ flex: 1; }}
.nocontent {{ opacity: .8; }}
</style></head>
<body><div class="extension-editor" role="document">
<div class="header"><div class="icon-container">{icon}</div><div class="details">
<div class="title">{title}</div>
<div class="subtitle">{subtitle}</div>
<div class="description">{description}</div>
<div class="actions-status-container{list_layout}"><div class="monaco-action-bar"><div class="actions-container">{actions}</div></div><div class="status">{status}</div></div>
</div></div>
<div class="body">
<div class="navbar"><div class="monaco-action-bar"><div class="actions-container" role="tablist">{navbar}</div></div></div>
<div class="content">{panes}</div>
</div></div>
<script>
const vscode = acquireVsCodeApi();
const post = m => vscode.postMessage(m);
let tab = {current:?};
let feature = {feature:?};
const saveState = () => post({{ command: 'state', tab, feature }});
document.querySelectorAll('.navbar [data-tab]').forEach(a => a.addEventListener('click', () => {{
  tab = a.dataset.tab;
  document.querySelectorAll('.navbar [data-tab]').forEach(x => x.classList.toggle('checked', x === a));
  document.querySelectorAll('.tab-pane').forEach(p => p.classList.toggle('active', p.dataset.pane === tab));
  saveState();
}}));
document.querySelectorAll('[data-feature]').forEach(row => row.addEventListener('click', () => {{
  feature = row.dataset.feature;
  document.querySelectorAll('[data-feature]').forEach(r => r.classList.toggle('selected', r === row));
  document.querySelectorAll('[data-feature-view]').forEach(v => v.classList.toggle('active', v.dataset.featureView === feature));
  saveState();
}}));
document.querySelectorAll('[data-cmd]').forEach(b => b.addEventListener('click', e => {{ e.stopPropagation(); post({{ command: b.dataset.cmd }}); }}));
document.querySelectorAll('[data-menu]').forEach(b => b.addEventListener('click', e => {{
  e.stopPropagation();
  const r = b.getBoundingClientRect();
  post({{ command: 'menu', menu: b.dataset.menu, x: r.left, y: r.bottom }});
}}));
document.querySelectorAll('[data-href]').forEach(el => el.addEventListener('click', () => post({{ command: 'openLink', url: el.dataset.href }})));
document.querySelectorAll('[data-open]').forEach(el => el.addEventListener('click', () => post({{ command: 'openExtension', id: el.dataset.open }})));
document.querySelectorAll('[data-category]').forEach(el => el.addEventListener('click', () => post({{ command: 'search', query: '@category:"' + el.dataset.category + '"' }})));
document.querySelectorAll('[data-search]').forEach(el => el.addEventListener('click', e => {{ if (!e.target.closest('[data-href]')) post({{ command: 'search', query: el.dataset.search }}); }}));
document.querySelectorAll('[data-reveal]').forEach(el => el.addEventListener('click', () => post({{ command: 'reveal', path: el.dataset.reveal }})));
document.addEventListener('click', e => {{
  const a = e.target.closest('.md a[href]');
  if (!a) return;
  const href = a.getAttribute('href');
  if (href.startsWith('#')) return;
  e.preventDefault();
  post({{ command: 'openLink', url: a.href }});
}});
// date.ts fromNow(date, true, true, true)
function fromNow(ms) {{
  const seconds = Math.round((Date.now() - ms) / 1000);
  const units = [[60, 1, 'second'], [3600, 60, 'minute'], [86400, 3600, 'hour'], [604800, 86400, 'day'], [2592000, 604800, 'week'], [31536000, 2592000, 'month'], [Infinity, 31536000, 'year']];
  for (const [limit, size, name] of units) {{
    if (seconds < limit) {{
      const v = size === 1 ? seconds : Math.round(seconds / size);
      return v + ' ' + name + (v === 1 ? '' : 's') + ' ago';
    }}
  }}
}}
const times = () => document.querySelectorAll('[data-time]').forEach(el => {{
  const ms = Number(el.dataset.time);
  el.textContent = fromNow(ms);
  el.title = new Date(ms).toString();
}});
times(); setInterval(times, 60000);
// Narrow editors drop the details column (extensionEditor.ts layout).
const narrow = () => document.querySelectorAll('.tab-pane').forEach(p => p.classList.toggle('narrow', window.innerWidth < 500));
narrow(); window.addEventListener('resize', narrow);
</script>
</body></html>"##,
        handle = d.handle,
        description = esc(&d.description),
        list_layout = if d.state == InstallState::Installed { " list-layout" } else { "" },
        feature = d.feature.clone().unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readme_markdown_becomes_safe_html() {
        let html = markdown_html("# Title\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n<script>alert(1)</script><img src=x onerror=\"boom()\">");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<table>"));
        assert!(!html.contains("<script"));
        assert!(!html.contains("onerror"));
    }

    #[test]
    fn untranslated_titles_become_words() {
        assert_eq!(readable("%python.command.python.createNewFile.title%"), "Create New File");
        assert_eq!(readable("Plain title"), "Plain title");
    }

    #[test]
    fn numbers_dates_and_sizes_read_like_vs_code() {
        assert_eq!(group_digits(150138697), "150,138,697");
        assert_eq!(group_digits(999), "999");
        assert_eq!(format_size(42_258_000), "40.30MB");
        assert_eq!(iso_to_ms("1970-01-02T00:00:00Z"), Some(86_400_000));
        assert_eq!(iso_to_ms("2026-05-01T04:48:48.461Z"), Some(1_777_610_928_000));
    }

    #[test]
    fn keybindings_are_boxes_per_key() {
        let html = keybinding_html("cmd+shift+p");
        assert_eq!(html.matches("monaco-keybinding-key\"").count(), 3);
        assert!(html.contains("⇧") && html.contains("⌘") && html.contains(">P<"));
    }

    #[test]
    fn features_follow_the_manifest() {
        let manifest = serde_json::json!({
            "activationEvents": ["onLanguage:python"],
            "contributes": {
                "commands": [{ "command": "x.run", "title": "Run", "category": "X" }],
                "keybindings": [{ "command": "x.run", "key": "ctrl+r", "mac": "cmd+r" }],
                "menus": { "editor/title": [{ "command": "x.run" }] },
                "configuration": { "properties": { "x.flag": { "type": "boolean", "description": "A flag" } } },
                "debuggers": [{ "type": "x", "label": "X Debug" }]
            }
        });
        let f = features(&manifest, Some(&Runtime { activated: true, activation: Some(("onLanguage:python".into(), 12)), error: None }));
        assert!(f[0].html.contains("Activated by <code>onLanguage:python</code> event: <code>12ms</code>"));
        let labels: Vec<&str> = f.iter().map(|f| f.label).collect();
        assert_eq!(labels, ["Runtime Status", "Activation Events", "Commands", "Debuggers", "Settings"]);
        let commands = &f[2].html;
        assert!(commands.contains("<code>x.run</code>"));
        assert!(commands.contains("<code>commandPalette</code>&nbsp;<code>editor/title</code>"));
        assert!(commands.contains("⌘"));
        // A setting without a default shows its type's default.
        assert!(f[4].html.contains("false"));
    }

    #[test]
    fn markdown_styles_stay_inside_the_readme() {
        let css = scope_css("body { padding: 1px; }\nh1, h2 { color: red; }\n.vscode-dark th { x: y; }\n@media (x) { body { a: b; } }", ".md");
        assert!(css.contains(".md { padding: 1px; }"));
        assert!(css.contains(".md h1, .md h2"));
        assert!(css.contains(".vscode-dark .md th"));
        assert!(css.contains("@media (x) {\n.md { a: b; }"));
    }

    #[test]
    fn page_has_header_tabs_and_actions() {
        let mut d = DetailsData {
            id: "sample.hello-world".into(),
            name: "Hello World Sample".into(),
            publisher: "sample".into(),
            state: InstallState::Installed,
            can_uninstall: true,
            auto_update: true,
            readme: Some("Hello **there**".into()),
            install_info: Some(InstallInfo { version: "0.0.1".into(), ..Default::default() }),
            dependencies: vec![ExtRef { id: "a.b".into(), name: "B".into(), ..Default::default() }],
            ..Default::default()
        };
        d.read_manifest(&serde_json::json!({
            "contributes": { "commands": [{ "command": "x.run", "title": "Run", "category": "X" }] },
            "categories": ["Other"]
        }));
        let html = page_html(&d);
        assert!(html.contains("Hello World Sample"));
        assert!(html.contains("<strong>there</strong>"));
        assert!(html.contains(r#"data-cmd="disable""#));
        assert!(html.contains(r#"data-menu="uninstall""#));
        assert!(html.contains("Auto Update"));
        assert!(html.contains(r#"data-menu="manage""#));
        assert!(html.contains(r#"data-tab="features""#));
        assert!(html.contains(r#"data-tab="dependencies""#));
        assert!(!html.contains(r#"data-tab="changelog""#));
        assert!(html.contains("Installation"));
        assert!(html.contains(r#"data-category="Other""#));
    }
}
