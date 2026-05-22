//! Search view, modelled after VS Code's `searchView` /
//! `searchActionsTopBar` source.
//!
//! Layout (top → bottom):
//!   1. Section title row ("SEARCH") + action icons (refresh, clear, collapse)
//!   2. Search input row with three internal toggles (Aa, ab, .*)
//!   3. Result stats ("N results in M files")
//!   4. Tree of file-match rows + nested match-rows

use std::collections::HashSet;
use std::path::PathBuf;

use egui::{Color32, FontId, Key, RichText, ScrollArea, Sense, Stroke, Ui};

use crate::icons::{self, codicon_font};
use crate::theme::Palette;

use super::engine::{run, SearchOutcome, SearchQuery};

pub struct SearchState {
    pub query: String,
    pub match_case: bool,
    pub whole_word: bool,
    pub regex: bool,
    pub outcome: Option<SearchOutcome>,
    pub focus_input: bool,
    pub collapsed: HashSet<PathBuf>,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            match_case: false,
            whole_word: false,
            regex: false,
            outcome: None,
            focus_input: false,
            collapsed: HashSet::new(),
        }
    }
}

pub struct SearchOutput {
    pub navigate_to: Option<(PathBuf, usize, usize)>,
}

pub fn show(ui: &mut Ui, workspace_root: &Option<PathBuf>, state: &mut SearchState) -> SearchOutput {
    let mut out = SearchOutput { navigate_to: None };

    let mut needs_search = false;

    // Search input row
    ui.add_space(6.0);
    needs_search |= search_input_row(ui, state);
    ui.add_space(6.0);

    if workspace_root.is_none() {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            crate::vscode_widgets::forms::form_helper(
                ui,
                &crate::vscode_widgets::forms::FormHelperProps::new(
                    "You have not yet opened a folder.",
                ),
            );
        });
        return out;
    }

    if needs_search {
        execute(workspace_root.as_ref().unwrap(), state);
    }

    if let Some(o) = &state.outcome {
        if let Some(err) = &o.error {
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                let msg = format!("⚠ {err}");
                crate::vscode_widgets::forms::form_helper(
                    ui,
                    &crate::vscode_widgets::forms::FormHelperProps::new(&msg).error(),
                );
            });
            return out;
        }

        stats_row(ui, o);

        let root_clone = workspace_root.clone();
        ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                render_results(ui, &root_clone, o, &mut state.collapsed, &mut out);
            });
    }

    out
}

/// One-line search input that visually matches VS Code's monaco-inputbox:
/// a thin rectangular input with three borderless toggle icons aligned to the
/// right edge inside the input.
fn search_input_row(ui: &mut Ui, state: &mut SearchState) -> bool {
    let mut needs_search = false;
    let mut input_rect = egui::Rect::NOTHING;

    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let avail = ui.available_width() - 10.0;

        // VS Code-style input box.
        let input_h = 24.0;
        let toggle_zone = 78.0; // 3 toggles × 22 + padding
        let (full_rect, _) =
            ui.allocate_exact_size(egui::vec2(avail, input_h), Sense::hover());
        input_rect = full_rect;

        // Background + border
        ui.painter().rect(
            full_rect,
            0.0,
            Palette::INPUT_BG,
            Stroke::new(1.0, Palette::INPUT_BORDER),
            egui::StrokeKind::Inside,
        );

        // Text input occupies left portion
        let text_rect = egui::Rect::from_min_size(
            full_rect.min + egui::vec2(6.0, 0.0),
            egui::vec2(full_rect.width() - 6.0 - toggle_zone, full_rect.height()),
        );

        let resp = ui.allocate_new_ui(egui::UiBuilder::new().max_rect(text_rect), |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.query)
                    .desired_width(text_rect.width())
                    .hint_text(RichText::new("Search").color(Palette::INPUT_PLACEHOLDER))
                    .font(FontId::proportional(13.0))
                    .frame(egui::Frame::default().fill(Palette::INPUT_BG))
                    .vertical_align(egui::Align::Center),
            )
        });
        let edit_resp = resp.inner;

        if state.focus_input {
            edit_resp.request_focus();
            state.focus_input = false;
        }
        if edit_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            needs_search = true;
        }
        if edit_resp.changed() && state.query.is_empty() {
            state.outcome = None;
        }

        // Toggles aligned to right edge inside input
        let mut tx = full_rect.right() - 6.0;
        for (active_ptr, glyph, tip) in [
            (&mut state.regex, ".*", "Use Regular Expression"),
            (&mut state.whole_word, "ab", "Match Whole Word"),
            (&mut state.match_case, "Aa", "Match Case"),
        ] {
            let size = 20.0;
            let tg_rect = egui::Rect::from_min_size(
                egui::pos2(tx - size, full_rect.center().y - size / 2.0),
                egui::vec2(size, size),
            );
            tx -= size + 2.0;

            let id = ui.id().with(("search_toggle", glyph));
            let resp = ui.interact(tg_rect, id, Sense::click());

            let bg = if *active_ptr {
                Palette::INPUT_OPTION_ACTIVE_BG
            } else if resp.hovered() {
                Palette::LIST_HOVER_BG
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(tg_rect, 3.0, bg);
            if *active_ptr {
                ui.painter().rect_stroke(
                    tg_rect,
                    3.0,
                    Stroke::new(1.0, Palette::INPUT_OPTION_ACTIVE_BORDER),
                    egui::StrokeKind::Inside,
                );
            }
            ui.painter().text(
                tg_rect.center(),
                egui::Align2::CENTER_CENTER,
                glyph,
                FontId::proportional(11.0),
                Palette::FG,
            );

            let resp = resp.on_hover_text(tip);
            if resp.clicked() {
                *active_ptr = !*active_ptr;
                needs_search = true;
            }
        }
    });

    let _ = input_rect;
    needs_search
}

