use egui::{Align, Layout, Ui};

use crate::icons;
use crate::theme::Palette;
use crate::vscode_widgets::primitives::{icon_button, IconButtonProps};

use super::ActivityView;

const ITEM_HEIGHT: f32 = 48.0;

pub fn show(ui: &mut Ui, current: &mut ActivityView, sidebar_visible: &mut bool) {
    let painter = ui.painter();
    let rect = ui.max_rect();
    painter.rect_filled(rect, 0.0, Palette::ACTIVITY_BAR_BG);

    let right_border = egui::Rect::from_min_size(
        egui::pos2(rect.right() - 1.0, rect.top()),
        egui::vec2(1.0, rect.height()),
    );
    painter.rect_filled(right_border, 0.0, Palette::BORDER);

    ui.allocate_ui_with_layout(
        ui.available_size(),
        Layout::top_down(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (view, glyph, label) in [
                (ActivityView::Explorer, icons::FILES, "Explorer (⇧⌘E)"),
                (ActivityView::Search, icons::SEARCH, "Search (⇧⌘F)"),
            ] {
                let is_selected = *current == view && *sidebar_visible;
                let mut props = IconButtonProps::new(glyph)
                    .size(ITEM_HEIGHT)
                    .icon_size(24.0)
                    .no_hover_bg()
                    .color(Palette::ACTIVITY_BAR_INACTIVE_FG)
                    .hover_color(Palette::ACTIVITY_BAR_FG);
                if is_selected {
                    props = props.active_stripe().color(Palette::ACTIVITY_BAR_FG);
                }
                let response = icon_button(ui, &props);

                // Make the icon findable in the AccessKit tree (kittest get_by_label).
                let tip = label.to_string();
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip.clone())
                });
                let response = response.on_hover_text(label);

                if response.clicked() {
                    if *current == view && *sidebar_visible {
                        *sidebar_visible = false;
                    } else {
                        *current = view;
                        *sidebar_visible = true;
                    }
                }
            }
        },
    );
}
