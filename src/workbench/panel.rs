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

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::icons;
use crate::terminal::Terminal;
use crate::theme::Palette;

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
pub fn show(ui: &mut Ui, rect: Rect, state: &mut PanelState, problems: usize, cwd_label: &str) -> PanelOutput {
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
    for view in PanelView::ALL {
        let g = painter.layout_no_wrap(view.label().to_string(), FontId::proportional(13.0), Color32::PLACEHOLDER);
        let r = Rect::from_min_size(Pos2::new(x, ty), Vec2::new(g.size().x + 16.0, 24.0));
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
        painter.galley(Pos2::new(r.left() + 8.0, r.center().y - g.size().y / 2.0), g, colour);
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
        PanelView::Output => &[("ellipsis", "More Actions..."), ("unlock", "Turn Auto Scrolling Off"), ("clear-all", "Clear Output")],
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
            painter.text(Pos2::new(dd.left() + 7.0, dd.center().y), Align2::LEFT_CENTER, "Tasks", FontId::proportional(12.0), Palette::FG());
            painter.text(Pos2::new(dd.right() - 12.0, dd.center().y), Align2::CENTER_CENTER, icons::codicon_by_name("chevron-down").unwrap_or('v').to_string(), icons::codicon_font(16.0), Palette::FG());
            let r = Rect::from_min_max(Pos2::new(card.right() - 518.0, ty), Pos2::new(card.right() - 318.0, ty + 24.0));
            filter_box(ui, &painter, r, &mut state.output_filter, "Filter (e.g. text, !excludeText, text1,text2)", false, id.with("filter-output"));
        }
        PanelView::Ports => {}
    }

    // Content.
    let content = Rect::from_min_max(Pos2::new(card.left() + 1.0, card.top() + 35.0), Pos2::new(card.right() - 1.0, card.bottom() - 1.0));
    match state.view {
        PanelView::Problems => {
            let text = if problems == 0 { "No problems have been detected in the workspace." } else { "Problems are shown in the editor (hover a squiggle)." };
            painter.text(Pos2::new(card.left() + 22.0, card.top() + 45.0), Align2::LEFT_CENTER, text, FontId::proportional(13.0), Palette::FG());
        }
        PanelView::Output => {}
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