fn stats_row(ui: &mut Ui, o: &SearchOutcome) {
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let suffix = if o.truncated { " (truncated)" } else { "" };
        let label_text = if o.total_hits == 0 {
            "No results found.".to_string()
        } else {
            format!(
                "{} result{} in {} file{}{}",
                o.total_hits,
                plural(o.total_hits),
                o.results.len(),
                plural(o.results.len()),
                suffix,
            )
        };
        crate::vscode_widgets::forms::form_helper(
            ui,
            &crate::vscode_widgets::forms::FormHelperProps::new(&label_text).size(11.5),
        );
    });
    ui.add_space(2.0);
}

fn execute(root: &std::path::Path, state: &mut SearchState) {
    if state.query.trim().is_empty() {
        state.outcome = None;
        return;
    }
    let query = SearchQuery {
        text: state.query.clone(),
        match_case: state.match_case,
        whole_word: state.whole_word,
        regex: state.regex,
    };
    state.outcome = Some(run(root, &query));
}

fn render_results(
    ui: &mut Ui,
    workspace_root: &Option<PathBuf>,
    outcome: &SearchOutcome,
    collapsed: &mut HashSet<PathBuf>,
    out: &mut SearchOutput,
) {
    for fr in &outcome.results {
        let is_collapsed = collapsed.contains(&fr.path);
        let file_name = fr
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let parent_str = workspace_root
            .as_ref()
            .and_then(|root| fr.path.parent().and_then(|p| p.strip_prefix(root).ok()))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();

        // File row — chevron + file icon + name + folder + match badge
        let row_h = 22.0;
        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, 0.0, Palette::LIST_HOVER_BG);
        }
        let painter = ui.painter();
        let mid_y = rect.center().y;

        // Chevron
        let chev = if is_collapsed {
            icons::CHEVRON_RIGHT
        } else {
            icons::CHEVRON_DOWN
        };
        painter.text(
            egui::pos2(rect.left() + 10.0, mid_y),
            egui::Align2::LEFT_CENTER,
            chev.to_string(),
            codicon_font(12.0),
            Palette::FG_DESCRIPTION,
        );

        // File icon — Seti file-type glyph (matches the Explorer tree).
        if let Some((glyph, color)) = crate::file_icons::icon_for(&fr.path) {
            painter.text(
                egui::pos2(rect.left() + 28.0, mid_y),
                egui::Align2::LEFT_CENTER,
                glyph.to_string(),
                crate::file_icons::seti_font(15.0),
                color,
            );
        } else {
            painter.text(
                egui::pos2(rect.left() + 28.0, mid_y),
                egui::Align2::LEFT_CENTER,
                icons::FILE.to_string(),
                codicon_font(14.0),
                Palette::FG_DESCRIPTION,
            );
        }

        // File name
        let name_galley = painter.layout_no_wrap(
            file_name.clone(),
            FontId::proportional(13.0),
            Palette::FG,
        );
        let name_x = rect.left() + 48.0;
        painter.galley(
            egui::pos2(name_x, mid_y - name_galley.size().y / 2.0),
            name_galley.clone(),
            Palette::FG,
        );

        // Folder path (greyed out, smaller, after the name)
        let folder_x = name_x + name_galley.size().x + 8.0;
        if !parent_str.is_empty() {
            painter.text(
                egui::pos2(folder_x, mid_y),
                egui::Align2::LEFT_CENTER,
                parent_str,
                FontId::proportional(11.5),
                Palette::FG_DESCRIPTION,
            );
        }

        // Match count badge — VS Code uses a small rounded "monaco-count-badge"
        let count_text = format!("{}", fr.hits.len());
        let badge_w = (count_text.chars().count() as f32 * 6.5 + 12.0).max(20.0);
        let badge_h = 16.0;
        let badge_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - badge_w - 8.0, mid_y - badge_h / 2.0),
            egui::vec2(badge_w, badge_h),
        );
        ui.painter().rect_filled(
            badge_rect,
            badge_h / 2.0,
            Palette::BADGE_BG,
        );
        ui.painter().text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            count_text,
            FontId::proportional(11.0),
            Palette::FG,
        );

        if resp.clicked() {
            if is_collapsed {
                collapsed.remove(&fr.path);
            } else {
                collapsed.insert(fr.path.clone());
            }
        }

        if !is_collapsed {
            for hit in &fr.hits {
                draw_match_row(ui, fr.path.clone(), hit, out);
            }
        }
    }
}

