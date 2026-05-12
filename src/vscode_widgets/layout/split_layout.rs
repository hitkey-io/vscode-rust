//! vscode-split-layout
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-split-layout/vscode-split-layout.ts
//! Docs:     https://vscode-elements.github.io/components/split-layout/
//! VS Code analogue: src/vs/base/browser/ui/splitview/splitview.ts
//!                   src/vs/base/browser/ui/sash/sash.ts
//! Tokens:   --vscode-sash-hoverBorder → Palette::VSCE_SASH_HOVER_BORDER
//!           --vscode-textSeparator-foreground → Palette::VSCE_TEXT_SEPARATOR_FG
//!
//! Two-pane split with a draggable sash. `split_pos` is the size of the
//! first pane in CSS pixels, owned by the caller so it can be persisted
//! (e.g. to settings). The sash is 4 px wide; the hit area is 8 px wide
//! so users don't have to be pixel-perfect.

use crate::vscode_widgets::tokens;
use egui::{Color32, CursorIcon, Response, Sense, Stroke, Ui, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SplitOrientation {
    /// Two panes side-by-side, separated by a vertical sash.
    #[default]
    Vertical,
    /// Two panes stacked, separated by a horizontal sash.
    Horizontal,
}

#[derive(Clone, Copy, Debug)]
pub struct SplitLayoutProps {
    pub orientation: SplitOrientation,
    /// Minimum size of either pane in CSS pixels.
    pub min_size: f32,
    /// Whether the user can drag the sash. `false` freezes the split.
    pub resizable: bool,
}

impl Default for SplitLayoutProps {
    fn default() -> Self {
        Self {
            orientation: SplitOrientation::Vertical,
            min_size: 50.0,
            resizable: true,
        }
    }
}

impl SplitLayoutProps {
    pub fn vertical() -> Self {
        Self::default()
    }

    pub fn horizontal() -> Self {
        Self {
            orientation: SplitOrientation::Horizontal,
            ..Self::default()
        }
    }

    pub fn min_size(mut self, min_size: f32) -> Self {
        self.min_size = min_size;
        self
    }

    pub fn locked(mut self) -> Self {
        self.resizable = false;
        self
    }
}

pub fn split_layout<R1, R2>(
    ui: &mut Ui,
    props: &SplitLayoutProps,
    split_pos: &mut f32,
    first: impl FnOnce(&mut Ui) -> R1,
    second: impl FnOnce(&mut Ui) -> R2,
) -> Response {
    let avail = ui.available_size();
    let sash = 4.0;

    let full = match props.orientation {
        SplitOrientation::Vertical => avail.x,
        SplitOrientation::Horizontal => avail.y,
    };
    let max = (full - props.min_size - sash).max(props.min_size);
    *split_pos = split_pos.clamp(props.min_size, max);

    let outer = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(outer, Sense::hover());

    let (first_rect, sash_rect, second_rect) = match props.orientation {
        SplitOrientation::Vertical => {
            let split_x = outer.left() + *split_pos;
            (
                egui::Rect::from_min_max(
                    outer.left_top(),
                    egui::pos2(split_x, outer.bottom()),
                ),
                egui::Rect::from_min_max(
                    egui::pos2(split_x, outer.top()),
                    egui::pos2(split_x + sash, outer.bottom()),
                ),
                egui::Rect::from_min_max(
                    egui::pos2(split_x + sash, outer.top()),
                    outer.right_bottom(),
                ),
            )
        }
        SplitOrientation::Horizontal => {
            let split_y = outer.top() + *split_pos;
            (
                egui::Rect::from_min_max(
                    outer.left_top(),
                    egui::pos2(outer.right(), split_y),
                ),
                egui::Rect::from_min_max(
                    egui::pos2(outer.left(), split_y),
                    egui::pos2(outer.right(), split_y + sash),
                ),
                egui::Rect::from_min_max(
                    egui::pos2(outer.left(), split_y + sash),
                    outer.right_bottom(),
                ),
            )
        }
    };

    let sash_hit = sash_rect.expand2(match props.orientation {
        SplitOrientation::Vertical => Vec2::new(2.0, 0.0),
        SplitOrientation::Horizontal => Vec2::new(0.0, 2.0),
    });
    let sash_sense = if props.resizable {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let sash_resp = ui.interact(sash_hit, ui.id().with("vsce_sash"), sash_sense);

    if props.resizable {
        let cursor = match props.orientation {
            SplitOrientation::Vertical => CursorIcon::ResizeColumn,
            SplitOrientation::Horizontal => CursorIcon::ResizeRow,
        };
        if sash_resp.hovered() || sash_resp.dragged() {
            ui.ctx().set_cursor_icon(cursor);
        }
        if sash_resp.dragged() {
            let delta = match props.orientation {
                SplitOrientation::Vertical => sash_resp.drag_delta().x,
                SplitOrientation::Horizontal => sash_resp.drag_delta().y,
            };
            *split_pos = (*split_pos + delta).clamp(props.min_size, max);
        }
    }

    ui.painter().rect_filled(sash_rect, 0.0, Color32::TRANSPARENT);
    if sash_resp.hovered() || sash_resp.dragged() {
        ui.painter()
            .rect_filled(sash_rect, 0.0, tokens::SASH_HOVER_BORDER);
    } else {
        // Always render a 1 px hairline so the split is visible even at rest.
        let line = match props.orientation {
            SplitOrientation::Vertical => egui::Rect::from_min_max(
                egui::pos2(sash_rect.center().x - 0.5, sash_rect.top()),
                egui::pos2(sash_rect.center().x + 0.5, sash_rect.bottom()),
            ),
            SplitOrientation::Horizontal => egui::Rect::from_min_max(
                egui::pos2(sash_rect.left(), sash_rect.center().y - 0.5),
                egui::pos2(sash_rect.right(), sash_rect.center().y + 0.5),
            ),
        };
        ui.painter().rect_filled(line, 0.0, tokens::TEXT_SEPARATOR_FG);
    }

    let mut first_ui = ui.new_child(egui::UiBuilder::new().max_rect(first_rect));
    first(&mut first_ui);
    let mut second_ui = ui.new_child(egui::UiBuilder::new().max_rect(second_rect));
    second(&mut second_ui);

    let _ = Stroke::NONE;
    response
}
