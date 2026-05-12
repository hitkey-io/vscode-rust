//! vscode-context-menu / vscode-context-menu-item
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-context-menu/vscode-context-menu.ts
//! Docs:     https://vscode-elements.github.io/components/context-menu/
//! VS Code analogue: src/vs/base/browser/ui/menu/menu.ts
//! Tokens:   --vscode-menu-background → Palette::VSCE_DROPDOWN_BG
//!           --vscode-menu-foreground → Palette::VSCE_DROPDOWN_FG
//!           --vscode-menu-selectionBackground → Palette::VSCE_LIST_ACTIVE_SELECTION_BG
//!           --vscode-menu-selectionForeground → Palette::VSCE_LIST_ACTIVE_SELECTION_FG
//!           --vscode-menu-border → Palette::VSCE_DROPDOWN_BORDER
//!           --vscode-foregroundDisabled → Palette::VSCE_FG_DISABLED
//!
//! Popup menu container. The caller draws the menu inside an `egui::Area`
//! (or `popup_below_widget`) and passes the items. The widget returns the
//! index of the activated item — if any — so the caller can dismiss the
//! popup and execute the corresponding command.

use crate::icons::codicon_font;
use crate::vscode_widgets::tokens;
use egui::{Align2, Color32, CornerRadius, FontId, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct ContextMenuItem<'a> {
    pub label: &'a str,
    pub icon: Option<char>,
    pub shortcut: Option<&'a str>,
    pub disabled: bool,
    /// Render a separator instead of an item. When `true`, all other fields
    /// are ignored.
    pub separator: bool,
}

impl<'a> ContextMenuItem<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            shortcut: None,
            disabled: false,
            separator: false,
        }
    }

    pub fn separator() -> Self {
        Self {
            label: "",
            icon: None,
            shortcut: None,
            disabled: false,
            separator: true,
        }
    }

    pub fn icon(mut self, glyph: char) -> Self {
        self.icon = Some(glyph);
        self
    }

    pub fn shortcut(mut self, text: &'a str) -> Self {
        self.shortcut = Some(text);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ContextMenuProps {
    /// Menu width in CSS pixels.
    pub min_width: f32,
    /// Per-item height in CSS pixels.
    pub item_height: f32,
}

impl Default for ContextMenuProps {
    fn default() -> Self {
        Self {
            min_width: 220.0,
            item_height: 26.0,
        }
    }
}

#[derive(Debug, Default)]
pub struct ContextMenuResponse {
    /// Index into the *full* items slice (separators included) of the
    /// item the user activated, if any.
    pub selected: Option<usize>,
}

pub fn context_menu(
    ui: &mut Ui,
    props: &ContextMenuProps,
    items: &[ContextMenuItem<'_>],
) -> ContextMenuResponse {
    let mut out = ContextMenuResponse::default();
    let width = props.min_width.max(ui.available_width());

    // Frame: rounded background + 1 px border + soft shadow.
    let total_height: f32 = items
        .iter()
        .map(|it| if it.separator { 8.0 } else { props.item_height })
        .sum::<f32>()
        + 8.0;
    let frame_rect = egui::Rect::from_min_size(ui.cursor().min, Vec2::new(width, total_height));
    let painter = ui.painter().clone();
    painter.rect_filled(frame_rect, CornerRadius::same(5), tokens::DROPDOWN_BG);
    painter.rect_stroke(
        frame_rect,
        CornerRadius::same(5),
        Stroke::new(1.0, tokens::DROPDOWN_BORDER),
        StrokeKind::Inside,
    );

    ui.allocate_ui_at_rect(frame_rect, |ui| {
        ui.add_space(4.0);
        for (idx, item) in items.iter().enumerate() {
            if item.separator {
                ui.add_space(3.0);
                let r = egui::Rect::from_min_size(
                    egui::pos2(ui.cursor().min.x + 8.0, ui.cursor().min.y),
                    Vec2::new(width - 16.0, 1.0),
                );
                ui.painter()
                    .rect_filled(r, 0.0, tokens::DROPDOWN_BORDER);
                ui.allocate_exact_size(Vec2::new(width, 5.0), Sense::hover());
                continue;
            }
            if draw_menu_item(ui, props, width, item) {
                out.selected = Some(idx);
            }
        }
        ui.add_space(4.0);
    });
    out
}

fn draw_menu_item(
    ui: &mut Ui,
    props: &ContextMenuProps,
    width: f32,
    item: &ContextMenuItem<'_>,
) -> bool {
    let sense = if item.disabled { Sense::hover() } else { Sense::click() };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, props.item_height), sense);
    let hovered = response.hovered() && !item.disabled;

    let painter = ui.painter().clone();
    let fg = if item.disabled {
        tokens::FG_DISABLED
    } else if hovered {
        tokens::LIST_ACTIVE_SELECTION_FG
    } else {
        tokens::DROPDOWN_FG
    };
    if hovered {
        let inner = rect.shrink2(Vec2::new(4.0, 0.0));
        painter.rect_filled(inner, CornerRadius::same(3), tokens::LIST_ACTIVE_SELECTION_BG);
    }

    let cy = rect.center().y;
    let mut cursor = rect.left() + 16.0;
    if let Some(glyph) = item.icon {
        painter.text(
            egui::pos2(cursor, cy),
            Align2::LEFT_CENTER,
            glyph.to_string(),
            codicon_font(14.0),
            fg,
        );
        cursor += 24.0;
    }
    painter.text(
        egui::pos2(cursor, cy),
        Align2::LEFT_CENTER,
        item.label,
        FontId::proportional(13.0),
        fg,
    );
    if let Some(sc) = item.shortcut {
        painter.text(
            egui::pos2(rect.right() - 16.0, cy),
            Align2::RIGHT_CENTER,
            sc,
            FontId::proportional(12.0),
            if item.disabled { fg } else { tokens::FG_DESCRIPTION },
        );
    }

    let _ = Color32::TRANSPARENT;
    response.clicked()
}
