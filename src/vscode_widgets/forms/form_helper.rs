//! vscode-form-helper
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-form-helper/vscode-form-helper.ts
//! Docs:     https://vscode-elements.github.io/components/form-helper/
//! VS Code analogue: src/vs/workbench/browser/parts/preferences/settingsTree.ts
//! Tokens:   --vscode-descriptionForeground → Palette::VSCE_FG_DESCRIPTION
//!           --vscode-errorForeground → Palette::VSCE_FG_ERROR
//!
//! Helper / description text rendered beneath a form field.

use crate::vscode_widgets::tokens;
use egui::{FontId, Response, RichText, Ui};

#[derive(Clone, Copy, Debug)]
pub struct FormHelperProps<'a> {
    pub text: &'a str,
    pub error: bool,
    pub size: f32,
}

impl<'a> FormHelperProps<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            error: false,
            size: 12.0,
        }
    }

    pub fn error(mut self) -> Self {
        self.error = true;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

pub fn form_helper(ui: &mut Ui, props: &FormHelperProps<'_>) -> Response {
    let color = if props.error {
        tokens::FG_ERROR
    } else {
        tokens::FG_DESCRIPTION
    };
    ui.label(
        RichText::new(props.text)
            .color(color)
            .font(FontId::proportional(props.size)),
    )
}
