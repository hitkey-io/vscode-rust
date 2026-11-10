//! The Extensions view (⇧⌘X): marketplace search, installed, popular and
//! recommended extensions.
//!
//! VS Code analogues:
//! - `src/vs/workbench/contrib/extensions/browser/extensionsViewlet.ts`
//!   (search box "Search Extensions in Marketplace", sections)
//! - `src/vs/workbench/contrib/extensions/browser/extensionsList.ts`
//! - `src/vs/workbench/contrib/extensions/browser/media/extension.css`
//!   (72px rows, 16px left padding, 42px icon, bold name, install count and
//!   rating on the right of the name, description, publisher with the
//!   verified badge, actions bottom-right)
//!
//! Tokens: `--vscode-input-*`, `--vscode-focusBorder`, `--vscode-badge-*`,
//! `--vscode-extensionButton-prominent*`, `--vscode-extensionIcon-starForeground`,
//! `--vscode-extensionIcon-verifiedForeground`, `--vscode-descriptionForeground`.

use egui::{Align2, CornerRadius, FontId, Sense, Stroke, StrokeKind, Ui};

use crate::extensions::marketplace::{format_count, GalleryExtension, Icon, List};
use crate::extensions::{Activation, Extension, ExtensionService};
use crate::icons::{codicon_by_name, codicon_font};
use crate::theme::Palette;

const SECTION_H: f32 = 22.0;
const ROW_H: f32 = 72.0;
const PAD_LEFT: f32 = 16.0;
const ICON: f32 = 42.0;
const ICON_GAP: f32 = 14.0;
const PAD_RIGHT: f32 = 11.0;
const SEARCH_H: f32 = 26.0;

/// What the user asked for in the view.
#[derive(Default)]
pub struct ExtensionsViewOutput {
    pub install: Option<GalleryExtension>,
    pub uninstall: Option<String>,
    pub refresh: bool,
    /// A row was clicked: open that extension's page.
    pub open: Option<OpenDetails>,
    /// It was double-clicked: keep the page (not a preview).
    pub open_pinned: bool,
    /// The ⚙ of an installed extension's row was clicked: open its Manage
    /// menu at this point (window coordinates).
    pub manage: Option<(String, egui::Pos2)>,
}

pub use super::extension_details::OpenTarget as OpenDetails;

/// Which sections are collapsed (kept across frames by egui memory).
#[derive(Clone, Default)]
struct Collapsed {
    installed: bool,
    popular: bool,
    recommended: bool,
}

pub fn show(ui: &mut Ui, service: &mut ExtensionService) -> ExtensionsViewOutput {
    let mut out = ExtensionsViewOutput::default();
    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

    // View title with the Refresh action.
    let title = super::sidebar::title_bar(ui, "EXTENSIONS");
    let refresh = egui::Rect::from_center_size(
        egui::pos2(title.right() - 44.0, title.center().y),
        egui::vec2(22.0, 22.0),
    );
    let r = ui.interact(refresh, ui.id().with("ext_refresh"), Sense::click());
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Refresh"));
    if r.hovered() {
        ui.painter().rect_filled(refresh, 5.0, Palette::VSCE_QUICK_INPUT_ACTION_HOVER_BG());
    }
    paint_codicon(ui, refresh.center(), "refresh", 16.0, Palette::FG());
    if r.on_hover_text("Refresh").clicked() {
        out.refresh = true;
    }
    paint_codicon(ui, egui::pos2(title.right() - 18.0, title.center().y), "ellipsis", 16.0, Palette::FG());

    search_box(ui, service);
    ui.add_space(6.0);

    let collapsed_id = ui.id().with("ext_sections");
    let mut collapsed: Collapsed = ui.data(|d| d.get_temp(collapsed_id)).unwrap_or_default();

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        let query = service.marketplace.query.trim().to_string();

        if !query.is_empty() {
            let list = service.marketplace.current_search();
            let state = service.marketplace.list(&list).cloned();
            match state {
                Some(s) if !s.loading => {
                    if let Some(e) = &s.error {
                        message(ui, &format!("Couldn't search the marketplace: {e}"));
                    } else if s.items.is_empty() {
                        message(ui, "No extensions found.");
                    }
                    for ext in &s.items {
                        gallery_row(ui, service, ext, &mut out);
                    }
                }
                _ if !service.marketplace.enabled => message(ui, "The marketplace is turned off."),
                _ => message(ui, "Searching…"),
            }
            return;
        }

        // INSTALLED
        let count = service.installed().count();
        if section(ui, "INSTALLED", Some(count), &mut collapsed.installed) {
            if count == 0 {
                message(ui, "No extensions installed.");
            }
            let installed: Vec<Extension> = service.installed().cloned().collect();
            for ext in &installed {
                installed_row(ui, service, ext, &mut out);
            }
        }

        // POPULAR
        if section(ui, "POPULAR", None, &mut collapsed.popular) {
            list_rows(ui, service, &List::Popular, &mut out);
        }

        // RECOMMENDED (workspace recommendations)
        if !service.marketplace.recommendations.is_empty() {
            let n = service.marketplace.recommendations.len();
            if section(ui, "RECOMMENDED", Some(n), &mut collapsed.recommended) {
                list_rows(ui, service, &List::Recommended, &mut out);
            }
        }
    });

    ui.data_mut(|d| d.insert_temp(collapsed_id, collapsed));
    out
}

