//! List settings (settingsWidgets.ts `ListSettingWidget`,
//! `ExcludeSettingWidget` / `IncludeSettingWidget`, settingsWidgets.css):
//! one 24px row per item (glob patterns in the editor font), Edit and
//! Remove on hover at the right, "Add Pattern" / "Add Item" below, and an
//! edit row (input, OK, Cancel) while adding or changing an item.
//! Measured against 1.145.2: the list starts 1px into the control area,
//! the button (26px, 12px text, 10px padding) 7.5px under the last row
//! (3px without rows), and the row ends 30px under the button.

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use serde_json::{Map, Value};

use super::model::{SettingDef, ValueType};
use crate::icons;
use crate::theme::Palette;

pub const ROW_H: f32 = 24.0;
const BUTTON_H: f32 = 26.0;

pub fn is_list(vt: ValueType) -> bool {
    matches!(vt, ValueType::Exclude | ValueType::Include | ValueType::Array)
}

fn patterns(vt: ValueType) -> bool {
    matches!(vt, ValueType::Exclude | ValueType::Include)
}

/// One row: the value and, for patterns, its `when` sibling.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub value: String,
    pub sibling: Option<String>,
}

/// An item being added (`index` None) or changed.
#[derive(Clone, Debug)]
pub struct Edit {
    pub key: String,
    pub index: Option<usize>,
    pub text: String,
    pub focus: bool,
}

/// `getIncludeExcludeDisplayValue` / the list's display value.
pub fn items(s: &SettingDef, configured: Option<&Value>) -> Vec<Item> {
    if patterns(s.value_type) {
        let mut data: Map<String, Value> = s.default.as_object().cloned().unwrap_or_default();
        if let Some(Value::Object(c)) = configured {
            for (k, v) in c {
                data.insert(k.clone(), v.clone());
            }
        }
        data.into_iter()
            .filter(|(_, v)| !matches!(v, Value::Bool(false) | Value::Null))
            .map(|(k, v)| Item { value: k, sibling: v.get("when").and_then(Value::as_str).map(str::to_string) })
            .collect()
    } else {
        configured
            .unwrap_or(&s.default)
            .as_array()
            .map(|a| a.iter().map(|v| Item { value: v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()), sibling: None }).collect())
            .unwrap_or_default()
    }
}

/// Height of the control area under the description, gap included.
pub fn control_height(n: usize) -> f32 {
    let gap = if n > 0 { 1.0 + ROW_H * n as f32 + 7.5 } else { 3.0 };
    super::CONTROL_GAP + gap + BUTTON_H + 4.0
}

/// The new value after removing `original` and/or adding `new`
/// (`onDidChangeIncludeExclude`, or the array with the item replaced).
pub fn change(s: &SettingDef, configured: Option<&Value>, list: &[Item], original: Option<usize>, new: Option<&str>) -> Option<Value> {
    if patterns(s.value_type) {
        let default = s.default.as_object().cloned().unwrap_or_default();
        let mut value: Map<String, Value> = configured.and_then(Value::as_object).cloned().unwrap_or_default();
        if let Some(item) = original.and_then(|i| list.get(i)) {
            if default.contains_key(&item.value) {
                value.insert(item.value.clone(), Value::Bool(false));
            } else {
                value.remove(&item.value);
            }
        }
        if let Some(new) = new {
            if default.contains_key(new) {
                value.remove(new);
            } else {
                value.insert(new.to_string(), Value::Bool(true));
            }
        }
        if value.is_empty() {
            return None;
        }
        let mut keys: Vec<String> = value.keys().cloned().collect();
        keys.sort_by(|a, b| super::model::locale_compare(a, b));
        Some(Value::Object(keys.into_iter().map(|k| { let v = value[&k].clone(); (k, v) }).collect()))
    } else {
        let mut arr: Vec<Value> = configured.unwrap_or(&s.default).as_array().cloned().unwrap_or_default();
        let item = |t: &str| match s.array_item_type.as_deref() {
            Some("number") | Some("integer") => t.trim().parse::<f64>().ok().map(|n| if n.fract() == 0.0 { serde_json::json!(n as i64) } else { serde_json::json!(n) }).unwrap_or_else(|| Value::String(t.to_string())),
            _ => Value::String(t.to_string()),
        };
        match (original, new) {
            (Some(i), Some(t)) if i < arr.len() => arr[i] = item(t),
            (Some(i), None) if i < arr.len() => {
                arr.remove(i);
            }
            (None, Some(t)) => arr.push(item(t)),
            _ => {}
        }
        Some(Value::Array(arr))
    }
}

