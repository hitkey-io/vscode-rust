//! Reading installed extensions: each folder with a `package.json` in an
//! extensions directory (`~/.vscode/extensions` and friends).
//!
//! VS Code analogues:
//! - `src/vs/platform/extensions/common/extensions.ts` (`IExtensionManifest`)
//! - `src/vs/platform/extensionManagement/node/extensionsScanner.ts`
//! - `src/vs/platform/extensionManagement/common/extensionNls.ts` (`%key%`
//!   placeholders resolved from `package.nls.json`)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// A command an extension contributes (`contributes.commands`).
#[derive(Clone, Debug, PartialEq)]
pub struct ContributedCommand {
    pub id: String,
    pub title: String,
    pub category: String,
    /// Hidden from the command palette (`menus.commandPalette` with
    /// `"when": "false"`).
    pub hidden: bool,
}

/// One installed extension.
#[derive(Clone, Debug)]
pub struct Extension {
    /// `publisher.name`.
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    /// Folder holding `package.json`.
    pub path: PathBuf,
    /// Node entry point, relative to `path` (`main`); `None` for
    /// declarative-only extensions (themes, grammars, snippets…).
    pub main: Option<String>,
    pub activation_events: Vec<String>,
    pub commands: Vec<ContributedCommand>,
    /// `contributes.configuration` defaults, by full dotted key.
    pub config_defaults: BTreeMap<String, Value>,
    /// Required editor version (`engines.vscode`).
    pub engine: String,
    /// The raw manifest, handed to the extension as `packageJSON`.
    pub manifest: Value,
}

impl Extension {
    /// Read the extension in `dir`. `None` when there's no readable
    /// `package.json` or it lacks a name or publisher.
    pub fn read(dir: &Path) -> Option<Self> {
        let raw = std::fs::read_to_string(dir.join("package.json")).ok()?;
        let manifest: Value = serde_json::from_str(&raw).ok()?;
        let nls = read_nls(dir);
        let text = |v: Option<&Value>| -> String {
            let s = v.and_then(Value::as_str).unwrap_or_default();
            localize(s, &nls)
        };

        let name = manifest.get("name")?.as_str()?.to_string();
        let publisher = manifest.get("publisher")?.as_str()?.to_string();
        let display_name = {
            let d = text(manifest.get("displayName"));
            if d.is_empty() { name.clone() } else { d }
        };

        let contributes = manifest.get("contributes");
        let hidden_in_palette: Vec<String> = contributes
            .and_then(|c| c.pointer("/menus/commandPalette"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|i| i.get("when").and_then(Value::as_str) == Some("false"))
                    .filter_map(|i| i.get("command").and_then(Value::as_str).map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let commands = contributes
            .and_then(|c| c.get("commands"))
            .map(|c| match c {
                Value::Array(items) => items.clone(),
                single @ Value::Object(_) => vec![single.clone()],
                _ => Vec::new(),
            })
            .unwrap_or_default()
            .iter()
            .filter_map(|c| {
                let id = c.get("command")?.as_str()?.to_string();
                Some(ContributedCommand {
                    hidden: hidden_in_palette.contains(&id),
                    title: text(c.get("title")),
                    category: text(c.get("category")),
                    id,
                })
            })
            .collect();

        let mut config_defaults = BTreeMap::new();
        if let Some(cfg) = contributes.and_then(|c| c.get("configuration")) {
            let sections = match cfg {
                Value::Array(items) => items.clone(),
                other => vec![other.clone()],
            };
            for section in sections {
                if let Some(props) = section.get("properties").and_then(Value::as_object) {
                    for (key, schema) in props {
                        if let Some(default) = schema.get("default") {
                            config_defaults.insert(key.clone(), default.clone());
                        }
                    }
                }
            }
        }

        Some(Self {
            id: format!("{publisher}.{name}"),
            description: text(manifest.get("description")),
            version: manifest.get("version").and_then(Value::as_str).unwrap_or("0.0.0").to_string(),
            main: manifest.get("main").and_then(Value::as_str).map(str::to_string),
            activation_events: manifest
                .get("activationEvents")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default(),
            engine: manifest
                .pointer("/engines/vscode")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            path: dir.to_path_buf(),
            name,
            publisher,
            display_name,
            commands,
            config_defaults,
            manifest,
        })
    }

    /// Commands shown in the command palette, as "Category: Title".
    pub fn palette_commands(&self) -> impl Iterator<Item = &ContributedCommand> {
        self.commands.iter().filter(|c| !c.hidden && !c.title.is_empty())
    }
}

/// Every extension in `dirs`, one per id (the highest version wins, as when
/// several versions sit side by side before cleanup), sorted by display name.
/// Folders listed in a directory's `.obsolete` file are skipped, like VS
/// Code does for uninstalled-but-not-yet-deleted extensions.
pub fn scan(dirs: &[PathBuf]) -> Vec<Extension> {
    let mut by_id: BTreeMap<String, Extension> = BTreeMap::new();
    for dir in dirs {
        let obsolete: Vec<String> = std::fs::read_to_string(dir.join(".obsolete"))
            .ok()
            .and_then(|s| serde_json::from_str::<BTreeMap<String, Value>>(&s).ok())
            .map(|m| m.into_keys().collect())
            .unwrap_or_default();
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let folder = entry.file_name().to_string_lossy().into_owned();
            if !path.is_dir() || folder.starts_with('.') || obsolete.contains(&folder) {
                continue;
            }
            let Some(ext) = Extension::read(&path) else { continue };
            let key = ext.id.to_lowercase();
            let newer = by_id
                .get(&key)
                .is_none_or(|old| version_key(&ext.version) > version_key(&old.version));
            if newer {
                by_id.insert(key, ext);
            }
        }
    }
    let mut list: Vec<Extension> = by_id.into_values().collect();
    list.sort_by_key(|e| e.display_name.to_lowercase());
    list
}

/// Where extensions are looked for by default: the user's VS Code
/// extensions, then this editor's own folder.
pub fn default_dirs() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    vec![home.join(".vscode/extensions"), home.join(".vscode-rust/extensions")]
}

