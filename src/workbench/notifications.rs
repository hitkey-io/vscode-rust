//! Notification toasts in the bottom-right corner (extension messages from
//! `window.showInformationMessage` and friends).
//!
//! VS Code analogues:
//! - `src/vs/workbench/browser/parts/notifications/notificationsToasts.ts`
//! - `src/vs/workbench/browser/parts/notifications/media/notificationsToasts.css`
//!   (450px wide, 3px from the right edge, just above the 22px status bar,
//!   4px margin, small corner radius, `--vscode-shadow-lg`)
//! - `notificationsList.css` (10px 5px padding, 18px severity icon, 22px
//!   lines, 12px source line, buttons 4px 5px apart)
//!
//! Tokens: `--vscode-notifications-*`, `--vscode-notificationToast-border`,
//! `--vscode-notifications{Info,Warning,Error}Icon-foreground`,
//! `--vscode-button-*`.

use egui::{Align2, Context, CornerRadius, FontId, Order, Sense, Stroke, StrokeKind};

use crate::extensions::{Notification, Severity};
use crate::icons::{self, codicon_font};
use crate::theme::Palette;

const TOAST_W: f32 = 450.0;
const RIGHT: f32 = 3.0 + 4.0;
const STATUS_BAR_H: f32 = 22.0;
const BOTTOM: f32 = STATUS_BAR_H + 3.0 + 4.0;
const PAD_X: f32 = 5.0;
const PAD_Y: f32 = 10.0;
const ICON: f32 = 18.0;
const LINE: f32 = 22.0;
const BUTTON_H: f32 = 24.0;
/// Info toasts without buttons close on their own, as in VS Code.
const AUTO_HIDE: std::time::Duration = std::time::Duration::from_secs(10);
/// At most this many toasts show at once (newest at the bottom).
const MAX_VISIBLE: usize = 3;

/// What the user did to a toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dismissal {
    pub id: u64,
    /// The clicked button, or `None` for the close button / timeout.
    pub action: Option<usize>,
}

