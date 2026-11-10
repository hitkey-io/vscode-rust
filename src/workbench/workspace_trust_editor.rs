//! The Workspace Trust editor (workspaceTrustEditor.ts /
//! workspaceTrustEditor.css), opened by the Restricted Mode status item,
//! the banner's Manage link or "Workspaces: Manage Workspace Trust" — in
//! the modal editor part, like Settings. Measured against 1.145.2:
//!
//! - header (12px + 16px padding): 32px shield in `button.background` 8px
//!   left of the 24px semibold title on a 32px line, then the description
//!   (20px lines, max 600 wide, centred) 18px lower;
//! - two boxes (`max 400 + 2 × 36` wide, 16px apart, centred, 24px around):
//!   the current state's box has a 2px `button.background` outline, the
//!   other a 1px `button.secondaryBackground` border, radius 6; title 16px
//!   semibold on a 24px line with 16px padding, subtitle (18px lines), a
//!   list of 24px + 4px items with a 16px check (`debugIcon.startForeground`)
//!   or x (`editorError.foreground`) 6px before 14px text, and the Trust /
//!   Don't Trust button (26px) with "⌘Enter" under it at the box bottom;
//! - "Trusted Folders & Workspaces" below a 1px `editorWidget.border` rule
//!   (8px margin, 24px 36px padding), the Host / Path table and Add Folder.

use std::path::PathBuf;

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::icons;
use crate::theme::Palette;

#[derive(Default)]
pub struct TrustEditorState {
    scroll: f32,
}

#[derive(Default, Debug)]
pub struct TrustEditorOutput {
    pub trust: bool,
    pub dont_trust: bool,
    pub configure_settings: bool,
    pub open_url: Option<String>,
    /// "N workspace settings" (`settings.filterUntrusted`).
    pub show_untrusted_settings: bool,
    pub show_unsupported_extensions: bool,
    pub add_folder: bool,
    pub remove: Option<PathBuf>,
}

/// What the editor shows.
pub struct TrustView<'a> {
    pub trusted: bool,
    /// The open folder (`None`: the empty window).
    pub folder: Option<&'a std::path::Path>,
    pub trusted_folders: &'a [PathBuf],
    pub restricted_settings: usize,
    pub unsupported_extensions: usize,
}

const BOX_CONTENT_W: f32 = 400.0;
const BOX_PAD_X: f32 = 36.0;
const BOX_GAP: f32 = 16.0;

fn galley(ui: &Ui, text: &str, font: FontId, colour: Color32, wrap: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(text.to_string(), egui::TextFormat { font_id: font, color: colour, line_height: Some(18.0), ..Default::default() });
    job.wrap.max_width = wrap;
    ui.fonts_mut(|f| f.layout_job(job))
}

/// Text with `[links]`: (text, link id) runs on one line, centred at `cx`.
fn linked_line(ui: &mut Ui, painter: &egui::Painter, runs: &[(&str, Option<&'static str>)], cx: f32, cy: f32, id: egui::Id) -> Option<&'static str> {
    let font = FontId::proportional(13.0);
    let galleys: Vec<_> = runs
        .iter()
        .map(|(t, link)| painter.layout_no_wrap(t.to_string(), font.clone(), if link.is_some() { Palette::TEXT_LINK_FG() } else { Palette::FG() }))
        .collect();
    let total: f32 = galleys.iter().map(|g| g.size().x).sum();
    let mut x = cx - total / 2.0;
    let mut clicked = None;
    for ((_, link), g) in runs.iter().zip(galleys) {
        let size = g.size();
        let r = Rect::from_min_size(Pos2::new(x, cy - size.y / 2.0), size);
        if let Some(link) = link {
            let resp = ui.interact(r, id.with(link), Sense::click());
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                painter.hline(r.x_range(), r.bottom() - 1.5, Stroke::new(1.0, Palette::TEXT_LINK_FG()));
            }
            let l = link.to_string();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, l.clone()));
            if resp.clicked() {
                clicked = Some(*link);
            }
        }
        painter.galley(r.min, g, Color32::PLACEHOLDER);
        x += size.x;
    }
    clicked
}

