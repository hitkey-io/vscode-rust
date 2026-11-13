//! The Keyboard Shortcuts editor's model (services/preferences/browser/
//! keybindingsEditorModel.ts): every default keybinding plus every unbound
//! command, labelled "Category: Title", sorted like `compareKeybindingData`
//! and searched like `KeybindingItemMatches` (filters.ts `matchesWords`,
//! `matchesCamelCase`, contiguous substrings; keys by modifier names and
//! key labels, `"…"` for an exact keybinding; `@command:`, `@source:`,
//! `@ext:`, `@keybinding:`).
//!
//! Data: `assets/keybindings/default.json` (VS Code's default keybindings
//! file, dumped by tools/dump-settings) and `commands.json` (titles and
//! categories, tools/commands.mjs). User keybindings (`keybindings.json`)
//! are merged like `KeybindingResolver`: an entry adds a binding, `-command`
//! removes the default one with that key (and `when`).

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::sync::OnceLock;

use serde_json::Value;

use crate::workbench::settings_editor::model::locale_compare;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chord {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
    /// The key as the UI draws it (`K`, `↑`, `Escape`).
    pub key: String,
    /// The key as screen readers hear it (`K`, `UpArrow`).
    pub aria_key: String,
}

impl Chord {
    /// The keys drawn as boxes: modifiers in VS Code's order (⌃ ⇧ ⌥ ⌘),
    /// then the key.
    pub fn parts(&self) -> Vec<(KeyPart, String)> {
        let mut out = Vec::new();
        if self.ctrl {
            out.push((KeyPart::Ctrl, "⌃".to_string()));
        }
        if self.shift {
            out.push((KeyPart::Shift, "⇧".to_string()));
        }
        if self.alt {
            out.push((KeyPart::Alt, "⌥".to_string()));
        }
        if self.meta {
            out.push((KeyPart::Meta, "⌘".to_string()));
        }
        out.push((KeyPart::Key, self.key.clone()));
        out
    }

