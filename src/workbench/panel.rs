//! The Panel part (browser/parts/panel/panelPart.ts, the modern UI's
//! floating card and pill tabs) with its default views: Problems, Output,
//! Debug Console, Terminal and Ports. Measured against VS Code 1.145.2 at
//! 1300 × 820:
//!
//! - a card under the editor's, as wide, 4px apart (default height a third
//!   of the window, `LayoutStateKeys.PANEL_SIZE`), `panel.background`;
//! - pill tabs 24px tall 5px in (8px padding, 4.5px apart, 13px text,
//!   `modernTab.activeBackground` behind the active one,
//!   `panelTitle.inactiveForeground` for the others);
//! - actions from the right: Hide Panel 16px in, Maximize 42, a separator
//!   at 57.5, then the view's own 26px apart from 77 (Problems: View as
//!   Table, Collapse All and a 200 × 24 filter box ending 118px in; Output:
//!   More, Turn Auto Scrolling Off, Clear Output, the channel dropdown
//!   (170px, ending 144px in) and its filter; Debug Console: Clear Console,
//!   Find and the filter; Terminal: More, Kill, Split, Launch Profile,
//!   New Terminal);
//! - content from 35px down: Problems' message 22px in, the Debug
//!   Console's 21px input row at the bottom, Ports' message and its 300px
//!   Forward a Port button, the terminal's 12px Menlo cells (14px rows)
//!   20px in.
//!
//! Terminals live in groups (terminalGroup.ts): a group's terminals share
//! the view side by side ("Split"), 1px `terminal.border` between them.
//! With more than one terminal the tabs list shows on the right
//! (`terminal.integrated.tabs.hideCondition` "singleTerminal"), measured
//! against 1.145.2: 120px wide with a 1px `terminal.border` on its left,
//! from 33px under the card top; rows 22px inset 5 / 4, radius 4, the
//! `terminal` codicon 8px in, the name ("zsh") 24px in, split groups led by
//! ┌ ├ └; the active terminal in `list.inactiveSelection*`, hover
//! `list.hoverBackground` with Split (−38) and Kill (−18), 20px each. The
//! title then keeps only New Terminal (116), Launch Profile (100) and More
//! (77).

use std::collections::HashSet;
use std::path::PathBuf;

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::icons;
use crate::terminal::Terminal;
use crate::theme::Palette;

/// One problem of the Problems view (markersTreeViewer.ts `Marker`).
#[derive(Clone, Debug, PartialEq)]
pub struct ProblemItem {
    /// 0 error, 1 warning, 2 information, 3 hint.
    pub severity: u8,
    pub message: String,
    pub source: Option<String>,
    pub code: Option<String>,
    /// 0-based line and UTF-16 column.
    pub line: usize,
    pub col: usize,
}

/// A file with problems (`ResourceMarkers`).
#[derive(Clone, Debug, PartialEq)]
pub struct ProblemFile {
    pub path: PathBuf,
    pub name: String,
    /// The folder, relative to the workspace (the resource label's
    /// description); empty at the root.
    pub dir: String,
    pub items: Vec<ProblemItem>,
}

/// The files to list, in VS Code's order (`compareResourceMarkers`: the
/// first problem's severity, then path), problems by severity and position
/// (`_compareMarkers`).
pub fn sort_problems(mut files: Vec<ProblemFile>) -> Vec<ProblemFile> {
    for f in files.iter_mut() {
        f.items.sort_by_key(|i| (i.severity, i.line, i.col));
    }
    files.retain(|f| !f.items.is_empty());
    files.sort_by(|a, b| {
        let sa = a.items.first().map(|i| i.severity).unwrap_or(9);
        let sb = b.items.first().map(|i| i.severity).unwrap_or(9);
        sa.cmp(&sb).then_with(|| crate::workbench::settings_editor::model::locale_compare(&a.path.to_string_lossy(), &b.path.to_string_lossy())).then_with(|| a.name.cmp(&b.name))
    });
    files
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PanelView {
    Problems,
    Output,
    DebugConsole,
    #[default]
    Terminal,
    Ports,
}

impl PanelView {
    pub const ALL: [PanelView; 5] = [PanelView::Problems, PanelView::Output, PanelView::DebugConsole, PanelView::Terminal, PanelView::Ports];

    pub fn label(self) -> &'static str {
        match self {
            PanelView::Problems => "Problems",
            PanelView::Output => "Output",
            PanelView::DebugConsole => "Debug Console",
            PanelView::Terminal => "Terminal",
            PanelView::Ports => "Ports",
        }
    }
}

/// Channels VS Code always has (output service's built-ins), in the
/// dropdown's order below the extensions' own.
pub const SYSTEM_CHANNELS: &[&str] =
    &["Extension Host", "Main", "MCP Gateway", "Pty Host", "Remote Tunnel Service", "Settings Sync", "Shared", "Tasks", "Terminal", "Text Model Changes Reason", "Window"];

/// What the Output view needs of the app: the selected channel's text.
pub struct OutputText<'a> {
    pub text: &'a str,
}

#[derive(Clone, PartialEq, Eq)]
struct OutputKey {
    channel: String,
    len: usize,
    cols: usize,
    filter: String,
}

/// One wrapped row of output with its coloured spans (char ranges).
#[derive(Clone)]
struct OutRow {
    text: String,
    spans: Vec<(usize, usize, Color32)>,
}

#[derive(Default)]
pub struct PanelState {
    pub visible: bool,
    pub view: PanelView,
    pub maximized: bool,
    /// The card's height once dragged (`None`: a third of the window).
    pub height: Option<f32>,
    pub terminals: Vec<Terminal>,
    /// Index into `terminals` of the terminal with the keyboard.
    pub active_terminal: usize,
    /// Terminal groups (indices into `terminals`), in tab order.
    pub groups: Vec<Vec<usize>>,
    /// The active terminal should take the keyboard on the next frame.
    pub focus_pending: bool,
    pub problems_filter: String,
    /// The Output view's channel (the dropdown's selection).
    pub output_channel: String,
    /// Auto scrolling follows new output (the lock button).
    pub output_follow: bool,
    pub output_scroll: f32,
    output_seen: (String, usize),
    output_cache: Option<(OutputKey, Vec<OutRow>)>,
    /// Files collapsed in the Problems tree.
    pub problems_collapsed: HashSet<PathBuf>,
    /// The selected problem: file and index in its (sorted) problems.
    pub problems_selected: Option<(PathBuf, usize)>,
    pub problems_scroll: f32,
    /// The list keeps the keyboard after a click opened a problem in the
    /// editor (preserveFocus); only Enter moves focus to the editor.
    problems_refocus: u8,
    pub output_filter: String,
    pub debug_filter: String,
    /// The terminal has the keyboard.
    pub terminal_focused: bool,
    select_word: bool,
    /// The terminal's selection: anchor and head cells (row, col) on the
    /// screen as shown; cleared by typing, output scrolling it away or a click.
    pub selection: Option<((u16, u16), (u16, u16))>,
    dragging: bool,
}

impl PanelState {
    /// Adds a terminal: to the active group beside the active terminal
    /// (`split`), else as a new group. It becomes the active one.
    pub fn add_terminal(&mut self, t: Terminal, split: bool) {
        self.terminals.push(t);
        let i = self.terminals.len() - 1;
        match self.groups.iter_mut().find(|g| g.contains(&self.active_terminal)).filter(|_| split) {
            Some(g) => {
                let at = g.iter().position(|&x| x == self.active_terminal).map(|p| p + 1).unwrap_or(g.len());
                g.insert(at, i);
            }
            None => self.groups.push(vec![i]),
        }
        self.active_terminal = i;
        self.selection = None;
        self.focus_pending = true;
    }

