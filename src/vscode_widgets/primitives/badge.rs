//! vscode-badge
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-badge/vscode-badge.ts
//! Docs:     https://vscode-elements.github.io/components/badge/
//! VS Code analogue: src/vs/base/browser/ui/countBadge/countBadge.ts
//! Tokens:   --vscode-badge-background → Palette::VSCE_BADGE_BG
//!           --vscode-badge-foreground → Palette::VSCE_BADGE_FG
//!           --vscode-activityBarBadge-background → 0x0078D4 (uses VSCE_BUTTON_BG)
//!
//! Compact label rendered as a pill (default) or rounded rect (counter
//! variant). Activity-bar variant uses the accent blue background for the
//! file/search/etc. counter indicators on the activity bar.

use crate::vscode_widgets::tokens;
use egui::{Align2, Color32, CornerRadius, FontId, Response, Sense, Ui, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BadgeVariant {
    /// Pill shape (rounded ends, 18×18 minimum).
    #[default]
    Default,
    /// Rectangular counter — minimal padding, square corners (radius 2).
    Counter,
    /// Activity-bar counter (filled blue, white text).
    ActivityBar,
    /// Tab-header counter (subtler).
    TabHeader,
}

#[derive(Clone, Copy, Debug)]
pub struct BadgeProps<'a> {
    pub text: &'a str,
    pub variant: BadgeVariant,
}

impl<'a> BadgeProps<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            variant: BadgeVariant::Default,
        }
    }

    pub fn counter(text: &'a str) -> Self {
        Self {
            text,
            variant: BadgeVariant::Counter,
        }
    }

    pub fn activity_bar(text: &'a str) -> Self {
        Self {
            text,
            variant: BadgeVariant::ActivityBar,
        }
    }

    pub fn tab_header(text: &'a str) -> Self {
        Self {
            text,
            variant: BadgeVariant::TabHeader,
        }
    }
}

pub fn badge(ui: &mut Ui, props: &BadgeProps<'_>) -> Response {
    let font = FontId::proportional(11.0);
    let galley = ui.painter().layout_no_wrap(
        props.text.to_string(),
        font,
        Color32::WHITE,
    );

    let (pad_x, pad_y, radius, min_w, min_h, bg, fg) = match props.variant {
        BadgeVariant::Default => (
            5.0, 3.0, 11, 18.0, 18.0,
            tokens::BADGE_BG, tokens::BADGE_FG,
        ),
        BadgeVariant::Counter => (
            3.0, 2.0, 2, 0.0, 0.0,
            tokens::BADGE_BG, tokens::BADGE_FG,
        ),
        BadgeVariant::ActivityBar => (
            5.0, 3.0, 11, 18.0, 18.0,
            tokens::BUTTON_BG, Color32::WHITE,
        ),
        BadgeVariant::TabHeader => (
            4.0, 2.0, 9, 0.0, 0.0,
            tokens::BADGE_BG, tokens::BADGE_FG,
        ),
    };

    let w = (galley.size().x + pad_x * 2.0).max(min_w);
    let h = (galley.size().y + pad_y * 2.0).max(min_h);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());

    ui.painter()
        .rect_filled(rect, CornerRadius::same(radius), bg);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        props.text,
        FontId::proportional(11.0),
        fg,
    );

    response
}
