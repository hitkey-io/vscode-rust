//! vscode-scrollable
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-scrollable/vscode-scrollable.ts
//! Docs:     https://vscode-elements.github.io/components/scrollable/
//! VS Code analogue: src/vs/base/browser/ui/scrollbar/media/scrollbars.css
//! Tokens:   --vscode-scrollbarSlider-background → Palette::VSCE_SCROLLBAR_SLIDER_BG
//!           --vscode-scrollbarSlider-hoverBackground → Palette::VSCE_SCROLLBAR_SLIDER_HOVER
//!           --vscode-scrollbarSlider-activeBackground → Palette::VSCE_SCROLLBAR_SLIDER_ACTIVE
//!
//! Thin wrapper around `egui::ScrollArea` with the upstream slider colours
//! patched into the local `Visuals`. The wrapper is a function, not a
//! `*Builder` chain, so callers can pass their content inline.

use crate::vscode_widgets::tokens;
use egui::{Response, Stroke, Ui};

#[derive(Clone, Copy, Debug)]
pub struct ScrollableProps {
    pub vertical: bool,
    pub horizontal: bool,
    /// Container height. `None` lets the scroll area fill the parent.
    pub max_height: Option<f32>,
    /// Container width. `None` lets the scroll area fill the parent.
    pub max_width: Option<f32>,
}

impl Default for ScrollableProps {
    fn default() -> Self {
        Self {
            vertical: true,
            horizontal: false,
            max_height: None,
            max_width: None,
        }
    }
}

impl ScrollableProps {
    pub fn vertical() -> Self {
        Self::default()
    }

    pub fn horizontal() -> Self {
        Self {
            vertical: false,
            horizontal: true,
            ..Self::default()
        }
    }

    pub fn both() -> Self {
        Self {
            vertical: true,
            horizontal: true,
            ..Self::default()
        }
    }

    pub fn max_height(mut self, h: f32) -> Self {
        self.max_height = Some(h);
        self
    }

    pub fn max_width(mut self, w: f32) -> Self {
        self.max_width = Some(w);
        self
    }
}

pub fn scrollable<R>(
    ui: &mut Ui,
    props: &ScrollableProps,
    content: impl FnOnce(&mut Ui) -> R,
) -> Response {
    // VS Code (`vscode-elements <vscode-scrollable>`) behaviour:
    //   - At rest: scrollbar is fully invisible.
    //   - On hover: a translucent dark track appears under a thin grey
    //     pill-shaped slider, both pinned to the right edge.
    //   - The slider takes ~10 px of horizontal space, lives on top of the
    //     content (does not reserve layout space).
    //
    // Slider colour: `scrollbarSlider.background = rgba(121,121,121,0.4)`.
    // Track colour: a dark wash, ~rgba(0,0,0,0.34) over the editor bg,
    // which we approximate with the existing INPUT_BG token (#313131)
    // displayed at low alpha through `interact_background_opacity`.
    let style = ui.style_mut();
    let v = &mut style.visuals;
    v.widgets.inactive.bg_fill = tokens::SCROLLBAR_SLIDER_BG;
    v.widgets.hovered.bg_fill = tokens::SCROLLBAR_SLIDER_HOVER;
    v.widgets.active.bg_fill = tokens::SCROLLBAR_SLIDER_ACTIVE;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.active.bg_stroke = Stroke::NONE;
    let corner = egui::CornerRadius::same(2);
    v.widgets.inactive.corner_radius = corner;
    v.widgets.hovered.corner_radius = corner;
    v.widgets.active.corner_radius = corner;
    // The track is painted using `extreme_bg_color`. We want a dark grey
    // overlay; the actual visibility is controlled by the *_background
    // opacities below.
    v.extreme_bg_color = egui::Color32::from_rgba_premultiplied(0, 0, 0, 0x55);
    style.spacing.scroll.floating = true;
    style.spacing.scroll.bar_width = 10.0;
    style.spacing.scroll.floating_width = 10.0;
    style.spacing.scroll.floating_allocated_width = 0.0;
    style.spacing.scroll.bar_inner_margin = 0.0;
    style.spacing.scroll.bar_outer_margin = 0.0;
    style.spacing.scroll.handle_min_length = 20.0;
    style.spacing.scroll.foreground_color = false;
    // Track and slider both stay invisible at rest, both fade in on hover.
    style.spacing.scroll.dormant_background_opacity = 0.0;
    style.spacing.scroll.active_background_opacity = 0.7;
    style.spacing.scroll.interact_background_opacity = 1.0;
    style.spacing.scroll.dormant_handle_opacity = 0.0;
    style.spacing.scroll.active_handle_opacity = 1.0;
    style.spacing.scroll.interact_handle_opacity = 1.0;

    let mut scroll_area = egui::ScrollArea::new([props.horizontal, props.vertical])
        .auto_shrink([false, false]);
    if let Some(h) = props.max_height {
        scroll_area = scroll_area.max_height(h);
    }
    if let Some(w) = props.max_width {
        scroll_area = scroll_area.max_width(w);
    }
    scroll_area.show(ui, content).inner_rect;
    ui.response()
}
