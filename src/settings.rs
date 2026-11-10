//! User settings: `<user data>/User/settings.json`, as VS Code keeps it
//! (`--user-data-dir` picks another folder). Only the values the person set
//! are stored; every reader brings its own default.

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// The default user data folder: `~/Library/Application Support/vscode-rust`
/// on macOS (VS Code: `.../Code`), `~/.config/vscode-rust` elsewhere.
pub fn default_user_data_dir() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    Some(if cfg!(target_os = "macos") {
        home.join("Library/Application Support/vscode-rust")
    } else {
        home.join(".config/vscode-rust")
    })
}

#[derive(Debug, Default)]
pub struct Settings {
    /// `None`: kept in memory only (tests).
    path: Option<PathBuf>,
    values: Map<String, Value>,
}

impl Settings {
    /// Settings that are never written to disk.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Reads `<user_data_dir>/User/settings.json` (missing or broken files
    /// read as empty, like VS Code).
    pub fn load(user_data_dir: &Path) -> Self {
        let path = user_data_dir.join("User").join("settings.json");
        let values = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| crate::jsonc::parse(&t))
            .and_then(|v| match v {
                Value::Object(m) => Some(m),
                _ => None,
            })
            .unwrap_or_default();
        Self { path: Some(path), values }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }

    /// Every value the person set.
    pub fn values(&self) -> &Map<String, Value> {
        &self.values
    }

    /// Sets (or with `None` removes) a value and writes the file.
    pub fn update(&mut self, key: &str, value: Option<Value>) -> std::io::Result<()> {
        match value {
            Some(v) => {
                self.values.insert(key.to_string(), v);
            }
            None => {
                self.values.remove(key);
            }
        }
        self.save(key)
    }

    /// Re-reads the file (after it was edited in an editor tab).
    pub fn reload(&mut self) {
        if let Some(path) = &self.path {
            if let Some(dir) = path.parent().and_then(Path::parent) {
                self.values = Settings::load(dir).values;
            }
        }
    }

    fn save(&self, changed: &str) -> std::io::Result<()> {
        let Some(path) = &self.path else { return Ok(()) };
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        let text = edit_json(&existing, changed, self.values.get(changed));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, text)
    }
}

/// Sets one top-level property in a JSONC document, keeping the rest of the
/// text (comments, order, formatting) as it is, the way VS Code's
/// `jsonEdit.setProperty` does for settings.json. Tab indentation.
pub fn edit_json(text: &str, key: &str, value: Option<&Value>) -> String {
    let rendered = value.map(|v| indent_value(v));
    let Some((open, close)) = top_level_braces(text) else {
        return match rendered {
            Some(v) => format!("{{\n\t{}: {}\n}}", Value::String(key.into()), v),
            None => "{}".into(),
        };
    };
    let members = top_level_members(text, open, close);
    if let Some(m) = members.iter().position(|m| m.key == key) {
        let member = &members[m];
        return match rendered {
            Some(v) => format!("{}{}{}", &text[..member.value_start], v, &text[member.value_end..]),
            None => {
                // Remove the member with its comma and its line.
                if members.len() == 1 {
                    let inner = format!(
                        "{}{}",
                        &text[open + 1..line_start(text, member.key_start).max(open + 1)],
                        &text[member.value_end..close]
                    );
                    if inner.trim().is_empty() {
                        format!("{}{{}}{}", &text[..open], &text[close + 1..])
                    } else {
                        format!("{}{}{}", &text[..open + 1], inner, &text[close..])
                    }
                } else if m + 1 < members.len() {
                    let (start, end) = (line_start(text, member.key_start), line_start(text, members[m + 1].key_start));
                    format!("{}{}", &text[..start], &text[end..])
                } else {
                    format!("{}{}", &text[..members[m - 1].value_end], &text[member.value_end..])
                }
            }
        };
    }
    let Some(v) = rendered else { return text.to_string() };
    let entry = format!("\t{}: {}", Value::String(key.into()), v);
    match members.last() {
        Some(last) => format!("{},\n{}{}", &text[..last.value_end], entry, &text[last.value_end..]),
        None => format!("{}\n{}\n{}", &text[..open + 1], entry, &text[close..]),
    }
}

fn indent_value(v: &Value) -> String {
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b"\t");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    serde::Serialize::serialize(v, &mut ser).ok();
    String::from_utf8(buf).unwrap_or_default().replace('\n', "\n\t")
}

fn line_start(text: &str, i: usize) -> usize {
    text[..i].rfind('\n').map_or(0, |p| p + 1)
}

