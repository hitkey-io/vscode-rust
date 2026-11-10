//! The activity bar as VS Code 1.145 draws it with Modern UI on: a card on
//! the shell background (`floatingPanels.css` `.part.activitybar`), 32px
//! rounded item indicators for the checked and hovered entries
//! (`contrib/modernUI/.../activityBar.css`), entries 44px apart. Geometry
//! measured against VS Code 1.145.2.

use egui::{Align2, CornerRadius, FontId, Rect, Sense, Ui};

use super::cards;
use super::ActivityView;
use crate::icons;
use crate::theme::Palette;

/// Distance between two entries (36px action + 8px gap).
const PITCH: f32 = 44.0;
/// The checked / hovered indicator (`--activity-bar-action-height` − 4).
const INDICATOR: f32 = 32.0;
/// From the card's top edge to the first indicator.
const TOP: f32 = 6.0;
/// From the card's left edge to the indicators.
const LEFT: f32 = 6.0;
/// From the card's bottom edge to the last indicator's bottom.
const BOTTOM: f32 = 7.0;
const ICON: f32 = 24.0;

/// `scm_count` paints the Source Control count badge (VS Code `scm.countBadge`).
/// Returns `true` if the user clicked one of the visual-only icons (Run/Debug,
/// Accounts, Manage gear); the caller opens the command palette.
pub fn show(ui: &mut Ui, current: &mut ActivityView, sidebar_visible: &mut bool, scm_count: usize) -> bool {
    show_with_containers(ui, current, sidebar_visible, scm_count, &[], &mut None)
}

/// An extension's activity bar entry (`viewsContainers`).
pub struct ContainerIcon {
    pub id: String,
    pub title: String,
    /// White mask tinted like the built-in icons, if the file loaded.
    pub icon: Option<egui::TextureHandle>,
}

enum Glyph<'a> {
    Codicon(char),
    Texture(&'a egui::TextureHandle),
}

/// One entry: indicator when checked or hovered, the icon, its tooltip and
/// accessible name.
fn entry(ui: &mut Ui, top_left: egui::Pos2, glyph: Glyph<'_>, checked: bool, label: &str) -> egui::Response {
    let indicator = Rect::from_min_size(top_left, egui::vec2(INDICATOR, INDICATOR));
    // The action item is 36px square around the indicator.
    let hit = indicator.expand(2.0);
    let resp = ui.interact(hit, ui.id().with(("activity", label)), Sense::click());
    let painter = ui.painter();
    let fg = if checked {
        painter.rect_filled(indicator, cards::RADIUS_SMALL, Palette::MODERN_ACTIVITY_ITEM_ACTIVE_BG());
        Palette::MODERN_ACTIVITY_ITEM_ACTIVE_FG()
    } else if resp.hovered() {
        painter.rect_filled(indicator, cards::RADIUS_SMALL, Palette::MODERN_ACTIVITY_ITEM_HOVER_BG());
        Palette::MODERN_ACTIVITY_ITEM_HOVER_FG()
    } else {
        Palette::ACTIVITY_BAR_INACTIVE_FG()
    };
    match glyph {
        Glyph::Codicon(c) => {
            painter.text(indicator.center(), Align2::CENTER_CENTER, c.to_string(), icons::codicon_font(ICON), fg);
        }
        Glyph::Texture(tex) => {
            painter.image(
                tex.id(),
                Rect::from_center_size(indicator.center(), egui::vec2(ICON, ICON)),
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                fg,
            );
        }
    }
    let tip = label.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip.clone()));
    resp.on_hover_text(label)
}

