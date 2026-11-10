//! Modern UI "floating cards" (VS Code 1.145 `workbench.experimental.modernUI`,
//! on by default): the activity bar, side bar and editor sit on the shell
//! background as rounded, outlined cards 4px apart
//! (`browser/media/floatingPanels.css`, `contrib/modernUI/.../editorBorder.css`).

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Stroke, StrokeKind};

/// `--modern-ui-floating-card-margin` / `-outer-margin` (spacing-size40).
pub const MARGIN: f32 = 4.0;
/// `--vscode-cornerRadius-large`.
pub const RADIUS: f32 = 8.0;
/// `--vscode-cornerRadius-small` (activity items, status items, list rows).
pub const RADIUS_SMALL: f32 = 4.0;

/// Paints over everything outside the rounded corners of `rect` with
/// `outside` (content drawn into the card can't be clipped to a rounded
/// shape), then strokes the outline. `seam_left` leaves the left edge
/// unstroked, as where the side bar meets the activity bar.
pub fn finish(painter: &Painter, rect: Rect, radius: CornerRadius, border: Color32, outside: Color32, seam_left: bool) {
    mask_corners(painter, rect, radius, outside);
    painter.rect_stroke(rect, radius, Stroke::new(1.0, border), StrokeKind::Inside);
    if seam_left {
        let top = rect.top() + 1.0 + radius.nw as f32;
        let bottom = rect.bottom() - 1.0 - radius.sw as f32;
        if bottom > top {
            painter.rect_filled(
                Rect::from_min_max(egui::pos2(rect.left(), top), egui::pos2(rect.left() + 1.0, bottom)),
                0.0,
                outside,
            );
        }
    }
}

/// Fills the area between each rounded corner's arc and the square corner.
pub fn mask_corners(painter: &Painter, rect: Rect, radius: CornerRadius, color: Color32) {
    let corners = [
        (rect.left_top(), radius.nw as f32, egui::vec2(1.0, 1.0)),
        (rect.right_top(), radius.ne as f32, egui::vec2(-1.0, 1.0)),
        (rect.left_bottom(), radius.sw as f32, egui::vec2(1.0, -1.0)),
        (rect.right_bottom(), radius.se as f32, egui::vec2(-1.0, -1.0)),
    ];
    for (corner, r, dir) in corners {
        if r <= 0.0 {
            continue;
        }
        let centre = corner + dir * r;
        let steps = 12;
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(corner, color);
        for i in 0..=steps {
            let a = std::f32::consts::FRAC_PI_2 * i as f32 / steps as f32;
            // From the horizontal edge to the vertical edge.
            let p = Pos2::new(centre.x - dir.x * r * a.sin(), centre.y - dir.y * r * a.cos());
            mesh.colored_vertex(p, color);
        }
        for i in 1..=steps as u32 {
            mesh.add_triangle(0, i, i + 1);
        }
        painter.add(egui::Shape::mesh(mesh));
    }
}
