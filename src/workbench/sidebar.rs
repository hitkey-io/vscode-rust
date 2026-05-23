use std::path::PathBuf;

use egui::{Align, FontId, Layout, RichText, ScrollArea, Sense, Ui};

use crate::fs::FileNode;
use crate::icons::{self, codicon_font};
use crate::search::SearchState;
use crate::theme::Palette;
use crate::vscode_widgets::primitives::{button, label, ButtonProps, LabelProps};

use super::ActivityView;

pub struct SidebarOutput {
    pub file_to_open: Option<PathBuf>,
    pub open_folder_requested: bool,
    pub navigate_to: Option<(PathBuf, usize, usize)>,
    /// Source Control events (multi-repo). Default = no-op.
    pub scm: super::source_control::ScmOutput,
}

pub fn show(
    ui: &mut Ui,
    view: ActivityView,
    workspace_root: &Option<PathBuf>,
    tree: &mut Option<FileNode>,
    search: &mut SearchState,
    git_model: &crate::git::Model,
    git_history: &[crate::git::GraphRow],
    git_graph_root: Option<&std::path::Path>,
    scm_ui: &mut super::source_control::ScmUiState,
    git_decorations: &std::collections::BTreeMap<PathBuf, crate::git::ChangeKind>,
    active_file: Option<&std::path::Path>,
    outline_expanded: &mut bool,
    timeline_expanded: &mut bool,
) -> SidebarOutput {
    let mut out = SidebarOutput {
        file_to_open: None,
        open_folder_requested: false,
        navigate_to: None,
        scm: super::source_control::ScmOutput::default(),
    };

    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, Palette::SIDEBAR_BG);

    ui.vertical(|ui| {
        // The SCM view draws its own "SOURCE CONTROL" title; others get the
        // sidebar section header here.
        if view != ActivityView::SourceControl {
            header(ui, view);
        }

        match view {
            ActivityView::Explorer => {
                explorer_panel(
                    ui, workspace_root, tree, &mut out, git_decorations, active_file,
                    outline_expanded, timeline_expanded,
                );
            }
            ActivityView::Search => {
                let search_out = crate::search::ui::show(ui, workspace_root, search);
                out.navigate_to = search_out.navigate_to;
            }
            ActivityView::SourceControl => {
                out.scm = super::source_control::show(
                    ui,
                    git_model,
                    git_history,
                    git_graph_root,
                    scm_ui,
                );
            }
        }
    });

    out
}

fn header(ui: &mut Ui, view: ActivityView) {
    let title = match view {
        ActivityView::Explorer => "EXPLORER",
        ActivityView::Search => "SEARCH",
        ActivityView::SourceControl => "SOURCE CONTROL",
    };
    let (header_rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 35.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(header_rect, 0.0, Palette::SIDEBAR_SECTION_HEADER_BG);
    painter.text(
        header_rect.left_center() + egui::vec2(20.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        FontId::proportional(11.0),
        Palette::SIDEBAR_SECTION_HEADER_FG,
    );
    let bottom_border = egui::Rect::from_min_size(
        egui::pos2(header_rect.left(), header_rect.bottom() - 1.0),
        egui::vec2(header_rect.width(), 1.0),
    );
    painter.rect_filled(bottom_border, 0.0, Palette::BORDER);
}


fn collapse_all(node: &mut FileNode) {
    if node.is_dir {
        node.expanded = false;
        if let Some(children) = node.children.as_mut() {
            for c in children {
                collapse_all(c);
            }
        }
    }
}

fn refresh_tree(node: &mut FileNode) {
    if node.is_dir {
        node.children = None;
        node.ensure_loaded();
    }
}

type Decorations = std::collections::BTreeMap<PathBuf, crate::git::ChangeKind>;

fn explorer_panel(
    ui: &mut Ui,
    workspace_root: &Option<PathBuf>,
    tree: &mut Option<FileNode>,
    out: &mut SidebarOutput,
    decorations: &Decorations,
    active_file: Option<&std::path::Path>,
    outline_expanded: &mut bool,
    timeline_expanded: &mut bool,
) {
    if workspace_root.is_none() || tree.is_none() {
        // Section subheader like VS Code
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            label(
                ui,
                &LabelProps::new("NO FOLDER OPENED").size(11.0).color(Palette::FG),
            );
        });
        ui.add_space(8.0);

        let inset = 10.0;

        ui.horizontal_wrapped(|ui| {
            ui.add_space(inset);
            label(
                ui,
                &LabelProps::new("You have not yet opened a folder.")
                    .normal()
                    .description(),
            );
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(inset);
            let resp = button(ui, &ButtonProps::new("Open Folder").block());
            if resp.clicked() {
                out.open_folder_requested = true;
            }
            ui.add_space(inset);
        });
        return;
    }

    // Reserve room at the bottom for the OUTLINE + TIMELINE section headers,
    // which VS Code pins under the file tree in the Explorer. When a section
    // is expanded, it gets extra space for its (currently empty) body.
    let header_h = 22.0;
    let body_h = 80.0;
    let bottom_h = header_h * 2.0
        + if *outline_expanded { body_h } else { 0.0 }
        + if *timeline_expanded { body_h } else { 0.0 };
    let tree_h = (ui.available_height() - bottom_h).max(0.0);
    ScrollArea::both()
        .auto_shrink([false, false])
        .max_height(tree_h)
        .show(ui, |ui| {
            if let Some(root) = tree.as_mut() {
                root_row(ui, root);
                if root.expanded {
                    if let Some(children) = root.children.as_mut() {
                        for child in children {
                            render_node(ui, child, 1, out, decorations, active_file);
                        }
                    }
                }
            }
        });

    explorer_section(ui, "OUTLINE", outline_expanded, "The active editor cannot provide outline information.");
    explorer_section(ui, "TIMELINE", timeline_expanded, "No timeline providers available.");
}