pub fn show(ctx: &Context, notifications: &[Notification]) -> Vec<Dismissal> {
    let mut out = Vec::new();
    let screen = ctx.content_rect();
    let mut bottom = screen.bottom() - BOTTOM;
    let start = notifications.len().saturating_sub(MAX_VISIBLE);

    for n in notifications[start..].iter().rev() {
        if n.severity == Severity::Info && n.actions.is_empty() && n.created.elapsed() >= AUTO_HIDE {
            out.push(Dismissal { id: n.id, action: None });
            continue;
        }
        if n.severity == Severity::Info && n.actions.is_empty() {
            ctx.request_repaint_after(AUTO_HIDE.saturating_sub(n.created.elapsed()));
        }

        let text_w = TOAST_W - PAD_X * 2.0 - (ICON + 8.0) - 28.0;
        let galley = ctx.fonts_mut(|f| {
            f.layout(n.text.clone(), FontId::proportional(13.0), Palette::VSCE_NOTIFICATIONS_FG(), text_w)
        });
        let message_h = galley.size().y.max(LINE);
        let footer = n.source.is_some() || !n.actions.is_empty();
        let height = PAD_Y * 2.0 + message_h + if footer { BUTTON_H + 8.0 } else { 0.0 };
        let rect = egui::Rect::from_min_size(
            egui::pos2(screen.right() - RIGHT - TOAST_W, bottom - height),
            egui::vec2(TOAST_W, height),
        );
        bottom = rect.top() - 8.0;

        egui::Area::new(egui::Id::new(("notification_toast", n.id)))
            .order(Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(rect.size(), Sense::hover());
                let a11y = n.text.clone();
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, a11y.clone()));
                let p = ui.painter();
                p.add(egui::epaint::Shadow {
                    offset: [0, 0],
                    blur: 12,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(36),
                }
                .as_shape(rect, CornerRadius::same(4)));
                p.rect(
                    rect,
                    CornerRadius::same(4),
                    Palette::VSCE_NOTIFICATIONS_BG(),
                    Stroke::new(1.0, Palette::VSCE_NOTIFICATION_TOAST_BORDER()),
                    StrokeKind::Inside,
                );

                let (glyph, color) = match n.severity {
                    Severity::Info => (icons::INFO_ICON, Palette::VSCE_NOTIFICATIONS_INFO_ICON()),
                    Severity::Warning => (icons::WARNING_ICON, Palette::VSCE_NOTIFICATIONS_WARNING_ICON()),
                    Severity::Error => (icons::ERROR_ICON, Palette::VSCE_NOTIFICATIONS_ERROR_ICON()),
                };
                let top = rect.top() + PAD_Y;
                let left = rect.left() + PAD_X;
                p.text(
                    egui::pos2(left + 4.0 + ICON / 2.0, top + LINE / 2.0),
                    Align2::CENTER_CENTER,
                    glyph.to_string(),
                    codicon_font(ICON - 2.0),
                    color,
                );
                let text_left = left + ICON + 8.0;
                p.galley(
                    egui::pos2(text_left, top + (LINE - 17.0) / 2.0),
                    galley,
                    Palette::VSCE_NOTIFICATIONS_FG(),
                );

                // Close button (top-right toolbar).
                let close = egui::Rect::from_center_size(
                    egui::pos2(rect.right() - PAD_X - 12.0, top + LINE / 2.0),
                    egui::vec2(22.0, 22.0),
                );
                let close_resp = ui
                    .interact(close, egui::Id::new(("notification_close", n.id)), Sense::click())
                    .on_hover_text("Clear Notification");
                close_resp.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Clear Notification")
                });
                if close_resp.hovered() {
                    ui.painter().rect_filled(close, 5.0, Palette::VSCE_QUICK_INPUT_ACTION_HOVER_BG());
                }
                ui.painter().text(
                    close.center(),
                    Align2::CENTER_CENTER,
                    icons::CLOSE.to_string(),
                    codicon_font(16.0),
                    Palette::VSCE_NOTIFICATIONS_FG(),
                );
                if close_resp.clicked() {
                    out.push(Dismissal { id: n.id, action: None });
                }

                if !footer {
                    return;
                }
                let footer_cy = rect.bottom() - PAD_Y - BUTTON_H / 2.0;
                if let Some(source) = &n.source {
                    ui.painter().text(
                        egui::pos2(text_left, footer_cy),
                        Align2::LEFT_CENTER,
                        format!("Source: {source}"),
                        FontId::proportional(12.0),
                        Palette::FG_DESCRIPTION(),
                    );
                }
                // Buttons right-aligned; the first one is primary.
                let mut x = rect.right() - PAD_X - 5.0;
                for (i, title) in n.actions.iter().enumerate().rev() {
                    let w = ui
                        .painter()
                        .layout_no_wrap(title.clone(), FontId::proportional(13.0), Palette::FG())
                        .size()
                        .x
                        + 22.0;
                    let r = egui::Rect::from_min_size(
                        egui::pos2(x - w, footer_cy - BUTTON_H / 2.0),
                        egui::vec2(w, BUTTON_H),
                    );
                    x = r.left() - 8.0;
                    let resp = ui.interact(r, egui::Id::new(("notification_action", n.id, i)), Sense::click());
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));
                    let primary = i == 0;
                    let fill = match (primary, resp.hovered()) {
                        (true, false) => Palette::VSCE_BUTTON_2026_BG(),
                        (true, true) => Palette::VSCE_BUTTON_2026_HOVER_BG(),
                        (false, true) => Palette::VSCE_BUTTON_2026_SECONDARY_HOVER_BG(),
                        (false, false) => egui::Color32::TRANSPARENT,
                    };
                    ui.painter().rect(
                        r,
                        CornerRadius::same(4),
                        fill,
                        Stroke::new(1.0, Palette::VSCE_BUTTON_2026_BORDER()),
                        StrokeKind::Inside,
                    );
                    ui.painter().text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        title,
                        FontId::proportional(13.0),
                        if primary { egui::Color32::WHITE } else { Palette::VSCE_NOTIFICATIONS_FG() },
                    );
                    if resp.clicked() {
                        out.push(Dismissal { id: n.id, action: Some(i) });
                    }
                }
            });
    }
    out
}
