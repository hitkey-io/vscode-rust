use egui::{Align, Align2, FontId, Layout, Ui};

use crate::icons;
use crate::theme::Palette;
use crate::vscode_widgets::primitives::{icon_button, IconButtonProps};

use super::ActivityView;

const ITEM_HEIGHT: f32 = 48.0;

/// `scm_count` paints the Source Control count badge (VS Code `scm.countBadge`).
/// Returns `true` if the user clicked one of the visual-only icons (Run/Debug,
/// Extensions, Accounts, Manage gear); the caller opens the command palette.
pub fn show(
    ui: &mut Ui,
    current: &mut ActivityView,
    sidebar_visible: &mut bool,
    scm_count: usize,
) -> bool {
    show_with_containers(ui, current, sidebar_visible, scm_count, &[], &mut None)
}

/// An extension's activity bar entry (`viewsContainers`).
pub struct ContainerIcon {
    pub id: String,
    pub title: String,
    /// White mask tinted like the built-in icons, if the file loaded.
    pub icon: Option<egui::TextureHandle>,
}

/// [`show`] plus the extensions' containers below the built-in views;
/// `selected` is the container showing when `current` is
/// `ActivityView::ExtensionContainer`.
pub fn show_with_containers(
    ui: &mut Ui,
    current: &mut ActivityView,
    sidebar_visible: &mut bool,
    scm_count: usize,
    containers: &[ContainerIcon],
    selected: &mut Option<String>,
) -> bool {
    let mut palette_requested = false;
    let painter = ui.painter();
    let rect = ui.max_rect();
    painter.rect_filled(rect, 0.0, Palette::ACTIVITY_BAR_BG());

    let right_border = egui::Rect::from_min_size(
        egui::pos2(rect.right() - 1.0, rect.top()),
        egui::vec2(1.0, rect.height()),
    );
    painter.rect_filled(right_border, 0.0, Palette::BORDER());

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
                (ActivityView::Extensions, icons::EXTENSIONS, "Extensions (⇧⌘X)"),
            ] {
                // Run & Debug sits between Source Control and Extensions
                // (VS Code's default order). It has no view yet: clicking it
                // opens the command palette.
                if view == ActivityView::Extensions {
                    let resp = icon_button(
                        ui,
                        &IconButtonProps::new(icons::DEBUG_ALT)
                            .size(ITEM_HEIGHT)
                            .icon_size(24.0)
                            .no_hover_bg()
                            .color(Palette::ACTIVITY_BAR_INACTIVE_FG())
                            .hover_color(Palette::ACTIVITY_BAR_FG()),
                    );
                    if resp.clicked() {
                        palette_requested = true;
                    }
                    resp.on_hover_text("Run and Debug (⇧⌘D)");
                }
                let is_selected = *current == view && *sidebar_visible;
                let mut props = IconButtonProps::new(glyph)
                    .size(ITEM_HEIGHT)
                    .icon_size(24.0)
                    .no_hover_bg()
                    .color(Palette::ACTIVITY_BAR_INACTIVE_FG())
                    .hover_color(Palette::ACTIVITY_BAR_FG());
                if is_selected {
                    props = props.active_stripe().color(Palette::ACTIVITY_BAR_FG());
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
                        Palette::ACCENT(),
                    );
                    ui.painter().text(
                        center,
                        Align2::CENTER_CENTER,
                        txt,
                        FontId::proportional(10.5),
                        Palette::FG_BRIGHT(),
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
            for c in containers {
                let is_selected = *current == ActivityView::ExtensionContainer
                    && selected.as_deref() == Some(c.id.as_str())
                    && *sidebar_visible;
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(ITEM_HEIGHT, ITEM_HEIGHT), egui::Sense::click());
                let tint = if is_selected || resp.hovered() {
                    Palette::ACTIVITY_BAR_FG()
                } else {
                    Palette::ACTIVITY_BAR_INACTIVE_FG()
                };
                if is_selected {
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(rect.min, egui::vec2(2.0, rect.height())),
                        0.0,
                        Palette::ACTIVITY_BAR_FG(),
                    );
                }
                let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(24.0, 24.0));
                match &c.icon {
                    Some(tex) => {
                        ui.painter().image(
                            tex.id(),
                            icon_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            tint,
                        );
                    }
                    None => {
                        ui.painter().text(rect.center(), Align2::CENTER_CENTER, icons::EXTENSIONS.to_string(), icons::codicon_font(24.0), tint);
                    }
                }
                let title = c.title.clone();
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title.clone()));
                let resp = resp.on_hover_text(&c.title);
                if resp.clicked() {
                    if is_selected {
                        *sidebar_visible = false;
                    } else {
                        *current = ActivityView::ExtensionContainer;
                        *selected = Some(c.id.clone());
                        *sidebar_visible = true;
                    }
                }
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
                        .color(Palette::ACTIVITY_BAR_INACTIVE_FG())
                        .hover_color(Palette::ACTIVITY_BAR_FG()),
                );
                if resp.clicked() {
                    palette_requested = true;
                }
                resp.on_hover_text(label);
            }
        },
    );

    palette_requested
}
