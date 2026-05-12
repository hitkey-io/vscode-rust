//! vscode-tree / vscode-tree-item
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-tree/vscode-tree.ts
//! Docs:     https://vscode-elements.github.io/components/tree/
//! VS Code analogue: src/vs/base/browser/ui/tree/objectTreeModel.ts
//! Tokens:   --vscode-list-hoverBackground → Palette::VSCE_LIST_HOVER_BG
//!           --vscode-list-activeSelectionBackground → Palette::VSCE_LIST_ACTIVE_SELECTION_BG
//!           --vscode-list-activeSelectionForeground → Palette::VSCE_LIST_ACTIVE_SELECTION_FG
//!           --vscode-tree-indentGuidesStroke → Palette::VSCE_TREE_INDENT_GUIDE
//!           --vscode-icon-foreground → Palette::VSCE_ICON_FG
//!           --vscode-focusBorder → Palette::VSCE_FOCUS_BORDER
//!
//! Hierarchical list with twistie chevrons, file icons, and selection.
//! `TreeItem` is `Clone + 'static` so the caller can store the structure
//! anywhere convenient; expansion state lives on the item itself via
//! `open: bool`.

use crate::icons::{codicon_font, CHEVRON_DOWN, CHEVRON_RIGHT};
use crate::vscode_widgets::tokens;
use egui::{Align2, Color32, CornerRadius, FontId, Response, Sense, Stroke, Ui, Vec2};

#[derive(Clone, Debug)]
pub struct TreeItem {
    pub label: String,
    pub icon: Option<char>,
    pub open: bool,
    pub disabled: bool,
    pub children: Vec<TreeItem>,
}

impl TreeItem {
    pub fn leaf(label: impl Into<String>, icon: Option<char>) -> Self {
        Self {
            label: label.into(),
            icon,
            open: false,
            disabled: false,
            children: Vec::new(),
        }
    }

    pub fn folder(label: impl Into<String>, icon: Option<char>, children: Vec<TreeItem>) -> Self {
        Self {
            label: label.into(),
            icon,
            open: false,
            disabled: false,
            children,
        }
    }

    pub fn open(mut self) -> Self {
        self.open = true;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TreeProps {
    pub row_height: f32,
    pub indent: f32,
    pub show_indent_guides: bool,
}

impl Default for TreeProps {
    fn default() -> Self {
        Self {
            row_height: 22.0,
            indent: 16.0,
            show_indent_guides: true,
        }
    }
}

#[derive(Debug, Default)]
pub struct TreeResponse {
    /// Path of indices into the tree (root child first). `None` if no item
    /// was clicked this frame.
    pub clicked: Option<Vec<usize>>,
    pub right_clicked: Option<Vec<usize>>,
    pub double_clicked: Option<Vec<usize>>,
}

pub fn tree(
    ui: &mut Ui,
    props: &TreeProps,
    items: &mut [TreeItem],
    selected: &mut Option<Vec<usize>>,
) -> TreeResponse {
    let mut out = TreeResponse::default();
    let mut path: Vec<usize> = Vec::new();
    for (idx, item) in items.iter_mut().enumerate() {
        path.push(idx);
        draw_branch(ui, props, item, &mut path, 0, selected, &mut out);
        path.pop();
    }
    out
}

fn draw_branch(
    ui: &mut Ui,
    props: &TreeProps,
    item: &mut TreeItem,
    path: &mut Vec<usize>,
    depth: usize,
    selected: &mut Option<Vec<usize>>,
    out: &mut TreeResponse,
) {
    let has_children = !item.children.is_empty();
    let is_selected = selected.as_ref().is_some_and(|s| s == path);

    let response = draw_row(ui, props, item, depth, is_selected, has_children);
    if response.toggled && has_children {
        item.open = !item.open;
    }
    if response.row_clicked && !item.disabled {
        *selected = Some(path.clone());
        out.clicked = Some(path.clone());
    }
    if response.right_clicked {
        out.right_clicked = Some(path.clone());
    }
    if response.double_clicked {
        out.double_clicked = Some(path.clone());
    }

    if has_children && item.open {
        for (idx, child) in item.children.iter_mut().enumerate() {
            path.push(idx);
            draw_branch(ui, props, child, path, depth + 1, selected, out);
            path.pop();
        }
    }
}

struct RowResp {
    row_clicked: bool,
    toggled: bool,
    right_clicked: bool,
    double_clicked: bool,
}

fn draw_row(
    ui: &mut Ui,
    props: &TreeProps,
    item: &TreeItem,
    depth: usize,
    selected: bool,
    has_children: bool,
) -> RowResp {
    let width = ui.available_width();
    let sense = if item.disabled { Sense::hover() } else { Sense::click() };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, props.row_height), sense);

