//! In-editor find widget (Cmd+F).
//! VS Code analogue: src/vs/editor/contrib/find/browser/findWidget.ts
//! Tokens: --vscode-editorWidget-background → Palette::EDITOR_WIDGET_BG
//!         --vscode-editorWidget-border     → Palette::EDITOR_WIDGET_BORDER
//!         --vscode-editor-findMatchBackground / findMatchHighlightBackground
//!
//! A small panel docked to the editor's top-right corner: query input, a
//! "N of M" match counter, previous / next arrows and a close button.
//! Matches are case-insensitive; the editor paints them via
//! `highlight::FindHighlight` (all matches washed, the current one brighter).

use egui::{Align2, Color32, Context, CornerRadius, FontId, Key, Order, Sense, Stroke,
    StrokeKind, TextEdit, Ui};

use crate::icons::{self, codicon_font};
use crate::theme::Palette;

use super::highlight::FindHighlight;

#[derive(Default)]
pub struct FindState {
    pub open: bool,
    pub query: String,
    /// Index of the current match within the computed match list.
    pub current: usize,
    request_focus: bool,
}

impl FindState {
    pub fn open(&mut self) {
        self.open = true;
        self.request_focus = true;
    }
    pub fn close(&mut self) {
        self.open = false;
    }
}

/// Case-insensitive byte ranges of `query` inside `text`.
pub fn compute_matches(text: &str, query: &str) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }
    match regex::RegexBuilder::new(&regex::escape(query))
        .case_insensitive(true)
        .build()
    {
        Ok(re) => re.find_iter(text).map(|m| (m.start(), m.end())).collect(),
        Err(_) => Vec::new(),
    }
}

pub struct FindResponse {
    /// Navigate the editor to this byte offset (start of the current match).
    pub goto: Option<usize>,
}

/// Render the widget (only when `state.open`). `matches` must be the ranges
/// computed from the active document's current text.
pub fn show(
    ctx: &Context,
    state: &mut FindState,
    matches: &[(usize, usize)],
) -> FindResponse {
    let mut out = FindResponse { goto: None };
    if !state.open {
        return out;
    }
    if state.current >= matches.len() {
        state.current = matches.len().saturating_sub(1);
    }

    // Escape closes the widget from anywhere while it is open.
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
        state.open = false;
        return out;
    }

    let panel_w = 355.0;
    egui::Area::new(egui::Id::new("editor_find_widget"))
        .order(Order::Foreground)
        // Docked to the editor's top-right: below title bar (35) + tab strip
        // (35) + breadcrumbs (22), clear of the minimap on the right.
        .anchor(Align2::RIGHT_TOP, egui::vec2(-14.0, 92.0))
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(panel_w, 33.0), Sense::hover());
            let p = ui.painter();
            p.rect(
                rect,
                CornerRadius { nw: 0, ne: 0, sw: 4, se: 4 },
                Palette::EDITOR_WIDGET_BG,
                Stroke::new(1.0, Palette::EDITOR_WIDGET_BORDER),
                StrokeKind::Inside,
            );

            // --- query input -------------------------------------------------
            let input_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left() + 8.0, rect.top() + 5.0),
                egui::vec2(150.0, 23.0),
            );
            ui.painter().rect(
                input_rect,
                CornerRadius::same(2),
                Palette::VSCE_INPUT_BG,
                Stroke::new(1.0, Palette::INPUT_BORDER),
                StrokeKind::Inside,
            );
            let edit_inner = input_rect.shrink2(egui::vec2(5.0, 2.0));
            let before = state.query.clone();
            let resp = ui
                .scope_builder(
                    egui::UiBuilder::new().max_rect(edit_inner).layout(*ui.layout()),
                    |ui| {
                        let r = ui.add(
                            TextEdit::singleline(&mut state.query)
                                .background_color(Color32::TRANSPARENT)
                                .desired_width(f32::INFINITY)
                                .hint_text("Find")
                                .font(FontId::proportional(12.5))
                                .text_color(Palette::FG),
                        );
                        if state.request_focus {
                            r.request_focus();
                            state.request_focus = false;
                        }
                        r
                    },
                )
                .inner;
            if state.query != before {
                state.current = 0;
                if let Some(&(s, _)) = matches.first() {
                    let _ = s; // first match will be highlighted; no auto-jump on type
                }
            }
            // Enter / Shift+Enter cycle matches while the input is focused.
            if resp.has_focus() && !matches.is_empty() {
                let (enter, shift) = ui.input(|i| {
                    (i.key_pressed(Key::Enter), i.modifiers.shift)
                });
                if enter {
                    if shift {
                        state.current =
                            (state.current + matches.len() - 1) % matches.len();
                    } else {
                        state.current = (state.current + 1) % matches.len();
                    }
                    out.goto = Some(matches[state.current].0);
                    resp.request_focus();
                }
            }

            // --- match counter -----------------------------------------------
            let count_text = if state.query.is_empty() {
                String::new()
            } else if matches.is_empty() {
                "No results".to_string()
            } else {
                format!("{} of {}", state.current + 1, matches.len())
            };
            let count_color = if matches.is_empty() && !state.query.is_empty() {
                Palette::GIT_DELETED_FG
            } else {
                Palette::FG_DESCRIPTION
            };
            ui.painter().text(
                egui::pos2(input_rect.right() + 10.0, rect.center().y),
                Align2::LEFT_CENTER,
                count_text,
                FontId::proportional(11.5),
                count_color,
            );

            // --- prev / next / close buttons ---------------------------------
            let mut bx = rect.right() - 22.0;
            let buttons: [(char, &str); 3] = [
                (icons::CLOSE, "Close (Escape)"),
                (icons::ARROW_DOWN, "Next Match (Enter)"),
                (icons::ARROW_UP, "Previous Match (⇧Enter)"),
            ];
            for (i, (glyph, tip)) in buttons.into_iter().enumerate() {
                let r = egui::Rect::from_center_size(
                    egui::pos2(bx, rect.center().y),
                    egui::vec2(22.0, 22.0),
                );
                let bresp = ui.interact(r, ui.id().with(("findbtn", i)), Sense::click());
                let enabled = i == 0 || !matches.is_empty();
                let fg = if !enabled {
                    Palette::FG_DESCRIPTION.gamma_multiply(0.5)
                } else if bresp.hovered() {
                    Palette::FG_BRIGHT
                } else {
                    Palette::FG
                };
                if bresp.hovered() && enabled {
                    ui.painter()
                        .rect_filled(r, CornerRadius::same(3), Palette::LIST_HOVER_BG);
                }
                ui.painter().text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    glyph.to_string(),
                    codicon_font(14.0),
                    fg,
                );
                if bresp.clicked() && enabled {
                    match i {
                        0 => state.open = false,
                        1 => {
                            state.current = (state.current + 1) % matches.len();
                            out.goto = Some(matches[state.current].0);
                        }
                        2 => {
                            state.current =
                                (state.current + matches.len() - 1) % matches.len();
                            out.goto = Some(matches[state.current].0);
                        }
                        _ => {}
                    }
                }
                bresp.on_hover_text(tip);
                bx -= 24.0;
            }
        });

    out
}

/// Build the editor highlight decoration from the current widget state.
pub fn highlight_for(state: &FindState, matches: &[(usize, usize)]) -> Option<FindHighlight> {
    if !state.open || matches.is_empty() {
        return None;
    }
    Some(FindHighlight {
        ranges: matches.to_vec(),
        current: state.current.min(matches.len() - 1),
    })
}
