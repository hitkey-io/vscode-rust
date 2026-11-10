//! The Welcome page (`contrib/welcomeGettingStarted`, gettingStarted.ts /
//! gettingStarted.css, Modern UI font ramp). Grid `1fr 6fr 1fr 6fr 1fr`
//! (≤ 1200px) inside 12px / 24px padding; the header sits at the bottom of
//! the first 25% of the height; Start + Recent on the left, Walkthroughs on
//! the right, "Show welcome page on startup" at the bottom. Vertical
//! offsets measured against VS Code 1.145.2.

use egui::{Align2, CornerRadius, FontId, Pos2, Rect, Sense, Ui};
use std::path::PathBuf;

use crate::icons::{self, codicon_font};
use crate::theme::Palette;
use crate::ui_font;

/// A Start entry (`startEntries`, gettingStartedContent.ts).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartEntry {
    NewFile,
    Open,
    CloneRepository,
    ConnectTo,
}

impl StartEntry {
    fn label(self) -> &'static str {
        match self {
            StartEntry::NewFile => "New File...",
            StartEntry::Open => "Open...",
            StartEntry::CloneRepository => "Clone Git Repository...",
            StartEntry::ConnectTo => "Connect to...",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            StartEntry::NewFile => "new-file",
            StartEntry::Open => "folder-opened",
            StartEntry::CloneRepository => "source-control",
            StartEntry::ConnectTo => "remote",
        }
    }

    fn tooltip(self) -> &'static str {
        match self {
            StartEntry::NewFile => "Open a new untitled text file, notebook, or custom editor. (⌃⌥⌘N)",
            StartEntry::Open => "Open a file or folder to start working (⌘O)",
            StartEntry::CloneRepository => "Clone a remote repository to a local folder",
            StartEntry::ConnectTo => "Connect to remote development workspaces.",
        }
    }
}

/// What the page shows.
pub struct WelcomeModel<'a> {
    /// `config.git.enabled && !git.missing`.
    pub git_available: bool,
    /// Recently opened folders, most recent first.
    pub recent: &'a [PathBuf],
    /// `workbench.startupEditor` is `welcomePage`.
    pub show_on_startup: bool,
}

/// What the person did on the page.
#[derive(Default, Debug, PartialEq)]
pub struct WelcomeAction {
    pub start: Option<StartEntry>,
    pub open_recent: Option<PathBuf>,
    pub remove_recent: Option<PathBuf>,
    pub toggle_show_on_startup: bool,
}

const H1: f32 = 26.0;
const H2: f32 = 18.0;
const BODY: f32 = 13.0;
/// From the header row's bottom edge, measured: title and subtitle centres
/// above it; the column headings' centre below it.
const TITLE_DY: f32 = -62.0;
const SUBTITLE_DY: f32 = -31.0;
const HEADING_DY: f32 = 11.0;
const FIRST_ENTRY_DY: f32 = 43.0;
const ENTRY_PITCH: f32 = 31.0;
const RECENT_HEADING_GAP: f32 = 78.0;
const RECENT_FIRST: f32 = 28.0;
const RECENT_PITCH: f32 = 24.0;
const RECENT_LIMIT: usize = 5;
const CARD_TOP_DY: f32 = 35.0;
const CARD_GAP: f32 = 17.0;
const FEATURED_HEIGHT: f32 = 68.0;
const PLAIN_HEIGHT: f32 = 30.0;

