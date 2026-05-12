//! vscode-textfield
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-textfield/vscode-textfield.ts
//! Docs:     https://vscode-elements.github.io/components/textfield/
//! VS Code analogue: src/vs/base/browser/ui/inputbox/inputBox.ts
//! Tokens:   --vscode-input-background → Palette::VSCE_INPUT_BG
//!           --vscode-input-foreground → Palette::VSCE_INPUT_FG
//!           --vscode-input-border → Palette::VSCE_INPUT_BORDER
//!           --vscode-input-placeholderForeground → Palette::VSCE_INPUT_PLACEHOLDER_FG
//!           --vscode-focusBorder → Palette::VSCE_FOCUS_BORDER
//!           --vscode-inputValidation-errorBorder → Palette::VSCE_INPUT_ERROR_BORDER
//!
//! Single-line text input. Padding 4×6, border-radius 4, optional
//! leading/trailing codicon slot. The widget wraps `egui::TextEdit` so it
//! is fully interactive — backspace, selection, IME, clipboard all work.

use crate::icons::codicon_font;
use crate::vscode_widgets::tokens;
use egui::{Align2, Color32, CornerRadius, FontId, Response, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct TextFieldProps<'a> {
    pub placeholder: &'a str,
    pub disabled: bool,
    pub readonly: bool,
    pub invalid: bool,
    pub prefix_icon: Option<char>,
    pub suffix_icon: Option<char>,
    /// Force the focus ring even without keyboard focus. Useful for state
    /// cards in the storybook.
    pub focused: bool,
    pub password: bool,
    /// Width in CSS pixels. `None` fills the available width.
    pub width: Option<f32>,
}

impl<'a> Default for TextFieldProps<'a> {
    fn default() -> Self {
        Self {
            placeholder: "",
            disabled: false,
            readonly: false,
            invalid: false,
            prefix_icon: None,
            suffix_icon: None,
            focused: false,
            password: false,
            width: None,
        }
    }
}

impl<'a> TextFieldProps<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn placeholder(mut self, text: &'a str) -> Self {
        self.placeholder = text;
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

    pub fn prefix_icon(mut self, glyph: char) -> Self {
        self.prefix_icon = Some(glyph);
        self
    }

    pub fn suffix_icon(mut self, glyph: char) -> Self {
        self.suffix_icon = Some(glyph);
        self
    }

    pub fn focused(mut self) -> Self {
        self.focused = true;
        self
    }

    pub fn password(mut self) -> Self {
        self.password = true;
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
}

pub fn textfield(ui: &mut Ui, props: &TextFieldProps<'_>, value: &mut String) -> Response {
    let height = 24.0;
    let pad_x = 6.0;
    let icon_slot = 20.0;
    let width = props.width.unwrap_or_else(|| ui.available_width().min(220.0));
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

    let mut text_rect = rect.shrink2(Vec2::new(pad_x, 4.0));
    if let Some(glyph) = props.prefix_icon {
        painter.text(
            egui::pos2(text_rect.left() + 8.0, text_rect.center().y),
            Align2::CENTER_CENTER,
            glyph.to_string(),
            codicon_font(14.0),
            tokens::ICON_FG,
        );
        text_rect.min.x += icon_slot;
    }
    if let Some(glyph) = props.suffix_icon {
        painter.text(
            egui::pos2(rect.right() - pad_x - 6.0, text_rect.center().y),
            Align2::CENTER_CENTER,
            glyph.to_string(),
            codicon_font(14.0),
            tokens::ICON_FG,
        );
        text_rect.max.x -= icon_slot;
    }

    // Embed the actual editable surface. We draw the text/cursor through
    // egui's TextEdit, telling it to draw no frame of its own. The text
    // colour is forced to the input fg token to honour the theme.
    let text_color = if props.disabled {
        with_alpha(tokens::INPUT_FG, 0.6)
    } else {
        tokens::INPUT_FG
    };

    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(text_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );

    let edit_response = child_ui.add_enabled(
        !props.disabled && !props.readonly,
        TextEdit::singleline(value)
            .frame(egui::Frame::NONE)
            .hint_text(props.placeholder)
            .text_color(text_color)
            .password(props.password)
            .desired_width(text_rect.width())
            .font(FontId::proportional(13.0)),
    );

    if props.focused {
        edit_response.request_focus();
    }

    response.union(edit_response)
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
