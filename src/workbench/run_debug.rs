//! The Run and Debug view (contrib/debug/browser/welcomeView.ts): with no
//! launch configuration it is the view's welcome content — "[Open a file]
//! which can be debugged or run." while the active editor isn't one a
//! debugger is interested in, the "Run and Debug" button, and "To customize
//! Run and Debug [create a launch.json file]." (with no folder: "[open a
//! folder] and create a launch.json file."). The title is "Run And Debug:
//! Run" (the container's title and its only view). Text and buttons follow
//! the view welcome layout of `viewPaneContainer` (20px inset, 26px
//! buttons), the same as the Source Control welcome.

use egui::{Align2, Color32, FontId, Sense, Stroke, Ui};

use crate::theme::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunDebugAction {
    OpenFile,
    OpenFolder,
    Start,
    CreateLaunchJson,
}

/// Files the built-in JavaScript debugger is interested in
/// (`CONTEXT_DEBUGGER_INTERESTED_IN_ACTIVE_EDITOR`: JavaScript,
/// TypeScript, their React variants, HTML).
pub fn debuggable(path: &std::path::Path) -> bool {
    matches!(crate::editor::buffer::detect_language(path), "js" | "ts" | "tsx" | "jsx" | "html")
}

/// A welcome paragraph: `[label](target)` runs are links. Returns the link
/// clicked.
pub fn paragraph(ui: &mut Ui, text: &str, id: egui::Id) -> Option<String> {
    let w = (ui.available_width() - 40.0).max(60.0);
    let normal = egui::TextFormat { font_id: FontId::proportional(13.0), color: Palette::FG(), line_height: Some(18.0), ..Default::default() };
    let link = egui::TextFormat { color: Palette::TEXT_LINK_FG(), ..normal.clone() };
    let mut job = egui::text::LayoutJob::default();
    let mut links = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find("](").map(|c| open + c) else { break };
        let Some(end) = rest[close..].find(')').map(|e| close + e) else { break };
        job.append(&rest[..open], 0.0, normal.clone());
        let start = job.text.chars().count();
        job.append(&rest[open + 1..close], 0.0, link.clone());
        links.push((start..job.text.chars().count(), rest[close + 2..end].to_string()));
        rest = &rest[end + 1..];
    }
    job.append(rest, 0.0, normal);
    job.wrap.max_width = w;
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        let (rect, resp) = ui.allocate_exact_size(galley.size(), Sense::click());
        if let Some(p) = resp.hover_pos() {
            let ci = galley.cursor_from_pos(p - rect.min).index;
            if let Some((_, target)) = links.iter().find(|(r, _)| r.contains(&ci)) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                if resp.clicked() {
                    clicked = Some(target.clone());
                }
            }
        }
        let plain: String = galley.job.text.clone();
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, plain.clone()));
        // Text sits 1.5px lower in its 18px line than egui centres it.
        ui.painter().galley(rect.min + egui::vec2(0.0, 1.5), galley.clone(), Color32::PLACEHOLDER);
    });
    for (i, (_, target)) in links.iter().enumerate() {
        // Each link is also reachable by name (tests, screen readers).
        let label: String = galley.job.text.chars().skip(links[i].0.start).take(links[i].0.len()).collect();
        let at = egui::Rect::from_min_size(ui.max_rect().min, egui::Vec2::ZERO);
        let resp = ui.interact(at, id.with(("link", i)), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, label.clone()));
        if resp.clicked() {
            clicked = Some(target.clone());
        }
    }
    clicked
}

/// A full-width welcome button (`button.background`, 26px, radius 4).
pub fn button(ui: &mut Ui, label: &str) -> bool {
    let w = (ui.available_width() - 40.0).max(60.0);
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.add_space(20.0);
        let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 26.0), Sense::click());
        let bg = if resp.hovered() { Palette::BUTTON_HOVER() } else { Palette::BUTTON_BG() };
        ui.painter().rect(r, 4.0, bg, Stroke::NONE, egui::StrokeKind::Inside);
        ui.painter().text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(13.0), Palette::BUTTON_FG());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        clicked = resp.clicked();
    });
    clicked
}

/// The view under its title. `folder_open`: a folder is open;
/// `active_debuggable`: the active editor's language has a debugger.
pub fn show(ui: &mut Ui, folder_open: bool, active_debuggable: bool) -> Option<RunDebugAction> {
    let id = ui.id().with("run-debug");
    let mut action = None;
    ui.add_space(13.0);
    if !active_debuggable {
        if let Some(t) = paragraph(ui, "[Open a file](open-file) which can be debugged or run.", id.with(0)) {
            action = Some(if t == "open-file" { RunDebugAction::OpenFile } else { RunDebugAction::OpenFolder });
        }
        ui.add_space(13.0);
    }
    if button(ui, "Run and Debug") {
        action = Some(RunDebugAction::Start);
    }
    ui.add_space(13.0);
    let customise = if folder_open {
        "To customize Run and Debug [create a launch.json file](create-launch)."
    } else {
        "To customize Run and Debug, [open a folder](open-folder) and create a launch.json file."
    };
    if let Some(t) = paragraph(ui, customise, id.with(1)) {
        action = Some(match t.as_str() {
            "create-launch" => RunDebugAction::CreateLaunchJson,
            _ => RunDebugAction::OpenFolder,
        });
    }
    action
}

/// The `launch.json` VS Code writes before any configuration is added
/// (debugConfigurationManager's initial content).
pub const LAUNCH_JSON: &str = "{\n\t// Use IntelliSense to learn about possible attributes.\n\t// Hover to view descriptions of existing attributes.\n\t// For more information, visit: https://go.microsoft.com/fwlink/?linkid=830387\n\t\"version\": \"0.2.0\",\n\t\"configurations\": []\n}\n";
