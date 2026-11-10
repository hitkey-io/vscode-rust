//! The extension editor: the page that opens when an extension is clicked in
//! the Extensions view — header (icon, name, publisher, installs, rating,
//! description, actions) and the DETAILS (README), FEATURES (commands,
//! settings) and CHANGELOG tabs, with "Installation" / "Marketplace"
//! details on the right. Rendered as an HTML page in a native web view, as
//! VS Code renders it in a webview.
//!
//! VS Code analogues:
//! - `src/vs/workbench/contrib/extensions/browser/extensionEditor.ts`
//! - `src/vs/workbench/contrib/extensions/browser/media/extensionEditor.css`
//!   (20px header padding, 128px icon, 26px name, 36px navbar with 11px
//!   uppercase tabs, content 75% wide with the details column at 25%)
//! - `src/vs/workbench/contrib/markdown/browser/markdownDocumentRenderer.ts`
//!   (`DEFAULT_MARKDOWN_STYLES`, in `assets/ext-host/markdown.css`)

/// Which extension's page to open.
#[derive(Clone, Debug)]
pub enum OpenTarget {
    /// Installed: its id.
    Installed(String),
    /// From the catalogue.
    Gallery(crate::extensions::marketplace::GalleryExtension),
}

const MARKDOWN_CSS: &str = include_str!("../../assets/ext-host/markdown.css");

/// Everything the page shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DetailsData {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub verified: bool,
    pub version: String,
    pub description: String,
    pub downloads: Option<u64>,
    pub rating: Option<f32>,
    pub review_count: Option<u64>,
    /// Image URL (`https://…` or `vscode-resource://…`).
    pub icon: Option<String>,
    pub installed: bool,
    pub can_uninstall: bool,
    pub installing: bool,
    /// "Activated", "Not activated", "Activation failed: …"
    pub status: Option<String>,
    pub readme: Option<String>,
    pub changelog: Option<String>,
    /// Where relative links and images in the README point.
    pub base_url: Option<String>,
    /// Contributed commands: (title, id).
    pub commands: Vec<(String, String)>,
    /// Contributed settings: (key, description, default as JSON).
    pub settings: Vec<(String, String, String)>,
    pub categories: Vec<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub last_updated: Option<String>,
    /// Still fetching the README from the marketplace.
    pub loading: bool,
}

impl DetailsData {
    /// Commands and settings from an extension's `package.json`.
    pub fn read_features(&mut self, manifest: &serde_json::Value) {
        let contributes = manifest.get("contributes");
        self.commands = contributes
            .and_then(|c| c.get("commands"))
            .and_then(|c| c.as_array())
            .into_iter()
            .flatten()
            .filter_map(|c| {
                let id = c.get("command")?.as_str()?.to_string();
                let title = readable(c.get("title").and_then(|t| t.as_str()).unwrap_or_default());
                let category = readable(c.get("category").and_then(|t| t.as_str()).unwrap_or_default());
                let title = if category.is_empty() { title.to_string() } else { format!("{category}: {title}") };
                Some((title, id))
            })
            .collect();
        let sections = match contributes.and_then(|c| c.get("configuration")) {
            Some(serde_json::Value::Array(a)) => a.clone(),
            Some(v) => vec![v.clone()],
            None => Vec::new(),
        };
        self.settings = sections
            .iter()
            .filter_map(|s| s.get("properties").and_then(|p| p.as_object()))
            .flatten()
            .map(|(key, schema)| {
                let desc = readable(
                    schema
                        .get("markdownDescription")
                        .or_else(|| schema.get("description"))
                        .and_then(|d| d.as_str())
                        .unwrap_or_default(),
                );
                let default = schema.get("default").map(|d| d.to_string()).unwrap_or_default();
                (key.clone(), desc, default)
            })
            .collect();
        if let Some(cats) = manifest.get("categories").and_then(|c| c.as_array()) {
            self.categories = cats.iter().filter_map(|c| c.as_str()).map(str::to_string).collect();
        }
        if self.repository.is_none() {
            self.repository = manifest
                .get("repository")
                .and_then(|r| r.as_str().map(str::to_string).or_else(|| r.get("url").and_then(|u| u.as_str()).map(str::to_string)))
                .map(|u| u.trim_start_matches("git+").trim_end_matches(".git").to_string());
        }
        if self.license.is_none() {
            self.license = manifest.get("license").and_then(|l| l.as_str()).map(str::to_string);
        }
    }
}

