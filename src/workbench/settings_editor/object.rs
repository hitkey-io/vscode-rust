//! Object settings (settingsWidgets.ts `ObjectSettingDropdownWidget`,
//! settingsTree.ts `getObjectDisplayValue` / `onDidChangeObject`,
//! settingsWidgets.css): a table with a bold "Item | Value" header, one
//! 24px row per entry (odd data rows shaded), Edit / Remove / Reset on
//! hover, "Add Item" below and an edit row (key input, value input or
//! select, OK, Cancel). Measured against 1.145.2: header and rows start 1px
//! into the control area, the key column is 40% and the value column 60% of
//! the row less 4px, text 4px in; the button sits 6px under the last row
//! (the edit row takes the place of the button, with no gap).

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use serde_json::{Map, Value};

use super::model::{SettingDef, ValueType};
use crate::icons;
use crate::theme::Palette;

const ROW_H: f32 = 24.0;
const BUTTON_H: f32 = 26.0;

pub fn is_object(vt: ValueType) -> bool {
    vt == ValueType::Object
}

/// How a value is edited (`getObjectValueType`): a text box, or a select
/// (booleans offer true / false).
#[derive(Clone, Debug, PartialEq)]
pub enum Spec {
    Text,
    Select(Vec<String>),
}

fn spec_of(schema: &Value) -> Spec {
    match schema.get("type").and_then(Value::as_str) {
        Some("boolean") => Spec::Select(vec!["true".into(), "false".into()]),
        Some("enum") => Spec::Select(schema.get("enum").and_then(Value::as_array).map(|a| a.iter().map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).collect()).unwrap_or_default()),
        _ => Spec::Text,
    }
}

/// The value editor for `key`: its fixed property, else the first matching
/// pattern, else the additional-properties schema.
pub fn spec_for(s: &SettingDef, key: &str) -> Spec {
    if let Some(p) = s.object_properties.get(key) {
        return spec_of(p);
    }
    for (pattern, schema) in &s.object_pattern_properties {
        if regex::Regex::new(pattern).is_ok_and(|re| re.is_match(key)) {
            return spec_of(schema);
        }
    }
    s.object_additional.as_ref().map(spec_of).unwrap_or(Spec::Text)
}

/// `objectSettingSupportsRemoveDefaultValue`.
fn supports_remove_default(key: &str) -> bool {
    key == "workbench.editor.customLabels.patterns"
}

/// One row of the table.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub key: String,
    /// The value as shown (`'' + data`).
    pub value: String,
    pub spec: Spec,
    /// Has a default that the user hasn't changed or removed is not
    /// removable (only editable); `resettable` when the user changed a
    /// default.
    pub removable: bool,
    pub resettable: bool,
}

fn shown(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// `getObjectDisplayValue`: the defaults overlaid with the configured
/// object (all of it when nothing is configured).
pub fn rows(s: &SettingDef, configured: Option<&Value>) -> Vec<Row> {
    let default = s.default.as_object().cloned().unwrap_or_default();
    let scope = configured.and_then(Value::as_object);
    let mut data = default.clone();
    if let Some(sc) = scope {
        for (k, v) in sc {
            data.insert(k.clone(), v.clone());
        }
    }
    data.iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| {
            let d = default.get(k);
            Row {
                key: k.clone(),
                value: shown(v),
                spec: spec_for(s, k),
                removable: d.is_none() || supports_remove_default(&s.key),
                resettable: d.is_some_and(|d| d != v),
            }
        })
        .collect()
}

/// What happened to the table.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    /// `original` replaced by `key` = `value` (either may be the same).
    Update { original: String, key: String, value: String },
    Remove(String),
    Reset(String),
    Add { key: String, value: String },
}

fn json_value(spec: &Spec, text: &str) -> Value {
    match spec {
        Spec::Select(opts) if opts.len() == 2 && opts[0] == "true" && opts[1] == "false" => Value::Bool(text == "true"),
        _ => Value::String(text.to_string()),
    }
}