    let painter = ui.painter().clone();
    let hovered = response.hovered() && !item.disabled;
    let bg = if selected {
        tokens::LIST_ACTIVE_SELECTION_BG
    } else if hovered {
        tokens::LIST_HOVER_BG
    } else {
        Color32::TRANSPARENT
    };
    if bg != Color32::TRANSPARENT {
        // Inset the row slightly so the selection highlight has a 4-px gutter
        // on each side, matching VS Code's `monaco-list-row` rendering.
        let highlight = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 4.0, rect.top()),
            egui::pos2(rect.right() - 4.0, rect.bottom()),
        );
        painter.rect_filled(highlight, CornerRadius::same(3), bg);
    }

    // Indent guides: VS Code draws them centred on the chevron column for
    // each ancestor level. Skip level 0 because the root row has no ancestor.
    if props.show_indent_guides && depth > 0 {
        for level in 0..depth {
            // Chevron centre for `level` is at base_x(level) + 6 = level*16 + 14.
            let x = rect.left() + props.indent * level as f32 + 14.0;
            painter.line_segment(
                [
                    egui::pos2(x, rect.top()),
                    egui::pos2(x, rect.bottom()),
                ],
                Stroke::new(1.0, tokens::TREE_INDENT_GUIDE),
            );
        }
    }

    let base_x = rect.left() + props.indent * depth as f32 + 8.0;
    let cy = rect.center().y;

    let mut toggled = false;
    if has_children {
        let chev_center = egui::pos2(base_x + 6.0, cy);
        let chev_rect = egui::Rect::from_center_size(chev_center, Vec2::splat(16.0));
        let chev_resp = ui.interact(chev_rect, response.id.with("chev"), Sense::click());
        let glyph = if item.open { CHEVRON_DOWN } else { CHEVRON_RIGHT };
        // Chevrons render in the description tone — never as prominent as
        // the label or icon. Matches `monaco-tl-twistie` which inherits
        // descriptionForeground.
        painter.text(
            chev_center,
            Align2::CENTER_CENTER,
            glyph.to_string(),
            codicon_font(12.0),
            tokens::FG_DESCRIPTION,
        );
        if chev_resp.clicked() {
            toggled = true;
        }
    }

    let mut text_x = base_x + 18.0;
    let fg = if item.disabled {
        tokens::FG_DISABLED
    } else if selected {
        tokens::LIST_ACTIVE_SELECTION_FG
    } else {
        tokens::FG
    };
    if let Some(glyph) = item.icon {
        painter.text(
            egui::pos2(text_x, cy),
            Align2::LEFT_CENTER,
            glyph.to_string(),
            codicon_font(14.0),
            tokens::ICON_FG,
        );
        text_x += 18.0;
    }
    painter.text(
        egui::pos2(text_x, cy),
        Align2::LEFT_CENTER,
        &item.label,
        FontId::proportional(13.0),
        fg,
    );

    // Whole-row click selects (but if the user hit the chevron we don't
    // double-fire — chevron toggle is enough).
    let row_clicked = response.clicked() && !toggled;

    RowResp {
        row_clicked,
        toggled: toggled || (row_clicked && has_children && item.children.is_empty() == false && false),
        right_clicked: response.secondary_clicked(),
        double_clicked: response.double_clicked(),
    }
}
