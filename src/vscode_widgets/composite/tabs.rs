//! vscode-tabs / vscode-tab-header / vscode-tab-panel
//! Upstream: https://github.com/vscode-elements/elements/blob/main/src/vscode-tabs/vscode-tabs.ts
//! Docs:     https://vscode-elements.github.io/components/tabs/
//! VS Code analogue: src/vs/workbench/browser/parts/editor/multiEditorTabsControl.ts
//! Tokens:   --vscode-tab-activeBackground → Palette::VSCE_TAB_ACTIVE_BG()
//!           --vscode-tab-inactiveBackground → Palette::VSCE_TAB_INACTIVE_BG()
//!           --vscode-tab-activeForeground → Palette::VSCE_TAB_ACTIVE_FG()
//!           --vscode-tab-inactiveForeground → Palette::VSCE_TAB_INACTIVE_FG()
//!           --vscode-tab-activeBorderTop → Palette::VSCE_TAB_ACTIVE_BORDER_TOP()
//!           --vscode-tab-border → Palette::VSCE_TAB_BORDER()
//!
//! Horizontal tab strip. Stateless wrt selection — the caller passes
//! `active: &mut usize` and we mutate it on click. Each `Tab` may have a
//! codicon, a "dirty" indicator (filled circle on the close slot), and an
//! optional close button. The widget returns a typed response with the
//! interesting domain events.

use crate::icons::{codicon_font, CIRCLE_FILLED, CLOSE, PINNED};
use crate::vscode_widgets::tokens;
use egui::{Align2, Color32, CornerRadius, FontId, Response, Sense, Ui, Vec2};

#[derive(Clone, Copy, Debug)]
pub struct Tab<'a> {
    pub label: &'a str,
    pub icon: Option<char>,
    /// File-type icon from the Seti icon font (glyph + theme colour). Takes
    /// precedence over `icon`, which renders from the codicon font.
    pub seti_icon: Option<(char, egui::Color32)>,
    pub closable: bool,
    pub dirty: bool,
    pub disabled: bool,
    /// Preview tab — VS Code renders the label in italics for files opened
    /// with a single click that haven't been "kept" (double-clicked).
    pub preview: bool,
    /// Pinned tab — sits on the left, shows a pin glyph instead of a close
    /// button, and isn't closed by middle-click / close-others.
    pub pinned: bool,
    /// Full path (or any longer description) shown as a hover tooltip.
    pub tooltip: Option<&'a str>,
    /// A full-colour image in place of the icon (the Welcome tab's logo).
    pub image: Option<egui::TextureId>,
}