    /// Removes terminal `i`; the neighbour in its group (else the previous
    /// group) becomes active.
    pub fn remove_terminal(&mut self, i: usize) {
        if i >= self.terminals.len() {
            return;
        }
        self.terminals.remove(i);
        let mut next = None;
        for (gi, g) in self.groups.iter_mut().enumerate() {
            if let Some(p) = g.iter().position(|&x| x == i) {
                g.remove(p);
                next = Some(if g.is_empty() { (gi, None) } else { (gi, Some(g[p.saturating_sub(1).min(g.len() - 1)])) });
            }
        }
        for g in self.groups.iter_mut() {
            for x in g.iter_mut() {
                if *x > i {
                    *x -= 1;
                }
            }
        }
        self.groups.retain(|g| !g.is_empty());
        if i == self.active_terminal || self.active_terminal >= self.terminals.len() {
            self.active_terminal = match next {
                Some((_, Some(n))) => if n > i { n - 1 } else { n },
                Some((gi, None)) => self.groups.get(gi.saturating_sub(1)).or(self.groups.first()).and_then(|g| g.first().copied()).unwrap_or(0),
                None => 0,
            };
        } else if self.active_terminal > i {
            self.active_terminal -= 1;
        }
        self.selection = None;
    }

    /// The group of the active terminal.
    pub fn active_group(&self) -> Vec<usize> {
        self.groups.iter().find(|g| g.contains(&self.active_terminal)).cloned().unwrap_or_else(|| if self.terminals.is_empty() { Vec::new() } else { vec![self.active_terminal.min(self.terminals.len() - 1)] })
    }
}

#[derive(Default, Debug)]
pub struct PanelOutput {
    pub close: bool,
    pub new_terminal: bool,
    /// Split the active terminal (a new one in its group).
    pub split_terminal: bool,
    pub kill_terminal: bool,
    /// Kill this terminal (a tab's trash button).
    pub kill_index: Option<usize>,
    /// The channel dropdown was clicked: where its menu opens.
    pub output_menu: Option<Pos2>,
    /// Clear Output on the selected channel.
    pub output_clear: bool,
    /// A problem was activated: open the file at (line, UTF-16 column),
    /// both 0-based.
    pub open_problem: Option<(PathBuf, usize, usize)>,
    pub forward_port: bool,
}

/// Default card height for a window `window_h` tall.
pub fn default_height(window_h: f32) -> f32 {
    (window_h / 3.0 - 15.0).round()
}

