//! Colour themes: the ones VS Code ships (`extensions/theme-*`) and loading
//! a theme file with its `include` chain (`colorThemeData.ts`
//! `_loadColorTheme`).

use super::color::Color;
use super::registry::{ColorSource, ThemeKind};
use serde_json::Value;
use std::collections::HashMap;

/// A theme contribution: `workbench.colorTheme` id, picker label, kind and
/// its JSON file.
#[derive(Clone, Copy, Debug)]
pub struct ThemeInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: ThemeKind,
    pub file: &'static str,
}

macro_rules! theme_files {
    ($($file:literal),* $(,)?) => {
        fn embedded(file: &str) -> Option<&'static str> {
            match file {
                $($file => Some(include_str!(concat!("../../assets/themes/", $file))),)*
                _ => None,
            }
        }
    };
}

theme_files!(
    "2026-dark.json",
    "2026-light.json",
    "dark_modern.json",
    "dark_plus.json",
    "dark_vs.json",
    "light_modern.json",
    "light_plus.json",
    "light_vs.json",
    "hc_black.json",
    "hc_light.json",
    "abyss-color-theme.json",
    "kimbie-dark-color-theme.json",
    "monokai-color-theme.json",
    "dimmed-monokai-color-theme.json",
    "quietlight-color-theme.json",
    "Red-color-theme.json",
    "solarized-dark-color-theme.json",
    "solarized-light-color-theme.json",
    "tomorrow-night-blue-color-theme.json",
);

/// `ThemeSettingDefaults.COLOR_THEME_DARK`.
pub const DEFAULT_THEME: &str = "Dark 2026";
/// `ThemeSettingDefaults.COLOR_THEME_LIGHT`.
pub const DEFAULT_LIGHT_THEME: &str = "Light 2026";

use ThemeKind::*;

/// The built-in colour themes, in contribution order.
pub const BUILTIN: &[ThemeInfo] = &[
    ThemeInfo { id: "Light 2026", label: "Light 2026", kind: Light, file: "2026-light.json" },
    ThemeInfo { id: "Dark 2026", label: "Dark 2026", kind: Dark, file: "2026-dark.json" },
    ThemeInfo { id: "Dark+", label: "Dark+", kind: Dark, file: "dark_plus.json" },
    ThemeInfo { id: "Dark Modern", label: "Dark Modern", kind: Dark, file: "dark_modern.json" },
    ThemeInfo { id: "Light+", label: "Light+", kind: Light, file: "light_plus.json" },
    ThemeInfo { id: "Light Modern", label: "Light Modern", kind: Light, file: "light_modern.json" },
    ThemeInfo { id: "Visual Studio Dark", label: "Dark (Visual Studio)", kind: Dark, file: "dark_vs.json" },
    ThemeInfo { id: "Visual Studio Light", label: "Light (Visual Studio)", kind: Light, file: "light_vs.json" },
    ThemeInfo { id: "Default High Contrast", label: "Dark High Contrast", kind: HcDark, file: "hc_black.json" },
    ThemeInfo {
        id: "Default High Contrast Light",
        label: "Light High Contrast",
        kind: HcLight,
        file: "hc_light.json",
    },
    ThemeInfo { id: "Abyss", label: "Abyss", kind: Dark, file: "abyss-color-theme.json" },
    ThemeInfo { id: "Kimbie Dark", label: "Kimbie Dark", kind: Dark, file: "kimbie-dark-color-theme.json" },
    ThemeInfo { id: "Monokai", label: "Monokai", kind: Dark, file: "monokai-color-theme.json" },
    ThemeInfo { id: "Monokai Dimmed", label: "Monokai Dimmed", kind: Dark, file: "dimmed-monokai-color-theme.json" },
    ThemeInfo { id: "Quiet Light", label: "Quiet Light", kind: Light, file: "quietlight-color-theme.json" },
    ThemeInfo { id: "Red", label: "Red", kind: Dark, file: "Red-color-theme.json" },
    ThemeInfo { id: "Solarized Dark", label: "Solarized Dark", kind: Dark, file: "solarized-dark-color-theme.json" },
    ThemeInfo { id: "Solarized Light", label: "Solarized Light", kind: Light, file: "solarized-light-color-theme.json" },
    ThemeInfo {
        id: "Tomorrow Night Blue",
        label: "Tomorrow Night Blue",
        kind: Dark,
        file: "tomorrow-night-blue-color-theme.json",
    },
];

/// The built-in theme whose settings id or label is `id`.
pub fn find(id: &str) -> Option<&'static ThemeInfo> {
    BUILTIN.iter().find(|t| t.id == id).or_else(|| BUILTIN.iter().find(|t| t.label == id))
}

/// A TextMate theming rule (`tokenColors` entry).
#[derive(Clone, Debug, Default)]
pub struct TokenRule {
    /// Scope selectors; empty for the global rule.
    pub scopes: Vec<String>,
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub font_style: Option<String>,
}

