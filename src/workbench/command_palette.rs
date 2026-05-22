use egui::{
    Context, FontId, Frame, Key, Margin, Modifiers, Order, RichText, Sense, Stroke, Ui,
};

use crate::theme::Palette;
use crate::vscode_widgets::forms::{textfield, TextFieldProps};
use crate::vscode_widgets::primitives::{label, LabelProps};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum CommandId {
    OpenFolder,
    OpenFile,
    CloseFolder,
    Save,
    SaveAll,
    CloseFile,
    CloseAllFiles,
    ToggleSidebar,
    ShowExplorer,
    ShowSearch,
    Quit,
}

pub struct CommandEntry {
    pub id: CommandId,
    pub title: &'static str,
    pub category: &'static str,
    pub shortcut: Option<&'static str>,
}

pub const COMMANDS: &[CommandEntry] = &[
    CommandEntry {
        id: CommandId::OpenFolder,
        title: "Open Folder…",
        category: "File",
        shortcut: None,
    },
    CommandEntry {
        id: CommandId::OpenFile,
        title: "Open File…",
        category: "File",
        shortcut: None,
    },
    CommandEntry {
        id: CommandId::CloseFolder,
        title: "Close Folder",
        category: "File",
        shortcut: Some("⌘K F"),
    },
    CommandEntry {
        id: CommandId::Save,
        title: "Save",
        category: "File",
        shortcut: Some("⌘S"),
    },
    CommandEntry {
        id: CommandId::SaveAll,
        title: "Save All",
        category: "File",
        shortcut: Some("⌥⌘S"),
    },
    CommandEntry {
        id: CommandId::CloseFile,
        title: "Close Editor",
        category: "File",
        shortcut: Some("⌘W"),
    },
    CommandEntry {
        id: CommandId::CloseAllFiles,
        title: "Close All Editors",
        category: "File",
        shortcut: None,
    },
    CommandEntry {
        id: CommandId::ToggleSidebar,
        title: "Toggle Sidebar Visibility",
        category: "View",
        shortcut: Some("⌘B"),
    },
    CommandEntry {
        id: CommandId::ShowExplorer,
        title: "Show Explorer",
        category: "View",
        shortcut: Some("⇧⌘E"),
    },
    CommandEntry {
        id: CommandId::ShowSearch,
        title: "Show Search",
        category: "View",
        shortcut: Some("⇧⌘F"),
    },
    CommandEntry {
        id: CommandId::Quit,
        title: "Quit",
        category: "File",
        shortcut: Some("⌘Q"),
    },
];

#[derive(Default)]
pub struct CommandPaletteState {
    pub visible: bool,
    pub query: String,
    pub selected: usize,
    just_opened: bool,
}

impl CommandPaletteState {
    pub fn open(&mut self) {
        self.visible = true;
        self.query.clear();
        self.selected = 0;
        self.just_opened = true;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn toggle(&mut self) {
        if self.visible {
            self.close();
        } else {
            self.open();
        }
    }
}

pub fn show(ctx: &Context, state: &mut CommandPaletteState) -> Option<CommandId> {
    if !state.visible {
        return None;
    }

    let filtered: Vec<usize> = filter(&state.query);
    if state.selected >= filtered.len() {
        state.selected = filtered.len().saturating_sub(1);
    }

    // Handle nav keys BEFORE the TextEdit consumes them.
    let mut chosen: Option<CommandId> = None;
    ctx.input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::Escape) {
            state.visible = false;
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowDown) && !filtered.is_empty() {
            state.selected = (state.selected + 1).min(filtered.len() - 1);
        }
        if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
            state.selected = state.selected.saturating_sub(1);
        }
        if i.consume_key(Modifiers::NONE, Key::Enter) {
            if let Some(&idx) = filtered.get(state.selected) {
                chosen = Some(COMMANDS[idx].id);
                state.visible = false;
            }
        }
    });

    if !state.visible {
        return chosen;
    }

    let area = egui::Area::new(egui::Id::new("command_palette_area"))
        .order(Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 38.0))
        .show(ctx, |ui| {
            Frame::default()
                .fill(Palette::PANEL_BG)
                .stroke(Stroke::new(1.0, Palette::BORDER))
                .corner_radius(4)
                .shadow(egui::epaint::Shadow {
                    offset: [0, 4],
                    blur: 16,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(140),
                })
                .inner_margin(Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(600.0);
                    palette_input(ui, state);
                    ui.add_space(4.0);
                    palette_list(ui, &filtered, state, &mut chosen);
                });
        });

    if area.response.clicked_elsewhere() {
        state.visible = false;
    }

    chosen
}