fn icon_button(ui: &mut Ui, painter: &egui::Painter, centre: Pos2, name: &str, tip: &str, id: egui::Id) -> bool {
    let r = Rect::from_center_size(centre, Vec2::splat(22.0));
    let resp = ui.interact(r, id.with(("action", name, tip)), Sense::click());
    if resp.hovered() {
        painter.rect_filled(r, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
    }
    let size = if name == "chevron-down" { 12.0 } else { 16.0 };
    painter.text(centre, Align2::CENTER_CENTER, icons::codicon_by_name(name).unwrap_or('?').to_string(), icons::codicon_font(size), Palette::FG());
    let label = tip.to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
    resp.on_hover_text(tip).clicked()
}

/// A 24px filter box (`viewFilter`), `text` typed into it.
fn filter_box(ui: &mut Ui, painter: &egui::Painter, rect: Rect, text: &mut String, placeholder: &str, funnel: bool, id: egui::Id) {
    let slot = painter.add(egui::Shape::Noop);
    let inner = Rect::from_min_max(Pos2::new(rect.left() + 7.0, rect.top() + 1.0), Pos2::new(rect.right() - if funnel { 22.0 } else { 6.0 }, rect.bottom() - 1.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.set_clip_rect(inner);
    child.visuals_mut().weak_text_color = Some(Palette::INPUT_PLACEHOLDER_FG());
    child.visuals_mut().text_cursor.stroke.color = Palette::INPUT_FG();
    let resp = child.add_sized(
        inner.size(),
        egui::TextEdit::singleline(text)
            .id_salt(id)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .hint_text(placeholder)
            .font(FontId::proportional(12.0))
            .text_color(Palette::INPUT_FG())
            .vertical_align(egui::Align::Center),
    );
    let border = if resp.has_focus() { Palette::FOCUS_BORDER() } else { Palette::INPUT_BORDER() };
    painter.set(slot, egui::epaint::RectShape::new(rect, 4.0, Color32::TRANSPARENT, Stroke::new(1.0, border), egui::StrokeKind::Inside));
    if funnel {
        let c = Pos2::new(rect.right() - 8.5 - 3.0, rect.center().y);
        painter.text(c, Align2::CENTER_CENTER, icons::codicon_by_name("filter").unwrap_or('?').to_string(), icons::codicon_font(16.0), Palette::FG());
    }
}

/// Draws the panel card in `rect` (the area under the editor card).
pub fn show(ui: &mut Ui, rect: Rect, state: &mut PanelState, problems: &[ProblemFile], output: &OutputText<'_>, cwd_label: &str) -> PanelOutput {
    if state.output_channel.is_empty() {
        state.output_channel = "Tasks".to_string();
        state.output_follow = true;
    }
    let _ = cwd_label;
    let mut out = PanelOutput::default();
    let id = ui.id().with("panel");
    let painter = ui.painter().clone();
    let card = rect;
    painter.rect(card, 8.0, Palette::PANEL_BG2(), Stroke::new(1.0, Palette::EDITOR_CARD_BORDER()), egui::StrokeKind::Inside);

    // Resize sash along the top edge.
    let sash = Rect::from_min_max(Pos2::new(card.left(), card.top() - 4.0), Pos2::new(card.right(), card.top() + 2.0));
    let sresp = ui.interact(sash, id.with("sash"), Sense::drag());
    if sresp.hovered() || sresp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if sresp.drag_started() {
        state.dragging = true;
    }
    if sresp.dragged() {
        let h = state.height.unwrap_or(card.height()) - sresp.drag_delta().y;
        state.height = Some(h.clamp(77.0, 2000.0));
    }
    if sresp.drag_stopped() {
        state.dragging = false;
    }

    // Tabs.
    let mut x = card.left() + 5.0;
    let ty = card.top() + 5.0;
    let problem_total: usize = problems.iter().map(|f| f.items.len()).sum();
    for view in PanelView::ALL {
        let g = painter.layout_no_wrap(view.label().to_string(), FontId::proportional(13.0), Color32::PLACEHOLDER);
        // Problems carries a count badge (16px, 10px text) 6px after its label.
        let badge = (view == PanelView::Problems && problem_total > 0).then(|| {
            let t = painter.layout_no_wrap(problem_total.to_string(), FontId::proportional(10.0), Palette::BADGE_FG());
            (t.size().x.max(6.0) + 10.2).max(16.2)
        });
        let width = g.size().x + 16.0 + badge.map(|b| 6.0 + b - 2.5).unwrap_or(0.0);
        let r = Rect::from_min_size(Pos2::new(x, ty), Vec2::new(width, 24.0));
        let resp = ui.interact(r, id.with(("tab", view.label())), Sense::click());
        let active = state.view == view;
        let colour = if active {
            painter.rect_filled(r, 4.0, Palette::MODERN_TAB_ACTIVE_BG());
            Palette::MODERN_TAB_ACTIVE_FG()
        } else if resp.hovered() {
            painter.rect_filled(r, 4.0, Palette::MODERN_TAB_HOVER_BG());
            Palette::MODERN_TAB_HOVER_FG()
        } else {
            Palette::PANEL_TITLE_INACTIVE_FG()
        };
        let text_w = g.size().x;
        painter.galley(Pos2::new(r.left() + 8.0, r.center().y - g.size().y / 2.0), g, colour);
        if let Some(bw) = badge {
            let b = Rect::from_min_size(Pos2::new(r.left() + 8.0 + text_w + 6.0, r.center().y - 8.0), Vec2::new(bw, 16.0));
            painter.rect_filled(b, 8.0, Palette::BADGE_BG());
            painter.text(b.center(), Align2::CENTER_CENTER, problem_total.to_string(), FontId::proportional(10.0), Palette::BADGE_FG());
        }
        let label = format!("panel:{}", view.label());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, label.clone()));
        if resp.clicked() {
            state.view = view;
            if view == PanelView::Terminal && state.terminals.is_empty() {
                out.new_terminal = true;
            }
        }
        x = r.right() + 4.5;
    }

    // Actions.
    let cy = ty + 12.0;
    let at = |from_right: f32| Pos2::new(card.right() - from_right, cy);
    if icon_button(ui, &painter, at(16.0), "close", "Hide Panel (⌘J)", id) {
        out.close = true;
    }
    let (max_icon, max_tip) = if state.maximized { ("screen-normal", "Restore Panel Size") } else { ("screen-full", "Maximize Panel Size") };
    if icon_button(ui, &painter, at(42.0), max_icon, max_tip, id) {
        state.maximized = !state.maximized;
    }
    let tabs_shown = state.view == PanelView::Terminal && state.terminals.len() > 1;
    let view_actions: &[(&str, &str)] = match state.view {
        PanelView::Terminal if tabs_shown => &[("ellipsis", "Views and More Actions...")],
        PanelView::Problems => &[("list-flat", "View as Table"), ("collapse-all", "Collapse All")],
        PanelView::Output if state.output_follow => &[("ellipsis", "Views and More Actions..."), ("lock", "Turn Auto Scrolling Off"), ("clear-all", "Clear Output")],
        PanelView::Output => &[("ellipsis", "Views and More Actions..."), ("unlock", "Turn Auto Scrolling On"), ("clear-all", "Clear Output")],
        PanelView::DebugConsole => &[("clear-all", "Clear Console"), ("search", "Find")],
        PanelView::Terminal => &[("ellipsis", "More Actions..."), ("trash", "Kill Terminal"), ("split-horizontal", "Split Terminal (⌘\\)")],
        PanelView::Ports => &[],
    };
    if !view_actions.is_empty() {
        painter.vline(card.right() - 57.5, (cy - 8.0)..=(cy + 8.0), Stroke::new(1.0, Palette::FG().gamma_multiply(0.3)));
    }
    for (i, (name, tip)) in view_actions.iter().enumerate() {
        let from = 77.0 + 25.75 * i as f32;
        if icon_button(ui, &painter, at(from), name, tip, id) {
            match *name {
                "collapse-all" => state.problems_collapsed = problems.iter().map(|f| f.path.clone()).collect(),
                "lock" | "unlock" => state.output_follow = !state.output_follow,
                "clear-all" if state.view == PanelView::Output => out.output_clear = true,
                "trash" => out.kill_terminal = true,
                "split-horizontal" => out.split_terminal = true,
                _ => {}
            }
        }
    }
    match state.view {
        PanelView::Terminal => {
            // With the tabs list, Kill and Split move into its rows.
            let (chevron, add) = if tabs_shown { (100.0, 116.0) } else { (152.0, 168.5) };
            if icon_button(ui, &painter, at(chevron), "chevron-down", "Launch Profile...", id) {
                out.new_terminal = true;
            }
            if icon_button(ui, &painter, at(add), "add", "New Terminal (⌃⇧`)", id) {
                out.new_terminal = true;
            }
        }
        PanelView::Problems | PanelView::DebugConsole => {
            let r = Rect::from_min_max(Pos2::new(card.right() - 318.0, ty), Pos2::new(card.right() - 118.0, ty + 24.0));
            let (text, placeholder, funnel) = if state.view == PanelView::Problems {
                (&mut state.problems_filter, "Filter (e.g. text, **/*.ts, !**/node_modules/**)", true)
            } else {
                (&mut state.debug_filter, "Filter (e.g. text, !exclude, \\escape)", false)
            };
            filter_box(ui, &painter, r, text, placeholder, funnel, id.with(("filter", state.view.label())));
        }
        PanelView::Output => {
            let dd = Rect::from_min_max(Pos2::new(card.right() - 314.0, ty), Pos2::new(card.right() - 144.0, ty + 24.0));
            painter.rect(dd, 4.0, Palette::SETTINGS_DROPDOWN_BG(), Stroke::new(1.0, Palette::SETTINGS_DROPDOWN_BORDER()), egui::StrokeKind::Inside);
            let dresp = ui.interact(dd, id.with("output-channel"), Sense::click());
            let channel = state.output_channel.clone();
            dresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, format!("Output channel: {channel}")));
            if dresp.clicked() {
                out.output_menu = Some(Pos2::new(dd.left(), dd.bottom() + 2.0));
            }
            painter.text(Pos2::new(dd.left() + 7.0, dd.center().y), Align2::LEFT_CENTER, &state.output_channel, FontId::proportional(12.0), Palette::FG());
            painter.text(Pos2::new(dd.right() - 12.0, dd.center().y), Align2::CENTER_CENTER, icons::codicon_by_name("chevron-down").unwrap_or('v').to_string(), icons::codicon_font(16.0), Palette::FG());
            let r = Rect::from_min_max(Pos2::new(card.right() - 518.0, ty), Pos2::new(card.right() - 318.0, ty + 24.0));
            filter_box(ui, &painter, r, &mut state.output_filter, "Filter (e.g. text, !excludeText, text1,text2)", false, id.with("filter-output"));
        }
        PanelView::Ports => {}
    }

    // Content.
    let content = Rect::from_min_max(Pos2::new(card.left() + 1.0, card.top() + 35.0), Pos2::new(card.right() - 1.0, card.bottom() - 1.0));
    match state.view {
        PanelView::Problems => problems_view(ui, &painter, card, problems, state, &mut out, id),
        PanelView::Output => output_view(ui, &painter, content, output.text, state, id),
        PanelView::DebugConsole => {
            let row = Rect::from_min_max(Pos2::new(content.left(), card.bottom() - 22.0), Pos2::new(content.right(), card.bottom() - 1.0));
            painter.hline(row.x_range(), row.top() + 0.5, Stroke::new(1.0, Palette::INPUT_BORDER()));
            painter.text(Pos2::new(card.left() + 19.0, row.center().y + 0.5), Align2::CENTER_CENTER, icons::codicon_by_name("chevron-right").unwrap_or('>').to_string(), icons::codicon_font(14.0), Palette::FG());
            painter.text(Pos2::new(card.left() + 33.0, row.center().y + 0.5), Align2::LEFT_CENTER, "Please start a debug session to evaluate expressions", crate::ui_font::mono(12.0), Palette::INPUT_PLACEHOLDER_FG());
        }
        PanelView::Ports => {
            painter.text(
                Pos2::new(card.left() + 22.0, card.top() + 56.0),
                Align2::LEFT_CENTER,
                "No forwarded ports. Forward a port to access your locally running services over the internet.",
                FontId::proportional(13.0),
                Palette::FG(),
            );
            let br = Rect::from_min_size(Pos2::new(card.left() + 21.0, card.top() + 77.0), Vec2::new(300.0, 26.0));
            let resp = ui.interact(br, id.with("forward"), Sense::click());
            painter.rect_filled(br, 4.0, if resp.hovered() { Palette::BUTTON_HOVER() } else { Palette::BUTTON_BG() });
            painter.text(br.center(), Align2::CENTER_CENTER, "Forward a Port", FontId::proportional(13.0), Palette::BUTTON_FG());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Forward a Port"));
            if resp.clicked() {
                out.forward_port = true;
            }
        }
        PanelView::Terminal => {
            let mut panes = content;
            if tabs_shown {
                let list = Rect::from_min_max(Pos2::new(card.right() - 121.0, card.top() + 33.0), Pos2::new(card.right() - 1.0, card.bottom() - 1.0));
                terminal_tabs(ui, &painter, list, state, &mut out, id);
                panes.max.x = list.left();
            }
            // The active group's terminals side by side.
            let group = state.active_group();
            let n = group.len().max(1) as f32;
            let w = panes.width() / n;
            for (k, &ti) in group.iter().enumerate() {
                let left = (panes.left() + w * k as f32).round();
                let right = if k + 1 == group.len() { panes.right() } else { (panes.left() + w * (k + 1) as f32).round() };
                let pane = Rect::from_min_max(Pos2::new(left, panes.top()), Pos2::new(right, panes.bottom()));
                if k > 0 {
                    painter.vline(left + 0.5, (card.top() + 33.0)..=(card.bottom() - 1.0), Stroke::new(1.0, Palette::TERMINAL_BORDER()));
                }
                terminal_view(ui, &painter, pane, state, id, ti);
            }
        }
    }
    out
}