/// Collapsible Explorer section (OUTLINE / TIMELINE): clickable chevron-row
/// header that toggles `expanded`, plus a muted placeholder body when open.
fn explorer_section(ui: &mut Ui, title: &str, expanded: &mut bool, empty_msg: &str) {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 0.0, Palette::LIST_HOVER_BG);
    }
    let mid = rect.center().y;
    let chev = if *expanded { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT };
    let p = ui.painter();
    p.text(
        egui::pos2(rect.left() + 12.0, mid),
        egui::Align2::CENTER_CENTER,
        chev.to_string(),
        codicon_font(12.0),
        Palette::FG_DESCRIPTION,
    );
    p.text(
        egui::pos2(rect.left() + 22.0, mid),
        egui::Align2::LEFT_CENTER,
        title,
        FontId::proportional(11.0),
        Palette::SIDEBAR_SECTION_HEADER_FG,
    );
    if resp.clicked() {
        *expanded = !*expanded;
    }
    if *expanded {
        let (body, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 80.0), Sense::hover());
        ui.painter().text(
            egui::pos2(body.left() + 18.0, body.top() + 14.0),
            egui::Align2::LEFT_TOP,
            empty_msg,
            FontId::proportional(12.0),
            Palette::FG_DESCRIPTION,
        );
    }
}

/// The workspace root as a bold, collapsible row (VS Code renders the open
/// folder this way, with its action icons appearing on hover).
fn root_row(ui: &mut Ui, root: &mut FileNode) {
    let row_h = 22.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), Sense::click());
    let hovered = response.hovered();
    if hovered {
        ui.painter().rect_filled(rect, 0.0, Palette::LIST_HOVER_BG);
    }
    let mid = rect.center().y;
    let chev = if root.expanded {
        icons::CHEVRON_DOWN
    } else {
        icons::CHEVRON_RIGHT
    };
    ui.painter().text(
        egui::pos2(rect.left() + 12.0, mid),
        egui::Align2::CENTER_CENTER,
        chev.to_string(),
        codicon_font(12.0),
        Palette::FG_DESCRIPTION,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 22.0, mid),
        egui::Align2::LEFT_CENTER,
        root.name.to_uppercase(),
        FontId::proportional(11.0),
        Palette::SIDEBAR_SECTION_HEADER_FG,
    );

    // Hover action cluster (new file / new folder / refresh / collapse all).
    // Collapse-all and refresh are wired; new file/folder are visual for now.
    let row_hovered = ui.rect_contains_pointer(rect);
    let mut collapse_clicked = false;
    let mut refresh_clicked = false;
    if row_hovered {
        let mut x = rect.right() - 18.0;
        for (i, glyph) in [
            icons::COLLAPSE_ALL,
            icons::REFRESH,
            icons::NEW_FOLDER,
            icons::NEW_FILE,
        ]
        .into_iter()
        .enumerate()
        {
            let hit = egui::Rect::from_center_size(egui::pos2(x, mid), egui::vec2(20.0, row_h));
            let r = ui.interact(hit, response.id.with(("rootact", i)), Sense::click());
            let col = if r.hovered() { Palette::FG } else { Palette::FG_DESCRIPTION };
            ui.painter().text(
                egui::pos2(x, mid),
                egui::Align2::CENTER_CENTER,
                glyph.to_string(),
                codicon_font(14.0),
                col,
            );
            if r.clicked() {
                match i {
                    0 => collapse_clicked = true,
                    1 => refresh_clicked = true,
                    _ => {}
                }
            }
            x -= 22.0;
        }
    }

    if collapse_clicked {
        collapse_all(root);
        root.expanded = true;
    } else if refresh_clicked {
        refresh_tree(root);
    } else if response.clicked() {
        root.toggle();
    }
}

