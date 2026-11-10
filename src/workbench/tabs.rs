use egui::Ui;

use crate::editor::Document;
use crate::icons;
use crate::theme::Palette;
use crate::vscode_widgets::composite::{tabs as vsce_tabs, Tab, TabsProps};

pub struct TabsAction {
    pub activate: Option<usize>,
    pub close: Option<usize>,
    pub close_welcome: bool,
    /// A document tab was right-clicked — index into `docs`. The caller
    /// opens the tab context menu at the current pointer position.
    pub right_clicked: Option<usize>,
    /// The pin affordance was clicked on a pinned tab — toggle its pin.
    pub toggle_pin: Option<usize>,
    /// A tab was double-clicked: keep its preview editor open.
    pub keep: Option<usize>,
    /// "More Actions..." (`…`) at the strip's right end: where to open its
    /// menu.
    pub more_actions: Option<egui::Pos2>,
}

pub fn show(
    ui: &mut Ui,
    docs: &[Document],
    active: Option<usize>,
    show_welcome_tab: bool,
) -> TabsAction {
    let mut action = TabsAction {
        activate: None,
        close: None,
        close_welcome: false,
        right_clicked: None,
        toggle_pin: None,
        keep: None,
        more_actions: None,
    };

    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, Palette::CONNECTED_TABS_BG());

    // Build the unified tab list: optional welcome tab, then every document.
    // The welcome tab is just a regular tab item with a distinct label and
    // codicon — the routing of clicks/closes is what makes it "welcome".
    let labels: Vec<String> = docs
        .iter()
        .map(|d| {
            if let Some(t) = &d.diff_title {
                t.clone()
            } else if d.diff_base.is_some() {
                format!("{} (Working Tree)", d.display_name())
            } else {
                d.display_name()
            }
        })
        .collect();
    let tooltips: Vec<String> = docs
        .iter()
        .map(|d| d.path.to_string_lossy().into_owned())
        .collect();
    let dirty_flags: Vec<bool> = docs.iter().map(|d| d.dirty).collect();
    let pinned_flags: Vec<bool> = docs.iter().map(|d| d.pinned).collect();
    let preview_flags: Vec<bool> = docs.iter().map(|d| d.preview).collect();
    let mut items: Vec<Tab<'_>> = Vec::with_capacity(docs.len() + 1);
    if show_welcome_tab {
        // The Welcome page opens as a preview editor (italic title).
        let mut welcome = Tab::new("Welcome").icon(icons::VSCODE).closable(true).preview();
        if let Some(logo) = crate::assets::svg_texture_in_colour(ui.ctx(), "vscode-logo", crate::assets::VSCODE_LOGO, 32) {
            welcome = welcome.image(logo.id());
        }
        items.push(welcome);
    }
    for (idx, label) in labels.iter().enumerate() {
        // File-type icon from the Seti theme (codicon file glyph as fallback).
        let extension_page = docs[idx].webview.is_some_and(|h| h >= crate::workbench::webviews::INTERNAL_HANDLE_BASE);
        let welcome_logo = docs[idx]
            .welcome
            .then(|| crate::assets::svg_texture_in_colour(ui.ctx(), "vscode-logo", crate::assets::VSCODE_LOGO, 32))
            .flatten();
        let mut t = match crate::file_icons::icon_for(&docs[idx].path) {
            // The Welcome page: the product logo.
            _ if welcome_logo.is_some() => Tab::new(label).image(welcome_logo.as_ref().map(|t| t.id()).unwrap()),
            // The extension editor's tab shows the Extensions icon.
            _ if extension_page => Tab::new(label).icon(icons::codicon_by_name("extensions").unwrap_or(icons::FILE)),
            Some((glyph, color)) => Tab::new(label).seti_icon(glyph, color),
            None => Tab::new(label).icon(icons::FILE),
        }
        .tooltip(&tooltips[idx]);
        if dirty_flags[idx] {
            t = t.dirty();
        }
        if pinned_flags[idx] {
            t = t.pinned();
        }
        if preview_flags[idx] {
            t = t.preview();
        }
        items.push(t);
    }

    // Map the workbench's `Option<usize>` active doc into the flat tab index.
    let welcome_offset = if show_welcome_tab { 1 } else { 0 };
    let mut active_idx = match (show_welcome_tab, active) {
        (_, Some(i)) => i + welcome_offset,
        (true, None) => 0,
        (false, None) => 0,
    };
    let before = active_idx;

    // The composite widget owns its own horizontal scroll area + scroll-into
    // view, so we hand it the full strip directly.
    let response = vsce_tabs(ui, &TabsProps::connected(), &items, &mut active_idx);

    // Translate the composite-widget response into the workbench's action
    // shape. The welcome tab occupies slot 0 when present; everything else
    // is offset by `welcome_offset`.
    if let Some(clicked) = response.clicked {
        if active_idx != before {
            if show_welcome_tab && clicked == 0 {
                // Clicking the welcome tab "deselects" any document — the
                // workbench treats no-active-doc as the welcome view.
                action.activate = None;
            } else {
                action.activate = Some(clicked - welcome_offset);
            }
        }
    }
    if let Some(idx) = response.close_requested {
        if show_welcome_tab && idx == 0 {
            action.close_welcome = true;
        } else {
            let doc_idx = idx - welcome_offset;
            // A close request on a pinned tab means "unpin", not "close".
            if pinned_flags.get(doc_idx).copied().unwrap_or(false) {
                action.toggle_pin = Some(doc_idx);
            } else {
                action.close = Some(doc_idx);
            }
        }
    }
    if let Some(idx) = response.double_clicked {
        if !(show_welcome_tab && idx == 0) {
            action.keep = Some(idx - welcome_offset);
        }
    }
    if let Some(idx) = response.right_clicked {
        if !(show_welcome_tab && idx == 0) {
            action.right_clicked = Some(idx - welcome_offset);
        }
    }

    // AccessKit labels for kittest queries.
    response_widget_info_helper(ui, &items, show_welcome_tab);

    // Editor title actions (`editor/title`): Split Editor Right and More
    // Actions..., 27px apart, the last centred 15px from the right edge.
    let cy = rect.top() + rect.height() / 2.0 - 0.5;
    for (i, (name, tip)) in [("split-horizontal", "Split Editor Right (⌘\\)"), ("ellipsis", "More Actions...")].into_iter().enumerate() {
        let centre = egui::pos2(rect.right() - 42.0 + 27.0 * i as f32, cy);
        let r = egui::Rect::from_center_size(centre, egui::vec2(22.0, 22.0));
        let resp = ui.interact(r, ui.id().with(("editor-title-action", i)), egui::Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
        }
        let glyph = icons::codicon_by_name(name).unwrap_or('?');
        ui.painter().text(centre, egui::Align2::CENTER_CENTER, glyph.to_string(), icons::codicon_font(16.0), Palette::VSCE_ICON_2026_FG());
        let label = tip.to_string();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
        let resp = resp.on_hover_text(tip);
        if i == 1 && resp.clicked() {
            action.more_actions = Some(r.left_bottom());
        }
    }

    action
}

/// Stamp AccessKit labels matching the legacy `tab:<name>` format so that
/// existing kittest queries continue to find specific tabs.
fn response_widget_info_helper(ui: &mut Ui, items: &[Tab<'_>], welcome_shown: bool) {
    // The composite::tabs widget doesn't expose per-tab AccessKit nodes; we
    // recreate them by painting transparent rectangles at the same locations.
    // Since we don't have the exact tab rects from the composite widget, we
    // rely on the catch-all "tab:<name>" label via a hidden Label widget
    // sequence — kittest searches the AccessKit tree by label text.
    for (idx, tab) in items.iter().enumerate() {
        let label = if welcome_shown && idx == 0 {
            "Welcome tab".to_string()
        } else {
            format!("tab:{}", tab.label)
        };
        // Allocate a 0×0 rect with the label so kittest can find it. This
        // doesn't paint anything visible but registers an AccessKit node.
        // An interaction without layout: allocating would push the row wider.
        let at = egui::Rect::from_min_size(ui.max_rect().min, egui::Vec2::ZERO);
        let resp = ui.interact(at, ui.id().with(("tab-a11y", idx)), egui::Sense::hover());
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone())
        });
    }
}