pub fn show(ui: &mut Ui, model: &WelcomeModel<'_>) -> WelcomeAction {
    let mut action = WelcomeAction::default();
    let area = ui.available_rect_before_wrap();
    ui.allocate_rect(area, Sense::hover());
    let painter = ui.painter().clone();

    // `.gettingStartedContainer` padding 12px 24px; grid ≤ 1200px centred.
    let inner_left = area.left() + 24.0;
    let inner_w = (area.width() - 48.0).max(0.0);
    let grid_w = inner_w.min(1200.0);
    let grid_left = inner_left + (inner_w - grid_w) / 2.0;
    let fr = grid_w / 15.0;
    let left = grid_left + fr;
    let right = grid_left + 8.0 * fr;
    let col_w = 6.0 * fr;
    let header_bottom = area.top() + 12.0 + 0.25 * (area.height() - 24.0);

    let fg = Palette::FG();
    painter.text(
        Pos2::new(left, header_bottom + TITLE_DY),
        Align2::LEFT_CENTER,
        "Visual Studio Code",
        ui_font::heading(H1),
        fg,
    );
    painter.text(
        Pos2::new(left, header_bottom + SUBTITLE_DY),
        Align2::LEFT_CENTER,
        "Editing evolved",
        ui_font::semibold(H2),
        Palette::FG_DESCRIPTION(),
    );

    // Start
    painter.text(Pos2::new(left, header_bottom + HEADING_DY), Align2::LEFT_CENTER, "Start", ui_font::semibold(H2), fg);
    let mut entries = vec![StartEntry::NewFile, StartEntry::Open];
    if model.git_available {
        entries.push(StartEntry::CloneRepository);
    }
    entries.push(StartEntry::ConnectTo);
    let mut y = header_bottom + FIRST_ENTRY_DY;
    for entry in &entries {
        if link(ui, Pos2::new(left, y), Some(entry.icon()), entry.label(), entry.tooltip()).clicked() {
            action.start = Some(*entry);
        }
        y += ENTRY_PITCH;
    }

    // Recent
    let heading = y - ENTRY_PITCH + RECENT_HEADING_GAP;
    painter.text(Pos2::new(left, heading), Align2::LEFT_CENTER, "Recent", ui_font::semibold(H2), fg);
    let mut y = heading + RECENT_FIRST;
    if model.recent.is_empty() {
        // "You have no recent folders, open a folder to start."
        let font = FontId::proportional(BODY);
        let first = painter.text(Pos2::new(left, y), Align2::LEFT_CENTER, "You have no recent folders,", font.clone(), fg);
        let r = link(ui, Pos2::new(first.right() + 4.0, y), None, "open a folder", "Open Folder");
        painter.text(Pos2::new(r.rect.right() + 4.0, y), Align2::LEFT_CENTER, "to start.", font, fg);
        if r.clicked() {
            action.start = Some(StartEntry::Open);
        }
    }
    for folder in model.recent.iter().take(RECENT_LIMIT) {
        let name = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| folder.display().to_string());
        let parent = folder.parent().map(tildify).unwrap_or_default();
        let row = Rect::from_min_max(Pos2::new(left, y - RECENT_PITCH / 2.0), Pos2::new(left + col_w, y + RECENT_PITCH / 2.0));
        let row_hovered = ui.rect_contains_pointer(row);
        let name_resp = link(ui, Pos2::new(left + 2.0, y), None, &name, &folder.display().to_string());
        if name_resp.clicked() {
            action.open_recent = Some(folder.clone());
        }
        let path_left = name_resp.rect.right() + BODY + 2.0;
        let path_room = (row.right() - 24.0 - path_left).max(0.0);
        let path_text = elide(ui, &parent, path_room, FontId::proportional(BODY));
        painter.text(Pos2::new(path_left, y), Align2::LEFT_CENTER, path_text, FontId::proportional(BODY), fg);
        if row_hovered {
            // "Remove from Recently Opened" (`.recently-opened-delete-button`).
            let r = Rect::from_center_size(Pos2::new(row.right() - 12.0, y), egui::vec2(24.0, 24.0));
            let resp = ui.interact(r, ui.id().with(("recent-remove", folder)), Sense::click());
            if resp.hovered() {
                painter.rect_filled(r, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
            }
            painter.text(r.center(), Align2::CENTER_CENTER, icons::CLOSE.to_string(), codicon_font(16.0), fg);
            if resp.on_hover_text("Remove from Recently Opened").clicked() {
                action.remove_recent = Some(folder.clone());
            }
        }
        y += RECENT_PITCH;
    }

    // Walkthroughs
    painter.text(Pos2::new(right, header_bottom + HEADING_DY), Align2::LEFT_CENTER, "Walkthroughs", ui_font::semibold(H2), fg);
    let card_w = col_w - 16.0;
    let top = header_bottom + CARD_TOP_DY;
    walkthrough(
        ui,
        Rect::from_min_size(Pos2::new(right, top), egui::vec2(card_w, FEATURED_HEIGHT)),
        None,
        "Get started with VS Code",
        "Customize your editor, learn the basics, and start coding",
        Some(1.0 / 3.0),
    );
    walkthrough(
        ui,
        Rect::from_min_size(Pos2::new(right, top + FEATURED_HEIGHT + CARD_GAP), egui::vec2(card_w, PLAIN_HEIGHT)),
        Some("lightbulb"),
        "Learn the Fundamentals",
        "",
        None,
    );

    // Footer: "Show welcome page on startup", centred 25px above the bottom.
    let label = "Show welcome page on startup";
    let galley = painter.layout_no_wrap(label.into(), FontId::proportional(BODY), fg);
    let total = 18.0 + 8.0 + galley.size().x;
    let cy = area.bottom() - 25.0;
    let box_rect = Rect::from_min_size(Pos2::new(area.center().x - total / 2.0, cy - 9.0), egui::vec2(18.0, 18.0));
    let hit = Rect::from_min_max(box_rect.min, Pos2::new(box_rect.left() + total, box_rect.bottom()));
    let resp = ui.interact(hit, ui.id().with("welcome-on-startup"), Sense::click());
    painter.rect(
        box_rect,
        3.0,
        Palette::VSCE_CHECKBOX_BG(),
        egui::Stroke::new(1.0, Palette::VSCE_CHECKBOX_BORDER()),
        egui::StrokeKind::Inside,
    );
    if model.show_on_startup {
        painter.text(box_rect.center(), Align2::CENTER_CENTER, icons::codicon_by_name("check").unwrap_or('✓').to_string(), codicon_font(16.0), Palette::VSCE_CHECKBOX_FG());
    }
    painter.galley(Pos2::new(box_rect.right() + 8.0, cy - galley.size().y / 2.0), galley, fg);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, model.show_on_startup, label));
    if resp.clicked() {
        action.toggle_show_on_startup = true;
    }
    action
}