/// The terminal tabs list (terminalTabsList.ts).
fn terminal_tabs(ui: &mut Ui, painter: &egui::Painter, list: Rect, state: &mut PanelState, out: &mut PanelOutput, id: egui::Id) {
    painter.vline(list.left() + 0.5, list.y_range(), Stroke::new(1.0, Palette::TERMINAL_BORDER()));
    let mut y = list.top();
    let groups = state.groups.clone();
    for g in &groups {
        for (k, &ti) in g.iter().enumerate() {
            let row = Rect::from_min_max(Pos2::new(list.left() + 5.0, y), Pos2::new(list.right() - 4.0, y + 22.0));
            y += 22.0;
            let Some(term) = state.terminals.get(ti) else { continue };
            let resp = ui.interact(row, id.with(("tab", ti)), Sense::click());
            let active = ti == state.active_terminal;
            if active {
                painter.rect_filled(row, 4.0, Palette::LIST_INACTIVE_SELECTION_BG());
            } else if resp.hovered() {
                painter.rect_filled(row, 4.0, Palette::LIST_HOVER_BG());
            }
            let fg = if active { Palette::LIST_INACTIVE_SELECTION_FG() } else { Palette::FG() };
            let cy = row.center().y;
            let mut x = row.left() + 8.0;
            if g.len() > 1 {
                let glyph = if k == 0 { "┌" } else if k + 1 == g.len() { "└" } else { "├" };
                painter.text(Pos2::new(x, cy - 0.5), Align2::LEFT_CENTER, glyph, FontId::proportional(13.0), fg);
                x += 16.5;
            }
            painter.text(Pos2::new(x + 8.0, cy + 0.5), Align2::CENTER_CENTER, icons::codicon_by_name("terminal").unwrap_or('>').to_string(), icons::codicon_font(16.0), fg);
            painter.text(Pos2::new(x + 19.5, cy - 0.5), Align2::LEFT_CENTER, &term.title, FontId::proportional(13.0), fg);
            let label = format!("terminal-tab:{} {}", ti + 1, term.title);
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, label.clone()));
            let mut on_action = false;
            if resp.hovered() || ui.rect_contains_pointer(row) {
                for (from_right, name, tip) in [(38.0, "split-horizontal", "Split (⌃⇧5)"), (18.0, "trash", "Kill (⌘Backspace)")] {
                    let r = Rect::from_center_size(Pos2::new(row.right() - from_right, cy), Vec2::splat(20.0));
                    let b = ui.interact(r, id.with(("tab-action", ti, name)), Sense::click());
                    if b.hovered() {
                        on_action = true;
                        painter.rect_filled(r, 4.0, Palette::VSCE_TOOLBAR_HOVER_BG());
                    }
                    painter.text(r.center(), Align2::CENTER_CENTER, icons::codicon_by_name(name).unwrap_or('?').to_string(), icons::codicon_font(16.0), fg);
                    b.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
                    if b.clicked() {
                        on_action = true;
                        if name == "trash" {
                            out.kill_index = Some(ti);
                        } else {
                            state.active_terminal = ti;
                            out.split_terminal = true;
                        }
                    }
                }
            }
            if resp.clicked() && !on_action {
                state.active_terminal = ti;
                state.selection = None;
                state.focus_pending = true;
            }
        }
    }
}

/// Log-output highlighting: the grammar of VS Code's `log` language
/// (extensions/log/syntaxes/log.tmLanguage.json, the parts that matter),
/// coloured by the theme's `tokenColors`.
fn log_regex() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r#"(?x)
            (?P<date>\b\d{4}-\d{2}-\d{2}\b|\b\d{1,2}:\d{2}(?::\d{2}(?:[.,]\d+)?)?\b)
            |(?P<verbose>\[(?i:verbose|verb|trace)\]|\b(?:TRACE|Trace)\b)
            |(?P<debug>\[(?i:debug|dbug|dbg)\]|\b(?:DEBUG|Debug)\b)
            |(?P<info>\[(?i:information|info|inf)\]|\b(?:INFO|Info|NOTICE|HINT)\b)
            |(?P<warn>\[(?i:warning|warn|wrn)\]|\b(?:WARNING|WARN|Warn)\b)
            |(?P<error>\[(?i:error|eror|err|fatal)\]|\b(?:ERROR|Error|FATAL|Fatal|CRITICAL)\b)
            |(?P<url>\b[a-z][a-z0-9+.-]*://\S+)
            |(?P<string>"[^"]*"|'[^']*')
            |(?P<constant>\b(?:0x[0-9a-fA-F]+|[0-9]+|true|false|null)\b)
            "#,
        )
        .expect("log grammar")
    })
}

/// Coloured char ranges of one output line.
fn log_spans(line: &str, colours: &[(&'static str, Color32)]) -> Vec<(usize, usize, Color32)> {
    let colour = |name: &str| colours.iter().find(|(n, _)| *n == name).map(|(_, c)| *c);
    let mut spans = Vec::new();
    for cap in log_regex().captures_iter(line) {
        let m = cap.get(0).unwrap();
        for (name, _) in colours {
            if cap.name(name).is_some() {
                if let Some(c) = colour(name) {
                    let a = line[..m.start()].chars().count();
                    spans.push((a, a + m.as_str().chars().count(), c));
                }
                break;
            }
        }
    }
    spans
}

/// Wraps `line` at `cols` columns on spaces, continuation rows keeping the
/// line's indentation (`wrappingIndent: same`); returns (start, end) char
/// ranges of each row.
fn wrap_ranges(line: &str, cols: usize) -> Vec<(usize, usize)> {
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= cols {
        return vec![(0, chars.len())];
    }
    let indent = chars.iter().take_while(|c| **c == ' ').count().min(cols.saturating_sub(8));
    let mut out = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let room = if start == 0 { cols } else { cols - indent };
        if chars.len() - start <= room {
            out.push((start, chars.len()));
            break;
        }
        // Break after the last space that fits, else hard at the limit.
        let limit = start + room;
        let mut at = (start + 1..=limit).rev().find(|&i| chars[i - 1] == ' ' && i > start).unwrap_or(limit);
        if at <= start {
            at = limit;
        }
        out.push((start, at));
        start = at;
    }
    out
}

/// Whether `line` passes the filter box (`text`, `!excluded`, `a,b` any of).
fn output_filter_passes(line: &str, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let lower = line.to_lowercase();
    let (mut includes, mut excludes) = (Vec::new(), Vec::new());
    for term in filter.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        match term.strip_prefix('!') {
            Some(x) if !x.is_empty() => excludes.push(x.to_lowercase()),
            Some(_) => {}
            None => includes.push(term.to_lowercase()),
        }
    }
    (includes.is_empty() || includes.iter().any(|t| lower.contains(t))) && !excludes.iter().any(|t| lower.contains(t))
}

