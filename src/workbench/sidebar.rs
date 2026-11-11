use std::path::PathBuf;

use egui::{Align, FontId, Layout, RichText, ScrollArea, Sense, Ui};

use crate::fs::FileNode;
use crate::icons::{self, codicon_font};
use crate::search::SearchState;
use crate::theme::Palette;
use crate::vscode_widgets::primitives::{button, label, ButtonProps, LabelProps};

use super::ActivityView;

#[derive(Default)]
pub struct SidebarOutput {
    pub file_to_open: Option<PathBuf>,
    /// `file_to_open` was double-clicked: keep the editor (not a preview).
    pub keep_open: bool,
    pub open_folder_requested: bool,
    pub navigate_to: Option<(PathBuf, usize, usize)>,
    /// Source Control events (multi-repo). Default = no-op.
    pub scm: super::source_control::ScmOutput,
    /// Search view requests (replace, search editor).
    pub search: crate::search::ui::SearchOutput,
    /// A link or button of the Run and Debug view.
    pub run_debug: Option<super::run_debug::RunDebugAction>,
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
    search_scope: &crate::search::ui::SearchScope,
) -> SidebarOutput {
    let mut out = SidebarOutput {
        file_to_open: None,
        keep_open: false,
        open_folder_requested: false,
        navigate_to: None,
        scm: super::source_control::ScmOutput::default(),
        search: Default::default(),
        run_debug: None,
    };

    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, Palette::SIDEBAR_BG());

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        // The SCM view draws its own "SOURCE CONTROL" title; others get the
        // sidebar section header here.
        let mut title = egui::Rect::NOTHING;
        if view != ActivityView::SourceControl {
            title = header(ui, view);
        }

        match view {
            ActivityView::Explorer => {
                explorer_panel(
                    ui, workspace_root, tree, &mut out, git_decorations, active_file,
                    outline_expanded, timeline_expanded,
                );
            }
            ActivityView::Search => {
                out.search = crate::search::ui::show(ui, workspace_root, search, title, search_scope);
                out.navigate_to = out.search.navigate_to.take();
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
            ActivityView::RunAndDebug => {
                let debuggable = active_file.is_some_and(super::run_debug::debuggable);
                out.run_debug = super::run_debug::show(ui, workspace_root.is_some(), debuggable);
            }
            // Drawn by the caller, which owns the extension service.
            ActivityView::Extensions | ActivityView::ExtensionContainer => {}
        }
    });

    out
}

fn header(ui: &mut Ui, view: ActivityView) -> egui::Rect {
    let more = matches!(view, ActivityView::Explorer | ActivityView::RunAndDebug);
    let title = match view {
        ActivityView::Explorer => "Explorer",
        ActivityView::Search => "Search",
        ActivityView::SourceControl => "Source Control",
        // The container's title and its single view's (`Run`).
        ActivityView::RunAndDebug => "Run And Debug: Run",
        ActivityView::Extensions => "Extensions",
        ActivityView::ExtensionContainer => "",
    };
    let rect = title_bar(ui, title);
    if more {
        // "Views and More Actions..." (`.title-actions`, 4px padding).
        let centre = egui::pos2(rect.right() - 15.0, rect.center().y);
        let hit = egui::Rect::from_center_size(centre, egui::vec2(22.0, 22.0));
        let resp = ui.interact(hit, ui.id().with("view-more"), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(hit, super::cards::RADIUS_SMALL, Palette::VSCE_TOOLBAR_HOVER_BG());
        }
        ui.painter().text(
            centre,
            egui::Align2::CENTER_CENTER,
            icons::codicon_by_name("ellipsis").unwrap_or('…').to_string(),
            codicon_font(16.0),
            Palette::VSCE_ICON_2026_FG(),
        );
        resp.on_hover_text("Views and More Actions...");
    }
    rect
}

/// The view title row ("Explorer", "Extensions"…) as Modern UI draws it:
/// 32px tall, no bottom border, 12px semibold title-case text 12px in
/// (`.part > .title` padding 4 + `.title-label` padding 8, fontRamp.css).
/// Returns its rect so callers can add title actions.
pub fn title_bar(ui: &mut Ui, title: &str) -> egui::Rect {
    let (header_rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), TITLE_HEIGHT), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(header_rect, 0.0, Palette::SIDEBAR_BG());
    painter.text(
        header_rect.left_center() + egui::vec2(12.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        crate::ui_font::semibold(12.0),
        Palette::SIDEBAR_TITLE_FG(),
    );
    header_rect
}

/// `PartLayout.AREA_HEIGHT_MODERN_UI`.
pub const TITLE_HEIGHT: f32 = 32.0;


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
                &LabelProps::new("NO FOLDER OPENED").size(11.0).color(Palette::FG()),
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
    ui.spacing_mut().item_spacing.y = 0.0;
    let body_h = 80.0;
    let bottom_h = PANE_HEADER_HEIGHT * 2.0
        + PANES_BOTTOM
        + if *outline_expanded { body_h } else { 0.0 }
        + if *timeline_expanded { body_h } else { 0.0 };
    // The workspace folder's pane header stays put above its tree.
    if let Some(root) = tree.as_mut() {
        root_row(ui, root);
    }
    let tree_h = (ui.available_height() - bottom_h).max(0.0);
    ScrollArea::both()
        .auto_shrink([false, false])
        .max_height(tree_h)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if let Some(root) = tree.as_mut() {
                if root.expanded {
                    if let Some(children) = root.children.as_mut() {
                        for child in children {
                            render_node(ui, child, 1, out, decorations, active_file);
                        }
                    }
                }
            }
        });

    explorer_section(ui, "Outline", outline_expanded, "The active editor cannot provide outline information.");
    explorer_section(ui, "Timeline", timeline_expanded, "No timeline providers available.");
    ui.add_space(PANES_BOTTOM);
}

