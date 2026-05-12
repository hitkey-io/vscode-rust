//! vscode-form-container / vscode-form-group
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-form-container/
//! Docs:     https://vscode-elements.github.io/components/form-container/
//! VS Code analogue: src/vs/workbench/browser/parts/preferences/preferencesWidgets.ts
//! Tokens:   None directly — composes child widgets.
//!
//! Vertical stack of form rows. `form_group` lays out a label and a control
//! side-by-side (horizontal) or stacked (vertical).

use crate::vscode_widgets::tokens;
use egui::{FontId, Layout, Response, RichText, Sense, Ui, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct FormContainerProps {
    /// Spacing between rows (CSS pixels). Default `8.0`.
    pub row_gap: f32,
}

impl Default for FormContainerProps {
    fn default() -> Self {
        Self { row_gap: 8.0 }
    }
}

impl FormContainerProps {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn row_gap(mut self, gap: f32) -> Self {
        self.row_gap = gap;
        self
    }
}

pub fn form_container<R>(
    ui: &mut Ui,
    props: &FormContainerProps,
    rows: impl FnOnce(&mut FormContainerCtx, &mut Ui) -> R,
) -> Response {
    let mut ctx = FormContainerCtx {
        row_gap: props.row_gap,
        first: true,
    };
    ui.vertical(|ui| {
        rows(&mut ctx, ui);
    })
    .response
}

pub struct FormContainerCtx {
    row_gap: f32,
    first: bool,
}

impl FormContainerCtx {
    /// Adds the configured row gap before the next form group.
    pub fn separator(&mut self, ui: &mut Ui) {
        if self.first {
            self.first = false;
            return;
        }
        ui.add_space(self.row_gap);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FormGroupProps<'a> {
    pub label: &'a str,
    pub required: bool,
    /// Stack the label above the control instead of placing them on the
    /// same row.
    pub vertical: bool,
    /// Width reserved for the label when laid out horizontally.
    pub label_width: f32,
}

impl<'a> FormGroupProps<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            required: false,
            vertical: false,
            label_width: 140.0,
        }
    }

    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn label_width(mut self, w: f32) -> Self {
        self.label_width = w;
        self
    }
}

pub fn form_group<R>(
    ui: &mut Ui,
    props: &FormGroupProps<'_>,
    control: impl FnOnce(&mut Ui) -> R,
) -> Response {
    let label_text = if props.required {
        format!("{} *", props.label)
    } else {
        props.label.to_string()
    };

    if props.vertical {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(label_text)
                    .color(tokens::LABEL_FG)
                    .font(FontId::proportional(13.0))
                    .strong(),
            );
            ui.add_space(4.0);
            control(ui);
        })
        .response
    } else {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(props.label_width, 0.0),
                Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.label(
                        RichText::new(label_text)
                            .color(tokens::LABEL_FG)
                            .font(FontId::proportional(13.0))
                            .strong(),
                    );
                },
            );
            control(ui);
        })
        .response
    }
}