fn list_rows(ui: &mut Ui, service: &mut ExtensionService, list: &List, out: &mut ExtensionsViewOutput) {
    match service.marketplace.list(list).cloned() {
        Some(s) if !s.loading => {
            if let Some(e) = &s.error {
                message(ui, &format!("Couldn't reach the marketplace: {e}"));
            }
            for ext in &s.items {
                gallery_row(ui, service, ext, out);
            }
        }
        _ if !service.marketplace.enabled => message(ui, "The marketplace is turned off."),
        _ => message(ui, "Loading…"),
    }
}

/// The "Search Extensions in Marketplace" box.
fn search_box(ui: &mut Ui, service: &mut ExtensionService) {
    let (row, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), SEARCH_H + 8.0), Sense::hover());
    let box_rect = egui::Rect::from_min_max(
        egui::pos2(row.left() + 20.0, row.top() + 8.0),
        egui::pos2(row.right() - 12.0, row.bottom()),
    );
    let slot = ui.painter().add(egui::Shape::Noop);
    let text_rect = egui::Rect::from_min_max(
        egui::pos2(box_rect.left() + 7.0, box_rect.top() + 1.0),
        egui::pos2(box_rect.right() - 48.0, box_rect.bottom() - 1.0),
    );
    let mut query = service.marketplace.query.clone();
    let mut focused = false;
    {
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(text_rect).layout(*ui.layout()));
        ui.visuals_mut().weak_text_color = Some(Palette::VSCE_QUICK_INPUT_PLACEHOLDER_FG());
        ui.visuals_mut().text_cursor.stroke.color = Palette::VSCE_QUICK_INPUT_BOX_FG();
        let resp = ui.add_sized(
            text_rect.size(),
            egui::TextEdit::singleline(&mut query)
                .id_salt("extensions_search")
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .hint_text("Search Extensions in Marketplace")
                .font(FontId::proportional(13.0))
                .text_color(Palette::VSCE_QUICK_INPUT_BOX_FG())
                .vertical_align(egui::Align::Center),
        );
        focused = resp.has_focus();
    }
    service.marketplace.set_query(query);

    ui.painter().set(
        slot,
        egui::epaint::RectShape::new(
            box_rect,
            CornerRadius::same(6),
            Palette::VSCE_QUICK_INPUT_BOX_BG(),
            Stroke::new(
                1.0,
                if focused { Palette::VSCE_QUICK_INPUT_FOCUS_BORDER() } else { Palette::INPUT_BORDER() },
            ),
            StrokeKind::Inside,
        ),
    );

    // Clear and filter actions inside the box.
    let clear = egui::Rect::from_center_size(
        egui::pos2(box_rect.right() - 36.0, box_rect.center().y),
        egui::vec2(20.0, 20.0),
    );
    let c = ui.interact(clear, ui.id().with("ext_clear"), Sense::click());
    c.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Clear Extensions Search Results"));
    if c.hovered() {
        ui.painter().rect_filled(clear, 4.0, Palette::VSCE_QUICK_INPUT_ACTION_HOVER_BG());
    }
    paint_codicon(ui, clear.center(), "clear-all", 16.0, Palette::FG());
    if c.on_hover_text("Clear Extensions Search Results").clicked() {
        service.marketplace.set_query(String::new());
    }
    paint_codicon(
        ui,
        egui::pos2(box_rect.right() - 13.0, box_rect.center().y),
        "filter",
        16.0,
        Palette::FG(),
    );
}

