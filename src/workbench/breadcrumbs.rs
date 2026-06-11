//! Breadcrumbs row under the tab strip.
//! VS Code analogue: src/vs/workbench/browser/parts/editor/breadcrumbsControl.ts
//! Renders the active file's path relative to the workspace root as
//! icon+label segments separated by a chevron. Height 22px, editor background.
//! Tokens: --vscode-breadcrumb-foreground, --vscode-breadcrumb-background.

use std::path::Path;

use egui::{Align2, FontId, Sense, Ui};

use crate::icons::{self, codicon_font};
use crate::theme::Palette;

/// Height of the breadcrumbs strip (matches VS Code's BreadcrumbsControl).
pub const HEIGHT: f32 = 22.0;

pub fn show(ui: &mut Ui, path: &Path, workspace_root: Option<&Path>) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), HEIGHT), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, Palette::EDITOR_BG);

    // Build segments relative to the workspace root (root name itself omitted,
    // matching VS Code which starts at the first folder under the root).
    let rel = workspace_root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);
    let comps: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if comps.is_empty() {
        return;
    }

    let label_font = FontId::proportional(12.0);
    let cy = rect.center().y;
    let mut x = rect.left() + 10.0;
    let last = comps.len() - 1;

    for (i, name) in comps.iter().enumerate() {
        if i > 0 {
            // chevron separator
            p.text(
                egui::pos2(x, cy),
                Align2::LEFT_CENTER,
                icons::CHEVRON_RIGHT.to_string(),
                codicon_font(11.0),
                Palette::FG_DESCRIPTION,
            );
            x += 13.0;
        }
        // Folder segments have no icon (matches VS Code with the Seti theme);
        // the file segment gets its Seti file-type glyph.
        if i == last {
            if let Some((glyph, color)) = crate::file_icons::icon_for(path) {
                p.text(
                    egui::pos2(x, cy),
                    Align2::LEFT_CENTER,
                    glyph.to_string(),
                    crate::file_icons::seti_font(14.0),
                    color,
                );
                x += 18.0;
            }
        }
        let g = p.layout_no_wrap(name.clone(), label_font.clone(), Palette::BREADCRUMB_FG);
        p.galley(egui::pos2(x, cy - g.size().y * 0.5), g.clone(), Palette::BREADCRUMB_FG);
        x += g.size().x + 6.0;
    }

    // Trailing "› …" symbol-path placeholder, like VS Code's breadcrumbs when
    // no document symbol is selected.
    p.text(
        egui::pos2(x, cy),
        Align2::LEFT_CENTER,
        icons::CHEVRON_RIGHT.to_string(),
        codicon_font(11.0),
        Palette::FG_DESCRIPTION,
    );
    p.text(
        egui::pos2(x + 13.0, cy),
        Align2::LEFT_CENTER,
        "…",
        label_font,
        Palette::FG_DESCRIPTION,
    );
}
