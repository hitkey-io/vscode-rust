//! vscode-progress-ring
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-progress-ring/vscode-progress-ring.ts
//! Docs:     https://vscode-elements.github.io/components/progress-ring/
//! VS Code analogue: src/vs/base/browser/ui/progressbar/progressbar.ts (the
//!                   indeterminate variant uses the same animation pattern)
//! Tokens:   --vscode-progressBar-background → Palette::VSCE_PROGRESS_BG
//!
//! Circular indeterminate spinner. Draws a 270°-long arc that rotates
//! around the centre at a fixed angular velocity. The widget calls
//! `ctx.request_repaint()` on every frame so the animation runs in any
//! window — UI is otherwise frozen between repaints in egui.

use crate::vscode_widgets::tokens;
use egui::{Color32, Pos2, Response, Sense, Stroke, Ui, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct ProgressRingProps {
    /// Outer diameter in CSS pixels. Default `16.0`.
    pub size: f32,
    /// Stroke thickness in CSS pixels. Default `1.5`.
    pub thickness: f32,
    /// Rotation period in seconds (one full turn). Default `1.5`.
    pub spin_duration: f32,
    /// Sweep angle in radians (length of the moving arc). Default 270°.
    pub sweep: f32,
    /// Override arc colour. `None` falls back to `tokens::FOCUS_BORDER`.
    pub color: Option<Color32>,
}

impl Default for ProgressRingProps {
    fn default() -> Self {
        Self {
            size: 16.0,
            thickness: 1.5,
            spin_duration: 1.5,
            sweep: 270f32.to_radians(),
            color: None,
        }
    }
}

impl ProgressRingProps {
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

pub fn progress_ring(ui: &mut Ui, props: &ProgressRingProps) -> Response {
    let desired = Vec2::splat(props.size);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::hover());
    let painter = ui.painter().clone();

    ui.ctx().request_repaint();
    let t = ui.input(|i| i.time) as f32;
    let phase = (t / props.spin_duration) * std::f32::consts::TAU;

    let center = rect.center();
    let radius = (props.size - props.thickness) * 0.5;
    let color = props.color.unwrap_or(tokens::FOCUS_BORDER);

    // Sample the sweep arc with enough segments to look smooth at 32px.
    let segments = 48usize;
    let mut points = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        let angle = phase + t * props.sweep;
        points.push(Pos2::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        ));
    }
    painter.add(egui::Shape::line(
        points,
        Stroke::new(props.thickness, color),
    ));

    response
}
