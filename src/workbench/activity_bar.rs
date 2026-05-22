use egui::{Align, Align2, FontId, Layout, Ui};

use crate::icons;
use crate::theme::Palette;
use crate::vscode_widgets::primitives::{icon_button, IconButtonProps};

use super::ActivityView;

const ITEM_HEIGHT: f32 = 48.0;

/// `scm_count` paints the Source Control count badge (VS Code `scm.countBadge`).
pub fn show(
    ui: &mut Ui,
    current: &mut ActivityView,
    sidebar_visible: &mut bool,
    scm_count: usize,
) {
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
                (
                    ActivityView::SourceControl,
                    icons::SOURCE_CONTROL,
                    "Source Control (⌃⇧G)",
                ),
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

                // Source Control count badge (blue pill, bottom-right of icon).
                if view == ActivityView::SourceControl && scm_count > 0 {
                    let r = response.rect;
                    let center = egui::pos2(r.right() - 13.0, r.bottom() - 13.0);
                    let txt = if scm_count > 99 { "99+".to_string() } else { scm_count.to_string() };
                    let w = (txt.len() as f32 * 6.0 + 8.0).max(16.0);
                    let badge = egui::Rect::from_center_size(center, egui::vec2(w, 16.0));
                    ui.painter().rect_filled(
                        badge,
                        egui::CornerRadius::same(8),
                        Palette::ACCENT,
                    );
                    ui.painter().text(
                        center,
                        Align2::CENTER_CENTER,
                        txt,
                        FontId::proportional(10.5),
                        Palette::FG_BRIGHT,
                    );
                }

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

            // Run & Debug and Extensions complete the icon column visually
            // (VS Code's default activity bar). They are inert for now.
            for (glyph, label) in [
                (icons::DEBUG_ALT, "Run and Debug (⇧⌘D)"),
                (icons::EXTENSIONS, "Extensions (⇧⌘X)"),
            ] {
                let resp = icon_button(
                    ui,
                    &IconButtonProps::new(glyph)
                        .size(ITEM_HEIGHT)
                        .icon_size(24.0)
                        .no_hover_bg()
                        .color(Palette::ACTIVITY_BAR_INACTIVE_FG)
                        .hover_color(Palette::ACTIVITY_BAR_FG),
                );
                resp.on_hover_text(label);
            }
        },
    );

    // Bottom cluster: Accounts + Manage (gear), anchored to the bar's bottom.
    let bottom = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.bottom() - 2.0 * ITEM_HEIGHT),
        rect.max,
    );
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(bottom).layout(Layout::top_down(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (glyph, label) in [
                (icons::ACCOUNT, "Accounts"),
                (icons::SETTINGS_GEAR, "Manage"),
            ] {
                let resp = icon_button(
                    ui,
                    &IconButtonProps::new(glyph)
                        .size(ITEM_HEIGHT)
                        .icon_size(24.0)
                        .no_hover_bg()
                        .color(Palette::ACTIVITY_BAR_INACTIVE_FG)
                        .hover_color(Palette::ACTIVITY_BAR_FG),
                );
                resp.on_hover_text(label);
            }
        },
    );
}
