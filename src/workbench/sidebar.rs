use std::path::PathBuf;

use egui::{Align, FontId, Layout, RichText, ScrollArea, Sense, Ui};

use crate::fs::FileNode;
use crate::icons::{self, codicon_font};
use crate::search::SearchState;
use crate::theme::Palette;
use crate::vscode_widgets::layout::{toolbar_container, ToolbarContainerProps};
use crate::vscode_widgets::primitives::{
    button, icon_button, label, ButtonProps, IconButtonProps, LabelProps,
};

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
                explorer_panel(ui, workspace_root, tree, &mut out, git_decorations);
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

fn explorer_actions(ui: &mut Ui, out: &mut SidebarOutput, tree: &mut Option<FileNode>) {
    let title_owned = tree
        .as_ref()
        .map(|n| n.name.to_uppercase())
        .unwrap_or_default();

    // Capture mutable refs via flags so the inline closure stays Fn-flavour.
    let mut collapse_clicked = false;
    let mut refresh_clicked = false;

    ui.allocate_ui(egui::vec2(ui.available_width(), 30.0), |ui| {
        ui.painter().rect_filled(
            ui.available_rect_before_wrap(),
            0.0,
            Palette::SIDEBAR_BG,
        );
        toolbar_container(
            ui,
            &ToolbarContainerProps::new().title(&title_owned),
            |ui| {
                let collapse = icon_button(
                    ui,
                    &IconButtonProps::new(icons::COLLAPSE_ALL).icon_size(14.0),
                )
                .on_hover_text("Collapse Folders in Explorer");
                if collapse.clicked() {
                    collapse_clicked = true;
                }
                let refresh = icon_button(
                    ui,
                    &IconButtonProps::new(icons::REFRESH).icon_size(14.0),
                )
                .on_hover_text("Refresh Explorer");
                if refresh.clicked() {
                    refresh_clicked = true;
                }
            },
        );
    });

    if collapse_clicked {
        if let Some(root) = tree.as_mut() {
            collapse_all(root);
            root.expanded = true;
        }
    }
    if refresh_clicked {
        if let Some(root) = tree.as_mut() {
            refresh_tree(root);
        }
    }
    let _ = out;
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

    explorer_actions(ui, out, tree);

    ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if let Some(root) = tree.as_mut() {
                if let Some(children) = root.children.as_mut() {
                    for child in children {
                        render_node(ui, child, 0, out, decorations);
                    }
                }
            }
        });
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
) {
    let indent = depth as f32 * 12.0 + 6.0;
    let row_height = 22.0;

    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), row_height), Sense::click());

    // Make the row findable by name (e.g. "tsconfig.json") for kittest.
    let label = node.name.clone();
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone())
    });

    if response.hovered() {
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

    let icon = if node.is_dir {
        if node.expanded {
            icons::FOLDER_OPENED
        } else {
            icons::FOLDER
        }
    } else {
        icons::FILE
    };
    // VS Code "2026 Dark" file icon theme paints folder/file icons in the same neutral
    // foreground as the surrounding text — no special folder accent color.
    let icon_color = Palette::FG_DESCRIPTION;
    painter.text(
        cursor + egui::vec2(0.0, mid_y),
        egui::Align2::LEFT_CENTER,
        icon.to_string(),
        codicon_font(15.0),
        icon_color,
    );
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
                render_node(ui, child, depth + 1, out, decorations);
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