/// `onDidChangeObject`: the new value for the setting (`None` resets it).
pub fn apply(s: &SettingDef, configured: Option<&Value>, change: &Change) -> Option<Value> {
    let default = s.default.as_object().cloned().unwrap_or_default();
    let scope: Map<String, Value> = configured.and_then(Value::as_object).cloned().unwrap_or_default();
    let mut value = scope.clone();
    let remove_default = supports_remove_default(&s.key);
    match change {
        Change::Update { original, key, value: text } => {
            if original != key && remove_default && default.contains_key(original) {
                value.insert(original.clone(), Value::Null);
            } else {
                value.remove(original);
            }
            value.insert(key.clone(), json_value(&spec_for(s, key), text));
        }
        Change::Add { key, value: text } => {
            value.insert(key.clone(), json_value(&spec_for(s, key), text));
        }
        Change::Remove(key) => {
            let shown_value = scope.get(key).or(default.get(key));
            if remove_default && default.get(key) == shown_value && default.contains_key(key) {
                value.insert(key.clone(), Value::Null);
            } else {
                value.remove(key);
            }
        }
        Change::Reset(key) => {
            value.remove(key);
        }
    }
    // Entries back at their default drop out of the user's value.
    let keys: Vec<String> = value.keys().cloned().collect();
    for k in keys {
        let v = value[&k].clone();
        if scope.get(&k) != Some(&v) && default.get(&k) == Some(&v) && !(remove_default && v.is_null()) {
            value.remove(&k);
        }
    }
    if value.is_empty() {
        None
    } else {
        Some(Value::Object(value))
    }
}

/// An item being added (`index` None) or changed.
#[derive(Clone, Debug)]
pub struct Edit {
    pub setting: String,
    pub index: Option<usize>,
    pub key: String,
    pub value: String,
    pub original_key: Option<String>,
    pub focus: bool,
}

/// Height of the control area under the description, gap included.
pub fn control_height(n: usize) -> f32 {
    super::CONTROL_GAP + 1.0 + ROW_H + ROW_H * n as f32 + 6.0 + BUTTON_H + 4.0
}

fn button(ui: &mut Ui, painter: &egui::Painter, rect: Rect, label: &str, primary: bool, enabled: bool, id: egui::Id, clip: Rect) -> bool {
    let resp = ui.interact(rect.intersect(clip), id, if enabled { Sense::click() } else { Sense::hover() });
    let (bg, fg) = if primary {
        (if resp.hovered() && enabled { Palette::BUTTON_HOVER() } else { Palette::BUTTON_BG() }, Palette::BUTTON_FG())
    } else {
        (if resp.hovered() { Palette::VSCE_BUTTON_SECONDARY_HOVER_BG() } else { Palette::VSCE_BUTTON_SECONDARY_BG() }, Palette::VSCE_BUTTON_SECONDARY_FG())
    };
    let alpha = if enabled { 1.0 } else { 0.4 };
    painter.rect_filled(rect, 4.0, bg.gamma_multiply(alpha));
    let g = painter.layout_no_wrap(label.to_string(), FontId::proportional(12.0), Color32::PLACEHOLDER);
    painter.galley(Pos2::new(rect.center().x - g.size().x / 2.0, rect.center().y - g.size().y / 2.0), g, fg.gamma_multiply(alpha));
    let text = label.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, text.clone()));
    enabled && resp.clicked()
}

fn text_width(painter: &egui::Painter, text: &str, size: f32) -> f32 {
    painter.layout_no_wrap(text.to_string(), FontId::proportional(size), Color32::PLACEHOLDER).size().x
}

