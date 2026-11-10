//! VS Code's custom dialog (base/browser/ui/dialog/dialog.ts +
//! dialog.css), as the workspace trust request uses it ("Do you trust the
//! authors of the files in this folder?" before a terminal starts in
//! Restricted Mode). Measured against 1.145.2: 450px wide, centred over a
//! 50% black backdrop, `editorWidget.background` with a 1px
//! `editorWidget.border` and radius 8; the icon centred 25px in and 32.5px
//! down, the 13px semibold message from 45px, Close 20px from the right;
//! paragraphs from 79.5px down on 18px lines, 14px apart (links underlined
//! in `textLink.foreground`); 26px buttons at the bottom right, 9px apart
//! (8px padding) and 9px from the bottom, 8px from the right, the primary
//! last.

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::icons;
use crate::theme::Palette;

const WIDTH: f32 = 450.0;
const TEXT_LEFT: f32 = 45.0;
// SF draws ≈1.5% wider than VS Code (no `trak`): a little more room keeps
// the line breaks.
const TEXT_W: f32 = 386.0;
const LINE: f32 = 18.0;

pub struct Dialog<'a> {
    pub icon: &'a str,
    pub message: &'a str,
    /// Paragraphs; `[label](url)` makes a link.
    pub details: &'a [&'a str],
    /// Buttons left to right; the last is the primary one.
    pub buttons: &'a [&'a str],
}

#[derive(Debug, PartialEq, Eq)]
pub enum DialogResult {
    Button(usize),
    Closed,
    Link(String),
}

fn paragraph_job(text: &str) -> (egui::text::LayoutJob, Vec<(std::ops::Range<usize>, String)>) {
    let normal = egui::TextFormat { font_id: FontId::proportional(13.0), color: Palette::FG(), line_height: Some(LINE), ..Default::default() };
    let link = egui::TextFormat {
        color: Palette::TEXT_LINK_FG(),
        underline: Stroke::new(1.0, Palette::TEXT_LINK_FG()),
        ..normal.clone()
    };
    let mut job = egui::text::LayoutJob::default();
    let mut links = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find("](").map(|c| open + c) else { break };
        let Some(end) = rest[close..].find(')').map(|e| close + e) else { break };
        job.append(&rest[..open], 0.0, normal.clone());
        let start = job.text.chars().count();
        job.append(&rest[open + 1..close], 0.0, link.clone());
        links.push((start..job.text.chars().count(), rest[close + 2..end].to_string()));
        rest = &rest[end + 1..];
    }
    job.append(rest, 0.0, normal);
    job.wrap.max_width = TEXT_W;
    (job, links)
}

