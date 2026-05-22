use egui::{Align2, FontId, Layout, Sense, Ui};

use crate::editor::Document;
use crate::icons::{self, codicon_font};
use crate::theme::Palette;
use crate::vscode_widgets::primitives::{label, LabelProps};

/// A plain text status item with the standard horizontal padding (used in the
/// right-to-left cluster: encoding, EOL, indentation, position).
fn plain(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(11.5), Palette::FG);
    let w = g.size().x + 6.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, ui.available_height()), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 0.0, Palette::STATUS_BAR_ITEM_HOVER_BG);
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(11.5),
        Palette::FG,
    );
}

/// A left-cluster status segment: a codicon glyph (painted with the codicon
/// font so it doesn't fall back to the proportional placeholder box) followed
/// by a proportional text run, as a single clickable, hover-highlighted item.
fn status_segment(ui: &mut Ui, glyph: char, text: &str, tip: &str) -> egui::Response {
    let icon_font = codicon_font(13.0);
    let text_font = FontId::proportional(11.5);
    let icon_w = ui
        .painter()
        .layout_no_wrap(glyph.to_string(), icon_font.clone(), Palette::FG)
        .size()
        .x;
    let text_w = ui
        .painter()
        .layout_no_wrap(text.to_string(), text_font.clone(), Palette::FG)
        .size()
        .x;
    let gap = 4.0;
    let pad = 4.0;
    let w = pad + icon_w + gap + text_w + pad;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(w, ui.available_height()), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 0.0, Palette::STATUS_BAR_ITEM_HOVER_BG);
    }
    let cy = rect.center().y;
    let p = ui.painter();
    p.text(
        egui::pos2(rect.left() + pad, cy),
        Align2::LEFT_CENTER,
        glyph.to_string(),
        icon_font,
        Palette::FG,
    );
    p.text(
        egui::pos2(rect.left() + pad + icon_w + gap, cy),
        Align2::LEFT_CENTER,
        text,
        text_font,
        Palette::FG,
    );
    resp.on_hover_text(tip)
}

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
            // Far-left: remote indicator (`><`), like VS Code's status bar.
            let _ = status_segment(ui, icons::REMOTE, "", "Open a Remote Window");
            ui.add_space(2.0);
            // Left cluster: clickable Git branch (with a "*N" change counter)
            // + a sync segment, then any transient action message.
            if let Some(branch) = git_branch {
                let text = if git_changes > 0 {
                    format!("{branch} *{git_changes}")
                } else {
                    branch.to_string()
                };
                let resp =
                    status_segment(ui, icons::GIT_BRANCH, &text, "Checkout branch…");
                if resp.clicked() {
                    out.branch_clicked = Some(resp.rect);
                }

                // Sync segment when an upstream exists (↓behind ↑ahead);
                // otherwise a "Publish Branch" affordance.
                let (ahead, behind) = git_ahead_behind;
                let sresp = if git_has_upstream {
                    status_segment(
                        ui,
                        icons::SYNC,
                        &format!("{behind}↓ {ahead}↑"),
                        "Synchronize Changes (pull, then push)",
                    )
                } else {
                    status_segment(
                        ui,
                        icons::CLOUD_UPLOAD,
                        "Publish Branch",
                        "Publish Branch",
                    )
                };
                if sresp.clicked() {
                    out.sync_clicked = true;
                }
            }
            // Problems counter (errors / warnings). Zeroed until diagnostics
            // are wired, matching VS Code's always-present indicator.
            let _ = status_segment(ui, icons::ERROR_ICON, "0", "No Problems");
            let _ = status_segment(ui, icons::WARNING_ICON, "0", "No Problems");

            // Left: action message (Save status, Open results, etc.).
            if !message.is_empty() {
                ui.add_space(4.0);
                label(
                    ui,
                    &LabelProps::new(message).normal().size(11.5).color(Palette::FG),
                );
            }
            // Right: document + editor state, right-to-left so the first item
            // added sits at the far right (feedback), matching VS Code order.
            if let Some(doc) = active {
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(6.0);
                    let _ = status_segment(ui, icons::FEEDBACK, "", "Tweet Feedback");
                    let _ = status_segment(ui, icons::BELL, "", "No Notifications");
                    ui.add_space(2.0);
                    let _ = status_segment(
                        ui,
                        icons::JSON_BRACES,
                        doc.language_label(),
                        "Select Language Mode",
                    );
                    plain(ui, "LF");
                    plain(ui, "Spaces: 2");
                    plain(ui, "UTF-8");
                    plain(ui, &format!("Ln {}, Col {}", doc.cursor_line, doc.cursor_col));
                });
            }
        });
    });
    out
}