impl<'a> Tab<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            seti_icon: None,
            closable: true,
            dirty: false,
            disabled: false,
            preview: false,
            pinned: false,
            tooltip: None,
            image: None,
        }
    }

    pub fn icon(mut self, glyph: char) -> Self {
        self.icon = Some(glyph);
        self
    }

    pub fn seti_icon(mut self, glyph: char, color: egui::Color32) -> Self {
        self.seti_icon = Some((glyph, color));
        self
    }

    pub fn dirty(mut self) -> Self {
        self.dirty = true;
        self
    }

    pub fn closable(mut self, value: bool) -> Self {
        self.closable = value;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    pub fn preview(mut self) -> Self {
        self.preview = true;
        self
    }

    pub fn pinned(mut self) -> Self {
        self.pinned = true;
        self
    }

    pub fn tooltip(mut self, text: &'a str) -> Self {
        self.tooltip = Some(text);
        self
    }

    pub fn image(mut self, texture: egui::TextureId) -> Self {
        self.image = Some(texture);
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TabsProps {
    /// Height of the tab strip in CSS pixels.
    pub height: f32,
    /// Minimum tab width (so single short labels still look like tabs).
    pub min_tab_width: f32,
    /// VS Code 1.145's connected editor tabs (Modern UI,
    /// `connectedEditorTabs.css`): the strip in
    /// `editorGroupHeader.connectedTabsBackground` with a bottom border, the
    /// active tab in the editor's colour joined to the editor below it.
    pub connected: bool,
}

impl Default for TabsProps {
    fn default() -> Self {
        Self {
            height: 35.0,
            min_tab_width: 80.0,
            connected: false,
        }
    }
}

impl TabsProps {
    /// The workbench's editor tabs in VS Code 1.145 (32px strip incl. its
    /// bottom border).
    pub fn connected() -> Self {
        Self { height: 32.0, min_tab_width: 0.0, connected: true }
    }
}

/// Active connected tab: top corner radius
/// (`--modern-ui-connected-tab-cap-radius`, cornerRadius.medium + 1) and
/// the fillet joining its right edge to the strip's bottom border.
const CAP_RADIUS: f32 = 7.0;
const SHOULDER_RADIUS: f32 = 6.0;

#[derive(Debug, Default)]
pub struct TabsResponse {
    pub clicked: Option<usize>,
    pub close_requested: Option<usize>,
    pub right_clicked: Option<usize>,
    pub double_clicked: Option<usize>,
}

pub fn tabs(
    ui: &mut Ui,
    props: &TabsProps,
    items: &[Tab<'_>],
    active: &mut usize,
) -> TabsResponse {
    let mut out = TabsResponse::default();
    let height = props.height;

    let row_rect = egui::Rect::from_min_size(
        ui.cursor().min,
        Vec2::new(ui.available_width(), height),
    );
    // Paint the strip background once — individual tabs paint their own bg
    // on top.
    if props.connected {
        ui.painter().rect_filled(row_rect, 0.0, crate::theme::Palette::CONNECTED_TABS_BG());
        // The bottom border runs under every tab but the active one, which
        // paints over it.
        ui.painter().rect_filled(
            egui::Rect::from_min_max(egui::pos2(row_rect.left(), row_rect.bottom() - 1.0), row_rect.max),
            0.0,
            crate::theme::Palette::TABS_STRIP_BORDER(),
        );
    } else {
        ui.painter().rect_filled(row_rect, 0.0, tokens::TAB_INACTIVE_BG());
    }

    // Track whether the active index changed this frame so we can scroll it
    // into view (VS Code keeps the active editor tab visible).
    let prev_active = *active;

    egui::ScrollArea::horizontal()
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                for (idx, tab) in items.iter().enumerate() {
                    let is_active = idx == *active;
                    let tab_resp = draw_single_tab(ui, props, tab, is_active);
                    // Scroll the active tab into view when the selection
                    // changed (e.g. opening a file far down the strip).
                    if is_active && prev_active != *active {
                        ui.scroll_to_rect(tab_resp.rect, None);
                    }
                    if tab_resp.label_clicked && !tab.disabled {
                        *active = idx;
                        out.clicked = Some(idx);
                    }
                    if tab_resp.close_clicked {
                        out.close_requested = Some(idx);
                    }
                    if tab_resp.right_clicked {
                        out.right_clicked = Some(idx);
                    }
                    if tab_resp.double_clicked {
                        out.double_clicked = Some(idx);
                    }
                }
            });
        });

    if !props.connected {
        // Bottom hairline matches workbench border.
        ui.painter().line_segment(
            [
                egui::pos2(row_rect.left(), row_rect.bottom() - 0.5),
                egui::pos2(row_rect.right(), row_rect.bottom() - 0.5),
            ],
            egui::Stroke::new(1.0, tokens::TAB_BORDER()),
        );
    }

    out
}

struct SingleTabResp {
    label_clicked: bool,
    close_clicked: bool,
    right_clicked: bool,
    double_clicked: bool,
    rect: egui::Rect,
}