/// A `button.button-link`: optional 16px codicon, then the text, in
/// `textLink.foreground` (`textLink.activeForeground` on hover).
fn link(ui: &mut Ui, left_centre: Pos2, icon: Option<&str>, text: &str, tooltip: &str) -> egui::Response {
    let font = FontId::proportional(BODY);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, Palette::TEXT_LINK_FG());
    let text_x = if icon.is_some() { 30.0 } else { 0.0 };
    let rect = Rect::from_min_size(
        Pos2::new(left_centre.x, left_centre.y - 11.0),
        egui::vec2(text_x + galley.size().x, 22.0),
    );
    let resp = ui.interact(rect, ui.id().with(("welcome-link", text, left_centre.y as i32)), Sense::click());
    let colour = if resp.hovered() { Palette::TEXT_LINK_ACTIVE_FG() } else { Palette::TEXT_LINK_FG() };
    if let Some(name) = icon {
        if let Some(glyph) = icons::codicon_by_name(name) {
            ui.painter().text(Pos2::new(left_centre.x + 12.0, left_centre.y), Align2::CENTER_CENTER, glyph.to_string(), codicon_font(18.0), colour);
        }
    }
    let size = galley.size();
    ui.painter().galley(Pos2::new(left_centre.x + text_x, left_centre.y - size.y / 2.0), galley, colour);
    if resp.hovered() {
        // Links underline on hover.
        let y = left_centre.y + size.y / 2.0 - 1.0;
        ui.painter().hline(rect.left() + text_x..=rect.right(), y, egui::Stroke::new(1.0, colour));
    }
    let label = text.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, label.clone()));
    resp.on_hover_text(tooltip)
}