fn version_key(v: &str) -> Vec<u64> {
    v.split(|c: char| c == '.' || c == '-')
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

fn read_nls(dir: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(dir.join("package.nls.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<BTreeMap<String, Value>>(&s).ok())
        .map(|m| {
            m.into_iter()
                .filter_map(|(k, v)| {
                    let text = match v {
                        Value::String(s) => s,
                        // { "message": "...", "comment": [...] }
                        Value::Object(o) => o.get("message")?.as_str()?.to_string(),
                        _ => return None,
                    };
                    Some((k, text))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Resolve a `%key%` placeholder through `package.nls.json`.
fn localize(s: &str, nls: &BTreeMap<String, String>) -> String {
    if s.len() > 2 && s.starts_with('%') && s.ends_with('%') {
        if let Some(t) = nls.get(&s[1..s.len() - 1]) {
            return t.clone();
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/extensions")
    }

    #[test]
    fn reads_fixture_manifest_with_nls() {
        let list = scan(&[fixtures()]);
        let hello = list.iter().find(|e| e.id == "sample.hello-world").expect("fixture");
        assert_eq!(hello.display_name, "Hello World Sample");
        assert_eq!(hello.main.as_deref(), Some("./extension.js"));
        let cmd = hello.commands.iter().find(|c| c.id == "helloWorld.sayHello").unwrap();
        assert_eq!(cmd.title, "Say Hello");
        assert_eq!(cmd.category, "Hello");
        assert!(hello.commands.iter().any(|c| c.id == "helloWorld.internal" && c.hidden));
        assert_eq!(
            hello.config_defaults.get("helloWorld.greeting").and_then(Value::as_str),
            Some("Hello")
        );
    }

    #[test]
    fn version_ordering() {
        assert!(version_key("1.10.0") > version_key("1.9.3"));
    }
}