fn button(ui: &mut Ui, painter: &egui::Painter, at: Pos2, label: &str, primary: bool, id: egui::Id, clip: Rect) -> (Rect, bool) {
    let g = painter.layout_no_wrap(label.to_string(), FontId::proportional(12.0), Color32::PLACEHOLDER);
    let r = Rect::from_min_size(at, Vec2::new(g.size().x + 21.0, BUTTON_H));
    let resp = ui.interact(r.intersect(clip), id, Sense::click());
    let (bg, fg) = if primary {
        (if resp.hovered() { Palette::BUTTON_HOVER() } else { Palette::BUTTON_BG() }, Palette::BUTTON_FG())
    } else {
        (if resp.hovered() { Palette::VSCE_BUTTON_SECONDARY_HOVER_BG() } else { Palette::VSCE_BUTTON_SECONDARY_BG() }, Palette::VSCE_BUTTON_SECONDARY_FG())
    };
    painter.rect_filled(r, 4.0, bg);
    painter.galley(Pos2::new(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, fg);
    let text = label.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text.clone()));
    (r, resp.clicked())
}

/// Draws the list at `top` (where the control area starts, the 9px gap
/// included) and returns a new value when it changed (`Some(None)` resets).
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
    let list = items(s, configured);
    let mono = patterns(s.value_type);
    let mut result = None;
    let mut y = top + super::CONTROL_GAP + 1.0;
    let editing_here = edit.as_ref().filter(|e| e.key == s.key).cloned();
    let mut start_edit: Option<Option<usize>> = None;
    let mut finish: Option<Option<String>> = None; // Some(Some) = OK, Some(None) = cancel

    let mut edit_row = |ui: &mut Ui, y: f32, e: &Edit, finish: &mut Option<Option<String>>| {
        let input = Rect::from_min_size(Pos2::new(left + 1.0, y), Vec2::new(320.0f32.min(width - 120.0), ROW_H));
        let slot = painter.add(egui::Shape::Noop);
        let mut text = e.text.clone();
        let inner = input.shrink2(Vec2::new(6.0, 1.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
        child.set_clip_rect(inner.intersect(clip));
        child.visuals_mut().weak_text_color = Some(Palette::INPUT_PLACEHOLDER_FG());
        child.visuals_mut().text_cursor.stroke.color = Palette::INPUT_FG();
        let placeholder = match s.value_type {
            ValueType::Exclude => "Exclude Pattern...",
            ValueType::Include => "Include Pattern...",
            _ => "Item...",
        };
        let resp = child.add_sized(
            inner.size(),
            egui::TextEdit::singleline(&mut text)
                .id_salt(id.with("edit-input"))
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .hint_text(placeholder)
                .font(if mono { crate::ui_font::mono(13.0) } else { FontId::proportional(13.0) })
                .text_color(Palette::INPUT_FG())
                .vertical_align(egui::Align::Center),
        );
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, placeholder));
        if e.focus {
            resp.request_focus();
        }
        painter.set(slot, egui::epaint::RectShape::new(input, 2.0, Palette::INPUT_BG(), Stroke::new(1.0, Palette::FOCUS_BORDER()), egui::StrokeKind::Inside));
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let (ok_r, ok) = button(ui, painter, Pos2::new(input.right() + 4.0, y - 1.0), "OK", true, id.with("ok"), clip);
        let (_, cancel) = button(ui, painter, Pos2::new(ok_r.right() + 4.0, y - 1.0), "Cancel", false, id.with("cancel"), clip);
        if ok || enter {
            *finish = Some(Some(text.clone()));
        } else if cancel || escape {
            *finish = Some(None);
        }
        (text, resp.has_focus())
    };

    let mut typed: Option<(String, bool)> = None;
    for (i, item) in list.iter().enumerate() {
        let row = Rect::from_min_size(Pos2::new(left + 1.0, y), Vec2::new(width - 2.0, ROW_H));
        if let Some(e) = editing_here.as_ref().filter(|e| e.index == Some(i)) {
            typed = Some(edit_row(ui, y, e, &mut finish));
            y += ROW_H;
            continue;
        }
        let resp = ui.interact(row.intersect(clip), id.with(("item", i)), Sense::click());
        let hovered = ui.rect_contains_pointer(row.intersect(clip));
        if hovered {
            painter.rect_filled(row, 0.0, Palette::LIST_HOVER_BG());
        }
        let font = if mono { crate::ui_font::mono(13.0) } else { FontId::proportional(13.0) };
        let tx = row.left() + 3.5;
        let g = painter.layout_no_wrap(item.value.clone(), font, Palette::FG());
        let gw = g.size().x;
        painter.galley(Pos2::new(tx, row.center().y - g.size().y / 2.0), g, Palette::FG());
        if let Some(sib) = &item.sibling {
            painter.text(
                Pos2::new(tx + gw + 6.5, row.center().y),
                Align2::LEFT_CENTER,
                format!("When: {sib}"),
                FontId::proportional(11.7),
                Palette::FG().gamma_multiply(0.7),
            );
        }
        let label = format!("item:{}", item.value);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label.clone()));
        if resp.double_clicked() {
            start_edit = Some(Some(i));
        }
        if hovered {
            let (edit_tip, remove_tip) = if mono {
                (if s.value_type == ValueType::Exclude { "Edit Exclude Item" } else { "Edit Include Item" }, if s.value_type == ValueType::Exclude { "Remove Exclude Item" } else { "Remove Include Item" })
            } else {
                ("Edit Item", "Remove Item")
            };
            for (cx, name, tip) in [(row.right() - 12.0, "close", remove_tip), (row.right() - 36.0, "edit", edit_tip)] {
                let r = Rect::from_center_size(Pos2::new(cx, row.center().y), Vec2::new(20.0, 20.0));
                let a = ui.interact(r, id.with(("act", i, name)), Sense::click());
                if a.hovered() {
                    painter.rect_filled(r, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
                }
                painter.text(r.center(), Align2::CENTER_CENTER, icons::codicon_by_name(name).unwrap_or('x').to_string(), icons::codicon_font(16.0), Palette::FG());
                a.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
                if a.on_hover_text(tip).clicked() {
                    if name == "close" {
                        result = Some(change(s, configured, &list, Some(i), None));
                    } else {
                        start_edit = Some(Some(i));
                    }
                }
            }
        }
        y += ROW_H;
    }
    let button_top = if list.is_empty() { top + super::CONTROL_GAP + 3.0 } else { y + 7.5 };
    if let Some(e) = editing_here.as_ref().filter(|e| e.index.is_none()) {
        typed = Some(edit_row(ui, button_top, e, &mut finish));
    } else {
        let label = if mono { "Add Pattern" } else { "Add Item" };
        let (_, clicked) = button(ui, painter, Pos2::new(left, button_top), label, true, id.with("add"), clip);
        if clicked {
            start_edit = Some(None);
        }
    }

    if let Some(index) = start_edit {
        let text = index.and_then(|i| list.get(i)).map(|it| it.value.clone()).unwrap_or_default();
        *edit = Some(Edit { key: s.key.clone(), index, text, focus: true });
    } else if let Some(done) = finish {
        let e = editing_here.clone().unwrap();
        *edit = None;
        if let Some(text) = done.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
            if e.index.and_then(|i| list.get(i)).map(|it| it.value.as_str()) != Some(text.as_str()) {
                result = Some(change(s, configured, &list, e.index, Some(&text)));
            }
        }
    } else if let (Some(e), Some((text, _))) = (edit.as_mut().filter(|e| e.key == s.key), typed) {
        e.text = text;
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
    fn exclude_items_merge_defaults_and_overrides() {
        let s = def("files.exclude");
        let all = items(s, None);
        assert_eq!(all.iter().map(|i| i.value.as_str()).collect::<Vec<_>>(), ["**/.git", "**/.svn", "**/.hg", "**/.jj", "**/.DS_Store", "**/Thumbs.db"]);
        // Removing a default writes `false`; adding writes `true`.
        let v = change(s, None, &all, Some(1), None).unwrap();
        assert_eq!(v, serde_json::json!({"**/.svn": false}));
        let v = change(s, Some(&v), &items(s, Some(&v)), None, Some("**/node_modules")).unwrap();
        assert_eq!(v, serde_json::json!({"**/.svn": false, "**/node_modules": true}));
        assert_eq!(items(s, Some(&v)).len(), 6);
        // Adding a default back removes its override; nothing left resets.
        let v = change(s, Some(&serde_json::json!({"**/.svn": false})), &[], None, Some("**/.svn"));
        assert_eq!(v, None);
    }

    #[test]
    fn arrays_add_change_remove() {
        let s = model::settings().iter().find(|s| s.value_type == ValueType::Array && s.array_item_type.as_deref() == Some("string")).unwrap();
        let v = change(s, Some(&serde_json::json!(["a"])), &[], None, Some("b")).unwrap();
        assert_eq!(v, serde_json::json!(["a", "b"]));
        let v = change(s, Some(&v), &[], Some(0), Some("c")).unwrap();
        assert_eq!(v, serde_json::json!(["c", "b"]));
        let v = change(s, Some(&v), &[], Some(1), None).unwrap();
        assert_eq!(v, serde_json::json!(["c"]));
    }
}
