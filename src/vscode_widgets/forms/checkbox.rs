//! vscode-checkbox
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-checkbox/vscode-checkbox.ts
//! Docs:     https://vscode-elements.github.io/components/checkbox/
//! VS Code analogue: src/vs/base/browser/ui/toggle/toggle.ts
//! Tokens:   --vscode-checkbox-background → Palette::VSCE_CHECKBOX_BG
//!           --vscode-checkbox-border → Palette::VSCE_CHECKBOX_BORDER
//!           --vscode-button-foreground (tick) → Palette::VSCE_BUTTON_FG
//!           --vscode-inputOption-hoverBackground → Palette::VSCE_INPUT_OPTION_HOVER_BG
//!           --vscode-focusBorder → Palette::VSCE_FOCUS_BORDER
//!
//! 18×18 checkbox with optional indeterminate state. Renders the tick from
//! the codicon `check` glyph and the indeterminate mark as a horizontal
//! bar centred in the box.

use crate::icons::{codicon_font, CHECK};
use crate::vscode_widgets::tokens;
use egui::{Align2, Color32, CornerRadius, FontId, RichText, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CheckboxState {
    #[default]
    Unchecked,
    Checked,
    Indeterminate,
}

#[derive(Clone, Copy, Debug)]
pub struct CheckboxProps<'a> {
    /// Optional inline label rendered to the right of the box.
    pub label: Option<&'a str>,
    pub disabled: bool,
    pub focused: bool,
}

impl<'a> Default for CheckboxProps<'a> {
    fn default() -> Self {
        Self {
            label: None,
            disabled: false,
            focused: false,
        }
    }
}

impl<'a> CheckboxProps<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn label(mut self, text: &'a str) -> Self {
        self.label = Some(text);
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
}

pub fn checkbox(ui: &mut Ui, props: &CheckboxProps<'_>, state: &mut CheckboxState) -> Response {
    let box_size = 18.0;
    let label_text = props.label.unwrap_or("");
    let gap = if label_text.is_empty() { 0.0 } else { 9.0 };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        let label_galley = if !label_text.is_empty() {
            Some(
                ui.painter()
                    .layout_no_wrap(label_text.to_string(), FontId::proportional(13.0), Color32::WHITE),
            )
        } else {
            None
        };
        let label_w = label_galley.as_ref().map(|g| g.size().x).unwrap_or(0.0);

        let total = Vec2::new(box_size + gap + label_w, box_size.max(20.0));
        let sense = if props.disabled { Sense::hover() } else { Sense::click() };
        let (rect, response) = ui.allocate_exact_size(total, sense);

        let painter = ui.painter().clone();
        let cy = rect.center().y;
        let box_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left(), cy - box_size * 0.5),
            Vec2::splat(box_size),
        );

        let hovered = response.hovered() && !props.disabled;
        let alpha = if props.disabled { 0.5 } else { 1.0 };
        let bg = if hovered {
            blend(tokens::CHECKBOX_BG, tokens::INPUT_OPTION_HOVER_BG)
        } else {
            tokens::CHECKBOX_BG
        };
        painter.rect_filled(box_rect, CornerRadius::same(3), with_alpha(bg, alpha));

        let focus_visible = props.focused || (response.has_focus() && !props.disabled);
        let border = if focus_visible {
            tokens::FOCUS_BORDER
        } else {
            tokens::CHECKBOX_BORDER
        };
        painter.rect_stroke(
            box_rect,
            CornerRadius::same(3),
            Stroke::new(1.0, with_alpha(border, alpha)),
            StrokeKind::Inside,
        );

        match state {
            CheckboxState::Checked => {
                painter.text(
                    box_rect.center(),
                    Align2::CENTER_CENTER,
                    CHECK.to_string(),
                    codicon_font(14.0),
                    with_alpha(tokens::FG, alpha),
                );
            }
            CheckboxState::Indeterminate => {
                let bar = egui::Rect::from_min_max(
                    egui::pos2(box_rect.left() + 4.0, cy - 1.0),
                    egui::pos2(box_rect.right() - 4.0, cy + 1.0),
                );
                painter.rect_filled(bar, 1.0, with_alpha(tokens::FG, alpha));
            }
            CheckboxState::Unchecked => {}
        }

        if let Some(galley) = label_galley {
            let label_pos = egui::pos2(box_rect.right() + gap, cy - galley.size().y * 0.5);
            painter.galley(label_pos, galley, with_alpha(tokens::FG, alpha));
        }
        // Suppress unused warnings on `RichText` when no label is rendered.
        let _ = RichText::new("");

        if response.clicked() {
            *state = match *state {
                CheckboxState::Unchecked | CheckboxState::Indeterminate => CheckboxState::Checked,
                CheckboxState::Checked => CheckboxState::Unchecked,
            };
        }
        response
    })
    .inner
}

fn blend(base: Color32, overlay: Color32) -> Color32 {
    let [br, bg, bb, _] = base.to_array();
    let [or, og, ob, oa] = overlay.to_array();
    let a = oa as f32 / 255.0;
    let mix = |b: u8, o: u8| -> u8 {
        let b = b as f32;
        let o = o as f32;
        (b * (1.0 - a) + o) as u8
    };
    Color32::from_rgb(mix(br, or), mix(bg, og), mix(bb, ob))
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
