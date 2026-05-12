//! vscode-multi-select
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-multi-select/vscode-multi-select.ts
//! Docs:     https://vscode-elements.github.io/components/multi-select/
//! VS Code analogue: src/vs/base/browser/ui/selectBox/selectBox.ts (multi-select extension)
//! Tokens:   --vscode-dropdown-* family + checkbox + badge tokens.
//!
//! Same trigger/popup pattern as `single_select` but each option carries
//! an 18×18 checkbox and the trigger renders a count badge instead of the
//! selected label.

use crate::icons::{codicon_font, CHECK, CHEVRON_DOWN};
use crate::vscode_widgets::tokens;
use egui::{
    Align2, Color32, CornerRadius, FontId, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
};

#[derive(Clone, Copy, Debug)]
pub struct MultiSelectProps<'a> {
    pub options: &'a [&'a str],
    pub placeholder: &'a str,
    pub disabled: bool,
    pub invalid: bool,
    pub focused: bool,
    pub width: Option<f32>,
}

impl<'a> MultiSelectProps<'a> {
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
pub struct MultiSelectResponse {
    pub changed: Option<Vec<usize>>,
    pub opened: bool,
    pub dismissed: bool,
    pub count: usize,
}

pub fn multi_select(
    ui: &mut Ui,
    props: &MultiSelectProps<'_>,
    selected: &mut Vec<usize>,
    open: &mut bool,
) -> MultiSelectResponse {
    let mut out = MultiSelectResponse {
        count: selected.len(),
        ..Default::default()
    };
    let height = 26.0;
    let width = props.width.unwrap_or_else(|| ui.available_width().min(260.0));

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
    if selected.is_empty() {
        painter.text(
            egui::pos2(rect.left() + 8.0, cy),
            Align2::LEFT_CENTER,
            props.placeholder,
            FontId::proportional(13.0),
            tokens::INPUT_PLACEHOLDER_FG,
        );
    } else {
        // Count badge + summary text.
        let badge_text = format!("{}", selected.len());
        let badge_galley = painter.layout_no_wrap(
            badge_text.clone(),
            FontId::proportional(11.0),
            Color32::WHITE,
        );
        let badge_w = (badge_galley.size().x + 10.0).max(18.0);
        let badge_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 8.0, cy - 9.0),
            Vec2::new(badge_w, 18.0),
        );
        painter.rect_filled(badge_rect, CornerRadius::same(9), tokens::BADGE_BG);
        painter.text(
            badge_rect.center(),
            Align2::CENTER_CENTER,
            badge_text,
            FontId::proportional(11.0),
            tokens::BADGE_FG,
        );

        let labels: Vec<&str> = selected
            .iter()
            .filter_map(|&i| props.options.get(i).copied())
            .collect();
        painter.text(
            egui::pos2(badge_rect.right() + 8.0, cy),
            Align2::LEFT_CENTER,
            labels.join(", "),
            FontId::proportional(13.0),
            fg,
        );
    }
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
        let item_h = 24.0;
        let pad = 4.0;
        let popup_h = pad * 2.0 + props.options.len() as f32 * item_h;
        let popup_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left(), popup_y),
            Vec2::new(width, popup_h),
        );
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
            let row_resp = ui.interact(row_rect, ui.id().with(("ms_row", i)), Sense::click());
            let is_checked = selected.contains(&i);
            if row_resp.hovered() {
                pp.rect_filled(row_rect, CornerRadius::same(3), tokens::LIST_HOVER_BG);
            }

            // Checkbox: 14×14, vertically centred, 8 px from left edge.
            let box_size = 14.0;
            let box_rect = egui::Rect::from_center_size(
                egui::pos2(row_rect.left() + 8.0 + box_size * 0.5, row_rect.center().y),
                Vec2::splat(box_size),
            );
            pp.rect_filled(box_rect, CornerRadius::same(3), tokens::CHECKBOX_BG);
            pp.rect_stroke(
                box_rect,
                CornerRadius::same(3),
                Stroke::new(1.0, tokens::CHECKBOX_BORDER),
                StrokeKind::Inside,
            );
            if is_checked {
                pp.text(
                    box_rect.center(),
                    Align2::CENTER_CENTER,
                    CHECK.to_string(),
                    codicon_font(11.0),
                    tokens::FG,
                );
            }

            pp.text(
                egui::pos2(box_rect.right() + 8.0, row_rect.center().y),
                Align2::LEFT_CENTER,
                *option,
                FontId::proportional(13.0),
                tokens::DROPDOWN_FG,
            );

            if row_resp.clicked() {
                if is_checked {
                    selected.retain(|&j| j != i);
                } else {
                    selected.push(i);
                    selected.sort_unstable();
                }
                out.changed = Some(selected.clone());
                out.count = selected.len();
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
