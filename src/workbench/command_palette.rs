use egui::{
    Color32, Context, CornerRadius, FontId, Frame, Key, Margin, Modifiers, Order, Pos2, Sense,
    Stroke, StrokeKind, TextEdit, Ui,
};

use crate::theme::Palette;
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
    CommandEntry { id: CommandId::OpenFolder,    title: "Open Folder…",            category: "File", shortcut: None },
    CommandEntry { id: CommandId::OpenFile,      title: "Open File…",              category: "File", shortcut: None },
    CommandEntry { id: CommandId::CloseFolder,   title: "Close Folder",            category: "File", shortcut: Some("⌘K F") },
    CommandEntry { id: CommandId::Save,          title: "Save",                    category: "File", shortcut: Some("⌘S") },
    CommandEntry { id: CommandId::SaveAll,       title: "Save All",                category: "File", shortcut: Some("⌥⌘S") },
    CommandEntry { id: CommandId::CloseFile,     title: "Close Editor",            category: "File", shortcut: Some("⌘W") },
    CommandEntry { id: CommandId::CloseAllFiles, title: "Close All Editors",       category: "File", shortcut: None },
    CommandEntry { id: CommandId::ToggleSidebar, title: "Toggle Sidebar Visibility", category: "View", shortcut: Some("⌘B") },
    CommandEntry { id: CommandId::ShowExplorer,  title: "Show Explorer",           category: "View", shortcut: Some("⇧⌘E") },
    CommandEntry { id: CommandId::ShowSearch,    title: "Show Search",             category: "View", shortcut: Some("⇧⌘F") },
    CommandEntry { id: CommandId::Quit,          title: "Quit",                    category: "File", shortcut: Some("⌘Q") },
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
        // Pre-fill with ">" so the activator is visible in the input the way
        // VS Code does it when you press Cmd+Shift+P.
        self.query = ">".to_string();
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

    // Snapshot the just_opened flag now: palette_input (inside the area)
    // consumes it to grab focus, but the click-outside guard below must still
    // see it (otherwise the click that opened the palette gets re-evaluated as
    // "outside" and closes it immediately).
    let was_just_opened = state.just_opened;
    let area = egui::Area::new(egui::Id::new("command_palette_area"))
        .order(Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 38.0))
        .show(ctx, |ui| {
            Frame::default()
                .fill(Palette::PANEL_BG)
                .stroke(Stroke::new(1.0, Palette::BORDER))
                .corner_radius(6)
                .shadow(egui::epaint::Shadow {
                    offset: [0, 4],
                    blur: 16,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(140),
                })
                // No inner margin — the input and the list span the full width
                // of the container, like VS Code's Quick Pick. Padding is
                // applied per-row instead.
                .inner_margin(Margin::ZERO)
                .show(ui, |ui| {
                    ui.set_width(600.0);
                    palette_input(ui, state);
                    palette_list(ui, &filtered, state, &mut chosen);
                });
        });

    if !was_just_opened && area.response.clicked_elsewhere() {
        state.visible = false;
    }

    chosen
}

const INPUT_H: f32 = 32.0;
const ROW_H: f32 = 22.0;
const ROW_PAD_X: f32 = 10.0;
const MAX_VISIBLE_ROWS: usize = 16;

