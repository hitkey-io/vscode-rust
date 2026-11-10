//! The colour registry: every colour id VS Code registers with its default
//! per theme type (`registerColor` in `vs/platform/theme/common/colors/*`
//! and the workbench). Generated into `assets/themes/color-registry.json`
//! by `tools/color-registry.mjs`; evaluated here like `colorUtils.ts`'s
//! `resolveColorValue` / `executeTransform`.

use super::color::Color;
use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::HashMap;

/// `ColorScheme` of a theme: which column of the registry's defaults applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThemeKind {
    Dark,
    Light,
    HcDark,
    HcLight,
}

impl ThemeKind {
    /// From a theme contribution's `uiTheme`.
    pub fn from_ui_theme(ui: &str) -> ThemeKind {
        match ui {
            "vs" => ThemeKind::Light,
            "hc-black" => ThemeKind::HcDark,
            "hc-light" => ThemeKind::HcLight,
            _ => ThemeKind::Dark,
        }
    }

    pub fn is_dark(self) -> bool {
        matches!(self, ThemeKind::Dark | ThemeKind::HcDark)
    }

    pub fn is_high_contrast(self) -> bool {
        matches!(self, ThemeKind::HcDark | ThemeKind::HcLight)
    }

    fn column(self) -> usize {
        self as usize
    }
}

/// A `ColorValue`: nothing, a colour, another colour's id, a number
/// argument, or a transform.
#[derive(Clone, Debug)]
pub enum ColorValue {
    None,
    Literal(Color),
    Id(String),
    Number(f64),
    Transform(String, Vec<ColorValue>),
}

impl ColorValue {
    fn from_json(v: &Value) -> ColorValue {
        match v {
            Value::String(s) if s.starts_with('#') => ColorValue::Literal(Color::from_hex_or_red(s)),
            Value::String(s) => ColorValue::Id(s.clone()),
            Value::Number(n) => ColorValue::Number(n.as_f64().unwrap_or(0.0)),
            Value::Object(o) => ColorValue::Transform(
                o.get("f").and_then(Value::as_str).unwrap_or_default().to_string(),
                o.get("a")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().map(ColorValue::from_json).collect())
                    .unwrap_or_default(),
            ),
            _ => ColorValue::None,
        }
    }
}

static REGISTRY: Lazy<HashMap<String, [ColorValue; 4]>> = Lazy::new(|| {
    let json: Value = serde_json::from_str(include_str!("../../assets/themes/color-registry.json"))
        .expect("color-registry.json is valid JSON");
    let mut map = HashMap::new();
    if let Value::Object(entries) = json {
        for (id, d) in entries {
            let col = |k: &str| ColorValue::from_json(d.get(k).unwrap_or(&Value::Null));
            map.insert(id, [col("dark"), col("light"), col("hcDark"), col("hcLight")]);
        }
    }
    map
});

/// Every registered colour id.
pub fn ids() -> impl Iterator<Item = &'static String> {
    REGISTRY.keys()
}

/// The registered default of `id` for `kind`, unevaluated.
pub fn default_value(id: &str, kind: ThemeKind) -> Option<&'static ColorValue> {
    REGISTRY.get(id).map(|d| &d[kind.column()])
}

/// What a value resolves against: the theme's own colours and its kind.
pub trait ColorSource {
    fn kind(&self) -> ThemeKind;
    /// The theme (or user) colour for `id`, without defaults.
    fn defined(&self, id: &str) -> Option<Color>;

    fn defines(&self, id: &str) -> bool {
        self.defined(id).is_some()
    }

    /// `IColorTheme.getColor(id)`: the theme's colour, else the default.
    fn color(&self, id: &str) -> Option<Color> {
        resolve_id(self, id, 0)
    }
}

const MAX_DEPTH: usize = 64;

fn resolve_id<S: ColorSource + ?Sized>(src: &S, id: &str, depth: usize) -> Option<Color> {
    if let Some(c) = src.defined(id) {
        return Some(c);
    }
    if depth > MAX_DEPTH {
        return None;
    }
    resolve(src, default_value(id, src.kind())?, depth + 1)
}

fn number(v: Option<&ColorValue>) -> f64 {
    match v {
        Some(ColorValue::Number(n)) => *n,
        _ => 0.0,
    }
}

/// `resolveColorValue` + `executeTransform`.
pub fn resolve<S: ColorSource + ?Sized>(src: &S, value: &ColorValue, depth: usize) -> Option<Color> {
    let eval = |v: Option<&ColorValue>| v.and_then(|v| resolve(src, v, depth + 1));
    match value {
        ColorValue::None | ColorValue::Number(_) => None,
        ColorValue::Literal(c) => Some(*c),
        ColorValue::Id(id) => resolve_id(src, id, depth),
        ColorValue::Transform(op, a) => match op.as_str() {
            "darken" => eval(a.first()).map(|c| c.darken(number(a.get(1)))),
            "lighten" => eval(a.first()).map(|c| c.lighten(number(a.get(1)))),
            "transparent" => eval(a.first()).map(|c| c.transparent(number(a.get(1)))),
            "withOpacity" => eval(a.first()).map(|c| c.with_opacity(number(a.get(1)))),
            "mix" => {
                let primary = eval(a.first()).unwrap_or(Color::TRANSPARENT);
                let other = eval(a.get(1)).unwrap_or(Color::TRANSPARENT);
                Some(primary.mix(&other, number(a.get(2))))
            }
            "opaque" => match eval(a.get(1)) {
                None => eval(a.first()),
                Some(bg) => eval(a.first()).map(|c| c.make_opaque(&bg)),
            },
            "oneOf" => a.iter().find_map(|v| resolve(src, v, depth + 1)),
            "ifDefinedThenElse" => {
                let defined = matches!(a.first(), Some(ColorValue::Id(id)) if src.defines(id));
                eval(if defined { a.get(1) } else { a.get(2) })
            }
            "lessProminent" => {
                let from = eval(a.first())?;
                let factor = number(a.get(2));
                let transparency = number(a.get(3));
                match eval(a.get(1)) {
                    None => Some(from.transparent(factor * transparency)),
                    Some(bg) => Some(
                        if from.is_darker_than(&bg) {
                            Color::lighter_color(&from, &bg, factor)
                        } else {
                            Color::darker_color(&from, &bg, factor)
                        }
                        .transparent(transparency),
                    ),
                }
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Empty(ThemeKind);
    impl ColorSource for Empty {
        fn kind(&self) -> ThemeKind {
            self.0
        }
        fn defined(&self, _: &str) -> Option<Color> {
            None
        }
    }

    #[test]
    fn registry_defaults() {
        assert!(ids().count() > 900);
        let dark = Empty(ThemeKind::Dark);
        assert_eq!(dark.color("editor.background").unwrap().to_hex(), "#1e1e1e");
        assert_eq!(Empty(ThemeKind::Light).color("foreground").unwrap().to_hex(), "#616161");
        // transparent(#ffffff, 0.12)
        assert_eq!(dark.color("statusBarItem.hoverBackground").unwrap().to_hex(), "#ffffff1f");
        assert_eq!(dark.color("terminal.ansiRed").unwrap().to_hex(), "#cd3131");
    }
}