fn button(ui: &mut Ui, painter: &egui::Painter, centre: Pos2, label: &str, id: egui::Id) -> bool {
    let g = painter.layout_no_wrap(label.to_string(), FontId::proportional(13.0), Palette::BUTTON_FG());
    let r = Rect::from_center_size(centre, Vec2::new((g.size().x + 26.0).max(78.0), 26.0));
    let resp = ui.interact(r, id, Sense::click());
    painter.rect_filled(r, 4.0, if resp.hovered() { Palette::BUTTON_HOVER() } else { Palette::BUTTON_BG() });
    painter.galley(Pos2::new(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, Palette::BUTTON_FG());
    let l = label.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, l.clone()));
    resp.clicked()
}

pub fn show(ui: &mut Ui, rect: Rect, state: &mut TrustEditorState, view: &TrustView<'_>) -> TrustEditorOutput {
    let mut out = TrustEditorOutput::default();
    let id = ui.id().with("workspace-trust");
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, Palette::EDITOR_BG());
    if ui.rect_contains_pointer(rect) {
        state.scroll -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    // ⌘Enter: the button of the box that isn't the current state.
    if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter)) {
        if view.trusted {
            out.dont_trust = true;
        } else {
            out.trust = true;
        }
    }
    let w = rect.width().min(1600.0);
    let cx = rect.left() + w / 2.0;
    let mut y = rect.top() + 12.0 - state.scroll;
    let start = y;

    // Header.
    let kind = if view.folder.is_some() { "folder" } else { "window" };
    let title = if view.trusted { format!("You trust this {kind}") } else { "You are in Restricted Mode".to_string() };
    y += 16.0 + 10.0;
    let tg = painter.layout_no_wrap(title, crate::ui_font::heading(24.0), Palette::FG());
    let icon_w = 32.0 + 8.0;
    let tx = cx - (icon_w + tg.size().x) / 2.0;
    // `shieldIcon` (Codicon.shield) in both states.
    let glyph = icons::codicon_by_name("shield").unwrap_or('!');
    painter.text(Pos2::new(tx + 16.0, y + 16.0), Align2::CENTER_CENTER, glyph.to_string(), icons::codicon_font(32.0), Palette::BUTTON_BG());
    let tsz = tg.size();
    painter.galley(Pos2::new(tx + icon_w, y + 16.0 - tsz.y / 2.0), tg, Palette::FG());
    y += 32.0 + 10.0 + 8.0;
    let line1 = if view.trusted {
        "All features are enabled because trust has been granted to the workspace.".to_string()
    } else {
        "Code is in a restricted mode intended for safe code browsing.".to_string()
    };
    painter.text(Pos2::new(cx, y + 10.0), Align2::CENTER_CENTER, line1, FontId::proportional(13.0), Palette::FG());
    let runs = [("Configure your settings", Some("configure")), (" or ", None), ("learn more", Some("learn")), (".", None)];
    match linked_line(ui, &painter, &runs, cx, y + 30.0, id.with("header-links")) {
        Some("configure") => out.configure_settings = true,
        Some(_) => out.open_url = Some("https://aka.ms/vscode-workspace-trust".into()),
        None => {}
    }
    y += 40.0 + 8.0 + 16.0;

    // The two boxes.
    let folder_word = if view.folder.is_some() { "folder" } else { "window" };
    let trusted_title = if view.folder.is_some() { "In a Trusted Folder" } else { "In a Trusted Window" };
    let trusted_sub = format!("You trust the authors of the files in the current {folder_word}. All features are enabled:");
    let untrusted_sub = format!("You do not trust the authors of the files in the current {folder_word}. The following features are disabled:");
    let mut trusted_items: Vec<(String, Option<&'static str>)> = vec![("Tasks are allowed to run".into(), None), ("Debugging is enabled".into(), None)];
    if view.folder.is_some() {
        trusted_items.push(("All workspace settings are applied".into(), None));
    }
    trusted_items.push(("All enabled extensions are activated".into(), None));
    let mut untrusted_items: Vec<(String, Option<&'static str>)> = vec![("Tasks are not allowed to run".into(), None), ("Debugging is disabled".into(), None)];
    if view.folder.is_some() {
        untrusted_items.push((format!("{} workspace settings| are not applied", view.restricted_settings), Some("settings")));
    }
    untrusted_items.push((format!("{} extensions| are disabled or have limited functionality", view.unsupported_extensions), Some("extensions")));

    y += 24.0;
    let box_w = BOX_CONTENT_W + 2.0 * BOX_PAD_X + 2.0;
    let two_columns = w >= 2.0 * box_w + BOX_GAP + 48.0;
    let sub_h = |text: &str| galley(ui, text, FontId::proportional(13.0), Palette::FG(), BOX_CONTENT_W).rows.len() as f32 * 18.0;
    let list_h = |n: usize| 16.0 + n as f32 * 28.0 + 16.0;
    let buttons_h = 76.0;
    let box_h = |sub: &str, n: usize| 1.0 + 8.0 + 56.0 + sub_h(sub) + list_h(n) + buttons_h + 8.0 + 1.0;
    let h = box_h(&trusted_sub, trusted_items.len()).max(box_h(&untrusted_sub, untrusted_items.len()));
    let boxes: [(Rect, bool); 2] = if two_columns {
        let left = cx - box_w - BOX_GAP / 2.0;
        [
            (Rect::from_min_size(Pos2::new(left, y), Vec2::new(box_w, h)), true),
            (Rect::from_min_size(Pos2::new(left + box_w + BOX_GAP, y), Vec2::new(box_w, h)), false),
        ]
    } else {
        [
            (Rect::from_min_size(Pos2::new(cx - box_w / 2.0, y), Vec2::new(box_w, h)), true),
            (Rect::from_min_size(Pos2::new(cx - box_w / 2.0, y + h + BOX_GAP), Vec2::new(box_w, h)), false),
        ]
    };
    for (r, is_trusted_box) in boxes {
        let current = is_trusted_box == view.trusted;
        if current {
            painter.rect_stroke(r, 6.0, Stroke::new(2.0, Palette::BUTTON_BG()), egui::StrokeKind::Inside);
        } else {
            painter.rect_stroke(r, 6.0, Stroke::new(1.0, Palette::VSCE_BUTTON_SECONDARY_BG()), egui::StrokeKind::Inside);
        }
        let content_left = r.left() + 1.0 + BOX_PAD_X;
        let mut by = r.top() + 1.0 + 8.0 + 16.0;
        let title = if is_trusted_box { trusted_title } else { "In Restricted Mode" };
        painter.text(Pos2::new(r.center().x, by + 12.0), Align2::CENTER_CENTER, title, crate::ui_font::semibold(16.0), Palette::FG());
        by += 24.0 + 16.0;
        let sub = galley(ui, if is_trusted_box { &trusted_sub } else { &untrusted_sub }, FontId::proportional(13.0), Palette::FG(), BOX_CONTENT_W);
        let rows = sub.rows.len() as f32;
        painter.galley(Pos2::new(content_left, by), sub, Color32::PLACEHOLDER);
        by += rows * 18.0 + 16.0;
        let items = if is_trusted_box { &trusted_items } else { &untrusted_items };
        for (text, link) in items {
            let cy = by + 12.0;
            let (icon, colour) = if is_trusted_box { ("check", Palette::DEBUG_ICON_START_FG()) } else { ("close", Palette::EDITOR_ERROR_FG()) };
            painter.text(Pos2::new(content_left + 8.0, cy), Align2::CENTER_CENTER, icons::codicon_by_name(icon).unwrap_or('x').to_string(), icons::codicon_font(16.0), colour);
            let tx = content_left + 22.0;
            match text.split_once('|') {
                Some((linked, rest)) => {
                    let lg = painter.layout_no_wrap(linked.to_string(), FontId::proportional(14.0), Palette::TEXT_LINK_FG());
                    let lr = Rect::from_min_size(Pos2::new(tx, cy - lg.size().y / 2.0), lg.size());
                    let resp = ui.interact(lr, id.with(("item-link", *link)), Sense::click());
                    let label = linked.to_string();
                    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, label.clone()));
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        match link {
                            Some("settings") => out.show_untrusted_settings = true,
                            _ => out.show_unsupported_extensions = true,
                        }
                    }
                    painter.galley(lr.min, lg, Color32::PLACEHOLDER);
                    painter.text(Pos2::new(lr.right(), cy), Align2::LEFT_CENTER, rest, FontId::proportional(14.0), Palette::FG());
                }
                None => {
                    painter.text(Pos2::new(tx, cy), Align2::LEFT_CENTER, text, FontId::proportional(14.0), Palette::FG());
                }
            }
            by += 28.0;
        }
        // The other state's button at the box bottom.
        if !current && view.folder.is_some() {
            let label = if is_trusted_box { "Trust" } else { "Don't Trust" };
            let bc = Pos2::new(r.center().x, r.bottom() - 1.0 - 8.0 - 10.0 - 16.5 - 8.0 - 18.5);
            if button(ui, &painter, bc, label, id.with(label)) {
                if is_trusted_box {
                    out.trust = true;
                } else {
                    out.dont_trust = true;
                }
            }
            painter.text(Pos2::new(bc.x, bc.y + 13.0 + 13.5), Align2::CENTER_CENTER, "⌘Enter", crate::ui_font::italic(12.0), Palette::FG());
        }
    }
    y = boxes[1].0.bottom() + 24.0 + 8.0;

    // Trusted Folders & Workspaces.
    painter.hline(rect.left()..=rect.left() + w, y + 0.5, Stroke::new(1.0, Palette::EDITOR_WIDGET_BORDER()));
    y += 1.0 + 24.0;
    let left = rect.left() + 36.0;
    painter.text(Pos2::new(left, y + 9.0), Align2::LEFT_CENTER, "Trusted Folders & Workspaces", crate::ui_font::semibold(13.0), Palette::FG());
    y += 18.0;
    let description = if view.trusted_folders.is_empty() {
        "You haven't trusted any folders or workspace files yet."
    } else {
        "You trust the following folders, their subfolders, and workspace files."
    };
    painter.text(Pos2::new(left, y + 9.0), Align2::LEFT_CENTER, description, FontId::proportional(13.0), Palette::FG());
    y += 18.0;
    if !view.trusted_folders.is_empty() {
        y += 16.0;
        let table_w = w - 72.0;
        let host_w = (table_w - 75.0) / 9.0;
        painter.text(Pos2::new(left + 5.0, y + 11.0), Align2::LEFT_CENTER, "Host", crate::ui_font::bold(13.0), Palette::FG());
        painter.text(Pos2::new(left + host_w + 5.0, y + 11.0), Align2::LEFT_CENTER, "Path", crate::ui_font::bold(13.0), Palette::FG());
        y += 22.0;
        for (i, folder) in view.trusted_folders.iter().enumerate() {
            let row = Rect::from_min_size(Pos2::new(left, y), Vec2::new(table_w, 22.0));
            let hovered = ui.rect_contains_pointer(row);
            if hovered {
                painter.rect_filled(row, 0.0, Palette::LIST_HOVER_BG());
            }
            let parent = view.folder.is_some_and(|f| f.starts_with(folder));
            let font = if parent { crate::ui_font::bold(13.0) } else { FontId::proportional(13.0) };
            painter.text(Pos2::new(left + 5.0, row.center().y), Align2::LEFT_CENTER, "Local", font.clone(), Palette::FG());
            painter.text(Pos2::new(left + host_w + 5.0, row.center().y), Align2::LEFT_CENTER, folder.display().to_string(), font, Palette::FG());
            let label = format!("trusted:{}", folder.display());
            let resp = ui.interact(row, id.with(("row", i)), Sense::hover());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label.clone()));
            if hovered {
                let r = Rect::from_center_size(Pos2::new(row.right() - 14.0, row.center().y), Vec2::splat(20.0));
                let del = ui.interact(r, id.with(("delete", i)), Sense::click());
                if del.hovered() {
                    painter.rect_filled(r, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
                }
                painter.text(r.center(), Align2::CENTER_CENTER, icons::codicon_by_name("close").unwrap_or('x').to_string(), icons::codicon_font(16.0), Palette::FG());
                del.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Delete Path"));
                if del.on_hover_text("Delete Path").clicked() {
                    out.remove = Some(folder.clone());
                }
            }
            y += 22.0;
        }
    }
    y += 8.0;
    let g = painter.layout_no_wrap("Add Folder".into(), FontId::proportional(13.0), Palette::BUTTON_FG());
    let br = Rect::from_min_size(Pos2::new(left + 4.0, y + 8.0), Vec2::new(g.size().x + 26.0, 26.0));
    let resp = ui.interact(br.intersect(rect), id.with("add-folder"), Sense::click());
    painter.rect_filled(br, 4.0, if resp.hovered() { Palette::BUTTON_HOVER() } else { Palette::BUTTON_BG() });
    painter.galley(Pos2::new(br.center().x - g.size().x / 2.0, br.center().y - g.size().y / 2.0), g, Palette::BUTTON_FG());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Add Folder"));
    if resp.clicked() {
        out.add_folder = true;
    }
    y = br.bottom() + 8.0 + 24.0;

    let total = y - start + 12.0;
    state.scroll = state.scroll.clamp(0.0, (total - rect.height()).max(0.0));
    out
}
