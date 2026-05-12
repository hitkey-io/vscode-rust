//! vscode-collapsible
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-collapsible/vscode-collapsible.ts
//! Docs:     https://vscode-elements.github.io/components/collapsible/
//! VS Code analogue: src/vs/workbench/browser/parts/sidebar/sidebarPart.ts
//!                   (Explorer/Search section headers)
//! Tokens:   --vscode-sideBarSectionHeader-background → Palette::VSCE_SECTION_HEADER_BG
//!           --vscode-sideBarSectionHeader-foreground → Palette::VSCE_SECTION_HEADER_FG
//!           --vscode-sideBarSectionHeader-border → Palette::VSCE_SECTION_HEADER_BORDER
//!           --vscode-icon-foreground → Palette::VSCE_ICON_FG
//!
//! Header strip + collapsible body. The `open` flag is owned by the caller
//! so it can be wired to keyboard shortcuts or restored from settings.
//! Clicking the header toggles it.

use crate::icons::{codicon_font, CHEVRON_DOWN, CHEVRON_RIGHT};
use crate::vscode_widgets::tokens;
use egui::{
    Align2, Color32, FontId, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
};

#[derive(Clone, Copy, Debug)]
pub struct CollapsibleProps<'a> {
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub disabled: bool,
}

impl<'a> CollapsibleProps<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            description: None,
            disabled: false,
        }
    }

    pub fn description(mut self, text: &'a str) -> Self {
        self.description = Some(text);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

pub struct CollapsibleResponse {
    pub header: Response,
    /// `true` if the user clicked the header on this frame.
    pub toggled: bool,
}

pub fn collapsible<R>(
    ui: &mut Ui,
    props: &CollapsibleProps<'_>,
    open: &mut bool,
    body: impl FnOnce(&mut Ui) -> R,
) -> CollapsibleResponse {
    let header_height = 22.0;
    let pad_x = 8.0;
    let chevron_size = 16.0;

    let width = ui.available_width();
    let (header_rect, header_resp) = ui.allocate_exact_size(
        Vec2::new(width, header_height),
        if props.disabled { Sense::hover() } else { Sense::click() },
    );

    let painter = ui.painter().clone();
    let hovered = header_resp.hovered() && !props.disabled;
    let bg = if hovered {
        tokens::LIST_HOVER_BG
    } else {
        tokens::SECTION_HEADER_BG
    };
    painter.rect_filled(header_rect, 0.0, bg);
    painter.line_segment(
        [
            egui::pos2(header_rect.left(), header_rect.bottom() - 0.5),
            egui::pos2(header_rect.right(), header_rect.bottom() - 0.5),
        ],
        Stroke::new(1.0, tokens::SECTION_HEADER_BORDER),
    );

    let chevron_glyph = if *open { CHEVRON_DOWN } else { CHEVRON_RIGHT };
    let chevron_pos = egui::pos2(header_rect.left() + pad_x + chevron_size * 0.5, header_rect.center().y);
    painter.text(
        chevron_pos,
        Align2::CENTER_CENTER,
        chevron_glyph.to_string(),
        codicon_font(12.0),
        with_alpha(tokens::ICON_FG, if props.disabled { 0.5 } else { 1.0 }),
    );

    let title_pos = egui::pos2(
        chevron_pos.x + chevron_size * 0.5 + 4.0,
        header_rect.center().y,
    );
    painter.text(
        title_pos,
        Align2::LEFT_CENTER,
        props.title.to_uppercase(),
        FontId::proportional(11.0),
        with_alpha(tokens::SECTION_HEADER_FG, if props.disabled { 0.5 } else { 1.0 }),
    );

    if let Some(desc) = props.description {
        let title_galley = painter.layout_no_wrap(
            props.title.to_uppercase(),
            FontId::proportional(11.0),
            Color32::WHITE,
        );
        let desc_pos = egui::pos2(
            title_pos.x + title_galley.size().x + 8.0,
            header_rect.center().y,
        );
        painter.text(
            desc_pos,
            Align2::LEFT_CENTER,
            desc,
            FontId::proportional(11.0),
            tokens::FG_DESCRIPTION,
        );
    }

    if header_resp.has_focus() && !props.disabled {
        painter.rect_stroke(
            header_rect.shrink(1.0),
            0,
            Stroke::new(1.0, tokens::FOCUS_BORDER),
            StrokeKind::Inside,
        );
    }

    let toggled = header_resp.clicked();
    if toggled {
        *open = !*open;
    }

    if *open {
        ui.allocate_ui(Vec2::new(width, 0.0), |ui| {
            ui.add_space(2.0);
            body(ui);
            ui.add_space(2.0);
        });
    }

    CollapsibleResponse {
        header: header_resp,
        toggled,
    }
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