/// A collapsible section header; returns whether it's expanded.
fn section(ui: &mut Ui, title: &str, count: Option<usize>, collapsed: &mut bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), SECTION_H), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, title));
    if resp.clicked() {
        *collapsed = !*collapsed;
    }
    let p = ui.painter();
    if rect.top() > ui.min_rect().top() + 1.0 {
        p.hline(rect.x_range(), rect.top(), Stroke::new(1.0, Palette::BORDER()));
    }
    paint_codicon(
        ui,
        egui::pos2(rect.left() + 12.0, rect.center().y),
        if *collapsed { "chevron-right" } else { "chevron-down" },
        16.0,
        Palette::SIDEBAR_SECTION_HEADER_FG(),
    );
    ui.painter().text(
        egui::pos2(rect.left() + 22.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        crate::ui_font::bold(11.0),
        Palette::SIDEBAR_SECTION_HEADER_FG(),
    );
    if let Some(n) = count {
        let text = n.to_string();
        let w = ui
            .painter()
            .layout_no_wrap(text.clone(), FontId::proportional(11.0), Palette::FG())
            .size()
            .x
            + 10.0;
        let badge = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 12.0 - w.max(18.0) / 2.0, rect.center().y),
            egui::vec2(w.max(18.0), 18.0),
        );
        ui.painter().rect_filled(badge, 9.0, Palette::VSCE_BADGE_2026_BG());
        ui.painter().text(badge.center(), Align2::CENTER_CENTER, text, FontId::proportional(11.0), egui::Color32::WHITE);
    }
    !*collapsed
}

fn message(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left() + 20.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(13.0),
        Palette::FG_DESCRIPTION(),
    );
    ui.add_space(6.0);
}

/// Shared row layout: icon, name, metadata line, description, publisher.
struct RowText<'a> {
    name: &'a str,
    description: &'a str,
    publisher: &'a str,
    verified: bool,
    /// Right of the name: download count and rating.
    downloads: Option<u64>,
    rating: Option<f32>,
    /// Right of the name instead of the stats (installed rows).
    version: Option<&'a str>,
    /// Right of the name for a running extension: its activation time.
    activation_ms: Option<u64>,
    /// A disabled extension: publisher in `disabledForeground`.
    disabled: bool,
}

impl RowText<'_> {
    /// Install count and rating show only for extensions not installed
    /// (`InstallCountWidget` / `RatingsWidget` in small mode).
    fn for_state(self, installed: bool) -> Self {
        if installed { Self { downloads: None, rating: None, ..self } } else { self }
    }
}

fn row_frame(ui: &mut Ui, a11y: String) -> (egui::Rect, egui::Response) {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, a11y.clone()));
    if resp.hovered() {
        ui.painter().rect_filled(rect, 0.0, Palette::LIST_HOVER_BG());
    }
    (rect, resp)
}

