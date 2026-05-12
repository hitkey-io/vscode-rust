//! vscode-label
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-label/vscode-label.ts
//! Docs:     https://vscode-elements.github.io/components/label/
//! VS Code analogue: src/vs/base/browser/ui/iconLabel/iconLabel.ts
//! Tokens:   --vscode-foreground → Palette::VSCE_FG
//!           --vscode-settings-headerForeground → Palette::VSCE_LABEL_FG
//!           --vscode-errorForeground (required marker) → Palette::VSCE_FG_ERROR
//!
//! Renders a single-line label. Bold-by-default, with a `normal` variant
//! to opt out. `required: true` appends a red asterisk after the text.

use crate::vscode_widgets::tokens;
use egui::{FontId, Response, RichText, Sense, Ui};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LabelEmphasis {
    /// Bold — the upstream default.
    #[default]
    Strong,
    /// Plain weight — matches `<span class="normal">…</span>`.
    Normal,
}

#[derive(Clone, Debug)]
pub struct LabelProps<'a> {
    pub text: &'a str,
    /// Bold by default. Set to `Normal` to mimic the inline span variant.
    pub emphasis: LabelEmphasis,
    /// Appends a red asterisk after the text.
    pub required: bool,
    /// Render in description tone (dimmer) instead of label foreground.
    pub description: bool,
    /// Font size in CSS pixels. Default `13.0`.
    pub size: f32,
    /// Override colour. `None` defers to `emphasis`/`description`.
    pub color: Option<egui::Color32>,
}

impl<'a> LabelProps<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            emphasis: LabelEmphasis::Strong,
            required: false,
            description: false,
            size: 13.0,
            color: None,
        }
    }

    pub fn normal(mut self) -> Self {
        self.emphasis = LabelEmphasis::Normal;
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn description(mut self) -> Self {
        self.description = true;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: egui::Color32) -> Self {
        self.color = Some(color);
        self
    }
}

pub fn label(ui: &mut Ui, props: &LabelProps<'_>) -> Response {
    let fg = props.color.unwrap_or_else(|| {
        if props.description {
            tokens::FG_DESCRIPTION
        } else {
            tokens::LABEL_FG
        }
    });

    let mut text = RichText::new(props.text)
        .color(fg)
        .font(FontId::proportional(props.size));
    if matches!(props.emphasis, LabelEmphasis::Strong) {
        text = text.strong();
    }

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let response = ui.add(egui::Label::new(text).sense(Sense::hover()));
        if props.required {
            ui.label(
                RichText::new("*")
                    .color(tokens::FG_ERROR)
                    .font(FontId::proportional(props.size))
                    .strong(),
            );
        }
        response
    })
    .inner
}
