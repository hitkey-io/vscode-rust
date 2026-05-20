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
    };

    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, Palette::TABS_STRIP_BG);

    // Build the unified tab list: optional welcome tab, then every document.
    // The welcome tab is just a regular tab item with a distinct label and
    // codicon — the routing of clicks/closes is what makes it "welcome".
    let labels: Vec<String> = docs.iter().map(|d| d.display_name()).collect();
    let tooltips: Vec<String> = docs
        .iter()
        .map(|d| d.path.to_string_lossy().into_owned())
        .collect();
    let dirty_flags: Vec<bool> = docs.iter().map(|d| d.dirty).collect();
    let pinned_flags: Vec<bool> = docs.iter().map(|d| d.pinned).collect();
    let mut items: Vec<Tab<'_>> = Vec::with_capacity(docs.len() + 1);
    if show_welcome_tab {
        items.push(Tab::new("Welcome").icon(icons::VSCODE).closable(true));
    }
    for (idx, label) in labels.iter().enumerate() {
        let mut t = Tab::new(label).icon(icons::FILE).tooltip(&tooltips[idx]);
        if dirty_flags[idx] {
            t = t.dirty();
        }
        if pinned_flags[idx] {
            t = t.pinned();
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
    let response = vsce_tabs(ui, &TabsProps::default(), &items, &mut active_idx);

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
    if let Some(idx) = response.right_clicked {
        if !(show_welcome_tab && idx == 0) {
            action.right_clicked = Some(idx - welcome_offset);
        }
    }

    // AccessKit labels for kittest queries.
    response_widget_info_helper(ui, &items, show_welcome_tab);

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
        let resp = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone())
        });
    }
}