/// Draws the dialog; returns what was chosen (Escape closes it).
pub fn show(ctx: &egui::Context, id: egui::Id, dialog: &Dialog<'_>) -> Option<DialogResult> {
    let screen = ctx.content_rect();
    let mut result = None;
    egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(screen.min).movable(false).constrain(false).show(ctx, |ui| {
        ui.set_min_size(screen.size());
        let painter = ui.painter().clone();
        painter.rect_filled(screen, 0.0, Color32::from_black_alpha(128));
        ui.interact(screen, id.with("backdrop"), Sense::click());

        let galleys: Vec<_> = dialog
            .details
            .iter()
            .map(|p| {
                let (job, links) = paragraph_job(p);
                (ui.fonts_mut(|f| f.layout_job(job)), links)
            })
            .collect();
        let body_h: f32 = galleys.iter().map(|(g, _)| g.rows.len() as f32 * LINE).sum::<f32>() + 14.0 * galleys.len().saturating_sub(1) as f32;
        let height = 70.5 + body_h + 33.5 + 26.0 + 9.0;
        let rect = Rect::from_center_size(screen.center(), Vec2::new(WIDTH, height.round()));
        painter.add(egui::epaint::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(90) }.as_shape(rect, 8));
        painter.rect(rect, 8.0, Palette::EDITOR_WIDGET_BG(), Stroke::new(1.0, Palette::EDITOR_WIDGET_BORDER()), egui::StrokeKind::Inside);
        ui.interact(rect, id.with("body"), Sense::click());

        let glyph = icons::codicon_by_name(dialog.icon).unwrap_or('!');
        painter.text(Pos2::new(rect.left() + 25.0, rect.top() + 32.5), Align2::CENTER_CENTER, glyph.to_string(), icons::codicon_font(24.0), Palette::BUTTON_BG());
        painter.text(Pos2::new(rect.left() + TEXT_LEFT, rect.top() + 34.0), Align2::LEFT_CENTER, dialog.message, crate::ui_font::semibold(13.0), Palette::FG());

        let close = Rect::from_center_size(Pos2::new(rect.right() - 20.0, rect.top() + 19.5), Vec2::splat(22.0));
        let cresp = ui.interact(close, id.with("close"), Sense::click());
        if cresp.hovered() {
            painter.rect_filled(close, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
        }
        painter.text(close.center(), Align2::CENTER_CENTER, icons::codicon_by_name("close").unwrap_or('x').to_string(), icons::codicon_font(16.0), Palette::FG());
        cresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Close Dialog"));
        if cresp.clicked() {
            result = Some(DialogResult::Closed);
        }

        let mut y = rect.top() + 70.5;
        for (i, (g, links)) in galleys.iter().enumerate() {
            let origin = Pos2::new(rect.left() + TEXT_LEFT, y);
            let h = g.rows.len() as f32 * LINE;
            if !links.is_empty() {
                let area = Rect::from_min_size(origin, Vec2::new(TEXT_W, h));
                let resp = ui.interact(area, id.with(("para", i)), Sense::click());
                if let Some(p) = resp.hover_pos() {
                    let ci = g.cursor_from_pos(p - origin).index;
                    if let Some((_, url)) = links.iter().find(|(r, _)| r.contains(&ci)) {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        if resp.clicked() {
                            result = Some(DialogResult::Link(url.clone()));
                        }
                    }
                }
            }
            painter.galley(origin, g.clone(), Color32::PLACEHOLDER);
            y += h + 14.0;
        }

        // Buttons, right-aligned; the primary (last) one has the focus ring.
        let mut x = rect.right() - 8.0;
        let by = rect.bottom() - 9.0 - 26.0;
        for (i, label) in dialog.buttons.iter().enumerate().rev() {
            let primary = i + 1 == dialog.buttons.len();
            let g = painter.layout_no_wrap(label.to_string(), FontId::proportional(13.0), Color32::PLACEHOLDER);
            let w = g.size().x + 16.0;
            let r = Rect::from_min_size(Pos2::new(x - w, by), Vec2::new(w, 26.0));
            let resp = ui.interact(r, id.with(("button", i)), Sense::click());
            let (bg, fg, border) = if primary {
                (if resp.hovered() { Palette::BUTTON_HOVER() } else { Palette::BUTTON_BG() }, Palette::BUTTON_FG(), Color32::TRANSPARENT)
            } else {
                (
                    if resp.hovered() { Palette::VSCE_BUTTON_SECONDARY_HOVER_BG() } else { Palette::VSCE_BUTTON_SECONDARY_BG() },
                    Palette::VSCE_BUTTON_SECONDARY_FG(),
                    Palette::VSCE_BUTTON_SECONDARY_BORDER(),
                )
            };
            painter.rect(r, 4.0, bg, Stroke::new(1.0, border), egui::StrokeKind::Inside);
            if primary {
                painter.rect_stroke(r.expand(2.0), 5.0, Stroke::new(1.0, Palette::FOCUS_BORDER()), egui::StrokeKind::Inside);
            }
            painter.galley(Pos2::new(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, fg);
            let l = label.to_string();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, l.clone()));
            if resp.clicked() {
                result = Some(DialogResult::Button(i));
            }
            x = r.left() - if primary { 9.0 } else { 9.0 };
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            result = Some(DialogResult::Closed);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) && !dialog.buttons.is_empty() {
            result = Some(DialogResult::Button(dialog.buttons.len() - 1));
        }
    });
    result
}
