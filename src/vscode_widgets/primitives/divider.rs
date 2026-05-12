//! vscode-divider
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-divider/vscode-divider.ts
//! Docs:     https://vscode-elements.github.io/components/divider/
//! VS Code analogue: src/vs/platform/theme/common/colors/baseColors.ts:64 (textSeparator-foreground)
//! Tokens:   --vscode-textSeparator-foreground → Palette::VSCE_TEXT_SEPARATOR_FG
//!
//! A 1px line separator. Renders either horizontally (full width of the
//! parent layout) or vertically (full height).

use crate::vscode_widgets::tokens;
use egui::{Response, Sense, Ui, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DividerOrientation {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug)]
pub struct DividerProps {
    pub orientation: DividerOrientation,
    /// Line thickness in CSS pixels. Default `1.0`.
    pub thickness: f32,
    /// Length in CSS pixels. `None` means "fill the available cross axis".
    pub length: Option<f32>,
    /// Override colour. `None` falls back to `tokens::TEXT_SEPARATOR_FG`.
    pub color: Option<egui::Color32>,
}

impl Default for DividerProps {
    fn default() -> Self {
        Self {
            orientation: DividerOrientation::Horizontal,
            thickness: 1.0,
            length: None,
            color: None,
        }
    }
}

impl DividerProps {
    pub fn horizontal() -> Self {
        Self::default()
    }

    pub fn vertical() -> Self {
        Self {
            orientation: DividerOrientation::Vertical,
            ..Self::default()
        }
    }

    pub fn length(mut self, length: f32) -> Self {
        self.length = Some(length);
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }
}

pub fn divider(ui: &mut Ui, props: &DividerProps) -> Response {
    let color = props.color.unwrap_or(tokens::TEXT_SEPARATOR_FG);
    let desired = match props.orientation {
        DividerOrientation::Horizontal => {
            let w = props.length.unwrap_or_else(|| ui.available_width());
            Vec2::new(w, props.thickness)
        }
        DividerOrientation::Vertical => {
            let h = props.length.unwrap_or_else(|| ui.available_height());
            Vec2::new(props.thickness, h)
        }
    };
    let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
    ui.painter().rect_filled(rect, 0.0, color);
    response
}