/// The Output view (output editor): the channel's text in the editor font
/// (12px Menlo, 18px lines, 20px in, no gutter), word-wrapped, log lines
/// coloured like VS Code's `log` language, following new output while
/// auto-scroll is on.
fn output_view(ui: &mut Ui, painter: &egui::Painter, content: Rect, text: &str, state: &mut PanelState, id: egui::Id) {
    const LINE: f32 = 18.0;
    // The editor starts 33px under the card top (content is at +35).
    let area = Rect::from_min_max(Pos2::new(content.left() + 20.0, content.top() - 2.0), Pos2::new(content.right() - 14.0, content.bottom() - 5.0));
    let font = icons::editor_mono_font(12.0);
    let cell_w = ui.fonts_mut(|f| f.glyph_width(&font, 'W'));
    let cols = ((area.width() / cell_w).floor() as usize).max(8);
    let filter = state.output_filter.trim().to_string();
    let key = OutputKey { channel: state.output_channel.clone(), len: text.len(), cols, filter: filter.clone() };
    if state.output_cache.as_ref().map(|(k, _)| k) != Some(&key) {
        let colours: Vec<(&'static str, Color32)> = [
            ("date", &["comment", "log.date"][..]),
            ("verbose", &["comment", "log.verbose"][..]),
            ("debug", &["markup.changed", "log.debug"][..]),
            ("info", &["markup.inserted", "log.info"][..]),
            ("warn", &["markup.deleted", "log.warning"][..]),
            ("error", &["string.regexp", "strong", "log.error"][..]),
            ("url", &["constant.language", "log.constant"][..]),
            ("string", &["string", "log.string"][..]),
            ("constant", &["constant.language", "log.constant"][..]),
        ]
        .into_iter()
        .filter_map(|(n, scopes)| crate::theme::token_style(scopes).0.map(|c| (n, c)))
        .collect();
        let mut rows = Vec::new();
        for line in text.split('\n').map(|l| l.trim_end_matches('\r')) {
            if !output_filter_passes(line, &filter) {
                continue;
            }
            let spans = log_spans(line, &colours);
            let chars: Vec<char> = line.chars().collect();
            let indent = chars.iter().take_while(|c| **c == ' ').count().min(cols.saturating_sub(8));
            for (k, (a, b)) in wrap_ranges(line, cols).into_iter().enumerate() {
                let prefix = if k == 0 { 0 } else { indent };
                let mut row_text: String = " ".repeat(prefix);
                row_text.extend(chars[a..b].iter());
                let row_spans = spans
                    .iter()
                    .filter(|(s, e, _)| *s < b && *e > a)
                    .map(|(s, e, c)| (prefix + (*s).max(a) - a, prefix + (*e).min(b) - a, *c))
                    .collect();
                rows.push(OutRow { text: row_text, spans: row_spans });
            }
        }
        // The text ends with a newline: that final empty line isn't a row.
        if text.ends_with('\n') && rows.last().is_some_and(|r| r.text.is_empty()) {
            rows.pop();
        }
        state.output_cache = Some((key, rows));
    }
    let rows = &state.output_cache.as_ref().unwrap().1;
    let total = rows.len() as f32 * LINE;
    let viewport = area.height();
    let max_scroll = (total - viewport).max(0.0);
    // New output (or another channel) scrolls to the end while following.
    let seen = (state.output_channel.clone(), text.len());
    if state.output_follow && state.output_seen != seen {
        state.output_scroll = max_scroll;
    }
    state.output_seen = seen;
    if ui.rect_contains_pointer(Rect::from_min_max(content.min, content.max)) {
        state.output_scroll -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    state.output_scroll = state.output_scroll.clamp(0.0, max_scroll);
    let clip = painter.with_clip_rect(Rect::from_min_max(Pos2::new(content.left(), area.top()), Pos2::new(content.right(), area.bottom() + 5.0)));
    let first = (state.output_scroll / LINE).floor() as usize;
    let fg = Palette::EDITOR_FG();
    let mut y = area.top() + first as f32 * LINE - state.output_scroll;
    for row in rows.iter().skip(first) {
        if y > area.bottom() {
            break;
        }
        let chars: Vec<char> = row.text.chars().collect();
        let mut job = egui::text::LayoutJob::default();
        let mut at = 0;
        let mut push = |job: &mut egui::text::LayoutJob, a: usize, b: usize, c: Color32| {
            if b > a {
                job.append(&chars[a..b].iter().collect::<String>(), 0.0, egui::TextFormat { font_id: font.clone(), color: c, line_height: Some(LINE), ..Default::default() });
            }
        };
        let mut spans = row.spans.clone();
        spans.sort_by_key(|s| s.0);
        for (a, b, c) in spans {
            if a < at {
                continue;
            }
            push(&mut job, at, a, fg);
            push(&mut job, a, b, c);
            at = b;
        }
        push(&mut job, at, chars.len(), fg);
        let g = ui.fonts_mut(|f| f.layout_job(job));
        clip.galley(Pos2::new(area.left(), y), g, fg);
        y += LINE;
    }
    // Overlay scrollbar (10px) while hovered.
    if max_scroll > 0.0 && ui.rect_contains_pointer(content) {
        let track = Rect::from_min_max(Pos2::new(content.right() - 10.0, area.top()), Pos2::new(content.right(), area.bottom()));
        let thumb_h = (viewport / total * viewport).max(20.0);
        let thumb_top = track.top() + (state.output_scroll / max_scroll) * (viewport - thumb_h);
        painter.rect_filled(Rect::from_min_size(Pos2::new(track.left(), thumb_top), Vec2::new(10.0, thumb_h)), 0.0, Palette::VSCE_SCROLLBAR_SLIDER_BG());
    }
    let _ = id;
}

fn severity_icon(severity: u8) -> (&'static str, Color32) {
    match severity {
        0 => ("error", Palette::PROBLEMS_ERROR_FG()),
        1 => ("warning", Palette::PROBLEMS_WARNING_FG()),
        _ => ("info", Palette::PROBLEMS_INFO_FG()),
    }
}

/// Whether `file` or one of its problems matches the filter box text (words
/// of the text, each in the message, source, code or file name).
fn problem_matches(f: &ProblemFile, i: &ProblemItem, filter: &str) -> bool {
    let hay = format!("{} {} {} {}", i.message, i.source.as_deref().unwrap_or(""), i.code.as_deref().unwrap_or(""), f.name).to_lowercase();
    filter.split_whitespace().all(|w| match w.strip_prefix('!') {
        Some(ex) => ex.is_empty() || !hay.contains(&ex.to_lowercase()),
        None => hay.contains(&w.to_lowercase()),
    })
}

/// The Problems view (markersView.ts, markersTreeViewer.ts), the tree: a
/// file row (twistie, file icon, name, folder, count badge) with its
/// problems under it. Measured against 1.145.2: rows 22px inset 5 from the
/// card edge (4 from the content), twistie centred 18 in, file icon 30,
/// label 52; problem icon 44 (16px), message 66, then source and `(code)`
/// right after it and `[Ln, Col]` 6px on, those at 70% opacity; indent
/// guide 16px in; selection `list.activeSelectionBackground` with a
/// `list.focusOutline` while the list has focus; `problemsErrorIcon.*`.
fn problems_view(ui: &mut Ui, painter: &egui::Painter, card: Rect, problems: &[ProblemFile], state: &mut PanelState, out: &mut PanelOutput, id: egui::Id) {
    let top = card.top() + 33.0;
    let area = Rect::from_min_max(Pos2::new(card.left() + 1.0, top), Pos2::new(card.right() - 1.0, card.bottom() - 1.0));
    let filter = state.problems_filter.trim().to_string();
    let shown: Vec<(&ProblemFile, Vec<usize>)> = problems
        .iter()
        .map(|f| (f, (0..f.items.len()).filter(|&k| filter.is_empty() || problem_matches(f, &f.items[k], &filter)).collect::<Vec<_>>()))
        .filter(|(_, idx)| !idx.is_empty())
        .collect();
    if shown.is_empty() {
        let text = if problems.is_empty() { "No problems have been detected in the workspace." } else { "No results found with provided filter criteria." };
        painter.text(Pos2::new(card.left() + 22.0, card.top() + 45.0), Align2::LEFT_CENTER, text, FontId::proportional(13.0), Palette::FG());
        return;
    }
    // Flatten the visible rows.
    enum Row<'a> {
        File(&'a ProblemFile, usize),
        Item(&'a ProblemFile, usize),
    }
    let mut rows: Vec<Row> = Vec::new();
    for (f, idx) in &shown {
        rows.push(Row::File(f, idx.len()));
        if !state.problems_collapsed.contains(&f.path) {
            rows.extend(idx.iter().map(|&k| Row::Item(f, k)));
        }
    }
    let total_h = rows.len() as f32 * 22.0;
    let viewport = area.height();
    let max_scroll = (total_h - viewport).max(0.0);
    let resp = ui.interact(area, id.with("problems-list"), Sense::click());
    if ui.rect_contains_pointer(area) {
        state.problems_scroll -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    state.problems_scroll = state.problems_scroll.clamp(0.0, max_scroll);
    let list_id = id.with("problems-list");
    if state.problems_refocus > 0 {
        state.problems_refocus -= 1;
        ui.memory_mut(|m| m.request_focus(list_id));
    }
    let list_focused = ui.memory(|m| m.has_focus(list_id));
    // Keyboard: ↑ / ↓ move through the problems, Enter opens the selected one.
    if list_focused {
        ui.memory_mut(|m| m.set_focus_lock_filter(list_id, egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() }));
        let (up, down, enter) = ui.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::Enter)));
        let flat: Vec<(&ProblemFile, usize)> = rows.iter().filter_map(|r| if let Row::Item(f, k) = r { Some((*f, *k)) } else { None }).collect();
        let at = state.problems_selected.as_ref().and_then(|(p, i)| flat.iter().position(|(f, k)| &f.path == p && k == i));
        let next = if down {
            Some(at.map(|a| (a + 1).min(flat.len().saturating_sub(1))).unwrap_or(0))
        } else if up {
            Some(at.map(|a| a.saturating_sub(1)).unwrap_or(0))
        } else {
            None
        };
        if let Some(n) = next.filter(|_| !flat.is_empty()) {
            let (f, k) = flat[n];
            state.problems_selected = Some((f.path.clone(), k));
            // Keep the selected row in view.
            let row_index = rows.iter().position(|r| matches!(r, Row::Item(rf, rk) if rf.path == f.path && *rk == k)).unwrap_or(0);
            let top = row_index as f32 * 22.0;
            if top < state.problems_scroll {
                state.problems_scroll = top;
            } else if top + 22.0 > state.problems_scroll + viewport {
                state.problems_scroll = top + 22.0 - viewport;
            }
        }
        if enter {
            if let Some((p, i)) = &state.problems_selected {
                if let Some(item) = flat.iter().find(|(f, k)| &f.path == p && k == i).map(|(f, k)| &f.items[*k]) {
                    out.open_problem = Some((p.clone(), item.line, item.col));
                }
            }
        }
    }
    let clip = painter.with_clip_rect(area);
    let guide = Palette::VSCE_TREE_INACTIVE_INDENT_GUIDE();
    let guide_active = Palette::VSCE_TREE_ACTIVE_INDENT_GUIDE();
    let row_left = area.left() + 4.0;
    let row_right = area.right() - 4.0;
    let mut guide_runs: Vec<(f32, f32, bool)> = Vec::new();
    let mut y = area.top() - state.problems_scroll;
    let mut clicked_file: Option<PathBuf> = None;
    for row in &rows {
        let r = Rect::from_min_max(Pos2::new(row_left, y), Pos2::new(row_right, y + 22.0));
        y += 22.0;
        if r.bottom() < area.top() || r.top() > area.bottom() {
            if let Row::Item(f, _) = row {
                let sel = state.problems_selected.as_ref().is_some_and(|(p, _)| p == &f.path);
                match guide_runs.last_mut() {
                    Some(last) if last.1 >= r.top() - 0.5 && last.2 == sel => last.1 = r.bottom(),
                    _ => guide_runs.push((r.top(), r.bottom(), sel)),
                }
            }
            continue;
        }
        let visible = r.intersect(area);
        let (path, selected) = match row {
            Row::File(f, _) => (f.path.clone(), false),
            Row::Item(f, k) => (f.path.clone(), state.problems_selected.as_ref().is_some_and(|(p, i)| p == &f.path && i == k)),
        };
        let rid = match row {
            Row::File(f, _) => id.with(("problem-file", &f.path)),
            Row::Item(f, k) => id.with(("problem", &f.path, *k)),
        };
        let rresp = ui.interact(visible, rid, Sense::click());
        if selected {
            let bg = if list_focused { Palette::LIST_ACTIVE_SELECTION_BG() } else { Palette::LIST_INACTIVE_SELECTION_BG() };
            clip.rect_filled(r, 4.0, bg);
            if list_focused {
                clip.rect_stroke(r, 4.0, Stroke::new(1.0, Palette::VSCE_LIST_FOCUS_OUTLINE_2026()), egui::StrokeKind::Inside);
            }
        } else if rresp.hovered() {
            clip.rect_filled(r, 4.0, Palette::LIST_HOVER_BG());
        }
        let fg = if selected && list_focused { Palette::LIST_ACTIVE_SELECTION_FG() } else { Palette::FG() };
        let cy = r.center().y;
        match row {
            Row::File(f, count) => {
                let collapsed = state.problems_collapsed.contains(&f.path);
                let chev = if collapsed { "chevron-right" } else { "chevron-down" };
                clip.text(Pos2::new(r.left() + 18.0, cy), Align2::CENTER_CENTER, icons::codicon_by_name(chev).unwrap_or('v').to_string(), icons::codicon_font(16.0), Palette::FG());
                if let Some((glyph, colour)) = crate::file_icons::icon_for(&f.path) {
                    clip.text(Pos2::new(r.left() + 38.0, cy + 0.5), Align2::CENTER_CENTER, glyph.to_string(), crate::file_icons::seti_font(18.0), colour);
                }
                let g = clip.layout_no_wrap(f.name.clone(), FontId::proportional(13.0), fg);
                let name_w = g.size().x;
                clip.galley(Pos2::new(r.left() + 52.0, cy - g.size().y / 2.0 - 0.5), g, fg);
                let mut x = r.left() + 52.0 + name_w;
                if !f.dir.is_empty() {
                    let d = clip.layout_no_wrap(f.dir.clone(), FontId::proportional(11.7), Palette::FG_DESCRIPTION());
                    let dw = d.size().x;
                    clip.galley(Pos2::new(x + 6.0, cy - d.size().y / 2.0 - 0.5), d, Palette::FG_DESCRIPTION());
                    x += 6.0 + dw;
                }
                let t = clip.layout_no_wrap(count.to_string(), FontId::proportional(10.0), Palette::BADGE_FG());
                let bw = (t.size().x + 12.0).max(19.0);
                let b = Rect::from_min_size(Pos2::new(x + 10.0, cy - 9.5), Vec2::new(bw, 19.0));
                clip.rect_filled(b, 9.5, Palette::BADGE_BG());
                clip.galley(Pos2::new(b.center().x - t.size().x / 2.0, b.center().y - t.size().y / 2.0), t, Palette::BADGE_FG());
                let label = format!("{}, {} problems", f.name, count);
                rresp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
                if rresp.clicked() {
                    clicked_file = Some(f.path.clone());
                }
            }
            Row::Item(f, k) => {
                let item = &f.items[*k];
                let (icon, colour) = severity_icon(item.severity);
                let icon_colour = if selected && list_focused { Palette::LIST_ACTIVE_SELECTION_FG() } else { colour };
                clip.text(Pos2::new(r.left() + 52.0, cy), Align2::CENTER_CENTER, icons::codicon_by_name(icon).unwrap_or('!').to_string(), icons::codicon_font(16.0), icon_colour);
                let first_line = item.message.lines().next().unwrap_or("");
                let g = clip.layout_no_wrap(first_line.to_string(), FontId::proportional(13.0), fg);
                let mut x = r.left() + 66.0 + g.size().x;
                clip.galley(Pos2::new(r.left() + 66.0, cy - g.size().y / 2.0 - 0.5), g, fg);
                let dim = fg.gamma_multiply(0.7);
                if let Some(src) = &item.source {
                    let g = clip.layout_no_wrap(src.clone(), FontId::proportional(13.0), dim);
                    let w = g.size().x;
                    clip.galley(Pos2::new(x + 6.0, cy - g.size().y / 2.0 - 0.5), g, dim);
                    x += 6.0 + w;
                }
                if let Some(code) = &item.code {
                    let text = format!("({code})");
                    let g = clip.layout_no_wrap(text, FontId::proportional(13.0), dim);
                    let w = g.size().x;
                    // Directly after the source, or 6px after the message.
                    let gap = if item.source.is_some() { 0.0 } else { 6.0 };
                    clip.galley(Pos2::new(x + gap, cy - g.size().y / 2.0 - 0.5), g, dim);
                    x += gap + w;
                }
                let pos = format!("[Ln {}, Col {}]", item.line + 1, item.col + 1);
                let g = clip.layout_no_wrap(pos, FontId::proportional(13.0), dim);
                clip.galley(Pos2::new(x + 6.0, cy - g.size().y / 2.0 - 0.5), g, dim);
                let label = format!("{} {}, {}", icon, item.message, item.source.as_deref().unwrap_or(""));
                rresp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, label.clone()));
                if rresp.clicked() || rresp.double_clicked() {
                    state.problems_selected = Some((path.clone(), *k));
                    ui.memory_mut(|m| m.request_focus(id.with("problems-list")));
                    if rresp.double_clicked() || rresp.clicked() {
                        out.open_problem = Some((f.path.clone(), item.line, item.col));
                        // Opening from the list leaves the keyboard in it for a
                        // few frames, while the editor takes its cursor.
                        state.problems_refocus = 3;
                    }
                }
                match guide_runs.last_mut() {
                    Some(last) if last.1 >= r.top() - 0.5 && last.2 == state.problems_selected.as_ref().is_some_and(|(p, _)| p == &f.path) => last.1 = r.bottom(),
                    _ => guide_runs.push((r.top(), r.bottom(), state.problems_selected.as_ref().is_some_and(|(p, _)| p == &f.path))),
                }
            }
        }
    }
    for (a, b, active) in guide_runs {
        clip.vline(row_left + 12.0, a..=b, Stroke::new(1.0, if active { guide_active } else { guide }));
    }
    if let Some(p) = clicked_file {
        if !state.problems_collapsed.remove(&p) {
            state.problems_collapsed.insert(p);
        }
        ui.memory_mut(|m| m.request_focus(id.with("problems-list")));
    }
    let _ = resp;
    // Overlay scrollbar (10px).
    if max_scroll > 0.0 && ui.rect_contains_pointer(area) {
        let track = Rect::from_min_max(Pos2::new(area.right() - 10.0, area.top()), area.right_bottom());
        let thumb_h = (viewport / total_h * viewport).max(20.0);
        let thumb_top = track.top() + (state.problems_scroll / max_scroll) * (viewport - thumb_h);
        painter.rect_filled(Rect::from_min_size(Pos2::new(track.left(), thumb_top), Vec2::new(10.0, thumb_h)), 0.0, Palette::VSCE_SCROLLBAR_SLIDER_BG());
    }
}