/// [`show`] plus the extensions' containers below the built-in views;
/// `selected` is the container showing when `current` is
/// `ActivityView::ExtensionContainer`.
pub fn show_with_containers(
    ui: &mut Ui,
    current: &mut ActivityView,
    sidebar_visible: &mut bool,
    scm_count: usize,
    containers: &[ContainerIcon],
    selected: &mut Option<String>,
) -> bool {
    let mut palette_requested = false;
    // The card: 4px from the window's left edge and from the status bar.
    let full = ui.max_rect();
    let card = Rect::from_min_max(
        egui::pos2(full.left() + cards::MARGIN, full.top()),
        egui::pos2(full.right(), full.bottom() - cards::MARGIN),
    );
    let radius = CornerRadius { nw: cards::RADIUS as u8, ne: 0, sw: cards::RADIUS as u8, se: 0 };
    ui.painter().rect_filled(full, 0.0, Palette::SHELL_BG());
    ui.painter().rect_filled(card, radius, Palette::MODERN_ACTIVITY_BAR_BG());

    let x = card.left() + LEFT;
    let mut y = card.top() + TOP;
    // `None`: Run and Debug, which has no view yet.
    let views = [
        (Some(ActivityView::Explorer), icons::FILES, "Explorer (⇧⌘E)"),
        (Some(ActivityView::Search), icons::SEARCH, "Search (⇧⌘F)"),
        (Some(ActivityView::SourceControl), icons::SOURCE_CONTROL, "Source Control (⌃⇧G)"),
        (None, icons::DEBUG_ALT, "Run and Debug (⇧⌘D)"),
        (Some(ActivityView::Extensions), icons::EXTENSIONS, "Extensions (⇧⌘X)"),
    ];
    for (view, glyph, label) in views {
        let checked = view == Some(*current) && *sidebar_visible;
        let resp = entry(ui, egui::pos2(x, y), Glyph::Codicon(glyph), checked, label);
        if view == Some(ActivityView::SourceControl) && scm_count > 0 {
            badge(ui, Rect::from_min_size(egui::pos2(x, y), egui::vec2(INDICATOR, INDICATOR)), scm_count);
        }
        if resp.clicked() {
            match view {
                // No Run and Debug view yet: the command palette instead.
                None => palette_requested = true,
                Some(_) if checked => *sidebar_visible = false,
                Some(v) => {
                    *current = v;
                    *sidebar_visible = true;
                }
            }
        }
        y += PITCH;
    }
    for c in containers {
        let checked = *current == ActivityView::ExtensionContainer
            && selected.as_deref() == Some(c.id.as_str())
            && *sidebar_visible;
        let glyph = match &c.icon {
            Some(tex) => Glyph::Texture(tex),
            None => Glyph::Codicon(icons::EXTENSIONS),
        };
        if entry(ui, egui::pos2(x, y), glyph, checked, &c.title).clicked() {
            if checked {
                *sidebar_visible = false;
            } else {
                *current = ActivityView::ExtensionContainer;
                *selected = Some(c.id.clone());
                *sidebar_visible = true;
            }
        }
        y += PITCH;
    }

    // Accounts and Manage, anchored to the bottom.
    let mut y = card.bottom() - BOTTOM - INDICATOR - PITCH;
    for (glyph, label) in [(icons::ACCOUNT, "Accounts"), (icons::SETTINGS_GEAR, "Manage")] {
        if entry(ui, egui::pos2(x, y), Glyph::Codicon(glyph), false, label).clicked() {
            palette_requested = true;
        }
        y += PITCH;
    }

    // The right edge meets the side bar card (or is the card's own edge).
    let right_radius = if *sidebar_visible { 0 } else { cards::RADIUS as u8 };
    let radius = CornerRadius { ne: right_radius, se: right_radius, ..radius };
    cards::finish(ui.painter(), card, radius, Palette::MODERN_ACTIVITY_BAR_BORDER(), Palette::SHELL_BG(), false);
    palette_requested
}

/// The Source Control count badge, bottom-right of the indicator.
fn badge(ui: &Ui, indicator: Rect, count: usize) {
    let txt = if count > 99 { "99+".to_string() } else { count.to_string() };
    let w = (txt.len() as f32 * 6.0 + 8.0).max(16.0);
    let centre = egui::pos2(indicator.right() - 3.0, indicator.bottom() - 4.0);
    let rect = Rect::from_center_size(centre, egui::vec2(w, 16.0));
    ui.painter().rect_filled(rect, CornerRadius::same(8), Palette::ACTIVITY_BAR_BADGE_BG());
    ui.painter().text(centre, Align2::CENTER_CENTER, txt, FontId::proportional(10.5), Palette::ACTIVITY_BAR_BADGE_FG());
}