/// Draws the table at `top` (where the control area starts, the 9px gap
/// included); returns a new value when it changed (`Some(None)` resets).
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    painter: &egui::Painter,
    s: &SettingDef,
    configured: Option<&Value>,
    edit: &mut Option<Edit>,
    left: f32,
    width: f32,
    top: f32,
    id: egui::Id,
    clip: Rect,
) -> Option<Option<Value>> {
    let list = rows(s, configured);
    let mut result: Option<Option<Value>> = None;
    let editing = edit.as_ref().filter(|e| e.setting == s.key).cloned();
    let row_left = left + 1.0;
    let row_w = width - 2.0;
    let inner = row_w - 4.0 - 4.0; // padding-left 4, padding-right 4
    let key_w = (inner * 0.4).round();
    let key_x = row_left + 4.0;
    let val_x = key_x + key_w + 3.0;
    let mut y = top + super::CONTROL_GAP + 1.0;

    // Header (bold, shaded).
    let header = Rect::from_min_size(Pos2::new(row_left, y), Vec2::new(row_w, ROW_H));
    painter.rect_filled(header, 0.0, Color32::from_rgba_unmultiplied(130, 130, 130, 10));
    for (x, label) in [(key_x, "Item"), (val_x, "Value")] {
        painter.text(Pos2::new(x, header.center().y), Align2::LEFT_CENTER, label, crate::ui_font::bold(13.0), Palette::FG());
    }
    y += ROW_H;

    let mut start_edit: Option<(Option<usize>, String, String)> = None;
    let mut finish: Option<bool> = None;
    let mut typed: Option<(String, String)> = None;

    let mut edit_row = |ui: &mut Ui, y: f32, e: &Edit, finish: &mut Option<bool>| -> (String, String) {
        let mut key = e.key.clone();
        let mut value = e.value.clone();
        let spec = spec_for(s, &key);
        let ok_w = 35.2f32;
        let cancel_w = 56.8f32;
        let key_rect = Rect::from_min_size(Pos2::new(row_left, y), Vec2::new((inner * 0.4).round() + 4.0 - 4.0 + 4.0, 24.0 + 2.0));
        let key_box = Rect::from_min_size(key_rect.min, Vec2::new(300.0f32.min(key_rect.width() - 4.0), 24.0));
        let val_box = Rect::from_min_max(Pos2::new(val_x - 5.4, y), Pos2::new(row_left + row_w - ok_w - cancel_w - 8.0, y + 24.0));
        // Key.
        let closed_keys: Vec<String> = if s.object_closed { s.object_properties.keys().cloned().collect() } else { Vec::new() };
        if !closed_keys.is_empty() {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(key_box).layout(egui::Layout::left_to_right(egui::Align::Center)));
            egui::ComboBox::from_id_salt(id.with("edit-key-select")).selected_text(if key.is_empty() { "Key".to_string() } else { key.clone() }).width(key_box.width() - 8.0).show_ui(&mut child, |ui| {
                for k in &closed_keys {
                    ui.selectable_value(&mut key, k.clone(), k);
                }
            });
        } else {
            let slot = painter.add(egui::Shape::Noop);
            let inner_r = key_box.shrink2(Vec2::new(6.0, 1.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner_r).layout(egui::Layout::left_to_right(egui::Align::Center)));
            child.set_clip_rect(inner_r.intersect(clip));
            child.visuals_mut().weak_text_color = Some(Palette::INPUT_PLACEHOLDER_FG());
            child.visuals_mut().text_cursor.stroke.color = Palette::INPUT_FG();
            let resp = child.add_sized(
                inner_r.size(),
                egui::TextEdit::singleline(&mut key).id_salt(id.with("edit-key")).frame(egui::Frame::NONE).margin(egui::Margin::ZERO).hint_text("Key").font(FontId::proportional(13.0)).text_color(Palette::INPUT_FG()).vertical_align(egui::Align::Center),
            );
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Key"));
            if e.focus {
                resp.request_focus();
            }
            let border = if resp.has_focus() { Palette::FOCUS_BORDER() } else { Palette::SETTINGS_TEXT_INPUT_BORDER() };
            painter.set(slot, egui::epaint::RectShape::new(key_box, 2.0, Palette::INPUT_BG(), Stroke::new(1.0, border), egui::StrokeKind::Inside));
        }
        // Value: a text box, or a select of the options.
        let mut enter = false;
        match &spec {
            Spec::Text => {
                let slot = painter.add(egui::Shape::Noop);
                let inner_r = val_box.shrink2(Vec2::new(6.0, 1.0));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner_r).layout(egui::Layout::left_to_right(egui::Align::Center)));
                child.set_clip_rect(inner_r.intersect(clip));
                child.visuals_mut().weak_text_color = Some(Palette::INPUT_PLACEHOLDER_FG());
                child.visuals_mut().text_cursor.stroke.color = Palette::INPUT_FG();
                let resp = child.add_sized(
                    inner_r.size(),
                    egui::TextEdit::singleline(&mut value).id_salt(id.with("edit-value")).frame(egui::Frame::NONE).margin(egui::Margin::ZERO).hint_text("Value").font(FontId::proportional(13.0)).text_color(Palette::INPUT_FG()).vertical_align(egui::Align::Center),
                );
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Value"));
                let border = if resp.has_focus() { Palette::FOCUS_BORDER() } else { Palette::SETTINGS_TEXT_INPUT_BORDER() };
                painter.set(slot, egui::epaint::RectShape::new(val_box, 2.0, Palette::INPUT_BG(), Stroke::new(1.0, border), egui::StrokeKind::Inside));
                enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            }
            Spec::Select(options) => {
                if !options.contains(&value) {
                    value = options.first().cloned().unwrap_or_default();
                }
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(val_box).layout(egui::Layout::left_to_right(egui::Align::Center)));
                egui::ComboBox::from_id_salt(id.with("edit-value-select")).selected_text(value.clone()).width(val_box.width() - 8.0).show_ui(&mut child, |ui| {
                    for o in options {
                        ui.selectable_value(&mut value, o.clone(), o);
                    }
                });
            }
        }
        let valid = !key.trim().is_empty() && (!matches!(spec, Spec::Text) || !value.is_empty());
        let ok_r = Rect::from_min_size(Pos2::new(row_left + row_w - ok_w - cancel_w - 4.0, y), Vec2::new(ok_w, BUTTON_H));
        let cancel_r = Rect::from_min_size(Pos2::new(ok_r.right() + 4.0, y), Vec2::new(cancel_w, BUTTON_H));
        let ok = button(ui, painter, ok_r, "OK", true, valid, id.with("ok"), clip);
        let cancel = button(ui, painter, cancel_r, "Cancel", false, true, id.with("cancel"), clip);
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if ok || (enter && valid) {
            *finish = Some(true);
        } else if cancel || escape {
            *finish = Some(false);
        }
        (key, value)
    };

    for (i, row) in list.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(row_left, y), Vec2::new(row_w, ROW_H));
        if let Some(e) = editing.as_ref().filter(|e| e.index == Some(i)) {
            typed = Some(edit_row(ui, y, e, &mut finish));
            y += BUTTON_H;
            continue;
        }
        let resp = ui.interact(r.intersect(clip), id.with(("row", i)), Sense::click());
        let hovered = ui.rect_contains_pointer(r.intersect(clip));
        if hovered {
            painter.rect_filled(r, 0.0, Palette::LIST_HOVER_BG());
        } else if i % 2 == 1 {
            painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(130, 130, 130, 10));
        }
        let cy = r.center().y;
        let font = FontId::proportional(13.0);
        let clip_p = painter.with_clip_rect(clip);
        // Key and value, each cut at its column (the value leaves 40px for
        // the actions).
        let kg = clip_p.layout(row.key.clone(), font.clone(), Palette::FG(), f32::INFINITY);
        clip_p.with_clip_rect(Rect::from_min_max(Pos2::new(key_x, r.top()), Pos2::new(key_x + key_w, r.bottom()))).galley(Pos2::new(key_x, cy - kg.size().y / 2.0 - 0.5), kg, Palette::FG());
        let vg = clip_p.layout(row.value.clone(), font, Palette::FG(), f32::INFINITY);
        clip_p.with_clip_rect(Rect::from_min_max(Pos2::new(val_x, r.top()), Pos2::new(r.right() - 40.0, r.bottom()))).galley(Pos2::new(val_x, cy - vg.size().y / 2.0 - 0.5), vg, Palette::FG());
        let label = format!("item:{}", row.key);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label.clone()));
        if resp.double_clicked() {
            start_edit = Some((Some(i), row.key.clone(), row.value.clone()));
        }
        if hovered {
            // Edit, then Remove or Reset, 16px icons in 20px boxes at the right.
            let mut actions: Vec<(&str, &str)> = vec![("edit", "Edit Item")];
            if row.removable {
                actions.push(("close", "Remove Item"));
            }
            if row.resettable {
                actions.push(("discard", "Reset Item"));
            }
            for (n, (name, tip)) in actions.iter().rev().enumerate() {
                let c = Pos2::new(r.right() - 12.0 - n as f32 * 22.0 - if *name == "edit" { 2.0 } else { 0.0 }, cy);
                let b = Rect::from_center_size(c, Vec2::new(20.0, 20.0));
                let a = ui.interact(b, id.with(("act", i, *name)), Sense::click());
                if a.hovered() {
                    painter.rect_filled(b, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
                }
                painter.text(b.center(), Align2::CENTER_CENTER, icons::codicon_by_name(name).unwrap_or('x').to_string(), icons::codicon_font(16.0), Palette::FG());
                a.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, *tip));
                if a.on_hover_text(*tip).clicked() {
                    match *name {
                        "close" => result = Some(apply(s, configured, &Change::Remove(row.key.clone()))),
                        "discard" => result = Some(apply(s, configured, &Change::Reset(row.key.clone()))),
                        _ => start_edit = Some((Some(i), row.key.clone(), row.value.clone())),
                    }
                }
            }
        }
        y += ROW_H;
    }

    if let Some(e) = editing.as_ref().filter(|e| e.index.is_none()) {
        typed = Some(edit_row(ui, y, e, &mut finish));
    } else {
        let label = "Add Item";
        let w = text_width(painter, label, 12.0) + 21.0;
        let r = Rect::from_min_size(Pos2::new(left, y + 6.0), Vec2::new(w, BUTTON_H));
        if button(ui, painter, r, label, true, true, id.with("add"), clip) {
            start_edit = Some((None, String::new(), String::new()));
        }
    }

    if let Some((index, key, value)) = start_edit {
        *edit = Some(Edit { setting: s.key.clone(), index, original_key: index.map(|_| key.clone()), key, value, focus: true });
    } else if let Some(ok) = finish {
        let e = editing.clone().unwrap();
        *edit = None;
        if ok {
            if let Some((key, value)) = typed.map(|(k, v)| (k.trim().to_string(), v)) {
                let change = match &e.original_key {
                    Some(original) => Change::Update { original: original.clone(), key, value },
                    None => Change::Add { key, value },
                };
                result = Some(apply(s, configured, &change));
            }
        }
    } else if let (Some(e), Some((k, v))) = (edit.as_mut().filter(|e| e.setting == s.key), typed) {
        e.key = k;
        e.value = v;
        e.focus = false;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::settings_editor::model;

    fn def(key: &str) -> &'static SettingDef {
        &model::settings()[model::find(key).unwrap()]
    }

    #[test]
    fn rows_overlay_the_defaults_with_the_configured_object() {
        let s = def("explorer.fileNesting.patterns");
        let defaults = rows(s, None);
        assert_eq!(defaults[0].key, "*.ts");
        assert_eq!(defaults[0].value, "${capture}.js");
        assert!(!defaults[0].removable, "a default can't be removed, only changed");
        let configured = serde_json::json!({"*.ts": "x.js", "*.vue": "${capture}.css"});
        let r = rows(s, Some(&configured));
        assert_eq!(r.len(), defaults.len() + 1);
        assert!(r[0].resettable && r.iter().find(|x| x.key == "*.vue").unwrap().removable);
    }

    #[test]
    fn changes_keep_only_what_differs_from_the_defaults() {
        let s = def("files.associations");
        let v = apply(s, None, &Change::Add { key: "*.x".into(), value: "html".into() }).unwrap();
        assert_eq!(v, serde_json::json!({"*.x": "html"}));
        let v = apply(s, Some(&v), &Change::Update { original: "*.x".into(), key: "*.y".into(), value: "html".into() }).unwrap();
        assert_eq!(v, serde_json::json!({"*.y": "html"}));
        // Removing the last entry resets the setting.
        assert_eq!(apply(s, Some(&v), &Change::Remove("*.y".into())), None);
        // A default changed and put back leaves nothing.
        let n = def("explorer.fileNesting.patterns");
        let v = apply(n, None, &Change::Update { original: "*.ts".into(), key: "*.ts".into(), value: "a".into() }).unwrap();
        assert_eq!(v, serde_json::json!({"*.ts": "a"}));
        assert_eq!(apply(n, Some(&v), &Change::Update { original: "*.ts".into(), key: "*.ts".into(), value: "${capture}.js".into() }), None);
        assert_eq!(apply(n, Some(&v), &Change::Reset("*.ts".into())), None);
    }

    #[test]
    fn value_editors_follow_the_schema() {
        assert_eq!(spec_for(def("files.associations"), "*.x"), Spec::Text);
        assert_eq!(spec_for(def("notebook.cellToolbarLocation"), "default"), Spec::Select(vec!["left".into(), "right".into(), "hidden".into()]));
        assert_eq!(spec_for(def("terminal.integrated.suggest.quickSuggestions"), "commands"), Spec::Select(vec!["on".into(), "off".into()]));
    }
}