fn draw_single_tab(
    ui: &mut Ui,
    props: &TabsProps,
    tab: &Tab<'_>,
    is_active: bool,
) -> SingleTabResp {
    let pad_x = 10.0;
    let icon_size = 14.0;
    let icon_gap = 6.0;
    // Pinned tabs always reserve the trailing slot for the pin glyph; other
    // tabs reserve it when closable or dirty.
    let trailing_slot = if tab.pinned || tab.closable || tab.dirty {
        22.0
    } else {
        0.0
    };

    // Preview tabs (opened with one click, not kept yet) are italic.
    let label_font = if tab.preview { crate::ui_font::italic(13.0) } else { FontId::proportional(13.0) };
    let label_galley = ui.painter().layout_no_wrap(tab.label.to_string(), label_font, Color32::WHITE);

    let mut content_w = label_galley.size().x;
    if tab.icon.is_some() || tab.seti_icon.is_some() || tab.image.is_some() {
        content_w += icon_size + icon_gap;
    }
    content_w += trailing_slot;
    // Pinned tabs are compact (VS Code shrinks them to icon + pin width).
    let min_w = if tab.pinned { 0.0 } else { props.min_tab_width };
    // Connected tabs, measured in VS Code 1.145.2: icon centred 14px in,
    // label at 28px, close button centred 16px from the right edge, which
    // is 31px past the label.
    let tab_w = if props.connected {
        28.0 + label_galley.size().x + 31.0
    } else {
        (content_w + pad_x * 2.0).max(min_w)
    };

    let sense = if tab.disabled {
        Sense::hover()
    } else {
        Sense::click_and_drag()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(tab_w, props.height), sense);

    let painter = ui.painter().clone();
    let hovered = response.hovered() && !tab.disabled;
    if props.connected {
        paint_connected_tab(&painter, rect, is_active, hovered);
    } else {
        let bg = match (is_active, hovered) {
            (true, _) => tokens::TAB_ACTIVE_BG(),
            (false, true) => tokens::LIST_HOVER_BG(),
            (false, false) => tokens::TAB_INACTIVE_BG(),
        };
        painter.rect_filled(rect, 0.0, bg);
    }

    if is_active && !props.connected {
        // 1 px accent line at the top edge.
        let top = egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.right(), rect.top() + 1.0),
        );
        painter.rect_filled(top, 0.0, tokens::TAB_ACTIVE_BORDER_TOP());
    }
    // Right separator.
    if !props.connected {
        painter.line_segment(
            [
                egui::pos2(rect.right() - 0.5, rect.top()),
                egui::pos2(rect.right() - 0.5, rect.bottom()),
            ],
            egui::Stroke::new(1.0, tokens::TAB_BORDER()),
        );
    }

    let fg = if tab.disabled {
        tokens::FG_DISABLED()
    } else if is_active {
        tokens::TAB_ACTIVE_FG()
    } else {
        tokens::TAB_INACTIVE_FG()
    };

    let mut cursor = if props.connected { rect.left() + 14.0 - icon_size * 0.5 } else { rect.left() + pad_x };
    let cy = if props.connected { rect.center().y - 0.5 } else { rect.center().y };
    if let Some(texture) = tab.image {
        let r = egui::Rect::from_center_size(egui::pos2(cursor + icon_size * 0.5, cy), Vec2::splat(16.0));
        painter.image(texture, r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        cursor += icon_size + icon_gap;
    } else if let Some((glyph, color)) = tab.seti_icon {
        painter.text(
            egui::pos2(cursor + icon_size * 0.5, cy),
            Align2::CENTER_CENTER,
            glyph.to_string(),
            crate::file_icons::seti_font(icon_size + 1.0),
            color,
        );
        cursor += icon_size + icon_gap;
    } else if let Some(glyph) = tab.icon {
        let icon_size = if props.connected { 16.0 } else { icon_size };
        painter.text(
            egui::pos2(cursor + icon_size * 0.5, cy),
            Align2::CENTER_CENTER,
            glyph.to_string(),
            codicon_font(icon_size),
            fg,
        );
        cursor += icon_size + icon_gap;
    }
    if props.connected {
        cursor = rect.left() + 28.0;
    }
    let label_fg = fg;
    painter.galley(
        egui::pos2(cursor, cy - label_galley.size().y * 0.5),
        label_galley,
        label_fg,
    );

    // Trailing slot: pin glyph for pinned tabs, otherwise close / dirty dot.
    let mut close_clicked = false;
    if tab.pinned {
        let pin_center = egui::pos2(rect.right() - 12.0, cy);
        let pin_rect = egui::Rect::from_center_size(pin_center, Vec2::splat(16.0));
        let pin_resp = ui.interact(pin_rect, response.id.with("pin"), Sense::click());
        if pin_resp.hovered() {
            painter.rect_filled(pin_rect, CornerRadius::same(3), tokens::LIST_HOVER_BG());
        }
        // Hovering the pin offers an unpin affordance via the close glyph;
        // otherwise show the filled pin (or a dirty dot if unsaved).
        let glyph = if tab.dirty && !pin_resp.hovered() {
            CIRCLE_FILLED
        } else {
            PINNED
        };
        painter.text(
            pin_center,
            Align2::CENTER_CENTER,
            glyph.to_string(),
            codicon_font(12.0),
            fg,
        );
        if pin_resp.clicked() {
            // A click on the pin glyph is surfaced as a close request; the
            // caller decides whether that means "unpin" or "close".
            close_clicked = true;
        }
    } else if tab.closable || tab.dirty {
        let close_center = egui::pos2(rect.right() - if props.connected { 16.0 } else { 12.0 }, cy);
        let close_rect = egui::Rect::from_center_size(close_center, Vec2::splat(16.0));
        let close_resp =
            ui.interact(close_rect, response.id.with("close"), Sense::click());
        let close_hovered = close_resp.hovered();
        if close_hovered {
            painter.rect_filled(close_rect, CornerRadius::same(3), tokens::LIST_HOVER_BG());
        }
        let glyph = if tab.dirty && !close_hovered {
            CIRCLE_FILLED
        } else {
            CLOSE
        };
        painter.text(
            close_center,
            Align2::CENTER_CENTER,
            glyph.to_string(),
            codicon_font(12.0),
            fg,
        );
        if close_resp.clicked() {
            close_clicked = true;
        }
    }

    // Full-path tooltip on hover.
    let response = if let Some(tip) = tab.tooltip {
        response.on_hover_text(tip)
    } else {
        response
    };

    SingleTabResp {
        label_clicked: response.clicked() && !close_clicked,
        close_clicked,
        right_clicked: response.secondary_clicked(),
        double_clicked: response.double_clicked(),
        rect,
    }
}