fn draw_match_row(
    ui: &mut Ui,
    file: PathBuf,
    hit: &super::engine::SearchHit,
    out: &mut SearchOutput,
) {
    let row_h = 22.0;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 0.0, Palette::LIST_HOVER_BG);
    }
    let painter = ui.painter();
    let mid_y = rect.center().y;

    // Indent to align under file name
    let indent_x = rect.left() + 36.0;

    // Highlight the matched range inside the preview (engine gives us the
    // preview-relative byte offset + length).
    let preview = hit.preview.clone();
    let highlight_start = if hit.match_len > 0 && hit.preview_start <= preview.len() {
        Some(hit.preview_start)
    } else {
        None
    };
    let highlight_len = hit.match_len;
    let font = crate::icons::editor_mono_font(12.0);

    if let Some(start) = highlight_start {
        let before = &preview[..start];
        let mat = &preview[start..start + highlight_len.min(preview.len() - start)];
        let after = &preview[start + mat.len()..];

        let g_before = painter.layout_no_wrap(before.to_string(), font.clone(), Palette::FG);
        let g_match = painter.layout_no_wrap(mat.to_string(), font.clone(), Palette::FG);
        let g_after = painter.layout_no_wrap(after.to_string(), font.clone(), Palette::FG);

        let mut x = indent_x;
        painter.galley(
            egui::pos2(x, mid_y - g_before.size().y / 2.0),
            g_before.clone(),
            Palette::FG,
        );
        x += g_before.size().x;

        // Match highlight background (VS Code uses #ffd70060 on dark)
        let match_rect = egui::Rect::from_min_size(
            egui::pos2(x, mid_y - g_match.size().y / 2.0),
            egui::vec2(g_match.size().x, g_match.size().y),
        );
        painter.rect_filled(match_rect, 2.0, Palette::SEARCH_MATCH_BG);
        painter.galley(
            egui::pos2(x, mid_y - g_match.size().y / 2.0),
            g_match.clone(),
            Palette::FG,
        );
        x += g_match.size().x;

        painter.galley(
            egui::pos2(x, mid_y - g_after.size().y / 2.0),
            g_after.clone(),
            Palette::FG,
        );
    } else {
        painter.text(
            egui::pos2(indent_x, mid_y),
            egui::Align2::LEFT_CENTER,
            &preview,
            font,
            Palette::FG,
        );
    }

    if resp.clicked() {
        out.navigate_to = Some((file, hit.line, hit.byte_start));
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