fn paint_row(ui: &mut Ui, rect: egui::Rect, icon: Option<&egui::TextureHandle>, t: &RowText) {
    let p = ui.painter_at(rect);
    let icon_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + PAD_LEFT, rect.center().y - ICON / 2.0),
        egui::vec2(ICON, ICON),
    );
    match icon {
        Some(tex) => {
            p.image(tex.id(), icon_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        None => {
            p.text(
                icon_rect.center(),
                Align2::CENTER_CENTER,
                crate::icons::EXTENSIONS.to_string(),
                codicon_font(34.0),
                Palette::FG_DESCRIPTION(),
            );
        }
    }

    let x = icon_rect.right() + ICON_GAP;
    let right = rect.right() - PAD_RIGHT;
    let line1 = rect.top() + 14.0;

    // Right of the name: stats or version, laid out right to left.
    let mut stats_left = right;
    let small = FontId::proportional(11.0);
    if let Some(ms) = t.activation_ms {
        // `ExtensionRuntimeStatusWidget`: history icon + "23ms".
        let g = p.layout_no_wrap(format!("{ms}ms"), small.clone(), Palette::FG());
        stats_left -= g.size().x;
        p.galley(egui::pos2(stats_left, line1 - g.size().y / 2.0), g, Palette::FG());
        stats_left -= 16.0;
        paint_codicon(ui, egui::pos2(stats_left + 7.0, line1), "history", 14.0, Palette::FG());
    } else if let Some(v) = t.version {
        let g = p.layout_no_wrap(format!("v{v}"), small.clone(), Palette::FG_DESCRIPTION());
        stats_left -= g.size().x;
        p.galley(egui::pos2(stats_left, line1 - g.size().y / 2.0), g, Palette::FG_DESCRIPTION());
    } else {
        if let Some(r) = t.rating.filter(|r| *r > 0.0) {
            let text = format!("{}", (r * 10.0).round() / 10.0);
            let g = p.layout_no_wrap(text, small.clone(), Palette::FG());
            stats_left -= g.size().x;
            p.galley(egui::pos2(stats_left, line1 - g.size().y / 2.0), g, Palette::FG());
            stats_left -= 16.0;
            paint_codicon(ui, egui::pos2(stats_left + 7.0, line1), "star-full", 14.0, Palette::VSCE_EXTENSION_STAR_FG());
            stats_left -= 6.0;
        }
        if let Some(d) = t.downloads {
            let g = p.layout_no_wrap(format_count(d), small.clone(), Palette::FG());
            stats_left -= g.size().x;
            p.galley(egui::pos2(stats_left, line1 - g.size().y / 2.0), g, Palette::FG());
            stats_left -= 18.0;
            paint_codicon(ui, egui::pos2(stats_left + 8.0, line1), "cloud-download", 14.0, Palette::FG());
        }
    }

    // Name (bold), clipped before the stats.
    let name_clip = egui::Rect::from_min_max(egui::pos2(x, rect.top()), egui::pos2(stats_left - 6.0, rect.bottom()));
    let name = p.layout_no_wrap(t.name.to_string(), crate::ui_font::bold(13.0), Palette::FG());
    let name_h = name.size().y;
    ui.painter_at(name_clip).galley(egui::pos2(x, line1 - name_h / 2.0), name, Palette::FG());

    // Description, one line.
    let desc_rect = egui::Rect::from_min_max(egui::pos2(x, rect.top() + 26.0), egui::pos2(right, rect.top() + 46.0));
    let desc = p.layout_no_wrap(t.description.to_string(), FontId::proportional(13.0), Palette::FG());
    ui.painter_at(desc_rect).galley(desc_rect.min, desc, Palette::FG());

    // Publisher, with the verified badge.
    let line3 = rect.bottom() - 15.0;
    let mut px = x;
    if t.verified {
        paint_codicon(ui, egui::pos2(px + 7.0, line3), "verified-filled", 14.0, Palette::VSCE_EXTENSION_VERIFIED_FG());
        px += 17.0;
    }
    ui.painter_at(rect).text(
        egui::pos2(px, line3),
        Align2::LEFT_CENTER,
        t.publisher,
        crate::ui_font::bold(12.0),
        if t.disabled { Palette::VSCE_DISABLED_FG() } else { Palette::FG_DESCRIPTION() },
    );
}

/// The ⚙ Manage action at a row's bottom-right (`ManageExtensionAction`,
/// 16px codicon with 1px 2px padding). Returns where its menu opens when
/// clicked: under the icon.
fn manage_gear(ui: &mut Ui, rect: egui::Rect, id: egui::Id) -> Option<egui::Pos2> {
    let r = egui::Rect::from_min_size(egui::pos2(rect.right() - PAD_RIGHT - 20.0, rect.bottom() - 15.0 - 9.0), egui::vec2(20.0, 18.0));
    let resp = ui.interact(r, id, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Manage"));
    if resp.hovered() {
        ui.painter().rect_filled(r, CornerRadius::same(5), Palette::VSCE_TOOLBAR_HOVER_BG());
    }
    paint_codicon(ui, r.center(), "gear", 16.0, Palette::FG());
    let clicked = resp.clicked();
    resp.on_hover_text("Manage");
    clicked.then(|| egui::pos2(r.left(), r.bottom() + 2.0))
}

/// `ExtensionStatusAction` in a row: an error or warning icon left of ⚙.
fn status_icon(ui: &mut Ui, rect: egui::Rect, state: &Activation) {
    let (name, color, tip) = match state {
        Activation::Failed(e) => ("error", Palette::VSCE_EDITOR_ERROR_FG(), format!("Activating the extension failed: {e}")),
        Activation::NeedsNode => ("warning", Palette::VSCE_EDITOR_WARNING_FG(), "This extension needs Node.js to run, and it wasn't found.".to_string()),
        _ => return,
    };
    let r = egui::Rect::from_min_size(egui::pos2(rect.right() - PAD_RIGHT - 20.0 - 6.0 - 20.0, rect.bottom() - 15.0 - 9.0), egui::vec2(20.0, 18.0));
    paint_codicon(ui, r.center(), name, 16.0, color);
    ui.interact(r, ui.id().with(("ext_status", rect.min.y as i64)), Sense::hover()).on_hover_text(tip);
}

/// A prominent action button at the row's bottom-right ("Install").
fn row_button(ui: &mut Ui, rect: egui::Rect, label: &str, id: egui::Id, enabled: bool, prominent: bool) -> bool {
    let font = FontId::proportional(12.0);
    let w = ui.painter().layout_no_wrap(label.to_string(), font.clone(), egui::Color32::WHITE).size().x + 14.0;
    let r = egui::Rect::from_min_size(
        egui::pos2(rect.right() - PAD_RIGHT - w, rect.bottom() - 15.0 - 9.0),
        egui::vec2(w, 18.0),
    );
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let resp = ui.interact(r, id, sense);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    let fill = match (prominent, resp.hovered() && enabled) {
        (true, false) => Palette::VSCE_EXTENSION_BUTTON_PROMINENT_BG(),
        (true, true) => Palette::VSCE_EXTENSION_BUTTON_PROMINENT_HOVER_BG(),
        (false, true) => Palette::VSCE_BUTTON_2026_SECONDARY_HOVER_BG(),
        (false, false) => egui::Color32::TRANSPARENT,
    };
    ui.painter().rect(
        r,
        CornerRadius::same(2),
        fill,
        Stroke::new(1.0, if prominent { egui::Color32::TRANSPARENT } else { Palette::VSCE_BUTTON_2026_BORDER() }),
        StrokeKind::Inside,
    );
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        label,
        font,
        if prominent { egui::Color32::WHITE } else { Palette::FG() },
    );
    enabled && resp.clicked()
}

fn gallery_row(ui: &mut Ui, service: &mut ExtensionService, ext: &GalleryExtension, out: &mut ExtensionsViewOutput) {
    let id = ext.id();
    let (rect, resp) = row_frame(ui, format!("{}, {}", ext.display_name, ext.namespace));
    if resp.clicked() || resp.double_clicked() {
        out.open_pinned = resp.double_clicked();
        out.open = Some(if service.is_installed(&id) {
            OpenDetails::Installed(service.installed().find(|e| e.id.eq_ignore_ascii_case(&id)).map(|e| e.id.clone()).unwrap_or(id.clone()))
        } else {
            OpenDetails::Gallery(ext.clone())
        });
    }
    let tooltip = format!("{id}\n{}", ext.description);
    resp.on_hover_text(tooltip);
    if !ui.is_rect_visible(rect) {
        return;
    }
    let icon = match &ext.icon_url {
        Some(url) => {
            service.marketplace.request_icon(url);
            match service.marketplace.icons.get(url) {
                Some(Icon::Ready(t)) => Some(t.clone()),
                _ => None,
            }
        }
        None => None,
    };
    paint_row(
        ui,
        rect,
        icon.as_ref(),
        &RowText {
            name: &ext.display_name,
            description: &ext.description,
            publisher: &ext.namespace,
            verified: ext.verified,
            downloads: Some(ext.downloads),
            rating: ext.rating,
            version: None,
            activation_ms: None,
            disabled: false,
        }
        .for_state(service.is_installed(&id)),
    );
    let installing = service.marketplace.installing.contains(&id);
    let installed = service.installed().find(|e| e.id.eq_ignore_ascii_case(&id)).map(|e| e.id.clone());
    let button_id = ui.id().with(("ext_install", &id));
    if let Some(installed) = installed {
        // Installed: the Manage gear, as in the INSTALLED list.
        if let Some(pos) = manage_gear(ui, rect, ui.id().with(("ext_manage_g", &id))) {
            out.manage = Some((installed, pos));
        }
    } else if installing {
        row_button(ui, rect, "Installing", button_id, false, true);
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    } else if row_button(ui, rect, "Install", button_id, true, true) {
        out.install = Some(ext.clone());
    }
}

fn installed_row(ui: &mut Ui, service: &mut ExtensionService, ext: &Extension, out: &mut ExtensionsViewOutput) {
    let state = service.activation.get(&ext.id).cloned().unwrap_or(Activation::Inactive);
    let disabled = service.enablement(&ext.id) != crate::extensions::Enablement::Enabled;
    let status = match &state {
        _ if disabled => "Disabled",
        Activation::Inactive if ext.main.is_none() => "",
        Activation::Inactive => "Not activated",
        Activation::Active => "Activated",
        Activation::Failed(_) => "Activation failed",
        Activation::NeedsNode => "Needs Node.js",
    };
    let (rect, resp) = row_frame(ui, format!("{}, {}, {}", ext.display_name, ext.publisher, status));
    if resp.clicked() || resp.double_clicked() {
        out.open_pinned = resp.double_clicked();
        out.open = Some(OpenDetails::Installed(ext.id.clone()));
    }
    let tooltip = match &state {
        Activation::Failed(e) => format!("{}\n{e}", ext.id),
        _ => format!("{} v{}\n{}", ext.id, ext.version, ext.path.display()),
    };
    resp.on_hover_text(tooltip);
    // Installed extensions show their own icon from disk.
    let icon_url = ext
        .manifest
        .get("icon")
        .and_then(|v| v.as_str())
        .map(|rel| format!("file://{}", ext.path.join(rel).display()));
    let icon = icon_url.as_deref().and_then(|url| local_icon(ui.ctx(), service, url));
    // The catalogue knows whether the publisher is verified.
    let verified = service
        .marketplace
        .lists
        .values()
        .flat_map(|l| l.items.iter())
        .find(|g| g.id().eq_ignore_ascii_case(&ext.id))
        .is_some_and(|g| g.verified);
    let activation_ms = match state {
        Activation::Active if !disabled => service.activation_times.get(&ext.id).map(|t| t.load_ms + t.activate_ms),
        _ => None,
    };
    paint_row(
        ui,
        rect,
        icon.as_ref(),
        &RowText {
            name: &ext.display_name,
            description: &ext.description,
            publisher: &ext.publisher,
            verified,
            downloads: None,
            rating: None,
            version: None,
            activation_ms,
            disabled,
        },
    );
    if !disabled {
        status_icon(ui, rect, &state);
    }
    if let Some(pos) = manage_gear(ui, rect, ui.id().with(("ext_manage", &ext.id))) {
        out.manage = Some((ext.id.clone(), pos));
    }
}

/// An installed extension's icon, read from its folder once.
fn local_icon(ctx: &egui::Context, service: &mut ExtensionService, url: &str) -> Option<egui::TextureHandle> {
    let icons = &mut service.marketplace.icons;
    if !icons.contains_key(url) {
        let path = url.trim_start_matches("file://");
        let icon = std::fs::read(path)
            .ok()
            .and_then(|b| image::load_from_memory(&b).ok())
            .map(|img| {
                let img = img.resize(128, 128, image::imageops::FilterType::Triangle).to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                Icon::Ready(ctx.load_texture(url, egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), egui::TextureOptions::LINEAR))
            })
            .unwrap_or(Icon::Missing);
        icons.insert(url.to_string(), icon);
    }
    match icons.get(url) {
        Some(Icon::Ready(t)) => Some(t.clone()),
        _ => None,
    }
}

fn paint_codicon(ui: &Ui, center: egui::Pos2, name: &str, size: f32, color: egui::Color32) {
    if let Some(glyph) = codicon_by_name(name) {
        ui.painter().text(center, Align2::CENTER_CENTER, glyph.to_string(), codicon_font(size), color);
    }
}
