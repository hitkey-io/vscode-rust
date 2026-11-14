//! Boolean-object settings (settingsWidgets.ts `ObjectSettingCheckboxWidget`,
//! settingsTree.ts `getBoolObjectDisplayValue` / `onDidChangeObject`,
//! settingsWidgets.css): one 24px row per property — a checkbox (18px, 4px
//! in, 3px down) and "description (key)" — odd rows shaded; clicking the
//! text or the box toggles it. Measured against 1.145.2: the rows start 1px
//! into the control area, the text starts 35px after the row's left edge,
//! the widget is 1px padded and 1px short of the next block.

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use serde_json::{Map, Value};

use super::model::{SettingDef, ValueType};
use crate::icons;
use crate::theme::Palette;

const ROW_H: f32 = 24.0;

pub fn is_bool_object(vt: ValueType) -> bool {
    vt == ValueType::BooleanObject
}

/// One checkbox: the property key, its label and whether it is on.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub key: String,
    pub label: String,
    pub on: bool,
}

/// `getBoolObjectDisplayValue`: every property of the schema, on when the
/// configured object (over the defaults) holds a truthy value for it.
pub fn rows(s: &SettingDef, configured: Option<&Value>) -> Vec<Row> {
    let default = s.default.as_object().cloned().unwrap_or_default();
    let mut data = default;
    if let Some(scope) = configured.and_then(Value::as_object) {
        for (k, v) in scope {
            data.insert(k.clone(), v.clone());
        }
    }
    s.properties
        .iter()
        .map(|(key, description)| {
            let description = description.as_str().unwrap_or_default();
            Row {
                key: key.clone(),
                label: if description.is_empty() { key.clone() } else { format!("{description} ({key})") },
                on: truthy(data.get(key)),
            }
        })
        .collect()
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// `onDidChangeObject` for a toggled `key`: every shown value is written
/// back over the configured object, then entries that moved back to the
/// default are dropped; an empty result resets the setting (`None`).
pub fn apply(s: &SettingDef, configured: Option<&Value>, key: &str, on: bool) -> Option<Value> {
    let default = s.default.as_object().cloned().unwrap_or_default();
    let scope = configured.and_then(Value::as_object).cloned().unwrap_or_default();
    let mut shown = rows(s, configured);
    if let Some(r) = shown.iter_mut().find(|r| r.key == key) {
        r.on = on;
    }
    let mut value: Map<String, Value> = scope.clone();
    for r in &shown {
        value.insert(r.key.clone(), Value::Bool(r.on));
    }
    let keys: Vec<String> = value.keys().cloned().collect();
    for k in keys {
        let v = &value[&k];
        if scope.get(&k) != Some(v) && default.get(&k) == Some(v) {
            value.remove(&k);
        }
    }
    if value.is_empty() {
        None
    } else {
        Some(Value::Object(value))
    }
}

/// Height of the control area under the description, gap included.
pub fn control_height(n: usize) -> f32 {
    super::CONTROL_GAP + 1.0 + ROW_H * n as f32 + 2.0
}

/// Draws the rows at `top` (where the control area starts, the 9px gap
/// included); returns the new value when a box was toggled (`Some(None)`
/// resets the setting).
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    painter: &egui::Painter,
    s: &SettingDef,
    configured: Option<&Value>,
    left: f32,
    width: f32,
    top: f32,
    id: egui::Id,
    clip: Rect,
) -> Option<Option<Value>> {
    let mut result = None;
    let row_left = left + 1.0;
    let row_w = width - 2.0;
    let mut y = top + super::CONTROL_GAP + 1.0;
    for (i, row) in rows(s, configured).iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(row_left, y), Vec2::new(row_w, ROW_H));
        if i % 2 == 0 {
            painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(130, 130, 130, 10));
        }
        let boxr = Rect::from_min_size(Pos2::new(r.left() + 4.0, r.top() + 3.0), Vec2::splat(18.0));
        painter.rect(boxr, 3.0, Palette::SETTINGS_CHECKBOX_BG(), Stroke::new(1.0, Palette::SETTINGS_CHECKBOX_BORDER()), egui::StrokeKind::Inside);
        if row.on {
            painter.text(boxr.center(), Align2::CENTER_CENTER, icons::codicon_by_name("check").unwrap_or('v').to_string(), icons::codicon_font(16.0), Palette::SETTINGS_CHECKBOX_FG());
        }
        let text_x = r.left() + 35.0;
        let g = painter.layout_no_wrap(row.label.clone(), FontId::proportional(13.0), Palette::FG());
        let ty = r.center().y - g.size().y / 2.0 - 0.5;
        painter.with_clip_rect(Rect::from_min_max(Pos2::new(text_x, r.top()), r.right_bottom()).intersect(clip)).galley(Pos2::new(text_x, ty), g, Palette::FG());
        let resp = ui.interact(r.intersect(clip), id.with(("row", &row.key)), Sense::click());
        let label = row.label.clone();
        let on = row.on;
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, on, label.clone()));
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            result = Some(apply(s, configured, &row.key, !row.on));
        }
        y += ROW_H;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::settings_editor::model;
    use serde_json::json;

    fn def(key: &str) -> &'static SettingDef {
        model::settings().iter().find(|s| s.key == key).unwrap()
    }

    #[test]
    fn shows_each_property_with_its_description_and_default() {
        let s = def("scm.diffDecorationsGutterPattern");
        let r = rows(s, None);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].label, "Use pattern for the diff decorations in gutter for added lines. (added)");
        assert!(!r[0].on);
        assert!(r[1].on);
    }

    #[test]
    fn toggling_writes_what_differs_from_the_default() {
        let s = def("scm.diffDecorationsGutterPattern");
        // added: false → true; modified stays at its default so it isn't written.
        assert_eq!(apply(s, None, "added", true), Some(json!({ "added": true })));
        // back to the default → the setting resets.
        let cfg = json!({ "added": true });
        assert_eq!(apply(s, Some(&cfg), "added", false), None);
        // modified turned off is written next to the existing entry.
        assert_eq!(apply(s, Some(&cfg), "modified", false), Some(json!({ "added": true, "modified": false })));
    }
}
