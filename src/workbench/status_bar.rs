use egui::{Layout, Ui};

use crate::editor::Document;
use crate::theme::Palette;
use crate::vscode_widgets::primitives::{label, LabelProps};

/// VS Code-style status bar. We only render segments that map to real, working
/// functionality. Stubs (git branch, sync, notifications, remote indicator,
/// error/warning counters) are intentionally omitted until they are wired up.
pub fn show(
    ui: &mut Ui,
    active: Option<&Document>,
    message: &str,
    _has_workspace: bool,
) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, Palette::STATUS_BAR_BG);

    let top_border = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.top()),
        egui::vec2(rect.width(), 1.0),
    );
    painter.rect_filled(top_border, 0.0, Palette::BORDER);

    let row = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.top() + 1.0),
        egui::vec2(rect.width(), rect.height() - 1.0),
    );

    ui.allocate_ui_at_rect(row, |ui| {
        ui.horizontal_centered(|ui| {
            ui.add_space(12.0);
            // Left: action message (Save status, Open results, etc.).
            if !message.is_empty() {
                label(
                    ui,
                    &LabelProps::new(message).normal().size(11.5).color(Palette::FG),
                );
            }
            // Right: document state (Ln/Col, encoding, language).
            if let Some(doc) = active {
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);
                    let cursor =
                        format!("Ln {}, Col {}", doc.cursor_line, doc.cursor_col);
                    label(
                        ui,
                        &LabelProps::new(doc.language_label())
                            .normal()
                            .size(11.5)
                            .color(Palette::FG),
                    );
                    ui.add_space(14.0);
                    label(
                        ui,
                        &LabelProps::new("UTF-8")
                            .normal()
                            .size(11.5)
                            .color(Palette::FG),
                    );
                    ui.add_space(14.0);
                    label(
                        ui,
                        &LabelProps::new(&cursor)
                            .normal()
                            .size(11.5)
                            .color(Palette::FG),
                    );
                });
            }
        });
    });
}