/// Modern UI pane header height (`MODERN_UI_PANE_HEADER_SIZE`).
pub const PANE_HEADER_HEIGHT: f32 = 28.0;
/// Space under the last pane header, measured in VS Code 1.145.2.
const PANES_BOTTOM: f32 = 5.0;
/// Pane headers and list rows sit 4px inside the side bar (padding.css).
pub const ROW_INSET: f32 = 4.0;

/// A Modern UI pane header (paneHeaders.css): 28px, inset 4px, rounded
/// hover, 16px chevron, 12px semibold title-case title; `separator` draws
/// the 1px line at its top (every pane but the first).
pub fn pane_header(ui: &mut Ui, title: &str, expanded: bool, separator: bool) -> (egui::Rect, egui::Response) {
    let (slot, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), PANE_HEADER_HEIGHT), Sense::click());
    let rect = slot.shrink2(egui::vec2(ROW_INSET, 0.0));
    let painter = ui.painter();
    if resp.hovered() {
        painter.rect_filled(rect, super::cards::RADIUS_SMALL, Palette::LIST_HOVER_BG());
    }
    if separator {
        painter.rect_filled(
            egui::Rect::from_min_size(rect.left_top() + egui::vec2(ROW_INSET, 0.0), egui::vec2(rect.width() - 2.0 * ROW_INSET, 1.0)),
            0.0,
            Palette::VSCE_SIDEBAR_SECTION_HEADER_BORDER(),
        );
    }
    let mid = rect.center().y;
    let chev = if expanded { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT };
    painter.text(
        egui::pos2(rect.left() + 13.0, mid),
        egui::Align2::CENTER_CENTER,
        chev.to_string(),
        codicon_font(16.0),
        Palette::SIDEBAR_SECTION_HEADER_FG(),
    );
    painter.text(
        egui::pos2(rect.left() + 24.0, mid),
        egui::Align2::LEFT_CENTER,
        title,
        crate::ui_font::semibold(12.0),
        Palette::SIDEBAR_SECTION_HEADER_FG(),
    );
    let label = title.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
    (rect, resp)
}

/// Collapsible Explorer section (OUTLINE / TIMELINE): clickable chevron-row
/// header that toggles `expanded`, plus a muted placeholder body when open.
fn explorer_section(ui: &mut Ui, title: &str, expanded: &mut bool, empty_msg: &str) {
    let (_, resp) = pane_header(ui, title, *expanded, true);
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
            Palette::FG_DESCRIPTION(),
        );
    }
}

/// The workspace root as a bold, collapsible row (VS Code renders the open
/// folder this way, with its action icons appearing on hover).
fn root_row(ui: &mut Ui, root: &mut FileNode) {
    let row_h = PANE_HEADER_HEIGHT;
    // The open folder's name keeps its case (fontRamp.css
    // `preserve-workspace-name-case`).
    let (rect, response) = pane_header(ui, &root.name, root.expanded, false);
    let mid = rect.center().y;

    // Hover action cluster (new file / new folder / refresh / collapse all).
    // Collapse-all and refresh are wired; new file/folder are visual for now.
    let row_hovered = ui.rect_contains_pointer(rect);
    let mut collapse_clicked = false;
    let mut refresh_clicked = false;
    if row_hovered {
        let mut x = rect.right() - 14.0;
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
            let col = if r.hovered() { Palette::FG() } else { Palette::FG_DESCRIPTION() };
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
    // Measured in VS Code 1.145.2: rows inset 4px with 4px corners; the
    // twistie and a file's icon share one 16px column, 8px per level.
    let indent = (depth as f32 - 1.0) * 8.0;
    let row_height = 22.0;

    let (slot, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), row_height), Sense::click());
    let rect = slot.shrink2(egui::vec2(ROW_INSET, 0.0));

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
            .rect_filled(rect, super::cards::RADIUS_SMALL, Palette::LIST_INACTIVE_SELECTION_BG());
    } else if response.hovered() {
        ui.painter().rect_filled(rect, super::cards::RADIUS_SMALL, Palette::LIST_HOVER_BG());
    }

    let painter = ui.painter();
    let mid_y = rect.center().y;
    let column = egui::pos2(rect.left() + 18.0 + indent, mid_y);

    if node.is_dir {
        let chev = if node.expanded {
            icons::CHEVRON_DOWN
        } else {
            icons::CHEVRON_RIGHT
        };
        painter.text(column, egui::Align2::CENTER_CENTER, chev.to_string(), codicon_font(16.0), Palette::FG());
    } else if let Some((glyph, color)) = crate::file_icons::icon_for(&node.path) {
        // Seti has no folder icons; a file's glyph sits in the twistie column.
        painter.text(
            column + egui::vec2(1.0, 0.0),
            egui::Align2::CENTER_CENTER,
            glyph.to_string(),
            crate::file_icons::seti_font(16.0),
            color,
        );
    } else {
        painter.text(column, egui::Align2::CENTER_CENTER, icons::FILE.to_string(), codicon_font(15.0), Palette::FG_DESCRIPTION());
    }
    let cursor = egui::pos2(rect.left() + 30.0 + indent, rect.top());
    let mid_y = row_height / 2.0;

    // Git decoration: tint the name + paint a status letter on the right.
    let deco = node_decoration(node, decorations);
    let name_color = deco.map(|k| k.decoration_color()).unwrap_or(Palette::FG());
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

    if response.double_clicked() && !node.is_dir {
        out.file_to_open = Some(node.path.clone());
        out.keep_open = true;
    } else if response.clicked() {
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
        ui.label(RichText::new(text).color(Palette::FG_DESCRIPTION()));
    });
}
