//! vscode-radio
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-radio/vscode-radio.ts
//! Docs:     https://vscode-elements.github.io/components/radio/
//! VS Code analogue: src/vs/base/browser/ui/radio/radio.ts (group container);
//!                   the classic circular radio is rendered by toggle.ts.
//! Tokens:   --vscode-input-background (ring fill) → Palette::VSCE_INPUT_BG
//!           --vscode-checkbox-border → Palette::VSCE_CHECKBOX_BORDER
//!           --vscode-button-background (dot) → Palette::VSCE_BUTTON_BG
//!           --vscode-focusBorder → Palette::VSCE_FOCUS_BORDER
//!
//! Classic 16×16 circular radio. The caller owns the radio-group state
//! (typically an `enum` or `usize` index) — pass `selected: bool` per
//! instance and `*selected = true` on click in the storyboard mapping.

use crate::vscode_widgets::tokens;
use egui::{Color32, FontId, Response, Sense, Stroke, Ui, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct RadioProps<'a> {
    pub label: Option<&'a str>,
    pub disabled: bool,
    pub focused: bool,
}

impl<'a> Default for RadioProps<'a> {
    fn default() -> Self {
        Self {
            label: None,
            disabled: false,
            focused: false,
        }
    }
}

impl<'a> RadioProps<'a> {
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

pub fn radio(ui: &mut Ui, props: &RadioProps<'_>, selected: &mut bool) -> Response {
    let dot_size = 16.0;
    let gap = 8.0;
    let label_text = props.label.unwrap_or("");

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        let label_galley = if !label_text.is_empty() {
            Some(ui.painter().layout_no_wrap(
                label_text.to_string(),
                FontId::proportional(13.0),
                Color32::WHITE,
            ))
        } else {
            None
        };
        let label_w = label_galley.as_ref().map(|g| g.size().x).unwrap_or(0.0);
        let total = Vec2::new(dot_size + gap + label_w, dot_size.max(20.0));
        let sense = if props.disabled { Sense::hover() } else { Sense::click() };
        let (rect, response) = ui.allocate_exact_size(total, sense);

        let alpha = if props.disabled { 0.5 } else { 1.0 };
        let painter = ui.painter().clone();
        let cy = rect.center().y;
        let center = egui::pos2(rect.left() + dot_size * 0.5, cy);

        painter.circle_filled(center, dot_size * 0.5, with_alpha(tokens::INPUT_BG, alpha));

        let focus_visible = props.focused || (response.has_focus() && !props.disabled);
        let border = if focus_visible {
            tokens::FOCUS_BORDER
        } else {
            tokens::CHECKBOX_BORDER
        };
        painter.circle_stroke(
            center,
            dot_size * 0.5 - 0.5,
            Stroke::new(1.0, with_alpha(border, alpha)),
        );

        if *selected {
            painter.circle_filled(center, 4.0, with_alpha(tokens::BUTTON_BG, alpha));
        }

        if let Some(galley) = label_galley {
            let label_pos = egui::pos2(rect.left() + dot_size + gap, cy - galley.size().y * 0.5);
            painter.galley(label_pos, galley, with_alpha(tokens::FG, alpha));
        }

        if response.clicked() {
            *selected = true;
        }
        response
    })
    .inner
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