fn palette_input(ui: &mut Ui, state: &mut CommandPaletteState) {
    let mut props =
        TextFieldProps::new().placeholder(">  Type the name of a command to run.");
    if state.just_opened {
        props = props.focused();
        state.just_opened = false;
    }
    let query_before = state.query.clone();
    let response = textfield(ui, &props, &mut state.query);
    if state.query != query_before {
        state.selected = 0;
    }
    let _ = response;
}

fn palette_list(
    ui: &mut Ui,
    filtered: &[usize],
    state: &mut CommandPaletteState,
    chosen: &mut Option<CommandId>,
) {
    if filtered.is_empty() {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            label(
                ui,
                &LabelProps::new("No matching commands").normal().description(),
            );
        });
        ui.add_space(6.0);
        return;
    }

    let max_visible = filtered.len().min(12);
    for (row, &cmd_idx) in filtered.iter().take(max_visible).enumerate() {
        let cmd = &COMMANDS[cmd_idx];
        let is_selected = row == state.selected;

        let row_h = 26.0;
        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), Sense::click());
        let label = format!("{}: {}", cmd.category, cmd.title);
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone())
        });

        if is_selected {
            ui.painter().rect_filled(rect, 3.0, Palette::SELECTION_BG);
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, 3.0, Palette::LIST_HOVER_BG);
        }

        let painter = ui.painter();
        painter.text(
            rect.left_center() + egui::vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            format!("{}: {}", cmd.category, cmd.title),
            FontId::proportional(13.0),
            Palette::FG,
        );

        if let Some(sc) = cmd.shortcut {
            painter.text(
                rect.right_center() - egui::vec2(10.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                sc,
                FontId::proportional(11.5),
                Palette::FG_DESCRIPTION,
            );
        }

        if resp.clicked() {
            *chosen = Some(cmd.id);
            state.visible = false;
        }
    }

    if filtered.len() > max_visible {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            let more = format!("… and {} more", filtered.len() - max_visible);
            label(ui, &LabelProps::new(&more).normal().description().size(11.5));
        });
    }
}

fn filter(query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return (0..COMMANDS.len()).collect();
    }
    let mut scored: Vec<(i32, usize)> = COMMANDS
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let label = format!("{} {}", c.category.to_lowercase(), c.title.to_lowercase());
            fuzzy_score(&label, &q).map(|s| (s, i))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, i)| i).collect()
}

fn fuzzy_score(haystack: &str, needle: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let h: Vec<char> = haystack.chars().collect();
    let n: Vec<char> = needle.chars().collect();
    let mut hi = 0usize;
    let mut ni = 0usize;
    let mut score = 0i32;
    let mut consecutive = 0i32;
    while hi < h.len() && ni < n.len() {
        if h[hi] == n[ni] {
            let mut delta = 10 + consecutive * 5;
            if hi == 0 || !h[hi - 1].is_alphanumeric() {
                delta += 15;
            }
            score += delta;
            consecutive += 1;
            ni += 1;
        } else {
            consecutive = 0;
            score -= 1;
        }
        hi += 1;
    }
    if ni == n.len() {
        Some(score)
    } else {
        None
    }
}
