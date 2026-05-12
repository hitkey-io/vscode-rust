//! vscode-toolbar-container + vscode-toolbar-button
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-toolbar-container/
//! Docs:     https://vscode-elements.github.io/components/toolbar-container/
//! VS Code analogue: src/vs/base/browser/ui/toolbar/toolbar.ts
//! Tokens:   --vscode-icon-foreground → Palette::VSCE_ICON_FG
//!           --vscode-list-hoverBackground → Palette::VSCE_LIST_HOVER_BG
//!
//! Container that lays out a row of action icons. Callers compose buttons
//! inside the closure via `vscode_widgets::primitives::icon_button`. This
//! container is responsible for the row layout, optional title slot, and
//! right-alignment of the action cluster.

use crate::vscode_widgets::tokens;
use egui::{Color32, FontId, Layout, Response, RichText, Ui, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct ToolbarContainerProps<'a> {
    /// Optional title rendered at the start of the row (e.g. "EXPLORER").
    pub title: Option<&'a str>,
    /// Right-align the action cluster (the common workbench pattern).
    pub right_aligned: bool,
    /// Vertical padding inside the row.
    pub pad_y: f32,
}

impl<'a> Default for ToolbarContainerProps<'a> {
    fn default() -> Self {
        Self {
            title: None,
            right_aligned: true,
            pad_y: 2.0,
        }
    }
}

impl<'a> ToolbarContainerProps<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, text: &'a str) -> Self {
        self.title = Some(text);
        self
    }

    pub fn left_aligned(mut self) -> Self {
        self.right_aligned = false;
        self
    }
}

pub fn toolbar_container<R>(
    ui: &mut Ui,
    props: &ToolbarContainerProps<'_>,
    actions: impl FnOnce(&mut Ui) -> R,
) -> Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
        ui.add_space(8.0);
        if let Some(title) = props.title {
            ui.label(
                RichText::new(title.to_uppercase())
                    .color(tokens::SECTION_HEADER_FG)
                    .font(FontId::proportional(11.0))
                    .strong(),
            );
        }
        ui.add_space(4.0);
        ui.add_space(props.pad_y);
        if props.right_aligned {
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                actions(ui);
            });
        } else {
            actions(ui);
        }
        // `tokens` import is needed to keep the doc-comment cross-reference
        // exact even when the row is empty.
        let _ = tokens::ICON_FG;
        let _ = Color32::TRANSPARENT;
    })
    .response
}
