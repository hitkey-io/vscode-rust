//! vscode-textarea
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-textarea/vscode-textarea.ts
//! Docs:     https://vscode-elements.github.io/components/textarea/
//! VS Code analogue: src/vs/base/browser/ui/inputbox/inputBox.ts (textarea mode)
//! Tokens:   same set as `textfield` (--vscode-input-* and validation).
//!
//! Multi-line text input. Same visuals as `textfield` but with an explicit
//! `rows` count driving the height.

use crate::vscode_widgets::tokens;
use egui::{Color32, CornerRadius, FontId, Response, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct TextareaProps<'a> {
    pub placeholder: &'a str,
    pub disabled: bool,
    pub readonly: bool,
    pub invalid: bool,
    pub focused: bool,
    /// Visible rows of text. Default `3`.
    pub rows: usize,
    /// Width in CSS pixels. `None` fills the available width.
    pub width: Option<f32>,
}

impl<'a> Default for TextareaProps<'a> {
    fn default() -> Self {
        Self {
            placeholder: "",
            disabled: false,
            readonly: false,
            invalid: false,
            focused: false,
            rows: 3,
            width: None,
        }
    }
}

impl<'a> TextareaProps<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn placeholder(mut self, text: &'a str) -> Self {
        self.placeholder = text;
        self
    }

    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    pub fn invalid(mut self) -> Self {
        self.invalid = true;
        self
    }

    pub fn focused(mut self) -> Self {
        self.focused = true;
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
}

pub fn textarea(ui: &mut Ui, props: &TextareaProps<'_>, value: &mut String) -> Response {
    let line_height = 18.0;
    let pad = 6.0;
    let height = line_height * props.rows as f32 + pad * 2.0;
    let width = props.width.unwrap_or_else(|| ui.available_width().min(280.0));

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(width, height),
        if props.disabled { Sense::hover() } else { Sense::click() },
    );

    let painter = ui.painter().clone();
    let bg = if props.disabled {
        with_alpha(tokens::INPUT_BG, 0.6)
    } else {
        tokens::INPUT_BG
    };
    painter.rect_filled(rect, CornerRadius::same(4), bg);

    let focus_visible = props.focused || (response.has_focus() && !props.disabled);
    let border = if props.invalid {
        tokens::INPUT_ERROR_BORDER
    } else if focus_visible {
        tokens::FOCUS_BORDER
    } else {
        tokens::INPUT_BORDER
    };
    painter.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );

    let text_color = if props.disabled {
        with_alpha(tokens::INPUT_FG, 0.6)
    } else {
        tokens::INPUT_FG
    };
    let inner = rect.shrink(pad);
    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    let edit = child_ui.add_enabled(
        !props.disabled && !props.readonly,
        TextEdit::multiline(value)
            .frame(egui::Frame::NONE)
            .hint_text(props.placeholder)
            .text_color(text_color)
            .desired_rows(props.rows)
            .desired_width(inner.width())
            .font(FontId::proportional(13.0)),
    );

    if props.focused {
        edit.request_focus();
    }

    response.union(edit)
}

fn with_alpha(color: Color32, alpha: f32) -> Color32 {
    let [r, g, b, a] = color.to_array();
    let a = (a as f32 * alpha) as u8;
    Color32::from_rgba_premultiplied(
        ((r as u16 * a as u16) / 255) as u8,
        ((g as u16 * a as u16) / 255) as u8,
        ((b as u16 * a as u16) / 255) as u8,
        a,
    )
}