/// A connected tab: inactive tabs show the strip (hover adds 6% of the
/// foreground); the active one is the editor's colour from the top edge
/// through the strip's bottom border, with a rounded top-right corner and
/// a fillet into the bottom border (connectedEditorTabs.css).
fn paint_connected_tab(painter: &egui::Painter, rect: egui::Rect, active: bool, hovered: bool) {
    use crate::theme::Palette;
    if !active {
        if hovered {
            let strip = Palette::CONNECTED_TABS_BG();
            let wash = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.bottom() - 1.0));
            painter.rect_filled(wash, 0.0, mix(strip, Palette::FG(), 0.06));
        }
        return;
    }
    let bg = Palette::EDITOR_BG();
    let border = Palette::TABS_STRIP_BORDER();
    let r = CAP_RADIUS;
    let s = SHOULDER_RADIUS;
    painter.rect_filled(rect, CornerRadius { ne: r as u8, ..Default::default() }, bg);
    // Fillet: editor colour below a quarter circle in the strip colour.
    let fillet = egui::Rect::from_min_size(egui::pos2(rect.right(), rect.bottom() - s), egui::vec2(s, s));
    painter.rect_filled(fillet, 0.0, bg);
    painter
        .with_clip_rect(fillet)
        .circle_filled(egui::pos2(rect.right() + s, rect.bottom() - s), s, Palette::CONNECTED_TABS_BG());
    // Outline: down from the rounded top-right corner, through the fillet,
    // into the bottom border.
    let x = rect.right() - 0.5;
    let mut points = Vec::new();
    let steps = 8;
    for i in 0..=steps {
        let a = std::f32::consts::FRAC_PI_2 * i as f32 / steps as f32;
        points.push(egui::pos2(x - r + r * a.sin(), rect.top() + r - r * a.cos()));
    }
    for i in 0..=steps {
        let a = std::f32::consts::FRAC_PI_2 * i as f32 / steps as f32;
        points.push(egui::pos2(x + s - s * a.cos(), rect.bottom() - 0.5 - s + s * a.sin()));
    }
    painter.add(egui::Shape::line(points, egui::Stroke::new(1.0, border)));
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let [ar, ag, ab, _] = a.to_array();
    let [br, bg, bb, _] = b.to_array();
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(ar, br), l(ag, bg), l(ab, bb))
}