/// A walkthrough tile (`.getting-started-category`): radius 6,
/// `welcomePage.tileBackground` (hover `tileHoverBackground`); the featured
/// one has the star badge, a description and a 4px progress bar.
fn walkthrough(ui: &mut Ui, rect: Rect, icon: Option<&str>, title: &str, description: &str, progress: Option<f32>) {
    let resp = ui.interact(rect, ui.id().with(("walkthrough", title)), Sense::click());
    let painter = ui.painter();
    let bg = if resp.hovered() { Palette::WELCOME_TILE_HOVER_BG() } else { Palette::WELCOME_TILE_BG() };
    painter.rect_filled(rect, 6.0, bg);
    let fg = Palette::FG();
    match icon {
        None => {
            // Featured badge: 20×20, radius 6 top-left / bottom-right.
            let badge = Rect::from_min_size(rect.min, egui::vec2(20.0, 20.0));
            painter.rect_filled(badge, CornerRadius { nw: 6, se: 6, ne: 0, sw: 0 }, Palette::ACTIVITY_BAR_BADGE_BG());
            if let Some(star) = icons::codicon_by_name("star-full") {
                painter.text(badge.center(), Align2::CENTER_CENTER, star.to_string(), codicon_font(12.0), Palette::ACTIVITY_BAR_BADGE_FG());
            }
        }
        Some(name) => {
            if let Some(glyph) = icons::codicon_by_name(name) {
                painter.text(Pos2::new(rect.left() + 15.0, rect.center().y), Align2::CENTER_CENTER, glyph.to_string(), codicon_font(20.0), Palette::TEXT_LINK_FG());
            }
        }
    }
    let title_y = if description.is_empty() { rect.center().y } else { rect.top() + 15.0 };
    painter.text(Pos2::new(rect.left() + 34.0, title_y), Align2::LEFT_CENTER, title, ui_font::semibold(BODY), fg);
    if !description.is_empty() {
        let wrap = rect.width() - 32.0;
        let job = egui::text::LayoutJob::simple(description.to_string(), FontId::proportional(BODY), fg, wrap);
        let galley = ui.fonts_mut(|f| f.layout_job(job));
        painter.galley(Pos2::new(rect.left() + 34.0, rect.top() + 26.0), galley, fg);
    }
    if let Some(done) = progress {
        let bar = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 4.0), rect.max);
        painter.rect_filled(bar, CornerRadius { sw: 6, se: 6, nw: 0, ne: 0 }, Palette::WELCOME_PROGRESS_BG());
        let filled = Rect::from_min_size(bar.min, egui::vec2(bar.width() * done, bar.height()));
        painter.rect_filled(filled, CornerRadius { sw: 6, ..Default::default() }, Palette::WELCOME_PROGRESS_FG());
    }
    let label = title.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
}

/// `/Users/me/Projects` → `~/Projects` (labelService's tildify).
pub fn tildify(path: &std::path::Path) -> String {
    let s = path.display().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && s.starts_with(&home) => format!("~{}", &s[home.len()..]),
        _ => s,
    }
}

/// `text` cut with "..." to fit `width` (`text-overflow: ellipsis`).
fn elide(ui: &Ui, text: &str, width: f32, font: FontId) -> String {
    let fits = |t: &str| ui.painter().layout_no_wrap(t.to_string(), font.clone(), egui::Color32::WHITE).size().x <= width;
    if fits(text) {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut n = chars.len();
    while n > 0 {
        let candidate: String = chars[..n].iter().collect::<String>() + "...";
        if fits(&candidate) {
            return candidate;
        }
        n -= 1;
    }
    String::new()
}