/// xterm's 256-colour palette entry `i` (16 theme colours, the 6×6×6 cube,
/// 24 greys).
fn ansi(i: u8) -> Color32 {
    let theme = [
        Palette::TERMINAL_ANSI_BLACK, Palette::TERMINAL_ANSI_RED, Palette::TERMINAL_ANSI_GREEN, Palette::TERMINAL_ANSI_YELLOW,
        Palette::TERMINAL_ANSI_BLUE, Palette::TERMINAL_ANSI_MAGENTA, Palette::TERMINAL_ANSI_CYAN, Palette::TERMINAL_ANSI_WHITE,
        Palette::TERMINAL_ANSI_BRIGHT_BLACK, Palette::TERMINAL_ANSI_BRIGHT_RED, Palette::TERMINAL_ANSI_BRIGHT_GREEN,
        Palette::TERMINAL_ANSI_BRIGHT_YELLOW, Palette::TERMINAL_ANSI_BRIGHT_BLUE, Palette::TERMINAL_ANSI_BRIGHT_MAGENTA,
        Palette::TERMINAL_ANSI_BRIGHT_CYAN, Palette::TERMINAL_ANSI_BRIGHT_WHITE,
    ];
    match i {
        0..=15 => theme[i as usize](),
        16..=231 => {
            let n = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            Color32::from_rgb(level(n / 36), level((n / 6) % 6), level(n % 6))
        }
        _ => {
            let g = 8 + (i - 232) * 10;
            Color32::from_rgb(g, g, g)
        }
    }
}

