//! The Extensions view (⇧⌘X): installed extensions with their state.
//!
//! VS Code analogues:
//! - `src/vs/workbench/contrib/extensions/browser/extensionsViewlet.ts`
//! - `src/vs/workbench/contrib/extensions/browser/extensionsList.ts`
//! - `src/vs/workbench/contrib/extensions/browser/media/extension.css`
//!   (72px rows, 42px icon, name in 600 weight, description and publisher
//!   lines)
//!
//! Tokens: `--vscode-sideBarSectionHeader-*`, `--vscode-descriptionForeground`,
//! `--vscode-list-hoverBackground`, `--vscode-notificationsErrorIcon-foreground`.

use egui::{Align2, FontId, Sense, Ui};

use crate::extensions::{Activation, ExtensionService};
use crate::icons::codicon_font;
use crate::theme::Palette;

const SECTION_H: f32 = 22.0;
const ROW_H: f32 = 72.0;
const PAD_LEFT: f32 = 16.0;
const ICON: f32 = 42.0;
const ICON_GAP: f32 = 14.0;

pub fn show(ui: &mut Ui, service: &ExtensionService) {
    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

    // Section header: "INSTALLED" with the count on the right.
    let (header, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), SECTION_H), Sense::hover());
    let p = ui.painter();
    p.text(
        egui::pos2(header.left() + 8.0, header.center().y),
        Align2::LEFT_CENTER,
        crate::icons::CHEVRON_DOWN.to_string(),
        codicon_font(16.0),
        Palette::SIDEBAR_SECTION_HEADER_FG,
    );
    p.text(
        egui::pos2(header.left() + 26.0, header.center().y),
        Align2::LEFT_CENTER,
        "INSTALLED",
        crate::ui_font::bold(11.0),
        Palette::SIDEBAR_SECTION_HEADER_FG,
    );
    let count = service.extensions.len().to_string();
    let badge_w = ui
        .painter()
        .layout_no_wrap(count.clone(), FontId::proportional(11.0), Palette::FG)
        .size()
        .x
        + 10.0;
    let badge = egui::Rect::from_center_size(
        egui::pos2(header.right() - 12.0 - badge_w / 2.0, header.center().y),
        egui::vec2(badge_w.max(18.0), 18.0),
    );
    ui.painter().rect_filled(badge, 9.0, Palette::VSCE_BADGE_BG);
    ui.painter().text(
        badge.center(),
        Align2::CENTER_CENTER,
        count,
        FontId::proportional(11.0),
        Palette::VSCE_BADGE_FG,
    );

    if service.extensions.is_empty() {
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.add_space(PAD_LEFT);
            ui.set_max_width(ui.available_width() - PAD_LEFT);
            ui.label(
                egui::RichText::new(
                    "No extensions installed. Extensions are read from ~/.vscode/extensions \
                     and ~/.vscode-rust/extensions.",
                )
                .size(13.0)
                .color(Palette::FG_DESCRIPTION),
            );
        });
        return;
    }

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        for ext in &service.extensions {
            let (rect, resp) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::hover());
            let state = service.activation.get(&ext.id).cloned().unwrap_or(Activation::Inactive);
            let status = match &state {
                Activation::Inactive if ext.main.is_none() => "",
                Activation::Inactive => "Not activated",
                Activation::Active => "Activated",
                Activation::Failed(_) => "Activation failed",
                Activation::NeedsNode => "Needs Node.js",
            };
            let a11y = format!("{}, {}, {}", ext.display_name, ext.publisher, status);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, a11y.clone()));
            let tooltip = match &state {
                Activation::Failed(e) => format!("{}\n{e}", ext.id),
                _ => format!("{} v{}\n{}", ext.id, ext.version, ext.path.display()),
            };
            let resp = resp.on_hover_text(tooltip);

            let p = ui.painter_at(rect);
            if resp.hovered() {
                p.rect_filled(rect, 0.0, Palette::LIST_HOVER_BG);
            }
            // Icon placeholder: the generic extension glyph in a 42px box.
            let icon = egui::Rect::from_min_size(
                egui::pos2(rect.left() + PAD_LEFT, rect.center().y - ICON / 2.0),
                egui::vec2(ICON, ICON),
            );
            p.rect_filled(icon, 4.0, Palette::INPUT_BG);
            p.text(
                icon.center(),
                Align2::CENTER_CENTER,
                crate::icons::EXTENSIONS.to_string(),
                codicon_font(26.0),
                Palette::FG_DESCRIPTION,
            );

            let x = icon.right() + ICON_GAP;
            let right = rect.right() - 12.0;
            let name = p.layout_no_wrap(ext.display_name.clone(), crate::ui_font::bold(13.0), Palette::FG);
            let name_w = name.size().x;
            p.galley(egui::pos2(x, rect.top() + 10.0), name, Palette::FG);
            p.text(
                egui::pos2(x + name_w + 6.0, rect.top() + 10.0 + 8.0),
                Align2::LEFT_CENTER,
                format!("v{}", ext.version),
                FontId::proportional(11.0),
                Palette::FG_DESCRIPTION,
            );
            let desc = p.layout(
                ext.description.clone(),
                FontId::proportional(13.0),
                Palette::FG_DESCRIPTION,
                (right - x).max(40.0),
            );
            // One line, clipped like `text-overflow: ellipsis`.
            let desc_rect = egui::Rect::from_min_max(
                egui::pos2(x, rect.top() + 29.0),
                egui::pos2(right, rect.top() + 47.0),
            );
            ui.painter_at(desc_rect).galley(desc_rect.min, desc, Palette::FG_DESCRIPTION);
            p.text(
                egui::pos2(x, rect.bottom() - 15.0),
                Align2::LEFT_CENTER,
                &ext.publisher,
                FontId::proportional(12.0),
                Palette::FG,
            );
            let status_color = match state {
                Activation::Failed(_) | Activation::NeedsNode => Palette::VSCE_NOTIFICATIONS_ERROR_ICON,
                _ => Palette::FG_DESCRIPTION,
            };
            p.text(
                egui::pos2(right, rect.bottom() - 15.0),
                Align2::RIGHT_CENTER,
                status,
                FontId::proportional(12.0),
                status_color,
            );
        }
    });
}
