//! vscode-single-select
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-single-select/vscode-single-select.ts
//! Docs:     https://vscode-elements.github.io/components/single-select/
//! VS Code analogue: src/vs/base/browser/ui/selectBox/selectBox.ts
//! Tokens:   --vscode-dropdown-background → Palette::VSCE_DROPDOWN_BG
//!           --vscode-dropdown-border → Palette::VSCE_DROPDOWN_BORDER
//!           --vscode-dropdown-foreground → Palette::VSCE_DROPDOWN_FG
//!           --vscode-list-hoverBackground → Palette::VSCE_LIST_HOVER_BG
//!           --vscode-list-activeSelectionBackground → Palette::VSCE_LIST_ACTIVE_SELECTION_BG
//!
//! Closed dropdown trigger + optional popup list. The trigger is always
//! rendered inline; the popup, when `open: true`, is drawn directly below
//! it in the same layout flow. Real workbench callers usually want the
//! popup in an `egui::Area` for floating behaviour — the storybook
//! showcases the inline mode because that's what screenshots need.

use crate::icons::{codicon_font, CHEVRON_DOWN};
use crate::vscode_widgets::tokens;
use egui::{
    Align2, Color32, CornerRadius, FontId, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
};

#[derive(Clone, Copy, Debug)]
pub struct SingleSelectProps<'a> {
    pub options: &'a [&'a str],
    pub placeholder: &'a str,
    pub disabled: bool,
    pub invalid: bool,
    pub focused: bool,
    /// Width of the trigger button. `None` fills the available width.
    pub width: Option<f32>,
}

impl<'a> SingleSelectProps<'a> {
    pub fn new(options: &'a [&'a str]) -> Self {
        Self {
            options,
            placeholder: "Select…",
            disabled: false,
            invalid: false,
            focused: false,
            width: None,
        }
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

    pub fn focused(mut self) -> Self {
        self.focused = true;
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
}

#[derive(Debug, Default)]
pub struct SingleSelectResponse {
    pub changed: Option<usize>,
    pub opened: bool,
    pub dismissed: bool,
}

pub fn single_select(
    ui: &mut Ui,
    props: &SingleSelectProps<'_>,
    selected: &mut Option<usize>,
    open: &mut bool,
) -> SingleSelectResponse {
    let mut out = SingleSelectResponse::default();
    let height = 26.0;
    let width = props.width.unwrap_or_else(|| ui.available_width().min(240.0));

    let sense = if props.disabled { Sense::hover() } else { Sense::click() };
    let (rect, trigger_resp) = ui.allocate_exact_size(Vec2::new(width, height), sense);

    let painter = ui.painter().clone();
    painter.rect_filled(rect, CornerRadius::same(4), tokens::DROPDOWN_BG);

    let focus_visible = props.focused || (trigger_resp.has_focus() && !props.disabled);
    let border = if props.invalid {
        tokens::INPUT_ERROR_BORDER
    } else if focus_visible {
        tokens::FOCUS_BORDER
    } else {
        tokens::DROPDOWN_BORDER
    };
    painter.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );

    let cy = rect.center().y;
    let fg = if props.disabled {
        with_alpha(tokens::DROPDOWN_FG, 0.5)
    } else {
        tokens::DROPDOWN_FG
    };
    let text = match selected {
        Some(i) if *i < props.options.len() => props.options[*i].to_string(),
        _ => props.placeholder.to_string(),
    };
    let placeholder = selected.is_none() || selected.map_or(true, |i| i >= props.options.len());
    let text_color = if placeholder {
        tokens::INPUT_PLACEHOLDER_FG
    } else {
        fg
    };
    painter.text(
        egui::pos2(rect.left() + 8.0, cy),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(13.0),
        text_color,
    );
    painter.text(
        egui::pos2(rect.right() - 12.0, cy),
        Align2::CENTER_CENTER,
        CHEVRON_DOWN.to_string(),
        codicon_font(12.0),
        fg,
    );

    if trigger_resp.clicked() && !props.disabled {
        *open = !*open;
        if *open {
            out.opened = true;
        } else {
            out.dismissed = true;
        }
    }

    if *open && !props.disabled {
        let popup_y = rect.bottom() + 2.0;
        let item_h = 22.0;
        let pad = 4.0;
        let popup_h = pad * 2.0 + props.options.len() as f32 * item_h;
        let popup_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left(), popup_y),
            Vec2::new(width, popup_h),
        );
        // Allocate vertical space below so the host UI advances past the popup.
        ui.allocate_exact_size(Vec2::new(width, popup_h + 2.0), Sense::hover());

        let pp = ui.painter().clone();
        pp.rect_filled(popup_rect, CornerRadius::same(4), tokens::DROPDOWN_BG);
        pp.rect_stroke(
            popup_rect,
            CornerRadius::same(4),
            Stroke::new(1.0, tokens::DROPDOWN_BORDER),
            StrokeKind::Inside,
        );

        for (i, option) in props.options.iter().enumerate() {
            let row_rect = egui::Rect::from_min_size(
                egui::pos2(popup_rect.left() + 2.0, popup_rect.top() + pad + i as f32 * item_h),
                Vec2::new(width - 4.0, item_h),
            );
            let row_resp = ui.interact(row_rect, ui.id().with(("ss_row", i)), Sense::click());
            let is_selected = *selected == Some(i);
            let bg = if is_selected {
                tokens::LIST_ACTIVE_SELECTION_BG
            } else if row_resp.hovered() {
                tokens::LIST_HOVER_BG
            } else {
                Color32::TRANSPARENT
            };
            if bg != Color32::TRANSPARENT {
                pp.rect_filled(row_rect, CornerRadius::same(3), bg);
            }
            let row_fg = if is_selected {
                tokens::LIST_ACTIVE_SELECTION_FG
            } else {
                tokens::DROPDOWN_FG
            };
            pp.text(
                egui::pos2(row_rect.left() + 8.0, row_rect.center().y),
                Align2::LEFT_CENTER,
                *option,
                FontId::proportional(13.0),
                row_fg,
            );
            if row_resp.clicked() {
                *selected = Some(i);
                *open = false;
                out.changed = Some(i);
                out.dismissed = true;
            }
        }
    }

    out
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
