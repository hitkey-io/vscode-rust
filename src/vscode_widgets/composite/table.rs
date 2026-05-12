//! vscode-table / table-header / table-row / table-cell / table-body
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-table/vscode-table.ts
//! Docs:     https://vscode-elements.github.io/components/table/
//! VS Code analogue: src/vs/base/browser/ui/table/table.ts
//! Tokens:   --vscode-dropdown-background → Palette::VSCE_DROPDOWN_BG (header bg)
//!           --vscode-list-hoverBackground → Palette::VSCE_LIST_HOVER_BG
//!           --vscode-list-activeSelectionBackground → Palette::VSCE_LIST_ACTIVE_SELECTION_BG
//!           --vscode-tree-tableColumnsBorder → Palette::VSCE_TABLE_COLUMN_BORDER
//!           --vscode-tree-tableOddRowsBackground → Palette::VSCE_TABLE_ODD_ROW_BG
//!
//! Single-API table that takes a slice of header strings and a `&[&[&str]]`
//! body matrix. Row selection is owned by the caller.

use crate::vscode_widgets::tokens;
use egui::{Align2, Color32, CornerRadius, FontId, Sense, Stroke, StrokeKind, Ui, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct TableProps<'a> {
    pub headers: &'a [&'a str],
    /// Optional fixed column widths. `None` means each column splits the
    /// available width equally.
    pub column_widths: Option<&'a [f32]>,
    pub striped: bool,
    pub border_columns: bool,
    pub row_height: f32,
    pub header_height: f32,
}

impl<'a> TableProps<'a> {
    pub fn new(headers: &'a [&'a str]) -> Self {
        Self {
            headers,
            column_widths: None,
            striped: false,
            border_columns: false,
            row_height: 22.0,
            header_height: 28.0,
        }
    }

    pub fn column_widths(mut self, widths: &'a [f32]) -> Self {
        self.column_widths = Some(widths);
        self
    }

    pub fn striped(mut self) -> Self {
        self.striped = true;
        self
    }

    pub fn border_columns(mut self) -> Self {
        self.border_columns = true;
        self
    }
}

#[derive(Debug, Default)]
pub struct TableResponse {
    pub clicked_row: Option<usize>,
    pub right_clicked_row: Option<usize>,
    pub double_clicked_row: Option<usize>,
}

pub fn table(
    ui: &mut Ui,
    props: &TableProps<'_>,
    body: &[&[&str]],
    selected_row: &mut Option<usize>,
) -> TableResponse {
    let mut out = TableResponse::default();

    let col_count = props.headers.len();
    let total_width = ui.available_width();
    let widths: Vec<f32> = match props.column_widths {
        Some(w) if w.len() == col_count => w.to_vec(),
        _ => vec![total_width / col_count as f32; col_count],
    };

    // Header row — VS Code's `monaco-list-table` header has no fill; it
    // blends with the body and is separated only by a 1-px hairline at the
    // bottom and the slightly bolder label colour.
    let header_rect = egui::Rect::from_min_size(
        ui.cursor().min,
        Vec2::new(total_width, props.header_height),
    );
    let painter = ui.painter().clone();
    let mut x = header_rect.left();
    for (col_idx, header) in props.headers.iter().enumerate() {
        let cw = widths[col_idx];
        let cell_rect = egui::Rect::from_min_size(
            egui::pos2(x, header_rect.top()),
            Vec2::new(cw, props.header_height),
        );
        painter.text(
            egui::pos2(cell_rect.left() + 8.0, cell_rect.center().y),
            Align2::LEFT_CENTER,
            *header,
            FontId::proportional(11.5),
            tokens::FG_DESCRIPTION,
        );
        if props.border_columns && col_idx + 1 < col_count {
            painter.line_segment(
                [
                    egui::pos2(cell_rect.right() - 0.5, cell_rect.top() + 4.0),
                    egui::pos2(cell_rect.right() - 0.5, cell_rect.bottom() - 4.0),
                ],
                Stroke::new(1.0, tokens::TABLE_COLUMN_BORDER),
            );
        }
        x += cw;
    }
    painter.line_segment(
        [
            egui::pos2(header_rect.left(), header_rect.bottom() - 0.5),
            egui::pos2(header_rect.right(), header_rect.bottom() - 0.5),
        ],
        Stroke::new(1.0, tokens::TABLE_COLUMN_BORDER),
    );
    ui.allocate_exact_size(Vec2::new(total_width, props.header_height), Sense::hover());

    // Body rows.
    for (row_idx, row) in body.iter().enumerate() {
        let row_rect = egui::Rect::from_min_size(
            ui.cursor().min,
            Vec2::new(total_width, props.row_height),
        );
        let row_resp = ui.interact(row_rect, ui.id().with(("table_row", row_idx)), Sense::click());

        let is_selected = *selected_row == Some(row_idx);
        let bg = if is_selected {
            tokens::LIST_ACTIVE_SELECTION_BG
        } else if row_resp.hovered() {
            tokens::LIST_HOVER_BG
        } else if props.striped && row_idx % 2 == 1 {
            tokens::TABLE_ODD_ROW_BG
        } else {
            Color32::TRANSPARENT
        };
        if bg != Color32::TRANSPARENT {
            ui.painter().rect_filled(row_rect, CornerRadius::ZERO, bg);
        }

        let mut x = row_rect.left();
        let fg = if is_selected {
            tokens::LIST_ACTIVE_SELECTION_FG
        } else {
            tokens::FG
        };
        for (col_idx, cell) in row.iter().enumerate() {
            let cw = widths.get(col_idx).copied().unwrap_or(80.0);
            let cell_rect = egui::Rect::from_min_size(
                egui::pos2(x, row_rect.top()),
                Vec2::new(cw, props.row_height),
            );
            ui.painter().text(
                egui::pos2(cell_rect.left() + 8.0, cell_rect.center().y),
                Align2::LEFT_CENTER,
                *cell,
                FontId::proportional(12.5),
                fg,
            );
            if props.border_columns && col_idx + 1 < widths.len() {
                ui.painter().line_segment(
                    [
                        egui::pos2(cell_rect.right() - 0.5, cell_rect.top()),
                        egui::pos2(cell_rect.right() - 0.5, cell_rect.bottom()),
                    ],
                    Stroke::new(1.0, tokens::TABLE_COLUMN_BORDER),
                );
            }
            x += cw;
        }

        if row_resp.clicked() {
            *selected_row = Some(row_idx);
            out.clicked_row = Some(row_idx);
        }
        if row_resp.secondary_clicked() {
            out.right_clicked_row = Some(row_idx);
        }
        if row_resp.double_clicked() {
            out.double_clicked_row = Some(row_idx);
        }
        ui.allocate_exact_size(Vec2::new(total_width, props.row_height), Sense::hover());
    }

    let _ = StrokeKind::Inside;
    out
}