/// A loaded colour theme.
#[derive(Clone, Debug)]
pub struct ColorTheme {
    pub id: String,
    pub label: String,
    pub kind: ThemeKind,
    colors: HashMap<String, Color>,
    /// Colours from `workbench.colorCustomizations` (`None` = "default").
    custom: HashMap<String, Option<Color>>,
    pub token_rules: Vec<TokenRule>,
    pub semantic_highlighting: bool,
}

impl ColorSource for ColorTheme {
    fn kind(&self) -> ThemeKind {
        self.kind
    }

    fn defined(&self, id: &str) -> Option<Color> {
        match self.custom.get(id) {
            Some(Some(c)) => Some(*c),
            Some(None) => None,
            None => self.colors.get(id).copied(),
        }
    }
}

impl ColorTheme {
    /// Loads a built-in theme.
    pub fn builtin(info: &ThemeInfo) -> ColorTheme {
        let mut theme = ColorTheme {
            id: info.id.to_string(),
            label: info.label.to_string(),
            kind: info.kind,
            colors: HashMap::new(),
            custom: HashMap::new(),
            token_rules: Vec::new(),
            semantic_highlighting: false,
        };
        let mut rules = Vec::new();
        theme.load(info.file, &mut rules, 0);
        theme.token_rules = rules.iter().filter_map(parse_rule).collect();
        theme
    }

    /// The default theme ("Dark 2026").
    pub fn default_theme() -> ColorTheme {
        ColorTheme::builtin(find(DEFAULT_THEME).expect("default theme is built in"))
    }

    fn load(&mut self, file: &str, rules: &mut Vec<Value>, depth: usize) {
        let Some(text) = embedded(file) else { return };
        let Some(json) = crate::jsonc::parse(text) else { return };
        if let Some(include) = json.get("include").and_then(Value::as_str) {
            if depth < 8 {
                let name = include.rsplit('/').next().unwrap_or(include);
                self.load(name, rules, depth + 1);
            }
        }
        self.semantic_highlighting |= json.get("semanticHighlighting").and_then(Value::as_bool).unwrap_or(false);
        if let Some(Value::Object(colors)) = json.get("colors") {
            for (id, v) in colors {
                match v.as_str() {
                    Some("default") => {
                        self.colors.remove(id);
                    }
                    Some(s) => {
                        self.colors.insert(id.clone(), Color::from_hex_or_red(s));
                    }
                    None => {}
                }
            }
        }
        if let Some(Value::Array(tc)) = json.get("tokenColors") {
            rules.extend(tc.iter().cloned());
        }
    }

    /// Applies `workbench.colorCustomizations` (global and `[Theme]`
    /// sections already merged by the caller).
    pub fn set_customizations(&mut self, custom: &serde_json::Map<String, Value>) {
        self.custom = custom
            .iter()
            .filter_map(|(k, v)| {
                let s = v.as_str()?;
                Some((k.clone(), if s == "default" { None } else { Color::from_hex(s) }))
            })
            .collect();
    }

    /// Every colour the theme defines itself.
    pub fn defined_ids(&self) -> impl Iterator<Item = &String> {
        self.colors.keys().chain(self.custom.keys())
    }
}

fn parse_rule(v: &Value) -> Option<TokenRule> {
    let settings = v.get("settings")?;
    let scopes = match v.get("scope") {
        Some(Value::String(s)) => s.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(|s| s.trim().to_string()).collect(),
        _ => Vec::new(),
    };
    let color = |k: &str| settings.get(k).and_then(Value::as_str).and_then(Color::from_hex);
    Some(TokenRule {
        scopes,
        foreground: color("foreground"),
        background: color("background"),
        font_style: settings.get("fontStyle").and_then(Value::as_str).map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_theme_loads() {
        for info in BUILTIN {
            let t = ColorTheme::builtin(info);
            assert!(t.color("editor.background").is_some(), "{}", info.id);
            assert!(!t.token_rules.is_empty(), "{}", info.id);
        }
    }

    #[test]
    fn include_chain_and_defaults() {
        let t = ColorTheme::default_theme();
        // 2026-dark.json itself
        assert_eq!(t.color("editor.background").unwrap().to_hex(), "#121314");
        assert_eq!(t.color("focusBorder").unwrap().to_hex(), "#3994bcb3");
        // dark_plus → dark_vs supplies token colours; registry fills the rest
        assert!(t.color("terminal.ansiBlue").is_some());
        let light = ColorTheme::builtin(find("Light+").unwrap());
        assert_eq!(light.color("editor.background").unwrap().to_hex(), "#ffffff");
        assert_eq!(find("Dark (Visual Studio)").unwrap().id, "Visual Studio Dark");
    }
}
