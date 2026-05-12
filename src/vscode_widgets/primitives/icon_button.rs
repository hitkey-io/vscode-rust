//! vscode-icon-button — derived from action-bar pattern.
//! Upstream pattern: src/vs/base/browser/ui/actionbar/actionbar.css
//! Tokens:   --vscode-icon-foreground → Palette::VSCE_ICON_FG
//!           --vscode-list-hoverBackground → Palette::VSCE_LIST_HOVER_BG
//!           --vscode-focusBorder → Palette::VSCE_FOCUS_BORDER
//!
//! Square click target for a single codicon. Used in toolbars, tabs, status
//! bar, activity bar — anywhere the upstream UI shows an icon-only button.

use crate::icons::codicon_font;
use crate::vscode_widgets::tokens;
use egui::{Align2, Color32, CornerRadius, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct IconButtonProps {
    pub glyph: char,
    /// Total side length of the click target in CSS pixels. Default `22.0`
    /// (16px icon + 3px padding each side).
    pub size: f32,
    /// Font size used to render the glyph. Default `16.0`.
    pub icon_size: f32,
    pub disabled: bool,
    /// Draw the active stripe on the left edge (activity-bar pattern).
    pub active_stripe: bool,
    /// Override icon colour. `None` falls back to `tokens::ICON_FG`.
    pub color: Option<Color32>,
    /// Icon colour when hovered (only applied if the cursor is over the
    /// click target). `None` keeps the resting colour.
    pub hover_color: Option<Color32>,
    /// Corner radius. Default `3.0`.
    pub corner_radius: f32,
    /// Disable the default hover background fill. The activity-bar
    /// pattern (VS Code 1.95+) doesn't paint a hover surface — it only
    /// brightens the icon — so the rest of the workbench can suppress
    /// the surface tint by setting this flag.
    pub no_hover_bg: bool,
}

impl Default for IconButtonProps {
    fn default() -> Self {
        Self {
            glyph: '\u{ea74}',
            size: 22.0,
            icon_size: 16.0,
            disabled: false,
            active_stripe: false,
            color: None,
            hover_color: None,
            corner_radius: 3.0,
            no_hover_bg: false,
        }
    }
}

impl IconButtonProps {
    pub fn new(glyph: char) -> Self {
        Self {
            glyph,
            ..Self::default()
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn icon_size(mut self, icon_size: f32) -> Self {
        self.icon_size = icon_size;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    pub fn active_stripe(mut self) -> Self {
        self.active_stripe = true;
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    pub fn hover_color(mut self, color: Color32) -> Self {
        self.hover_color = Some(color);
        self
    }

    pub fn no_hover_bg(mut self) -> Self {
        self.no_hover_bg = true;
        self
    }
}

pub fn icon_button(ui: &mut Ui, props: &IconButtonProps) -> Response {
    let desired = Vec2::splat(props.size);
    let sense = if props.disabled {
        Sense::hover()
    } else {
        Sense::click()
    };
    let (rect, response) = ui.allocate_exact_size(desired, sense);

    let painter = ui.painter().clone();
    let hovered = response.hovered() && !props.disabled;
    if hovered && !props.no_hover_bg {
        painter.rect_filled(rect, CornerRadius::same(props.corner_radius as u8), tokens::LIST_HOVER_BG);
    }

    if response.has_focus() && !props.disabled {
        painter.rect_stroke(
            rect,
            CornerRadius::same(props.corner_radius as u8),
            Stroke::new(1.0, tokens::FOCUS_BORDER),
            StrokeKind::Inside,
        );
    }

    let mut color = if hovered {
        props.hover_color.or(props.color).unwrap_or(tokens::ICON_FG)
    } else {
        props.color.unwrap_or(tokens::ICON_FG)
    };
    if props.disabled {
        let [r, g, b, _] = color.to_array();
        color = Color32::from_rgba_premultiplied(
            ((r as u16 * 102) / 255) as u8,
            ((g as u16 * 102) / 255) as u8,
            ((b as u16 * 102) / 255) as u8,
            102,
        );
    }

    if props.active_stripe {
        let stripe = egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.left() + 2.0, rect.bottom()),
        );
        painter.rect_filled(stripe, 0.0, tokens::FG);
    }

    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        props.glyph.to_string(),
        codicon_font(props.icon_size),
        color,
    );

    response
}
