//! The modal editor part (modalEditorPart.ts / modalEditorPart.css): an
//! editor floating over a 50% black backdrop, 80% of the window below the
//! title bar (at most 1400 × 900), centred; maximised it keeps 8px at the
//! sides and the title bar's height above. A 32px header in the title bar
//! colours carries the editor's icon and name and the actions (the
//! editor's own, a separator, Open in Main Window, Maximize, Close).
//! Clicking the backdrop or pressing Escape closes it.

use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, Ui};

use crate::icons;
use crate::theme::Palette;

pub const HEADER_HEIGHT: f32 = 32.0;
const MAX_WIDTH: f32 = 1400.0;
const MAX_HEIGHT: f32 = 900.0;
const MAXIMIZED_PADDING: f32 = 16.0;

#[derive(Clone, Debug, Default)]
pub struct ModalState {
    pub maximized: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalAction {
    Close,
    ToggleMaximized,
    OpenInMainWindow,
    /// One of the editor's own actions, by index.
    Editor(usize),
}

pub struct ModalHeader<'a> {
    pub icon: char,
    pub title: &'a str,
    /// The editor's actions: (codicon name, tooltip).
    pub actions: &'a [(&'a str, &'a str)],
}

/// The modal's outer rectangle (border included) in a window of `screen`
/// with a title bar `title_bar` tall (`getDefaultSize` / `layoutModal`).
pub fn modal_rect(screen: Rect, title_bar: f32, maximized: bool) -> Rect {
    let available = (screen.height() - title_bar).max(0.0);
    let (w, h) = if maximized {
        let vertical = title_bar.max(MAXIMIZED_PADDING);
        ((screen.width() - MAXIMIZED_PADDING).max(0.0), (available - vertical).max(0.0))
    } else {
        ((screen.width() * 0.8).min(MAX_WIDTH), (available * 0.8).min(MAX_HEIGHT).min(available))
    };
    let left = ((screen.width() - w) / 2.0).round();
    let top = title_bar.max((screen.height() - h) / 2.0).round();
    Rect::from_min_size(screen.min + egui::vec2(left, top), egui::vec2(w.round(), h.round()))
}

/// Paints the modal and runs `contents` in the area under the header.
/// Returns the action asked for, if any.
pub fn show(
    ctx: &egui::Context,
    id: egui::Id,
    state: &ModalState,
    header: &ModalHeader<'_>,
    title_bar: f32,
    contents: impl FnOnce(&mut Ui),
) -> Option<ModalAction> {
    let screen = ctx.content_rect();
    let rect = modal_rect(screen, title_bar, state.maximized);
    let mut action = None;
    egui::Area::new(id)
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .movable(false)
        .constrain(false)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            let painter = ui.painter().clone();
            painter.rect_filled(screen, 0.0, Color32::from_black_alpha(128));
            // The backdrop takes every click outside the modal.
            let backdrop = ui.interact(screen, id.with("backdrop"), Sense::click());
            if backdrop.clicked() && !backdrop.interact_pointer_pos().is_some_and(|p| rect.contains(p)) {
                action = Some(ModalAction::Close);
            }
            // Shadow (`--vscode-shadow-xl`), then the part itself.
            painter.add(egui::epaint::Shadow { offset: [0, 8], blur: 32, spread: 0, color: Color32::from_black_alpha(90) }.as_shape(rect, CornerRadius::same(8)));
            painter.rect(rect, CornerRadius::same(8), Palette::EDITOR_BG(), Stroke::NONE, egui::StrokeKind::Inside);
            let inner = rect.shrink(1.0);
            let head = Rect::from_min_size(inner.min, egui::vec2(inner.width(), HEADER_HEIGHT));
            painter.rect_filled(head, CornerRadius { nw: 7, ne: 7, sw: 0, se: 0 }, Palette::TITLE_BAR_BG());
            painter.hline(head.x_range(), head.bottom() + 0.5, Stroke::new(1.0, Palette::TITLE_BAR_BORDER()));
            // Block clicks on the modal from reaching the backdrop.
            ui.interact(rect, id.with("body"), Sense::click());

            // Title: icon at +10, name 8px after it, 12px semibold.
            let cy = head.center().y;
            painter.text(egui::pos2(head.left() + 19.0, cy), Align2::CENTER_CENTER, header.icon.to_string(), icons::codicon_font(16.0), Palette::TITLE_BAR_FG());
            painter.text(egui::pos2(head.left() + 32.5, cy), Align2::LEFT_CENTER, header.title, crate::ui_font::semibold(12.0), Palette::TITLE_BAR_FG());

            // Actions from the right: Close, Maximize, Open in Main Window
            // 26px apart, then a separator and the editor's own actions.
            let mut buttons: Vec<(f32, &str, String, ModalAction)> = vec![
                (19.0, "close", "Close (Escape)".into(), ModalAction::Close),
                (
                    45.0,
                    if state.maximized { "screen-normal" } else { "screen-full" },
                    if state.maximized { "Restore Modal Editor".into() } else { "Maximize Modal Editor".into() },
                    ModalAction::ToggleMaximized,
                ),
                (71.0, "open-in-product", "Open Modal Editor in Main Window".into(), ModalAction::OpenInMainWindow),
            ];
            let sep_x = head.right() - 86.5;
            painter.vline(sep_x, (cy - 8.0)..=(cy + 8.0), Stroke::new(1.0, Palette::TITLE_BAR_FG().gamma_multiply(0.3)));
            for (i, (name, tip)) in header.actions.iter().rev().enumerate() {
                buttons.push((103.5 + 26.0 * i as f32, name, tip.to_string(), ModalAction::Editor(header.actions.len() - 1 - i)));
            }
            for (from_right, name, tip, act) in buttons {
                let c = egui::pos2(head.right() - from_right, cy);
                let r = Rect::from_center_size(c, egui::vec2(22.0, 22.0));
                let resp = ui.interact(r, id.with(("modal-action", name)), Sense::click());
                if resp.hovered() {
                    painter.rect_filled(r, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
                }
                let glyph = icons::codicon_by_name(name).unwrap_or('?');
                painter.text(c, Align2::CENTER_CENTER, glyph.to_string(), icons::codicon_font(16.0), Palette::TITLE_BAR_FG());
                let label = tip.clone();
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
                if resp.on_hover_text(tip).clicked() {
                    action = Some(act);
                }
            }

            let body = Rect::from_min_max(egui::pos2(inner.left(), head.bottom() + 1.0), inner.max);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body).layout(egui::Layout::top_down(egui::Align::Min)));
            child.set_clip_rect(body);
            contents(&mut child);

            // The outline over everything, corners masked by nothing: the
            // backdrop shows through the rounding.
            painter.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, Palette::EDITOR_WIDGET_BORDER()), egui::StrokeKind::Inside);
        });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_size_matches_vs_code() {
        // 1300 × 820 window, 35px title bar: measured 130..1170 × 96..724.
        let r = modal_rect(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1300.0, 820.0)), 35.0, false);
        assert_eq!((r.left(), r.top(), r.right(), r.bottom()), (130.0, 96.0, 1170.0, 724.0));
        let m = modal_rect(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1300.0, 820.0)), 35.0, true);
        assert_eq!((m.left(), m.top(), m.width(), m.height()), (8.0, 35.0, 1284.0, 750.0));
    }
}
