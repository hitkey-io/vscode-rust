//! vscode-icon
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-icon/vscode-icon.ts
//! Docs:     https://vscode-elements.github.io/components/icon/
//! VS Code analogue: src/vs/base/browser/ui/codicons/codicon/codicon.css
//! Tokens:   --vscode-icon-foreground → Palette::VSCE_ICON_FG
//!
//! Renders a single codicon glyph using the bundled codicon.ttf font.
//! Supports `name`, `size`, `action-icon`, `spin` and `spin-duration`
//! attributes from the upstream component.

use crate::icons::codicon_font;
use crate::vscode_widgets::tokens;
use egui::{Color32, Response, Sense, Ui, Vec2};

/// Props for [`icon`].
///
/// `glyph` is the codicon codepoint (`char`). We don't accept a `name: &str`
/// like the web component because matching strings → codepoints at runtime
/// is dead weight — callers already have `crate::icons::SEARCH` constants.
#[derive(Clone, Copy, Debug)]
pub struct IconProps {
    pub glyph: char,
    /// Pixel size of the glyph. Default `16.0`.
    pub size: f32,
    /// Action-icon variant — picks up hover background like in toolbars.
    pub action_icon: bool,
    /// Animate via continuous rotation. The widget requests a repaint
    /// every frame while this is set.
    pub spin: bool,
    /// Full rotation duration in seconds. Default `1.5`.
    pub spin_duration: f32,
    /// Override colour. `None` falls back to `tokens::ICON_FG`.
    pub color: Option<Color32>,
}

impl Default for IconProps {
    fn default() -> Self {
        Self {
            glyph: '\u{ea74}', // codicon "info"
            size: 16.0,
            action_icon: false,
            spin: false,
            spin_duration: 1.5,
            color: None,
        }
    }
}

impl IconProps {
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

    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    pub fn action_icon(mut self) -> Self {
        self.action_icon = true;
        self
    }

    pub fn spin(mut self) -> Self {
        self.spin = true;
        self
    }
}

pub fn icon(ui: &mut Ui, props: &IconProps) -> Response {
    let desired = Vec2::splat(props.size);
    let sense = if props.action_icon {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(desired, sense);

    let color = props.color.unwrap_or(tokens::ICON_FG);

    if props.action_icon && response.hovered() {
        ui.painter()
            .rect_filled(rect.expand(2.0), 3.0, tokens::LIST_HOVER_BG);
    }

    let painter = ui.painter().clone();
    let center = rect.center();
    let font = codicon_font(props.size);

    if props.spin {
        ui.ctx().request_repaint();
        let t = ui.input(|i| i.time) as f32;
        let angle = (t / props.spin_duration) * std::f32::consts::TAU;
        let galley = painter.layout_no_wrap(props.glyph.to_string(), font, color);
        // `TextShape::with_angle` rotates around its `pos` (the glyph's
        // top-left). To rotate around the glyph's visual centre we offset
        // `pos` by `-R(angle) * half_size` so that after rotation, the
        // centre of the galley lands exactly on `center`.
        let half = galley.size() * 0.5;
        let (sin, cos) = angle.sin_cos();
        let rotated = egui::vec2(
            half.x * cos - half.y * sin,
            half.x * sin + half.y * cos,
        );
        let pos = center - rotated;
        let shape = egui::epaint::TextShape::new(pos, galley, color).with_angle(angle);
        painter.add(shape);
    } else {
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            props.glyph.to_string(),
            font,
            color,
        );
    }

    response
}