    fn aria(&self) -> String {
        let mut s = Vec::new();
        if self.ctrl {
            s.push("Control");
        }
        if self.shift {
            s.push("Shift");
        }
        if self.alt {
            s.push("Option");
        }
        if self.meta {
            s.push("Command");
        }
        s.push(&self.aria_key);
        s.join("+")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyPart {
    Ctrl,
    Shift,
    Alt,
    Meta,
    Key,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keybinding {
    /// As written in keybindings.json (`cmd+k cmd+c`).
    pub raw: String,
    pub chords: Vec<Chord>,
}

impl Keybinding {
    pub fn parse(raw: &str) -> Option<Keybinding> {
        let mut chords = Vec::new();
        for part in raw.split_whitespace() {
            let mut c = Chord::default();
            let lower = part.to_lowercase();
            // The key may itself be `+` (`cmd+=` is fine, `ctrl++` isn't used).
            let mut pieces: Vec<&str> = lower.split('+').collect();
            let key = if lower.ends_with("++") {
                pieces.truncate(pieces.len() - 2);
                "+"
            } else {
                pieces.pop()?
            };
            for m in pieces {
                match m {
                    "ctrl" => c.ctrl = true,
                    "shift" => c.shift = true,
                    "alt" | "option" => c.alt = true,
                    "cmd" | "meta" | "win" | "super" => c.meta = true,
                    _ => return None,
                }
            }
            if key.is_empty() {
                return None;
            }
            c.key = key_label(key, true);
            c.aria_key = key_label(key, false);
            chords.push(c);
        }
        if chords.is_empty() {
            return None;
        }
        Some(Keybinding { raw: normalise(raw), chords })
    }

    /// `getLabel()`: `⌘K ⌘C`.
    pub fn label(&self) -> String {
        self.chords.iter().map(|c| c.parts().into_iter().map(|(_, s)| s).collect::<String>()).collect::<Vec<_>>().join(" ")
    }

    /// `getAriaLabel()`: `Command+K Command+C`.
    pub fn aria_label(&self) -> String {
        self.chords.iter().map(Chord::aria).collect::<Vec<_>>().join(" ")
    }
}

/// keybindings.json spelling with modifiers in VS Code's order.
fn normalise(raw: &str) -> String {
    raw.split_whitespace()
        .map(|part| {
            let lower = part.to_lowercase();
            let (mods, key) = match lower.rfind('+') {
                Some(i) if i + 1 < lower.len() => (&lower[..i], &lower[i + 1..]),
                _ => ("", lower.as_str()),
            };
            let has = |m: &[&str]| mods.split('+').any(|p| m.contains(&p));
            let mut s = String::new();
            for (names, out) in [(&["ctrl"][..], "ctrl+"), (&["shift"][..], "shift+"), (&["alt", "option"][..], "alt+"), (&["cmd", "meta", "win", "super"][..], "cmd+")] {
                if has(names) {
                    s.push_str(out);
                }
            }
            s.push_str(key);
            s
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `KeyCodeUtils.toString` names (the UI uses arrows on macOS).
fn key_label(key: &str, ui: bool) -> String {
    let named = match key {
        "left" => if ui { "←" } else { "LeftArrow" },
        "right" => if ui { "→" } else { "RightArrow" },
        "up" => if ui { "↑" } else { "UpArrow" },
        "down" => if ui { "↓" } else { "DownArrow" },
        "enter" => "Enter",
        "escape" => "Escape",
        "tab" => "Tab",
        "space" => "Space",
        "backspace" => "Backspace",
        "delete" => if ui { "Del" } else { "Delete" },
        "insert" => "Insert",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        "browserback" => "BrowserBack",
        "browserforward" => "BrowserForward",
        "capslock" => "CapsLock",
        "pausebreak" => "PauseBreak",
        "contextmenu" => "ContextMenu",
        "numpad_add" => "NumPad_Add",
        "numpad_subtract" => "NumPad_Subtract",
        "numpad_multiply" => "NumPad_Multiply",
        "numpad_divide" => "NumPad_Divide",
        "numpad_decimal" => "NumPad_Decimal",
        "numpad_separator" => "NumPad_Separator",
        _ => "",
    };
    if !named.is_empty() {
        return named.to_string();
    }
    if let Some(n) = key.strip_prefix("numpad") {
        return format!("NumPad{n}");
    }
    if key.len() > 1 && key.starts_with('f') && key[1..].chars().all(|c| c.is_ascii_digit()) {
        return key.to_uppercase();
    }
    key.to_uppercase()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    System,
    User,
    /// A built-in extension, by display name.
    Extension(String),
}

impl Source {
    pub fn label(&self) -> &str {
        match self {
            Source::System => "System",
            Source::User => "User",
            Source::Extension(name) => name,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Item {
    pub command: String,
    /// "Category: Title", or empty when the command has no title (the id
    /// is shown instead).
    pub label: String,
    pub key: Option<Keybinding>,
    pub when: String,
    pub source: Source,
}

impl Item {
    /// `KeybindingsEditorModel.getId`.
    pub fn id(&self) -> String {
        format!("{}{}{}{}", self.command, self.key.as_ref().map(|k| k.aria_label()).unwrap_or_default(), self.when, self.source.label())
    }
}

struct Data {
    labels: HashMap<String, String>,
    /// key, command, when, source (an extension's display name).
    default: Vec<(String, String, String, Option<String>)>,
    unbound: Vec<(String, Option<String>)>,
}

fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        let commands: Value = serde_json::from_str(include_str!("../../../assets/keybindings/commands.json")).unwrap_or(Value::Null);
        let mut labels = HashMap::new();
        if let Some(map) = commands.as_object() {
            for (id, c) in map {
                let label = match c.get("label").and_then(Value::as_str) {
                    Some(l) => l.to_string(),
                    None => {
                        let title = c.get("title").and_then(Value::as_str).unwrap_or_default();
                        match c.get("category").and_then(Value::as_str) {
                            Some(cat) if !cat.is_empty() => format!("{cat}: {title}"),
                            _ => title.to_string(),
                        }
                    }
                };
                labels.insert(id.clone(), label);
            }
        }
        let default: Value = serde_json::from_str(include_str!("../../../assets/keybindings/default.json")).unwrap_or(Value::Null);
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
        let bindings = default
            .get("bindings")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|b| (s(b, "key"), s(b, "command"), user_file_when(&s(b, "when")), b.get("source").and_then(Value::as_str).map(str::to_string))).collect())
            .unwrap_or_default();
        let unbound = default
            .get("unbound")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|u| match u {
                        Value::String(id) => Some((id.clone(), None)),
                        Value::Array(p) => Some((p.first()?.as_str()?.to_string(), p.get(1).and_then(Value::as_str).map(str::to_string))),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Data { labels, default: bindings, unbound }
    })
}

/// The dump's own keybindings.json in a `when` (Define Keybinding's
/// `resource == 'vscode-userdata:…'`) becomes this app's.
fn user_file_when(when: &str) -> String {
    const DUMPED: &str = "vscode-userdata:/tmp/dump-profile/User/keybindings.json";
    if !when.contains(DUMPED) {
        return when.to_string();
    }
    let ours = crate::settings::default_user_data_dir().map(|d| d.join("User").join("keybindings.json").display().to_string()).unwrap_or_default();
    when.replace(DUMPED, &format!("vscode-userdata:{ours}"))
}

/// The command's "Category: Title" (empty when it has none).
pub fn command_label(command: &str) -> String {
    data().labels.get(command).cloned().unwrap_or_default()
}

fn source(ext: &Option<String>) -> Source {
    ext.clone().map(Source::Extension).unwrap_or(Source::System)
}

/// A user keybinding (`keybindings.json` entry).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserBinding {
    pub key: String,
    pub command: String,
    pub when: Option<String>,
}

/// Parses keybindings.json (JSONC).
pub fn parse_user(text: &str) -> Vec<UserBinding> {
    let Some(v) = crate::jsonc::parse(text) else { return Vec::new() };
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|b| {
                    Some(UserBinding {
                        key: b.get("key")?.as_str()?.to_string(),
                        command: b.get("command")?.as_str()?.to_string(),
                        when: b.get("when").and_then(Value::as_str).map(str::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Every item in precedence order (`resolve`): the default keybindings with
/// the user's applied, then each command left without a keybinding.
pub fn items_by_precedence(user: &[UserBinding]) -> Vec<Item> {
    let d = data();
    let mut bound: Vec<(String, String, String, Source)> =
        d.default.iter().map(|(k, c, w, e)| (k.clone(), c.clone(), w.clone(), source(e))).collect();
    let mut user_items = Vec::new();
    for u in user {
        if let Some(cmd) = u.command.strip_prefix('-') {
            let key = Keybinding::parse(&u.key).map(|k| k.raw);
            bound.retain(|(k, c, w, _)| {
                !(c == cmd
                    && (u.key.is_empty() || Keybinding::parse(k).map(|k| k.raw) == key)
                    && u.when.as_ref().is_none_or(|uw| uw == w))
            });
        } else {
            user_items.push((u.key.clone(), u.command.clone(), u.when.clone().unwrap_or_default(), Source::User));
        }
    }
    // User keybindings take precedence (they come last in the resolver, and
    // the list shows the highest first).
    let mut all: Vec<(String, String, String, Source)> = user_items.into_iter().rev().collect();
    all.extend(bound);
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut bound_commands = HashSet::new();
    for (k, c, w, src) in all {
        let Some(key) = Keybinding::parse(&k) else { continue };
        let item = Item { label: command_label(&c), command: c.clone(), key: Some(key), when: w, source: src };
        if seen.insert(item.id()) {
            bound_commands.insert(c);
            out.push(item);
        }
    }
    for (c, ext) in &d.unbound {
        if bound_commands.contains(c) {
            continue;
        }
        let item = Item { label: command_label(c), command: c.clone(), key: None, when: String::new(), source: source(ext) };
        if seen.insert(item.id()) {
            out.push(item);
        }
    }
    // Defaults removed by the user stay listed without a keybinding.
    for (_, c, _, ext) in &d.default {
        if !bound_commands.contains(c) && !d.unbound.iter().any(|(u, _)| u == c) {
            let item = Item { label: command_label(c), command: c.clone(), key: None, when: String::new(), source: source(ext) };
            if seen.insert(item.id()) {
                bound_commands.insert(c.clone());
                out.push(item);
            }
        }
    }
    out
}

/// `compareKeybindingData`: bound first, titled first, by label, then id.
pub fn compare(a: &Item, b: &Item) -> Ordering {
    match (a.key.is_some(), b.key.is_some()) {
        (true, false) => return Ordering::Less,
        (false, true) => return Ordering::Greater,
        _ => {}
    }
    match (a.label.is_empty(), b.label.is_empty()) {
        (false, true) => return Ordering::Less,
        (true, false) => return Ordering::Greater,
        _ => {}
    }
    if !a.label.is_empty() && a.label != b.label {
        return locale_compare(&a.label, &b.label);
    }
    if a.command == b.command {
        return Ordering::Equal;
    }
    locale_compare(&a.command, &b.command)
}

/// The keys of a chord a search matched.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyMatch {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
    pub key: bool,
}

impl KeyMatch {
    fn any(&self) -> bool {
        self.ctrl || self.shift || self.alt || self.meta || self.key
    }

    pub fn has(&self, part: KeyPart) -> bool {
        match part {
            KeyPart::Ctrl => self.ctrl,
            KeyPart::Shift => self.shift,
            KeyPart::Alt => self.alt,
            KeyPart::Meta => self.meta,
            KeyPart::Key => self.key,
        }
    }

    fn complete(c: &Chord) -> KeyMatch {
        KeyMatch { ctrl: c.ctrl, shift: c.shift, alt: c.alt, meta: c.meta, key: true }
    }
}

/// One row of the table: an item and what the search matched in it
/// (character ranges).
#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub index: usize,
    pub label: Vec<Range<usize>>,
    pub command_id: Option<Vec<Range<usize>>>,
    pub when: Vec<Range<usize>>,
    pub source: Vec<Range<usize>>,
    /// Per chord.
    pub keys: Vec<KeyMatch>,
}

impl Entry {
    fn plain(index: usize) -> Entry {
        Entry { index, ..Default::default() }
    }

    /// `Delegate.getHeight`: the command id under the label adds a line.
    pub fn height(&self, item: &Item) -> f32 {
        if self.command_id.is_some() && !item.label.is_empty() {
            40.0
        } else {
            24.0
        }
    }
}

// ── filters.ts ──────────────────────────────────────────────────────────

fn is_separator(c: char) -> bool {
    c.is_whitespace() || "()[]{}<>`'\"-/;:,.?!".contains(c)
}

fn chars_match(a: char, b: char) -> bool {
    a == b || (is_separator(a) && is_separator(b))
}

fn next_word(t: &[char], start: usize) -> usize {
    (start..t.len()).find(|&i| is_separator(t[i]) || (i > 0 && is_separator(t[i - 1]))).unwrap_or(t.len())
}

fn join(head: Range<usize>, mut tail: Vec<Range<usize>>) -> Vec<Range<usize>> {
    if tail.is_empty() {
        tail.push(head);
    } else if head.end == tail[0].start {
        tail[0].start = head.start;
    } else {
        tail.insert(0, head);
    }
    tail
}

fn base(s: &str) -> Vec<char> {
    s.to_lowercase().chars().collect()
}

/// `matchesWords`.
pub fn matches_words(word: &str, target: &str, contiguous: bool) -> Option<Vec<Range<usize>>> {
    if target.is_empty() {
        return None;
    }
    let w = base(word);
    let t = base(target);
    if w.len() > 64 {
        return None;
    }
    fn go(w: &[char], t: &[char], wi: usize, ti: usize, contiguous: bool, memo: &mut HashMap<(usize, usize), Option<Vec<Range<usize>>>>) -> Option<Vec<Range<usize>>> {
        if wi == w.len() {
            return Some(Vec::new());
        }
        if ti == t.len() {
            return None;
        }
        if let Some(r) = memo.get(&(wi, ti)) {
            return r.clone();
        }
        let r = (|| {
            if !chars_match(w[wi], t[ti]) {
                return None;
            }
            let mut next = ti + 1;
            let mut result = go(w, t, wi + 1, next, contiguous, memo);
            if !contiguous {
                while result.is_none() {
                    next = next_word(t, next);
                    if next >= t.len() {
                        break;
                    }
                    result = go(w, t, wi + 1, next, contiguous, memo);
                    next += 1;
                }
            }
            let result = result?;
            if w[wi] != t[ti] {
                return Some(result);
            }
            Some(join(ti..ti + 1, result))
        })();
        memo.insert((wi, ti), r.clone());
        r
    }
    let mut memo = HashMap::new();
    let mut ti = 0;
    while ti < t.len() {
        if let Some(r) = go(&w, &t, 0, ti, contiguous, &mut memo) {
            return Some(r);
        }
        ti = next_word(&t, ti + 1);
    }
    None
}

/// `matchesBaseContiguousSubString` / `matchesContiguousSubString`.
pub fn matches_contiguous(word: &str, target: &str) -> Option<Vec<Range<usize>>> {
    let w = base(word);
    let t = base(target);
    if w.is_empty() || w.len() > t.len() {
        return if w.is_empty() { Some(Vec::new()) } else { None };
    }
    (0..=t.len() - w.len()).find(|&i| t[i..i + w.len()] == w[..]).map(|i| vec![i..i + w.len()])
}

/// `matchesCamelCase`.
pub fn matches_camel_case(word: &str, camel: &str) -> Option<Vec<Range<usize>>> {
    let camel = camel.trim();
    if camel.is_empty() {
        return None;
    }
    let wc: Vec<char> = word.chars().collect();
    let (upper, lower, ws) = wc.iter().fold((0, 0, 0), |(u, l, s), c| {
        (u + c.is_ascii_uppercase() as usize, l + c.is_ascii_lowercase() as usize, s + c.is_whitespace() as usize)
    });
    let pattern = if (upper == 0 || lower == 0) && ws == 0 { wc.len() <= 30 } else { upper <= 5 };
    if !pattern {
        return None;
    }
    let mut cw: Vec<char> = camel.chars().take(60).collect();
    let n = cw.len() as f32;
    let count = |f: fn(&char) -> bool| cw.iter().filter(|c| f(c)).count() as f32 / n;
    let (up, lo, alpha, num) = (count(char::is_ascii_uppercase), count(char::is_ascii_lowercase), count(char::is_ascii_alphanumeric), count(char::is_ascii_digit));
    let is_camel = lo > 0.2 && up < 0.8 && alpha > 0.6 && num < 0.2;
    if !is_camel {
        if !(lo == 0.0 && up > 0.6) {
            return None;
        }
        cw = cw.iter().flat_map(|c| c.to_lowercase()).collect();
    }
    let w: Vec<char> = word.to_lowercase().chars().collect();
    fn anchor(cw: &[char], start: usize) -> usize {
        (start..cw.len())
            .find(|&i| cw[i].is_ascii_uppercase() || cw[i].is_ascii_digit() || (i > 0 && !cw[i - 1].is_ascii_alphanumeric()))
            .unwrap_or(cw.len())
    }
    fn go(w: &[char], cw: &[char], i: usize, j: usize) -> Option<Vec<Range<usize>>> {
        if i == w.len() {
            return Some(Vec::new());
        }
        if j == cw.len() || cw[j].to_lowercase().next() != Some(w[i]) {
            return None;
        }
        let mut result = go(w, cw, i + 1, j + 1);
        let mut next = j + 1;
        while result.is_none() {
            next = anchor(cw, next);
            if next >= cw.len() {
                break;
            }
            result = go(w, cw, i + 1, next);
            next += 1;
        }
        result.map(|r| join(j..j + 1, r))
    }
    let mut i = 0;
    while i < cw.len() {
        if let Some(r) = go(&w, &cw, 0, i) {
            return Some(r);
        }
        i = anchor(&cw, i + 1);
    }
    None
}

/// `KeybindingItemMatches.matches`: the whole search as a contiguous
/// substring or by words, else every word by `filter`.
fn matches(search: Option<&str>, target: &str, filter: &dyn Fn(&str, &str) -> Option<Vec<Range<usize>>>, words: &[&str]) -> Option<Vec<Range<usize>>> {
    let mut m = search.and_then(|s| matches_contiguous(s, target).or_else(|| matches_words(s, target, false)));
    if m.is_none() {
        let mut all = Vec::new();
        for w in words {
            all.extend(filter(w, target)?);
        }
        m = Some(all);
    }
    m.map(|mut v| {
        // `filterAndSort`: distinct, not inside another, by start.
        v.sort_by_key(|r| (r.start, r.end));
        v.dedup();
        let keep: Vec<Range<usize>> = v
            .iter()
            .filter(|r| !v.iter().any(|o| o != *r && o.start <= r.start && o.end >= r.end))
            .cloned()
            .collect();
        keep
    })
}

fn word_is(word: &str, names: &[&str]) -> bool {
    names.iter().any(|n| n.eq_ignore_ascii_case(word) || *n == word)
}
const META: &[&str] = &["⌘", "Command", "cmd", "meta"];
const CTRL: &[&str] = &["⌃", "Control", "ctrl"];
const SHIFT: &[&str] = &["⇧", "Shift", "shift"];
const ALT: &[&str] = &["⌥", "Option", "alt", "option"];

fn is_modifier(word: &str) -> bool {
    word_is(word, META) || word_is(word, CTRL) || word_is(word, SHIFT) || word_is(word, ALT)
}

fn match_part(chord: Option<&Chord>, m: &mut KeyMatch, word: &str, complete: bool) -> bool {
    let Some(c) = chord else { return false };
    let mut matched = false;
    if c.meta && word_is(word, META) {
        m.meta = true;
        matched = true;
    }
    if c.ctrl && word_is(word, CTRL) {
        m.ctrl = true;
        matched = true;
    }
    if c.shift && word_is(word, SHIFT) {
        m.shift = true;
        matched = true;
    }
    if c.alt && word_is(word, ALT) {
        m.alt = true;
        matched = true;
    }
    let aria = &c.aria_key;
    let key = if complete || aria.chars().count() == 1 || word.chars().count() == 1 {
        aria.to_lowercase() == word.to_lowercase()
    } else {
        aria.to_lowercase().contains(&word.to_lowercase())
    };
    if key {
        m.key = true;
        matched = true;
    }
    matched
}

fn is_complete(chord: Option<&Chord>, m: &KeyMatch) -> bool {
    let Some(c) = chord else { return true };
    m.key && (!c.meta || m.meta) && (!c.alt || m.alt) && (!c.ctrl || m.ctrl) && (!c.shift || m.shift)
}

/// `matchesKeybinding`.
fn matches_keybinding(kb: &Keybinding, search: &str, words: &[&str], complete: bool) -> Option<Vec<KeyMatch>> {
    let lower = search.to_lowercase();
    if lower == kb.raw.to_lowercase() || lower == kb.aria_label().to_lowercase() || lower == kb.label().to_lowercase() {
        return Some(kb.chords.iter().map(KeyMatch::complete).collect());
    }
    let first = kb.chords.first();
    let second = kb.chords.get(1);
    let mut first_m = KeyMatch::default();
    let mut chord_m = KeyMatch::default();
    let mut matched_words: Vec<usize> = Vec::new();
    let mut first_words: Vec<usize> = Vec::new();
    let mut chord_words: Vec<usize> = Vec::new();
    let mut match_first = true;
    for (i, word) in words.iter().enumerate() {
        let mut fm = false;
        let mut cm = false;
        match_first = match_first && !first_m.key;
        let mut match_chord = !chord_m.key;
        if match_first {
            fm = match_part(first, &mut first_m, word, complete);
            if first_m.key {
                for w in &chord_words {
                    if !first_words.contains(w) {
                        if let Some(p) = matched_words.iter().position(|x| x == w) {
                            matched_words.remove(p);
                        }
                    }
                }
                chord_m = KeyMatch::default();
                chord_words.clear();
                match_chord = false;
            }
        }
        if match_chord {
            cm = match_part(second, &mut chord_m, word, complete);
        }
        if fm {
            first_words.push(i);
        }
        if cm {
            chord_words.push(i);
        }
        if fm || cm {
            matched_words.push(i);
        }
        match_first = match_first && is_modifier(word);
    }
    if matched_words.len() != words.len() {
        return None;
    }
    if complete {
        if !is_complete(first, &first_m) {
            return None;
        }
        if chord_m.any() && !is_complete(second, &chord_m) {
            return None;
        }
    }
    if first_m.any() || chord_m.any() {
        let mut v = vec![first_m];
        if second.is_some() {
            v.push(chord_m);
        }
        Some(v)
    } else {
        None
    }
}

fn source_regex_value(search: &str) -> Option<(Range<usize>, String)> {
    let lower = search.to_lowercase();
    let at = lower.find("@source:")?;
    let rest = &lower[at + 8..];
    let trimmed = rest.trim_start();
    let skip = rest.len() - trimmed.len();
    for name in ["user", "default", "system", "extension"] {
        if trimmed.starts_with(name) {
            return Some((at..at + 8 + skip + name.len(), name.to_string()));
        }
    }
    None
}

/// `fetch`: the entries for a search, in display order.
pub fn fetch(items: &[Item], order: &[usize], search: &str) -> Vec<Entry> {
    let mut search = search.to_string();
    let mut pool: Vec<usize> = order.to_vec();
    let lower = search.to_lowercase();

    // @command:ID (+when:EXPR)
    if let Some(at) = lower.find("@command:") {
        let rest = &search[at + 9..];
        let (cmd, when) = match rest.find("+when:") {
            Some(w) => (rest[..w].trim(), Some(rest[w + 6..].trim().to_string())),
            None => (rest.trim(), None),
        };
        if !cmd.is_empty() {
            let mut out: Vec<usize> = pool.iter().copied().filter(|&i| items[i].command == cmd).collect();
            if let Some(w) = when.filter(|w| !w.is_empty()) {
                let with: Vec<usize> = out.iter().copied().filter(|&i| items[i].when == w).collect();
                if !with.is_empty() {
                    out = with;
                }
            }
            return out.into_iter().map(Entry::plain).collect();
        }
    }
    if let Some((range, name)) = source_regex_value(&search) {
        pool.retain(|&i| match (&items[i].source, name.as_str()) {
            (Source::System, "default" | "system") => true,
            (Source::User, "user") => true,
            (Source::Extension(_), "extension") => true,
            _ => false,
        });
        search.replace_range(range, "");
    } else if let Some(at) = lower.find("@ext:") {
        let rest = search[at + 5..].trim_start();
        let (ext, len) = if let Some(q) = rest.strip_prefix('"') {
            match q.find('"') {
                Some(e) => (q[..e].to_string(), e + 2),
                None => (String::new(), 0),
            }
        } else {
            let e = rest.find(char::is_whitespace).unwrap_or(rest.len());
            (rest[..e].to_string(), e)
        };
        if !ext.is_empty() {
            let ext_l = ext.to_lowercase();
            pool.retain(|&i| matches!(&items[i].source, Source::Extension(n) if n.to_lowercase() == ext_l));
            let start = at;
            let end = search.len() - (search[at + 5..].trim_start().len() - len);
            search.replace_range(start..end, "");
        }
    } else if let Some(at) = lower.find("@keybinding:") {
        let rest = search[at + 12..].trim().to_string();
        if !rest.is_empty() {
            search = if rest.starts_with('"') { rest } else { format!("\"{}\"", rest.split_whitespace().next().unwrap_or_default()) };
        }
    }

    let search = search.trim();
    if search.is_empty() {
        return pool.into_iter().map(Entry::plain).collect();
    }
    // filterByText
    let quote_first = search.starts_with('"');
    let quote_last = search.len() > 1 && search.ends_with('"');
    let complete = quote_first && quote_last;
    let mut s = search;
    if quote_first {
        s = &s[1..];
    }
    if quote_last {
        s = &s[..s.len() - 1];
    }
    let s = s.trim();
    let words: Vec<&str> = s.split(' ').collect();
    let key_words: Vec<&str> = words.iter().flat_map(|w| w.split('+')).filter(|w| !w.is_empty()).collect();
    let id_filter = |w: &str, t: &str| matches_words(w, t, false).or_else(|| matches_camel_case(w, t));

    let mut out = Vec::new();
    for i in pool {
        let item = &items[i];
        let mut e = Entry::plain(i);
        let mut any = false;
        if !complete {
            if let Some(m) = matches(Some(s), &item.command, &id_filter, &words) {
                e.command_id = Some(m);
                any = true;
            }
            if !item.label.is_empty() {
                let label = item.label.clone();
                if let Some(m) = matches(Some(s), &item.label, &|w, _| matches_words(w, &label, true), &words) {
                    e.label = m;
                    any = true;
                }
            }
            if !item.when.is_empty() {
                if let Some(m) = matches(None, &item.when, &id_filter, &words) {
                    e.when = m;
                    any = true;
                }
            }
            match &item.source {
                Source::Extension(name) => {
                    let label = item.label.clone();
                    if matches(Some(s), name, &|w, _| matches_words(w, &label, true), &words).is_some() {
                        any = true;
                    }
                }
                src => {
                    let name = src.label().to_string();
                    if let Some(m) = matches(Some(s), &name, &|w, _| matches_words(w, &name, true), &words) {
                        e.source = m;
                        any = true;
                    }
                }
            }
        }
        if let Some(kb) = &item.key {
            if let Some(k) = matches_keybinding(kb, s, &key_words, complete) {
                e.keys = k;
                any = true;
            }
        }
        if any {
            out.push(e);
        }
    }
    out
}

/// The whole model: items, alphabetical and precedence orders.
pub struct Model {
    pub items: Vec<Item>,
    pub alphabetical: Vec<usize>,
    pub precedence: Vec<usize>,
}

impl Model {
    pub fn new(user: &[UserBinding]) -> Model {
        let items = items_by_precedence(user);
        let precedence: Vec<usize> = (0..items.len()).collect();
        let mut alphabetical = precedence.clone();
        alphabetical.sort_by(|&a, &b| compare(&items[a], &items[b]));
        Model { items, alphabetical, precedence }
    }

    pub fn fetch(&self, search: &str, by_precedence: bool) -> Vec<Entry> {
        fetch(&self.items, if by_precedence { &self.precedence } else { &self.alphabetical }, search)
    }
}

/// The user settings label for a pressed chord (`cmd+shift+p`).
pub fn chord_from_key(key: egui::Key, m: egui::Modifiers) -> Option<String> {
    use egui::Key;
    let name = match key {
        Key::ArrowLeft => "left".to_string(),
        Key::ArrowRight => "right".to_string(),
        Key::ArrowUp => "up".to_string(),
        Key::ArrowDown => "down".to_string(),
        Key::Enter => "enter".to_string(),
        Key::Escape => "escape".to_string(),
        Key::Tab => "tab".to_string(),
        Key::Space => "space".to_string(),
        Key::Backspace => "backspace".to_string(),
        Key::Delete => "delete".to_string(),
        Key::Insert => "insert".to_string(),
        Key::Home => "home".to_string(),
        Key::End => "end".to_string(),
        Key::PageUp => "pageup".to_string(),
        Key::PageDown => "pagedown".to_string(),
        Key::Comma => ",".to_string(),
        Key::Period => ".".to_string(),
        Key::Slash => "/".to_string(),
        Key::Backslash => "\\".to_string(),
        Key::Semicolon => ";".to_string(),
        Key::Quote => "'".to_string(),
        Key::OpenBracket => "[".to_string(),
        Key::CloseBracket => "]".to_string(),
        Key::Backtick => "`".to_string(),
        Key::Minus => "-".to_string(),
        Key::Equals => "=".to_string(),
        k => {
            let n = k.name();
            if n.len() == 1 || (n.starts_with('F') && n[1..].chars().all(|c| c.is_ascii_digit())) {
                n.to_lowercase()
            } else {
                return None;
            }
        }
    };
    let mut s = String::new();
    if m.ctrl {
        s.push_str("ctrl+");
    }
    if m.shift {
        s.push_str("shift+");
    }
    if m.alt {
        s.push_str("alt+");
    }
    if m.mac_cmd || (m.command && !m.ctrl) {
        s.push_str("cmd+");
    }
    s.push_str(&name);
    Some(s)
}

/// What a pressed key sequence means (`KeybindingResolver.resolve`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// Run this command.
    Command(String),
    /// The first chord of a two-chord keybinding: wait for the second.
    Prefix,
    /// Nothing is bound to it.
    None,
}

/// The keybindings in effect: VS Code's defaults with keybindings.json
/// applied, for dispatching keys and labelling menus.
pub struct Keymap {
    items: Vec<Item>,
    user: Vec<UserBinding>,
}

impl Keymap {
    pub fn new(user: &[UserBinding]) -> Keymap {
        Keymap { items: items_by_precedence(user), user: user.to_vec() }
    }

    /// keybindings.json changes nothing.
    pub fn is_default(&self) -> bool {
        self.user.is_empty()
    }

    /// Bound items, highest precedence first: keybindings.json (its last
    /// entry first), then the defaults (the later the stronger).
    fn by_precedence(&self) -> impl Iterator<Item = &Item> {
        let user = self.items.iter().filter(|i| i.source == Source::User && i.key.is_some());
        let defaults = self.items.iter().filter(|i| i.source != Source::User && i.key.is_some()).collect::<Vec<_>>();
        user.chain(defaults.into_iter().rev())
    }

    /// First chords whose meaning keybindings.json changed: its own keys
    /// and the default keys it removed. Other keys keep their built-in
    /// handling.
    pub fn affected(&self) -> HashSet<String> {
        let mut out = HashSet::new();
        for u in &self.user {
            if let Some(cmd) = u.command.strip_prefix('-') {
                if u.key.is_empty() {
                    for (k, c, ..) in &data().default {
                        if c == cmd {
                            if let Some(kb) = Keybinding::parse(k) {
                                out.insert(first_chord(&kb));
                            }
                        }
                    }
                    continue;
                }
            }
            if let Some(kb) = Keybinding::parse(&u.key) {
                out.insert(first_chord(&kb));
            }
        }
        out
    }

    /// Default keys keybindings.json removed (`-command` entries).
    pub fn removed(&self) -> HashSet<String> {
        let mut out = HashSet::new();
        for u in &self.user {
            let Some(cmd) = u.command.strip_prefix('-') else { continue };
            for (k, c, ..) in &data().default {
                if c == cmd && (u.key.is_empty() || Keybinding::parse(k).map(|k| k.raw) == Keybinding::parse(&u.key).map(|k| k.raw)) {
                    if let Some(kb) = Keybinding::parse(k) {
                        out.insert(first_chord(&kb));
                    }
                }
            }
        }
        out
    }

    /// The command for `pressed` (normalised chords, `shift+cmd+p`): the
    /// strongest keybinding starting with them whose `when` holds decides,
    /// a longer one meaning "wait for the next chord".
    pub fn resolve(&self, pressed: &[String], when: &dyn Fn(&str) -> bool) -> Resolution {
        for item in self.by_precedence() {
            let Some(kb) = &item.key else { continue };
            let chords: Vec<String> = chord_strings(kb);
            if chords.len() < pressed.len() || chords[..pressed.len()] != *pressed {
                continue;
            }
            if !(item.when.is_empty() || when(&item.when)) {
                continue;
            }
            return if chords.len() == pressed.len() { Resolution::Command(item.command.clone()) } else { Resolution::Prefix };
        }
        Resolution::None
    }

    /// The key a menu item for `command` shows: `None` keeps the built-in
    /// one; `Some(key)` replaces it (`Some(None)` for none). Changes only
    /// what keybindings.json changed: the command's own entry, a removed
    /// default, or its key given to another command.
    pub fn menu_key(&self, command: &str) -> Option<Option<String>> {
        if let Some(item) = self.items.iter().find(|i| i.source == Source::User && i.command == command && i.key.is_some()) {
            return Some(item.key.as_ref().map(|k| k.raw.clone()));
        }
        let defaults: Vec<&String> = data().default.iter().filter(|(_, c, ..)| c == command).map(|(k, ..)| k).collect();
        let removed = self.user.iter().any(|u| u.command.strip_prefix('-') == Some(command));
        let taken = |key: &str| {
            let raw = Keybinding::parse(key).map(|k| k.raw);
            self.user.iter().any(|u| !u.command.starts_with('-') && u.command != command && Keybinding::parse(&u.key).map(|k| k.raw) == raw)
        };
        if !removed && !defaults.iter().any(|k| taken(k)) {
            return None;
        }
        // What remains of the command's defaults, the strongest (last) first.
        let remaining = self
            .items
            .iter()
            .filter(|i| i.source != Source::User && i.command == command)
            .filter_map(|i| i.key.as_ref())
            .filter(|k| !taken(&k.raw))
            .last()
            .map(|k| k.raw.clone());
        Some(remaining)
    }
}

fn chord_strings(kb: &Keybinding) -> Vec<String> {
    kb.raw.split(' ').map(str::to_string).collect()
}

fn first_chord(kb: &Keybinding) -> String {
    chord_strings(kb).into_iter().next().unwrap_or_default()
}

/// A pressed chord in keybindings.json spelling, normalised (`shift+cmd+p`).
pub fn pressed_chord(key: egui::Key, m: egui::Modifiers) -> Option<String> {
    chord_from_key(key, m).and_then(|c| Keybinding::parse(&c)).map(|k| k.raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keybinding_labels() {
        let k = Keybinding::parse("shift+alt+cmd+g").unwrap();
        assert_eq!(k.label(), "⇧⌥⌘G");
        assert_eq!(k.aria_label(), "Shift+Option+Command+G");
        let k = Keybinding::parse("cmd+k cmd+c").unwrap();
        assert_eq!(k.label(), "⌘K ⌘C");
        assert_eq!(Keybinding::parse("alt+cmd+up").unwrap().label(), "⌥⌘↑");
        assert_eq!(Keybinding::parse("cmd+shift+p").unwrap().raw, "shift+cmd+p");
    }

    #[test]
    fn default_items_are_sorted_like_vs_code() {
        let m = Model::new(&[]);
        let labels: Vec<&str> = m.alphabetical.iter().take(3).map(|&i| m.items[i].label.as_str()).collect();
        assert_eq!(labels[0], "Accept Inline Completion");
        let save = m.items.iter().find(|i| i.command == "workbench.action.files.save").unwrap();
        assert_eq!(save.label, "File: Save");
        assert_eq!(save.key.as_ref().unwrap().label(), "⌘S");
        // Unbound commands follow every bound one.
        let first_unbound = m.alphabetical.iter().position(|&i| m.items[i].key.is_none()).unwrap();
        assert!(m.alphabetical[first_unbound..].iter().all(|&i| m.items[i].key.is_none()));
    }

    #[test]
    fn search_by_words_and_keys() {
        let m = Model::new(&[]);
        let r = m.fetch("file save", false);
        assert!(r.iter().any(|e| m.items[e.index].command == "workbench.action.files.save"));
        let r = m.fetch("\"cmd+s\"", false);
        assert!(r.iter().all(|e| m.items[e.index].key.as_ref().is_some_and(|k| k.raw == "cmd+s")), "{:?}", r.iter().map(|e| &m.items[e.index].command).collect::<Vec<_>>());
        assert!(r.iter().any(|e| m.items[e.index].command == "workbench.action.files.save"));
        let r = m.fetch("@command:workbench.action.files.save", false);
        assert_eq!(r.len(), 1);
        let r = m.fetch("@source:user", false);
        assert!(r.is_empty());
    }

    #[test]
    fn user_keybindings_replace_defaults() {
        let user = parse_user("// Place your key bindings in this file to override the defaults\n[\n\t{ \"key\": \"cmd+alt+s\", \"command\": \"workbench.action.files.save\" },\n\t{ \"key\": \"cmd+s\", \"command\": \"-workbench.action.files.save\" }\n]");
        assert_eq!(user.len(), 2);
        let m = Model::new(&user);
        let saves: Vec<&Item> = m.items.iter().filter(|i| i.command == "workbench.action.files.save").collect();
        assert_eq!(saves.len(), 1);
        assert_eq!(saves[0].source, Source::User);
        assert_eq!(saves[0].key.as_ref().unwrap().raw, "alt+cmd+s");
    }

    #[test]
    fn keymap_follows_keybindings_json() {
        // Only unconditional keybindings apply in this test.
        let always = |w: &str| w.is_empty();
        let m = Keymap::new(&[]);
        assert!(m.is_default());
        assert_eq!(m.resolve(&["cmd+b".into()], &always), Resolution::Command("workbench.action.toggleSidebarVisibility".into()));
        assert_eq!(m.resolve(&["cmd+k".into()], &always), Resolution::Prefix);
        let user = parse_user(r#"[
            { "key": "alt+cmd+s", "command": "workbench.action.files.save" },
            { "key": "cmd+s", "command": "-workbench.action.files.save" },
            { "key": "cmd+b", "command": "workbench.action.files.saveAll" }
        ]"#);
        let m = Keymap::new(&user);
        assert_eq!(m.resolve(&["alt+cmd+s".into()], &always), Resolution::Command("workbench.action.files.save".into()));
        assert_eq!(m.resolve(&["cmd+b".into()], &always), Resolution::Command("workbench.action.files.saveAll".into()));
        assert!(m.affected().contains("cmd+s") && m.affected().contains("cmd+b") && m.affected().contains("alt+cmd+s"));
        assert_ne!(m.resolve(&["cmd+s".into()], &always), Resolution::Command("workbench.action.files.save".into()));
        assert_eq!(m.menu_key("workbench.action.files.save"), Some(Some("alt+cmd+s".into())));
        assert_eq!(m.menu_key("workbench.action.toggleSidebarVisibility"), Some(None));
        assert_eq!(m.menu_key("workbench.action.togglePanel"), None);
    }

    #[test]
    fn filters_match_like_vs_code() {
        assert_eq!(matches_words("fil sav", "File: Save", true), None);
        assert!(matches_words("save", "File: Save", true).is_some());
        assert_eq!(matches_camel_case("ts", "toggleSidebar"), Some(vec![0..1, 6..7]));
        assert_eq!(matches_contiguous("ave", "File: Save"), Some(vec![7..10]));
    }
}
#[cfg(test)]
mod dump {
    #[test]
    #[ignore]
    fn print_first_rows() {
        let m = super::Model::new(&[]);
        for &i in m.alphabetical.iter() {
            let it = &m.items[i];
            let keys = it.key.as_ref().map(|k| k.chords.iter().map(|c| c.parts().into_iter().map(|(_, s)| s).collect::<Vec<_>>().join("|")).collect::<Vec<_>>().join(" ")).unwrap_or_default();
            println!("ROW\t{}\t{}\t{}\t{}\t{}", if it.label.is_empty() { &it.command } else { &it.label }, keys, if it.when.is_empty() { "-" } else { &it.when }, it.source.label(), it.command);
        }
    }
}