/// A `%key%` placeholder whose translation isn't at hand (catalogue
/// manifests come without `package.nls.json`) shown as words from its key:
/// `%python.command.python.createNewFile.title%` → "Create New File".
fn readable(s: &str) -> String {
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

fn format_count(n: u64) -> String {
    crate::extensions::marketplace::format_count(n)
}

/// The whole page.
pub fn page_html(d: &DetailsData) -> String {
    let icon = match &d.icon {
        Some(src) => format!(r#"<img class="icon" src="{}" alt="">"#, esc(src)),
        None => r#"<div class="icon placeholder"></div>"#.to_string(),
    };
    let verified = if d.verified { r#"<span class="verified" title="Verified publisher">✔</span>"# } else { "" };
    let mut subtitle = vec![format!(r#"<span class="publisher">{verified}{}</span>"#, esc(&d.publisher))];
    if let Some(n) = d.downloads {
        subtitle.push(format!(r#"<span class="install" title="Install count">⤓<span class="count">{}</span></span>"#, format_count(n)));
    }
    if let Some(r) = d.rating.filter(|r| *r > 0.0) {
        let full = r.round() as usize;
        let stars: String = (0..5).map(|i| if i < full { '★' } else { '☆' }).collect();
        let count = d.review_count.map(|c| format!(" ({c})")).unwrap_or_default();
        subtitle.push(format!(r#"<span class="rating" title="Average rating: {r:.1} out of 5">{stars}{count}</span>"#));
    }
    let actions = if d.installing {
        r#"<button class="prominent" disabled>Installing</button>"#.to_string()
    } else if d.installed {
        let mut a = String::new();
        if d.can_uninstall {
            a.push_str(r#"<button class="secondary" data-command="uninstall">Uninstall</button>"#);
        }
        if let Some(s) = &d.status {
            a.push_str(&format!(r#"<span class="status">{}</span>"#, esc(s)));
        }
        a
    } else {
        r#"<button class="prominent" data-command="install">Install</button>"#.to_string()
    };
    let details_body = if d.loading {
        r#"<p class="nocontent">Loading…</p>"#.to_string()
    } else {
        match &d.readme {
            Some(md) => markdown_html(md),
            None => r#"<p class="nocontent">No README available.</p>"#.to_string(),
        }
    };
    let changelog_body = match &d.changelog {
        Some(md) => markdown_html(md),
        None if d.loading => r#"<p class="nocontent">Loading…</p>"#.to_string(),
        None => r#"<p class="nocontent">No changelog available.</p>"#.to_string(),
    };
    let mut features = String::new();
    if !d.commands.is_empty() {
        features.push_str(&format!("<h2>Commands ({})</h2><table><tr><th>Title</th><th>ID</th></tr>", d.commands.len()));
        for (title, id) in &d.commands {
            features.push_str(&format!("<tr><td>{}</td><td><code>{}</code></td></tr>", esc(title), esc(id)));
        }
        features.push_str("</table>");
    }
    if !d.settings.is_empty() {
        features.push_str(&format!("<h2>Settings ({})</h2><table><tr><th>ID</th><th>Description</th><th>Default</th></tr>", d.settings.len()));
        for (key, desc, default) in &d.settings {
            features.push_str(&format!(
                "<tr><td><code>{}</code></td><td>{}</td><td><code>{}</code></td></tr>",
                esc(key),
                markdown_html(desc),
                esc(default)
            ));
        }
        features.push_str("</table>");
    }
    if features.is_empty() {
        features = r#"<p class="nocontent">No contributions listed.</p>"#.to_string();
    }
    let mut more = format!(
        r#"<div class="more-info-entry"><div>Identifier</div><div><code>{}</code></div></div><div class="more-info-entry"><div>Version</div><div>{}</div></div>"#,
        esc(&d.id.to_lowercase()),
        esc(&d.version)
    );
    if let Some(u) = &d.last_updated {
        more.push_str(&format!(r#"<div class="more-info-entry"><div>Last Updated</div><div>{}</div></div>"#, esc(u)));
    }
    if let Some(l) = &d.license {
        more.push_str(&format!(r#"<div class="more-info-entry"><div>License</div><div>{}</div></div>"#, esc(l)));
    }
    let categories: String = d.categories.iter().map(|c| format!(r#"<span class="category">{}</span>"#, esc(c))).collect();
    let resources = d
        .repository
        .as_ref()
        .map(|r| format!(r#"<div class="resource"><a href="{}">Repository</a></div>"#, esc(r)))
        .unwrap_or_default();
    let base = d.base_url.as_ref().map(|b| format!(r#"<base href="{}">"#, esc(b))).unwrap_or_default();

    format!(
        r##"<!DOCTYPE html><html><head><meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src https: http: data: vscode-resource:; style-src 'unsafe-inline'; script-src 'unsafe-inline'; font-src https: data:;">
{base}
<style>
{MARKDOWN_CSS}
html, body {{ height: 100%; }}
body {{ padding: 0; max-width: none; margin: 0; color: var(--vscode-foreground); background: var(--vscode-editor-background); font-size: 13px; }}
.extension-editor {{ display: flex; flex-direction: column; margin: 0 auto; max-width: 95%; height: 100%; }}
.header {{ display: flex; padding: 20px 0 12px 20px; font-size: 14px; }}
.icon {{ width: 128px; height: 128px; object-fit: contain; flex-shrink: 0; }}
.icon.placeholder {{ background: var(--vscode-input-background); border-radius: 8px; }}
.details {{ padding-left: 12px; overflow: hidden; }}
.title {{ display: flex; align-items: center; }}
.name {{ font-size: 26px; line-height: 30px; font-weight: 600; white-space: nowrap; }}
.version {{ margin-left: 10px; font-size: 14px; opacity: .8; }}
.subtitle {{ padding-top: 6px; line-height: 20px; display: flex; align-items: center; flex-wrap: wrap; }}
.subtitle > span {{ margin-right: 14px; padding-right: 14px; border-right: 1px solid rgba(128,128,128,.7); }}
.subtitle > span:last-child {{ border-right: none; }}
.verified {{ color: var(--vscode-extensionIcon-verifiedForeground, var(--vscode-textLink-foreground)); margin-right: 4px; }}
.install .count {{ margin-left: 6px; }}
.rating {{ color: var(--vscode-extensionIcon-starForeground, #FF8E00); }}
.description {{ margin-top: 10px; }}
.actions {{ margin-top: 10px; display: flex; align-items: center; gap: 6px; }}
button {{ font: inherit; font-size: 13px; line-height: 22px; padding: 0 8px; border-radius: 4px; cursor: pointer; border: 1px solid var(--vscode-button-border, transparent); }}
button.prominent {{ background: var(--vscode-extensionButton-prominentBackground); color: var(--vscode-extensionButton-prominentForeground); }}
button.prominent:hover {{ background: var(--vscode-extensionButton-prominentHoverBackground); }}
button.secondary {{ background: var(--vscode-button-secondaryBackground, transparent); color: var(--vscode-button-secondaryForeground, var(--vscode-foreground)); }}
button.secondary:hover {{ background: var(--vscode-button-secondaryHoverBackground); }}
button[disabled] {{ opacity: .6; cursor: default; }}
.status {{ font-size: 90%; opacity: .9; margin-left: 4px; }}
.body {{ flex: 1; display: flex; flex-direction: column; min-height: 0; }}
.navbar {{ height: 36px; line-height: 36px; padding-left: 20px; border-bottom: 1px solid var(--vscode-panelSection-border, rgba(128,128,128,.35)); display: flex; }}
.navbar a {{ padding: 0 10px; font-size: 11px; text-transform: uppercase; color: var(--vscode-panelTitle-inactiveForeground); text-decoration: none; cursor: pointer; }}
.navbar a.checked {{ color: var(--vscode-panelTitle-activeForeground); border-bottom: 1px solid var(--vscode-panelTitle-activeBorder); }}
.content {{ flex: 1; display: flex; min-height: 0; }}
.content-container {{ flex: 1; overflow-y: auto; padding: 10px 20px; max-width: 75%; margin: 0 auto; line-height: 22px; }}
.additional {{ width: 25%; min-width: 175px; overflow-y: auto; padding: 12px; box-sizing: border-box; }}
.additional-title {{ font-size: 120%; padding-bottom: 12px; }}
.additional-element {{ padding-bottom: 16px; }}
.additional-element + .additional-element {{ padding-top: 16px; border-top: 1px solid rgba(128,128,128,.22); }}
.more-info-entry {{ font-size: 90%; display: grid; grid-template-columns: 40% 60%; gap: 6px; padding: 2px 4px; }}
.more-info-entry:nth-child(odd) {{ background: rgba(130,130,130,.04); }}
.category {{ display: inline-block; border: 1px solid rgba(136,136,136,.45); padding: 2px 4px; border-radius: 2px; font-size: 90%; margin: 0 6px 3px 0; }}
.resource a {{ padding: 2px 4px; display: block; }}
.nocontent {{ opacity: .8; }}
table {{ border-collapse: collapse; width: 100%; }}
th, td {{ text-align: left; padding: 4px 8px; border-bottom: 1px solid rgba(128,128,128,.22); vertical-align: top; }}
td p {{ margin: 0; }}
.tab {{ display: none; }} .tab.active {{ display: block; }}
</style></head>
<body><div class="extension-editor">
<div class="header">{icon}<div class="details">
<div class="title"><span class="name">{name}</span><span class="version">v{version}</span></div>
<div class="subtitle">{subtitle}</div>
<div class="description">{description}</div>
<div class="actions">{actions}</div>
</div></div>
<div class="body">
<div class="navbar"><a data-tab="details" class="checked">Details</a><a data-tab="features">Features</a><a data-tab="changelog">Changelog</a></div>
<div class="content">
<div class="content-container">
<div class="tab active" id="tab-details">{details_body}</div>
<div class="tab" id="tab-features">{features}</div>
<div class="tab" id="tab-changelog">{changelog_body}</div>
</div>
<div class="additional">
<div class="additional-element"><div class="additional-title">Installation</div>{more}</div>
{categories_block}
{resources_block}
</div>
</div></div></div>
<script>
const vscode = acquireVsCodeApi();
document.querySelectorAll('.navbar a').forEach(a => a.addEventListener('click', () => {{
  document.querySelectorAll('.navbar a').forEach(x => x.classList.toggle('checked', x === a));
  document.querySelectorAll('.tab').forEach(t => t.classList.toggle('active', t.id === 'tab-' + a.dataset.tab));
}}));
document.querySelectorAll('button[data-command]').forEach(b => b.addEventListener('click', () => vscode.postMessage({{ command: b.dataset.command }})));
document.addEventListener('click', e => {{
  const a = e.target.closest('a[href]');
  if (!a) return;
  const href = a.getAttribute('href');
  if (href.startsWith('#')) return;
  e.preventDefault();
  vscode.postMessage({{ command: 'openLink', url: a.href }});
}});
</script>
</body></html>"##,
        name = esc(&d.name),
        version = esc(&d.version),
        subtitle = subtitle.join(""),
        description = esc(&d.description),
        categories_block = if categories.is_empty() {
            String::new()
        } else {
            format!(r#"<div class="additional-element"><div class="additional-title">Categories</div>{categories}</div>"#)
        },
        resources_block = if resources.is_empty() {
            String::new()
        } else {
            format!(r#"<div class="additional-element"><div class="additional-title">Resources</div>{resources}</div>"#)
        },
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
    fn page_has_header_tabs_and_actions() {
        let mut d = DetailsData {
            id: "sample.hello-world".into(),
            name: "Hello World Sample".into(),
            publisher: "sample".into(),
            version: "0.0.1".into(),
            installed: true,
            can_uninstall: true,
            readme: Some("Hello **there**".into()),
            ..Default::default()
        };
        d.read_features(&serde_json::json!({
            "contributes": {
                "commands": [{ "command": "x.run", "title": "Run", "category": "X" }],
                "configuration": { "properties": { "x.flag": { "type": "boolean", "default": true, "description": "A flag" } } }
            },
            "categories": ["Other"]
        }));
        let html = page_html(&d);
        assert!(html.contains("Hello World Sample"));
        assert!(html.contains("<strong>there</strong>"));
        assert!(html.contains("X: Run"));
        assert!(html.contains("x.flag"));
        assert!(html.contains(r#"data-command="uninstall""#));
        assert!(html.contains("sample.hello-world"));
    }
}
