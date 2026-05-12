//! vscode-button
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-button/vscode-button.ts
//! Docs:     https://vscode-elements.github.io/components/button/
//! VS Code analogue: src/vs/base/browser/ui/button/button.ts
//! Tokens:   --vscode-button-background → Palette::VSCE_BUTTON_BG
//!           --vscode-button-foreground → Palette::VSCE_BUTTON_FG
//!           --vscode-button-hoverBackground → Palette::VSCE_BUTTON_HOVER_BG
//!           --vscode-button-secondaryBackground → Palette::VSCE_BUTTON_SECONDARY_BG
//!           --vscode-button-secondaryForeground → Palette::VSCE_BUTTON_SECONDARY_FG
//!           --vscode-button-secondaryHoverBackground → Palette::VSCE_BUTTON_SECONDARY_HOVER_BG
//!           --vscode-focusBorder → Palette::VSCE_FOCUS_BORDER
//!
//! Rectangular button. Padding 4×8, border-radius 4, font-size 12, line-height 16.
//! Supports primary/secondary variants, icon-before/icon-after, focus ring,
//! and a `block` flag that stretches the button to the layout width.

use crate::icons::codicon_font;
use crate::vscode_widgets::tokens;
use egui::{Align2, Color32, CornerRadius, FontId, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
}

#[derive(Clone, Copy, Debug)]
pub struct ButtonProps<'a> {
    pub label: &'a str,
    pub variant: ButtonVariant,
    pub disabled: bool,
    /// Programmatic focused state — force-draws the focus ring even without
    /// keyboard focus. Useful for storybook states and form defaults.
    pub focused: bool,
    /// Codicon glyph rendered before the label.
    pub icon: Option<char>,
    /// Codicon glyph rendered after the label.
    pub icon_after: Option<char>,
    /// Fill the available width.
    pub block: bool,
    /// Font/padding scale. Default `Regular`.
    pub size: ButtonSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ButtonSize {
    #[default]
    Regular,
    Small,
}

impl<'a> ButtonProps<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            variant: ButtonVariant::Primary,
            disabled: false,
            focused: false,
            icon: None,
            icon_after: None,
            block: false,
            size: ButtonSize::Regular,
        }
    }

    pub fn secondary(mut self) -> Self {
        self.variant = ButtonVariant::Secondary;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    pub fn focused(mut self) -> Self {
        self.focused = true;
        self
    }

    pub fn icon(mut self, glyph: char) -> Self {
        self.icon = Some(glyph);
        self
    }

    pub fn icon_after(mut self, glyph: char) -> Self {
        self.icon_after = Some(glyph);
        self
    }

    pub fn block(mut self) -> Self {
        self.block = true;
        self
    }

    pub fn small(mut self) -> Self {
        self.size = ButtonSize::Small;
        self
    }

    fn font_size(&self) -> f32 {
        match self.size {
            ButtonSize::Regular => 12.0,
            ButtonSize::Small => 11.0,
        }
    }

    fn padding(&self) -> Vec2 {
        match self.size {
            ButtonSize::Regular => Vec2::new(8.0, 4.0),
            ButtonSize::Small => Vec2::new(6.0, 3.0),
        }
    }

    fn line_height(&self) -> f32 {
        match self.size {
            ButtonSize::Regular => 16.0,
            ButtonSize::Small => 14.0,
        }
    }
}

pub fn button(ui: &mut Ui, props: &ButtonProps<'_>) -> Response {
    let pad = props.padding();
    let lh = props.line_height();
    let font = FontId::proportional(props.font_size());
    let icon_font = codicon_font(props.font_size() + 1.0);
    let gap = 4.0;

    let label_galley =
        ui.painter()
            .layout_no_wrap(props.label.to_string(), font.clone(), Color32::WHITE);

    let mut content_w = label_galley.size().x;
    if props.icon.is_some() {
        content_w += props.font_size() + gap;
    }
    if props.icon_after.is_some() {
        content_w += props.font_size() + gap;
    }

    let height = lh + pad.y * 2.0;
    let width = if props.block {
        ui.available_width()
    } else {
        content_w + pad.x * 2.0
    };

    let sense = if props.disabled {
        Sense::hover()
    } else {
        Sense::click()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), sense);

    let hovered = response.hovered() && !props.disabled;
    let (bg, fg) = match (props.variant, hovered) {
        (ButtonVariant::Primary, true) => (tokens::BUTTON_HOVER_BG, tokens::BUTTON_FG),
        (ButtonVariant::Primary, false) => (tokens::BUTTON_BG, tokens::BUTTON_FG),
        (ButtonVariant::Secondary, true) => {
            (tokens::BUTTON_SECONDARY_HOVER_BG, tokens::BUTTON_SECONDARY_FG)
        }
        (ButtonVariant::Secondary, false) => {
            (tokens::BUTTON_SECONDARY_BG, tokens::BUTTON_SECONDARY_FG)
        }
    };

    let alpha = if props.disabled { 0.4 } else { 1.0 };
    let bg = with_alpha(bg, alpha);
    let fg = with_alpha(fg, alpha);

    let painter = ui.painter().clone();
    painter.rect_filled(rect, CornerRadius::same(4), bg);

    if matches!(props.variant, ButtonVariant::Secondary) {
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.0, with_alpha(tokens::BUTTON_BORDER, alpha)),
            StrokeKind::Inside,
        );
    }

    if props.focused || (response.has_focus() && !props.disabled) {
        let outer = rect.expand(2.0);
        painter.rect_stroke(
            outer,
            CornerRadius::same(5),
            Stroke::new(1.0, tokens::FOCUS_BORDER),
            StrokeKind::Outside,
        );
    }

    let mut cursor_x = rect.left() + pad.x;
    let cy = rect.center().y;

    if let Some(glyph) = props.icon {
        painter.text(
            egui::pos2(cursor_x + props.font_size() * 0.5, cy),
            Align2::CENTER_CENTER,
            glyph.to_string(),
            icon_font.clone(),
            fg,
        );
        cursor_x += props.font_size() + gap;
    }

    let label_pos = egui::pos2(cursor_x, cy - label_galley.size().y * 0.5);
    painter.galley(label_pos, label_galley.clone(), fg);
    cursor_x += label_galley.size().x;

    if let Some(glyph) = props.icon_after {
        cursor_x += gap;
        painter.text(
            egui::pos2(cursor_x + props.font_size() * 0.5, cy),
            Align2::CENTER_CENTER,
            glyph.to_string(),
            icon_font,
            fg,
        );
    }

    // Register the button label in the AccessKit tree so kittest queries
    // (`get_by_label`) can find the action even without painting any
    // <input>-style native widget.
    let label = props.label.to_string();
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, !props.disabled, label.clone())
    });

    response
}

fn with_alpha(color: Color32, alpha: f32) -> Color32 {
    let [r, g, b, a] = color.to_array();
    let a = (a as f32 * alpha) as u8;
    Color32::from_rgba_premultiplied(
        ((r as u16 * a as u16) / 255) as u8,
        ((g as u16 * a as u16) / 255) as u8,
        ((b as u16 * a as u16) / 255) as u8,
        a,
    )
}