fn palette_input(ui: &mut Ui, state: &mut CommandPaletteState) {
    // Full-width single-line input that sits flush at the top of the palette.
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), INPUT_H), Sense::hover());

    // Place the TextEdit inside the row, vertically centred, with 10px of
    // horizontal breathing room on each side. The TextEdit has no frame of
    // its own — the palette's outer Frame is the only chrome.
    let pad_x = 10.0;
    let edit_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + pad_x, rect.center().y - 10.0),
        egui::vec2(rect.width() - pad_x * 2.0, 20.0),
    );

    let query_before = state.query.clone();
    let resp = ui.scope_builder(
        egui::UiBuilder::new().max_rect(edit_rect).layout(*ui.layout()),
        |ui| {
            let edit = TextEdit::singleline(&mut state.query)
                .background_color(egui::Color32::TRANSPARENT)
                .desired_width(f32::INFINITY)
                .hint_text("Type the name of a command to run.")
                .font(FontId::proportional(14.0))
                .text_color(Palette::FG);
            let resp = ui.add(edit);
            if state.just_opened {
                resp.request_focus();
                state.just_opened = false;
            }
            resp
        },
    );

    if state.query != query_before {
        state.selected = 0;
    }
    let _ = resp;

    // Hairline separator between the input and the list — gives VS Code's
    // two-section look without a heavy border.
    ui.painter().rect_filled(
        egui::Rect::from_min_size(
            egui::pos2(rect.left(), rect.bottom() - 1.0),
            egui::vec2(rect.width(), 1.0),
        ),
        0.0,
        Palette::BORDER,
    );
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
            ui.add_space(ROW_PAD_X);
            label(
                ui,
                &LabelProps::new("No matching commands").normal().description(),
            );
        });
        ui.add_space(6.0);
        return;
    }

    let max_visible = filtered.len().min(MAX_VISIBLE_ROWS);
    ui.add_space(4.0);
    for (row, &cmd_idx) in filtered.iter().take(max_visible).enumerate() {
        let cmd = &COMMANDS[cmd_idx];
        let is_selected = row == state.selected;

        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_H), Sense::click());
        let label = format!("{}: {}", cmd.category, cmd.title);
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone())
        });

        // Selection / hover fill — inset slightly so the rounded row sits
        // inside the container edges.
        let fill_rect = rect.shrink2(egui::vec2(4.0, 0.0));
        if is_selected {
            ui.painter()
                .rect_filled(fill_rect, 3.0, Palette::SELECTION_BG);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(fill_rect, 3.0, Palette::LIST_HOVER_BG);
        }

        // "{Category}: {Title}" in the foreground colour (VS Code's
        // commandsQuickAccess.ts renders these as a single string with no
        // colour split).
        ui.painter().text(
            egui::pos2(rect.left() + ROW_PAD_X, rect.center().y),
            egui::Align2::LEFT_CENTER,
            &label,
            FontId::proportional(13.0),
            Palette::FG,
        );

        // Right side: keybinding chips.
        if let Some(sc) = cmd.shortcut {
            let chip_fg = if is_selected { Palette::FG_BRIGHT } else { Palette::FG };
            paint_kbd(
                ui,
                egui::pos2(rect.right() - ROW_PAD_X, rect.center().y),
                sc,
                chip_fg,
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
            ui.add_space(ROW_PAD_X);
            let more = format!("… and {} more", filtered.len() - max_visible);
            label(ui, &LabelProps::new(&more).normal().description().size(11.5));
        });
    }
    ui.add_space(4.0);
}

/// Paint a shortcut string as a row of outlined kbd chips, right-aligned to
/// `anchor`. Each non-space character becomes its own chip (so `"⌘K F"` →
/// `[⌘][K]  [F]`). Returns the total width painted (left of `anchor`).
fn paint_kbd(ui: &Ui, anchor: Pos2, text: &str, color: Color32) -> f32 {
    let font = FontId::proportional(10.5);
    let p = ui.painter();
    let chip_h: f32 = 16.0;
    let chip_pad_x: f32 = 4.0;
    let chip_gap: f32 = 2.0;
    let gap_for_space: f32 = 4.0;

    // First pass: compute per-chip widths and the total.
    let mut items: Vec<(String, f32)> = Vec::new();
    for ch in text.chars() {
        if ch == ' ' {
            // A literal space is a chord separator (e.g. "⌘K F" → ⌘ K · F).
            items.push((String::new(), gap_for_space));
            continue;
        }
        let s = ch.to_string();
        let glyph_w = p.layout_no_wrap(s.clone(), font.clone(), color).size().x;
        let chip_w = (glyph_w + chip_pad_x * 2.0).max(18.0);
        items.push((s, chip_w));
    }
    let total: f32 = items.iter().map(|(_, w)| *w).sum::<f32>()
        + chip_gap * items.iter().filter(|(s, _)| !s.is_empty()).count().saturating_sub(1) as f32;

    // Second pass: paint left-to-right starting from anchor.x - total.
    let mut x = anchor.x - total;
    let y_top = anchor.y - chip_h / 2.0;
    let mut prev_was_chip = false;
    for (s, w) in items {
        if s.is_empty() {
            // gap between chord groups
            x += w;
            prev_was_chip = false;
            continue;
        }
        if prev_was_chip {
            x += chip_gap;
        }
        let chip = egui::Rect::from_min_size(egui::pos2(x, y_top), egui::vec2(w, chip_h));
        p.rect(
            chip,
            CornerRadius::same(3),
            Palette::COMMAND_CENTER_BG,
            Stroke::new(1.0, Palette::BORDER),
            StrokeKind::Inside,
        );
        p.text(chip.center(), egui::Align2::CENTER_CENTER, s, font.clone(), color);
        x += w;
        prev_was_chip = true;
    }
    total
}

fn filter(query: &str) -> Vec<usize> {
    // Strip the leading ">" activator before fuzzy matching — it's the mode
    // marker for the command palette, not part of the search term.
    let q = query.trim_start_matches('>').trim().to_lowercase();
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
