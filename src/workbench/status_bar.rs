use egui::{Layout, Ui};

use crate::editor::Document;
use crate::icons;
use crate::theme::Palette;
use crate::vscode_widgets::primitives::{label, LabelProps};

/// Domain events from the status bar (clicks on interactive segments).
#[derive(Default)]
pub struct StatusBarOutput {
    /// The branch segment was clicked, with its screen rect so the caller can
    /// anchor a branch-picker popup just above it.
    pub branch_clicked: Option<egui::Rect>,
    /// The sync (ahead/behind) segment was clicked.
    pub sync_clicked: bool,
}

/// VS Code-style status bar. We only render segments that map to real, working
/// functionality. Stubs (notifications, remote indicator, error/warning
/// counters) are intentionally omitted until they are wired up.
pub fn show(
    ui: &mut Ui,
    active: Option<&Document>,
    message: &str,
    _has_workspace: bool,
    git_branch: Option<&str>,
    git_changes: usize,
    git_ahead_behind: (usize, usize),
    git_has_upstream: bool,
) -> StatusBarOutput {
    let mut out = StatusBarOutput::default();
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
            ui.add_space(8.0);
            // Left cluster: clickable Git branch (with a "*N" change counter)
            // + a sync segment, then any transient action message.
            if let Some(branch) = git_branch {
                let glyph = format!("{} {}", icons::GIT_BRANCH, branch);
                let text = if git_changes > 0 {
                    format!("{glyph} *{git_changes}")
                } else {
                    glyph
                };
                let resp = ui
                    .add(
                        egui::Label::new(
                            egui::RichText::new(&text)
                                .size(11.5)
                                .color(Palette::FG),
                        )
                        .sense(egui::Sense::click()),
                    )
                    .on_hover_text("Checkout branch…");
                if resp.clicked() {
                    out.branch_clicked = Some(resp.rect);
                }
                ui.add_space(10.0);

                // Sync segment when an upstream exists (↓behind ↑ahead);
                // otherwise a "Publish Branch" affordance.
                let (ahead, behind) = git_ahead_behind;
                let (seg, tip) = if git_has_upstream {
                    (format!("{} {}↓ {}↑", icons::SYNC, behind, ahead),
                     "Synchronize Changes (pull, then push)")
                } else {
                    (format!("{} Publish Branch", icons::CLOUD_UPLOAD),
                     "Publish Branch")
                };
                let sresp = ui
                    .add(
                        egui::Label::new(
                            egui::RichText::new(&seg).size(11.5).color(Palette::FG),
                        )
                        .sense(egui::Sense::click()),
                    )
                    .on_hover_text(tip);
                if sresp.clicked() {
                    out.sync_clicked = true;
                }
                ui.add_space(10.0);
            }
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
    out
}
