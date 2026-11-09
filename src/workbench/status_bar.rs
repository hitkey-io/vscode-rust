use egui::{Align2, FontId, Layout, Sense, Ui};

use crate::editor::Document;
use crate::icons::{self, codicon_font};
use crate::theme::Palette;
use crate::vscode_widgets::primitives::{label, LabelProps};

/// A plain text status item with the standard horizontal padding (used in the
/// right-to-left cluster: encoding, EOL, indentation, position).
fn plain(ui: &mut Ui, text: &str) -> egui::Response {
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
    resp
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
    /// An inert status item (problems, language, EOL, indent, encoding, bell,
    /// feedback, remote, cursor position) was clicked — VS Code routes most of
    /// these through the command palette, so the caller opens it.
    pub palette_requested: bool,
    /// An extension's status bar item was clicked: its command and
    /// arguments.
    pub extension_command: Option<(String, serde_json::Value)>,
}

/// An extension status bar item (`window.createStatusBarItem`): text with
/// `$(icon)` references, tooltip, hover highlight when it has a command.
fn extension_segment(ui: &mut Ui, item: &crate::extensions::StatusItem) -> egui::Response {
    let parts = icons::parse_label_with_icons(&item.text);
    let text_font = FontId::proportional(11.5);
    let icon_font = codicon_font(14.0);
    let pad = 5.0;
    let galleys: Vec<_> = parts
        .iter()
        .map(|part| match part {
            icons::LabelPart::Text(t) => ui.painter().layout_no_wrap(t.clone(), text_font.clone(), Palette::FG),
            icons::LabelPart::Icon(g) => ui.painter().layout_no_wrap(g.to_string(), icon_font.clone(), Palette::FG),
        })
        .collect();
    let w = pad * 2.0 + galleys.iter().map(|g| g.size().x).sum::<f32>();
    let sense = if item.command.is_some() { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, ui.available_height()), sense);
    if resp.hovered() && item.command.is_some() {
        ui.painter().rect_filled(rect, 0.0, Palette::STATUS_BAR_ITEM_HOVER_BG);
    }
    let mut x = rect.left() + pad;
    for g in galleys {
        let size = g.size();
        ui.painter().galley(egui::pos2(x, rect.center().y - size.y / 2.0), g, Palette::FG);
        x += size.x;
    }
    let label = item.text.clone();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
    match &item.tooltip {
        Some(t) => resp.on_hover_text(t),
        None => resp,
    }
}

/// VS Code-style status bar. We only render segments that map to real, working
/// functionality. Stubs (notifications, remote indicator, error/warning
/// counters) are intentionally omitted until they are wired up.
pub fn show(
    ui: &mut Ui,
    active: Option<&Document>,
    message: &str,
    has_workspace: bool,
    git_branch: Option<&str>,
    git_changes: usize,
    git_ahead_behind: (usize, usize),
    git_has_upstream: bool,
) -> StatusBarOutput {
    show_with_extensions(
        ui, active, message, has_workspace, git_branch, git_changes, git_ahead_behind,
        git_has_upstream, &[], (0, 0),
    )
}

/// [`show`] plus the extensions' status bar items. Left items follow the
/// built-in ones, highest priority first; right items sit left of the
/// editor items, highest priority leftmost (as in VS Code).
#[allow(clippy::too_many_arguments)]
pub fn show_with_extensions(
    ui: &mut Ui,
    active: Option<&Document>,
    message: &str,
    _has_workspace: bool,
    git_branch: Option<&str>,
    git_changes: usize,
    git_ahead_behind: (usize, usize),
    git_has_upstream: bool,
    extension_items: &[crate::extensions::StatusItem],
    problems: (usize, usize),
) -> StatusBarOutput {
    let by_priority = |right: bool| {
        let mut v: Vec<&crate::extensions::StatusItem> =
            extension_items.iter().filter(|i| i.right == right).collect();
        v.sort_by(|a, b| b.priority.total_cmp(&a.priority).then(a.handle.cmp(&b.handle)));
        v
    };
    let left_items = by_priority(false);
    let right_items = by_priority(true);
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
            if status_segment(ui, icons::REMOTE, "", "Open a Remote Window").clicked() {
                out.palette_requested = true;
            }
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

                // Sync segment when an upstream exists; the ↓/↑ counters only
                // appear when there is something to sync (VS Code shows a bare
                // sync icon at 0/0). No upstream → "Publish Branch".
                let (ahead, behind) = git_ahead_behind;
                let sresp = if git_has_upstream {
                    let counters = if ahead + behind > 0 {
                        format!("{behind}↓ {ahead}↑")
                    } else {
                        String::new()
                    };
                    status_segment(
                        ui,
                        icons::SYNC,
                        &counters,
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
            let (errors, warnings) = problems;
            let tip = if errors + warnings == 0 {
                "No Problems".to_string()
            } else {
                format!("Errors: {errors}, Warnings: {warnings}")
            };
            if status_segment(ui, icons::ERROR_ICON, &errors.to_string(), &tip).clicked() {
                out.palette_requested = true;
            }
            if status_segment(ui, icons::WARNING_ICON, &warnings.to_string(), &tip).clicked() {
                out.palette_requested = true;
            }
            for item in &left_items {
                if extension_segment(ui, item).clicked() {
                    out.extension_command =
                        item.command.clone().map(|c| (c, item.command_args.clone()));
                }
            }

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
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
              if let Some(doc) = active {
                {
                    ui.add_space(6.0);
                    if status_segment(ui, icons::FEEDBACK, "", "Tweet Feedback").clicked() {
                        out.palette_requested = true;
                    }
                    if status_segment(ui, icons::BELL, "", "No Notifications").clicked() {
                        out.palette_requested = true;
                    }
                    ui.add_space(2.0);
                    if status_segment(
                        ui,
                        icons::JSON_BRACES,
                        doc.language_label(),
                        "Select Language Mode",
                    )
                    .clicked()
                    {
                        out.palette_requested = true;
                    }
                    // Right-to-left order: LF, then UTF-8, then Spaces — so the
                    // visual left-to-right order is Spaces, UTF-8, LF (VS Code's).
                    if plain(ui, "LF").clicked() { out.palette_requested = true; }
                    if plain(ui, "UTF-8").clicked() { out.palette_requested = true; }
                    if plain(ui, "Spaces: 2").clicked() { out.palette_requested = true; }
                    if plain(ui, &format!("Ln {}, Col {}", doc.cursor_line, doc.cursor_col))
                        .clicked() { out.palette_requested = true; }
                }
              }
              // Right-to-left: lowest priority first, so the highest ends
              // up leftmost.
              for item in right_items.iter().rev() {
                  if extension_segment(ui, item).clicked() {
                      out.extension_command =
                          item.command.clone().map(|c| (c, item.command_args.clone()));
                  }
              }
            });
        });
    });
    out
}