struct Member {
    key: String,
    key_start: usize,
    value_start: usize,
    value_end: usize,
}

/// Byte offsets of the outer `{` and its `}`, skipping comments and strings.
fn top_level_braces(text: &str) -> Option<(usize, usize)> {
    let tokens = scan(text);
    let open = tokens.iter().find(|(_, c)| *c == '{')?.0;
    let mut depth = 0;
    for &(i, c) in &tokens {
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some((open, i));
                }
            }
            _ => {}
        }
    }
    None
}

/// Structural characters outside strings and comments, plus `"` at the
/// start of each string.
fn scan(text: &str) -> Vec<(usize, char)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => {
                out.push((i, '"'));
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 1;
            }
            c @ (b'{' | b'}' | b'[' | b']' | b',' | b':') => out.push((i, c as char)),
            _ => {}
        }
        i += 1;
    }
    out
}

fn top_level_members(text: &str, open: usize, close: usize) -> Vec<Member> {
    let tokens: Vec<(usize, char)> = scan(text).into_iter().filter(|(i, _)| *i > open && *i < close).collect();
    let mut members = Vec::new();
    let mut depth = 0;
    let mut k = 0;
    while k < tokens.len() {
        let (i, c) = tokens[k];
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            '"' if depth == 0 => {
                // key string, then ':' then the value up to the next top-level ',' or the end
                let key_end = text[i + 1..].find('"').map(|p| i + 1 + p).unwrap_or(i);
                let key = serde_json::from_str::<String>(&text[i..=key_end]).unwrap_or_default();
                let Some(colon) = tokens[k..].iter().position(|t| t.1 == ':').map(|p| k + p) else { break };
                let mut d = 0;
                let mut end_tok = tokens.len();
                for (j, &(_, c2)) in tokens.iter().enumerate().skip(colon + 1) {
                    match c2 {
                        '{' | '[' => d += 1,
                        '}' | ']' => d -= 1,
                        ',' if d == 0 => {
                            end_tok = j;
                            break;
                        }
                        _ => {}
                    }
                }
                let value_start = tokens[colon].0 + 1;
                let raw_end = tokens.get(end_tok).map_or(close, |t| t.0);
                let value_text = strip_trailing_comment(&text[value_start..raw_end]);
                let lead = value_text.len() - value_text.trim_start().len();
                members.push(Member {
                    key,
                    key_start: i,
                    value_start: value_start + lead,
                    value_end: value_start + value_text.trim_end().len(),
                });
                k = end_tok;
                continue;
            }
            _ => {}
        }
        k += 1;
    }
    members
}

/// The value text without comments that follow it.
fn strip_trailing_comment(s: &str) -> &str {
    let stripped = crate::jsonc::strip(s);
    let len = stripped.trim_end().len();
    // `strip` only removes characters, so the value's own text is a prefix
    // of `s` as long as no comment sits inside it (values here are simple).
    let mut end = 0;
    let mut matched = 0;
    for (i, ch) in s.char_indices() {
        if matched >= len {
            break;
        }
        if stripped[matched..].starts_with(ch) {
            matched += ch.len_utf8();
            end = i + ch.len_utf8();
        }
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn edits_keep_the_rest_of_the_file() {
        assert_eq!(edit_json("", "a", Some(&json!(1))), "{\n\t\"a\": 1\n}");
        assert_eq!(edit_json("{}", "a", Some(&json!("x"))), "{\n\t\"a\": \"x\"\n}");
        let text = "{\n\t// mine\n\t\"a\": 1,\n\t\"b\": [1, 2] // note\n}";
        assert_eq!(
            edit_json(text, "c", Some(&json!(true))),
            "{\n\t// mine\n\t\"a\": 1,\n\t\"b\": [1, 2],\n\t\"c\": true // note\n}"
        );
        assert_eq!(edit_json(text, "a", Some(&json!(2))), "{\n\t// mine\n\t\"a\": 2,\n\t\"b\": [1, 2] // note\n}");
        assert_eq!(edit_json(text, "a", None), "{\n\t// mine\n\t\"b\": [1, 2] // note\n}");
        assert_eq!(edit_json("{\n\t\"a\": 1\n}", "a", None), "{}");
    }

    #[test]
    fn load_and_update() {
        let dir = std::env::temp_dir().join(format!("vscode-rust-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = Settings::load(&dir);
        assert!(s.get("workbench.colorTheme").is_none());
        s.update("workbench.colorTheme", Some(json!("Light 2026"))).unwrap();
        let again = Settings::load(&dir);
        assert_eq!(again.get_str("workbench.colorTheme"), Some("Light 2026"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