/// The git decoration that applies to a node: direct change for files, or the
/// strongest descendant change rolled up for folders (VS Code tints a folder
/// when anything inside it changed).
fn node_decoration(
    node: &FileNode,
    decorations: &Decorations,
) -> Option<crate::git::ChangeKind> {
    if node.is_dir {
        decorations
            .iter()
            .find(|(p, _)| p.starts_with(&node.path))
            .map(|(_, k)| *k)
    } else {
        decorations.get(&node.path).copied()
    }
}

fn render_node(
    ui: &mut Ui,
    node: &mut FileNode,
    depth: usize,
    out: &mut SidebarOutput,
    decorations: &Decorations,
    active_file: Option<&std::path::Path>,
) {
    let indent = depth as f32 * 8.0 + 8.0;
    let row_height = 22.0;

    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), row_height), Sense::click());

    // Make the row findable by name (e.g. "tsconfig.json") for kittest.
    let label = node.name.clone();
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone())
    });

    // Selected (active editor file) row gets the inactive-selection wash;
    // hover gets the lighter list-hover wash.
    let is_selected = !node.is_dir && active_file == Some(node.path.as_path());
    if is_selected {
        ui.painter()
            .rect_filled(rect, 0.0, Palette::LIST_INACTIVE_SELECTION_BG);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, Palette::LIST_HOVER_BG);
    }

    let mut cursor = rect.left_top() + egui::vec2(indent, 0.0);
    let painter = ui.painter();
    let mid_y = row_height / 2.0;

    if node.is_dir {
        let chev = if node.expanded {
            icons::CHEVRON_DOWN
        } else {
            icons::CHEVRON_RIGHT
        };
        painter.text(
            cursor + egui::vec2(0.0, mid_y),
            egui::Align2::LEFT_CENTER,
            chev.to_string(),
            codicon_font(12.0),
            Palette::FG_DESCRIPTION,
        );
    }
    cursor.x += 12.0;

    if node.is_dir {
        // Folders keep the codicon folder glyph (Seti defines no folder icons).
        let icon = if node.expanded {
            icons::FOLDER_OPENED
        } else {
            icons::FOLDER
        };
        painter.text(
            cursor + egui::vec2(0.0, mid_y),
            egui::Align2::LEFT_CENTER,
            icon.to_string(),
            codicon_font(15.0),
            Palette::FG_DESCRIPTION,
        );
    } else if let Some((glyph, color)) = crate::file_icons::icon_for(&node.path) {
        // Files use the VS Code Seti file-type icon (glyph + theme colour).
        painter.text(
            cursor + egui::vec2(1.0, mid_y),
            egui::Align2::LEFT_CENTER,
            glyph.to_string(),
            crate::file_icons::seti_font(16.0),
            color,
        );
    } else {
        painter.text(
            cursor + egui::vec2(0.0, mid_y),
            egui::Align2::LEFT_CENTER,
            icons::FILE.to_string(),
            codicon_font(15.0),
            Palette::FG_DESCRIPTION,
        );
    }
    cursor.x += 20.0;

    // Git decoration: tint the name + paint a status letter on the right.
    let deco = node_decoration(node, decorations);
    let name_color = deco.map(|k| k.decoration_color()).unwrap_or(Palette::FG);
    painter.text(
        cursor + egui::vec2(0.0, mid_y),
        egui::Align2::LEFT_CENTER,
        &node.name,
        FontId::proportional(13.0),
        name_color,
    );
    if let Some(kind) = deco {
        painter.text(
            egui::pos2(rect.right() - 16.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            kind.badge(),
            FontId::proportional(11.5),
            name_color,
        );
    }

    if response.clicked() {
        if node.is_dir {
            node.toggle();
        } else {
            out.file_to_open = Some(node.path.clone());
        }
    }

    if node.is_dir && node.expanded {
        if let Some(children) = node.children.as_mut() {
            for child in children {
                render_node(ui, child, depth + 1, out, decorations, active_file);
            }
        }
    }
}

fn placeholder(ui: &mut Ui, text: &str) {
    ui.add_space(20.0);
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.label(RichText::new(text).color(Palette::FG_DESCRIPTION));
    });
}
