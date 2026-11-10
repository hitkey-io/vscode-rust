//! The workbench banner (browser/parts/banner/bannerPart.ts + banner.css)
//! with the Restricted Mode message of workspace.contribution.ts: 26px
//! under the title bar across the window, `banner.background`, the shield
//! in `banner.iconForeground` centred 18px in, the 12px message from 32px, then
//! the underlined Manage and Learn More links (15.5px after it, 17.5px apart), and Close
//! centred 21px from the right (measured against 1.145.2).

use egui::{Align2, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::icons;
use crate::theme::Palette;

pub const HEIGHT: f32 = 26.0;

/// Clickable but kept out of egui's arrow-key focus walk: otherwise ↓ in a
/// list elsewhere hands the banner's links the keyboard and Enter
/// "clicks" them.
fn sense() -> Sense {
    Sense::CLICK
}

#[derive(Default, Debug)]
pub struct BannerOutput {
    pub manage: bool,
    pub learn_more: bool,
    pub close: bool,
}

/// The Restricted Mode message for a folder (or the empty window).
pub fn restricted_message(folder: bool) -> &'static str {
    if folder {
        "Restricted Mode is intended for safe code browsing. Trust this folder to enable all features."
    } else {
        "Restricted Mode is intended for safe code browsing. Trust this window to enable all features."
    }
}

pub fn show(ui: &mut Ui, message: &str) -> BannerOutput {
    let mut out = BannerOutput::default();
    let rect = ui.max_rect();
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 0.0, Palette::BANNER_BG());
    let cy = rect.center().y;
    let shield = icons::codicon_by_name("shield").unwrap_or('!');
    painter.text(Pos2::new(rect.left() + 18.0, cy), Align2::CENTER_CENTER, shield.to_string(), icons::codicon_font(14.0), Palette::BANNER_ICON_FG());
    let g = painter.layout_no_wrap(message.to_string(), FontId::proportional(12.0), Palette::BANNER_FG());
    let mut x = rect.left() + 32.0;
    let gw = g.size().x;
    painter.galley(Pos2::new(x, cy - g.size().y / 2.0), g, Palette::BANNER_FG());
    x += gw + 15.5;
    for (label, which) in [("Manage", 0), ("Learn More", 1)] {
        let g = painter.layout_no_wrap(label.to_string(), FontId::proportional(12.0), Palette::BANNER_FG());
        let r = Rect::from_min_size(Pos2::new(x, cy - g.size().y / 2.0), g.size());
        let resp = ui.interact(r, ui.id().with(("banner-link", which)), sense());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        painter.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, Palette::BANNER_FG()));
        painter.galley(r.min, g, Palette::BANNER_FG());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, label));
        if resp.clicked() {
            if which == 0 {
                out.manage = true;
            } else {
                out.learn_more = true;
            }
        }
        x = r.right() + 17.5;
    }
    let close = Rect::from_center_size(Pos2::new(rect.right() - 21.0, cy), Vec2::splat(22.0));
    let resp = ui.interact(close, ui.id().with("banner-close"), sense());
    if resp.hovered() {
        painter.rect_filled(close, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
    }
    painter.text(close.center(), Align2::CENTER_CENTER, icons::codicon_by_name("close").unwrap_or('x').to_string(), icons::codicon_font(16.0), Palette::BANNER_FG());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Close Banner"));
    if resp.on_hover_text("Close Banner").clicked() {
        out.close = true;
    }
    out
}