fn colour(c: vt100::Color, default: Color32) -> Color32 {
    match c {
        vt100::Color::Default => default,
        vt100::Color::Idx(i) => ansi(i),
        vt100::Color::Rgb(r, g, b) => Color32::from_rgb(r, g, b),
    }
}

/// `terminal.integrated.wordSeparators`' default: what ends a double-clicked
/// word.
const TERMINAL_WORD_SEPARATORS: &str = " ()[]{}',\"`─‘’“”|";

/// Terminal cells: Menlo 12px, 14px rows, 20px left padding.
pub const CELL_H: f32 = 14.0;
const PAD_LEFT: f32 = 20.0;
const PAD_TOP: f32 = 6.0;

fn terminal_view(ui: &mut Ui, painter: &egui::Painter, content: Rect, state: &mut PanelState, id: egui::Id, index: usize) {
    let is_active = index == state.active_terminal;
    let font = icons::editor_mono_font(12.0);
    let cell_w = ui.fonts_mut(|f| f.glyph_width(&font, 'W'));
    let area = Rect::from_min_max(Pos2::new(content.left() + PAD_LEFT - 1.0, content.top() + PAD_TOP), Pos2::new(content.right() - 14.0, content.bottom() - 2.0));
    let rows = ((area.height() / CELL_H).floor() as u16).max(1);
    let cols = ((area.width() / cell_w).floor() as u16).max(2);
    let tid = id.with(("terminal", index));
    let resp = ui.interact(content, tid, Sense::click_and_drag());
    if (resp.clicked() || resp.drag_started()) && !is_active {
        // Clicking another pane of the group makes it the active one.
        state.active_terminal = index;
        state.selection = None;
        state.focus_pending = true;
        return;
    }
    if resp.clicked() || resp.drag_started() {
        resp.request_focus();
    }
    // A terminal made active elsewhere (tabs list, split, new) takes the
    // keyboard once.
    if is_active && std::mem::take(&mut state.focus_pending) {
        resp.request_focus();
    }
    // Mouse → selection (xterm.js SelectionService: drag selects cells,
    // double click a word, a click clears).
    let cell_at = |p: Pos2| -> (u16, u16) {
        let c = ((p.x - area.left()) / cell_w).floor().clamp(0.0, (cols - 1) as f32) as u16;
        let r = ((p.y - area.top()) / CELL_H).floor().clamp(0.0, (rows - 1) as f32) as u16;
        (r, c)
    };
    if let Some(p) = resp.interact_pointer_pos().filter(|_| is_active) {
        if resp.drag_started() {
            // Anchor where the button went down, not where the drag began.
            let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(p);
            state.selection = Some((cell_at(origin), cell_at(p)));
        } else if resp.dragged() {
            if let Some((_, head)) = state.selection.as_mut() {
                *head = cell_at(p);
            }
        } else if resp.double_clicked() {
            state.selection = Some((cell_at(p), cell_at(p)));
            state.select_word = true;
        } else if resp.clicked() {
            state.selection = None;
        }
    }
    let focused = resp.has_focus();
    if is_active {
        state.terminal_focused = focused;
    }
    if focused {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(tid, egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true })
        });
    }
    let Some(term) = state.terminals.get_mut(index) else { return };
    term.resize(rows, cols);

    // Keyboard → the shell.
    if focused {
        let app_cursor = term.application_cursor();
        let events = ui.input(|i| i.events.clone());
        for ev in events {
            match ev {
                egui::Event::Copy => {
                    if let Some(text) = selected_text(term, state.selection) {
                        ui.ctx().copy_text(text);
                    }
                }
                egui::Event::Text(t) => {
                    state.selection = None;
                    term.write(t.as_bytes())
                }
                egui::Event::Paste(t) => term.paste(&t),
                egui::Event::Key { key, pressed: true, modifiers, .. } if !modifiers.mac_cmd && !modifiers.command => {
                    if let Some(bytes) = crate::terminal::key_bytes(key, modifiers, app_cursor) {
                        term.write(&bytes);
                    }
                }
                egui::Event::Key { key: egui::Key::K, pressed: true, modifiers, .. } if modifiers.command => {
                    // ⌘K: Terminal: Clear.
                    term.write(b"\x0c");
                }
                _ => {}
            }
        }
    }
    // Wheel → scrollback.
    if ui.rect_contains_pointer(content) {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            let lines = (dy / CELL_H).round() as isize;
            term.scroll_offset = (term.scroll_offset as isize + lines).max(0) as usize;
        }
    }

    // A double click widens the selection to the word under it.
    if std::mem::take(&mut state.select_word) {
        if let Some(((r, c), _)) = state.selection {
            let line: Vec<char> = term.with_screen(|s| (0..cols).map(|i| s.cell(r, i).map(|x| x.contents().chars().next().unwrap_or(' ')).unwrap_or(' ')).collect());
            let word = |ch: char| !ch.is_whitespace() && !TERMINAL_WORD_SEPARATORS.contains(ch);
            if word(line[c as usize]) {
                let mut a = c as usize;
                while a > 0 && word(line[a - 1]) {
                    a -= 1;
                }
                let mut b = c as usize;
                while b + 1 < line.len() && word(line[b + 1]) {
                    b += 1;
                }
                state.selection = Some(((r, a as u16), (r, b as u16)));
            }
        }
    }
    let selection = state.selection.filter(|_| is_active).map(|(a, b)| if a <= b { (a, b) } else { (b, a) });

    let fg_default = Palette::TERMINAL_FG();
    let bg_default = Palette::TERMINAL_BG();
    let scrolled = term.scroll_offset > 0;
    term.with_screen(|screen| {
        let (cur_row, cur_col) = screen.cursor_position();
        for r in 0..rows {
            let y = area.top() + r as f32 * CELL_H;
            let mut job = egui::text::LayoutJob::default();
            for c in 0..cols {
                let Some(cell) = screen.cell(r, c) else { continue };
                if cell.is_wide_continuation() {
                    continue;
                }
                let (mut fg, mut bg) = (colour(cell.fgcolor(), fg_default), colour(cell.bgcolor(), bg_default));
                if cell.bold() {
                    if let vt100::Color::Idx(i @ 0..=7) = cell.fgcolor() {
                        fg = ansi(i + 8);
                    }
                }
                if cell.inverse() {
                    std::mem::swap(&mut fg, &mut bg);
                }
                let x = area.left() + c as f32 * cell_w;
                if bg != bg_default {
                    let w = if cell.is_wide() { 2.0 } else { 1.0 };
                    painter.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::new(cell_w * w, CELL_H)), 0.0, bg);
                }
                let text = if cell.has_contents() { cell.contents() } else { " ".to_string() };
                job.append(
                    &text,
                    0.0,
                    egui::TextFormat {
                        font_id: font.clone(),
                        color: fg,
                        underline: if cell.underline() { Stroke::new(1.0, fg) } else { Stroke::NONE },
                        italics: cell.italic(),
                        line_height: Some(CELL_H),
                        ..Default::default()
                    },
                );
            }
            // Selection behind the text (`terminal.selectionBackground`).
            if let Some(((r0, c0), (r1, c1))) = selection {
                if r >= r0 && r <= r1 {
                    let from = if r == r0 { c0 } else { 0 };
                    let to = if r == r1 { c1 } else { cols - 1 };
                    let sel = Rect::from_min_max(
                        Pos2::new(area.left() + from as f32 * cell_w, y),
                        Pos2::new(area.left() + (to + 1) as f32 * cell_w, y + CELL_H),
                    );
                    painter.rect_filled(sel, 0.0, Palette::TERMINAL_SELECTION_BG());
                }
            }
            let g = ui.fonts_mut(|f| f.layout_job(job));
            painter.galley(Pos2::new(area.left(), y), g, fg_default);
        }
        // Cursor: a block while focused, an outline otherwise.
        if !screen.hide_cursor() && !scrolled {
            let r = Rect::from_min_size(Pos2::new(area.left() + cur_col as f32 * cell_w, area.top() + cur_row as f32 * CELL_H), Vec2::new(cell_w.ceil(), CELL_H));
            if focused {
                painter.rect_filled(r, 0.0, Palette::TERMINAL_CURSOR());
                if let Some(cell) = screen.cell(cur_row, cur_col).filter(|c| c.has_contents()) {
                    painter.text(r.left_top(), Align2::LEFT_TOP, cell.contents(), font.clone(), Palette::TERMINAL_BG());
                }
            } else {
                painter.rect_stroke(r, 0.0, Stroke::new(1.0, Palette::TERMINAL_CURSOR()), egui::StrokeKind::Inside);
            }
        }
    });
    let label = format!("Terminal {}", index + 1);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label.clone()));
}

/// The selected cells' text, lines joined with `\n` and trailing blanks
/// dropped (xterm.js `getSelection`).
fn selected_text(term: &mut Terminal, selection: Option<((u16, u16), (u16, u16))>) -> Option<String> {
    let (a, b) = selection?;
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    let text = term.with_screen(|s| s.contents_between(a.0, a.1, b.0, b.1 + 1));
    let text = text.lines().map(str::trim_end).collect::<Vec<_>>().join("\n");
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term() -> Terminal {
        Terminal::spawn(None, 24, 80, || {}).expect("shell")
    }

    #[test]
    fn output_wraps_on_spaces_keeping_the_indent() {
        assert_eq!(wrap_ranges("short", 20), vec![(0, 5)]);
        // "    at some function (file)" at 14 columns: breaks after spaces.
        let line = "    at some function (file)";
        let rows = wrap_ranges(line, 14);
        let chars: Vec<char> = line.chars().collect();
        assert!(rows.len() > 1);
        assert_eq!(rows.first().unwrap().0, 0);
        assert_eq!(rows.last().unwrap().1, chars.len());
        // Rows are contiguous and every one but the first fits in cols - indent.
        for w in rows.windows(2) {
            assert_eq!(w[0].1, w[1].0);
        }
        assert!(rows.iter().skip(1).all(|(a, b)| b - a <= 14 - 4));
    }

    #[test]
    fn output_filter_follows_the_box_syntax() {
        assert!(output_filter_passes("Hello World", ""));
        assert!(output_filter_passes("Hello World", "world"));
        assert!(!output_filter_passes("Hello World", "!hello"));
        assert!(output_filter_passes("Hello World", "foo,world"));
        assert!(!output_filter_passes("Hello World", "foo,bar"));
    }

    #[test]
    fn log_lines_are_tokenised_like_the_log_language() {
        let colours: Vec<(&'static str, Color32)> =
            vec![("date", Color32::GRAY), ("info", Color32::GREEN), ("error", Color32::RED), ("url", Color32::BLUE), ("constant", Color32::YELLOW)];
        let line = "2026-11-14 09:58:54.816 [error] [Network] #132: https://example.com/a - failed";
        let spans = log_spans(line, &colours);
        let text = |s: &(usize, usize, Color32)| line.chars().skip(s.0).take(s.1 - s.0).collect::<String>();
        assert_eq!(text(&spans[0]), "2026-11-14");
        assert_eq!(text(&spans[1]), "09:58:54.816");
        assert_eq!(text(&spans[2]), "[error]");
        assert_eq!(spans[2].2, Color32::RED);
        assert!(spans.iter().any(|s| text(s) == "132" && s.2 == Color32::YELLOW));
        assert!(spans.iter().any(|s| text(s) == "https://example.com/a" && s.2 == Color32::BLUE));
    }

    #[test]
    fn terminals_split_into_groups_and_close() {
        let mut s = PanelState::default();
        s.add_terminal(term(), false); // 0
        s.add_terminal(term(), false); // 1, its own group
        s.add_terminal(term(), true); // 2, split beside 1
        assert_eq!(s.groups, vec![vec![0], vec![1, 2]]);
        assert_eq!(s.active_terminal, 2);
        assert_eq!(s.active_group(), vec![1, 2]);
        // Killing the active split pane activates its neighbour.
        s.remove_terminal(2);
        assert_eq!(s.groups, vec![vec![0], vec![1]]);
        assert_eq!(s.active_terminal, 1);
        // Killing a whole group activates the previous group.
        s.remove_terminal(1);
        assert_eq!(s.groups, vec![vec![0]]);
        assert_eq!(s.active_terminal, 0);
        // Removing an earlier terminal renumbers the later ones.
        s.add_terminal(term(), true); // 1, split beside 0
        s.active_terminal = 1;
        s.remove_terminal(0);
        assert_eq!(s.groups, vec![vec![0]]);
        assert_eq!(s.active_terminal, 0);
    }
}
