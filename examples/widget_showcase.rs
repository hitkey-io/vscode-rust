//! Interactive storybook for the [`vscode_widgets`] library.
//!
//! Run with: `cargo run --example widget_showcase`
//!
//! Layout (three panes):
//!   1. **Top strip**: shows the upstream docs URL of the selected widget.
//!   2. **Left rail** (240px): list of widgets, grouped by category. Phases
//!      0–4 use a flat selectable list; Phase 5 will swap in the real
//!      `vscode_widgets::composite::tree` once it lands (dogfood).
//!   3. **Right pane**: scrollable list of "state cards" — one per
//!      `StoryState` of the currently-selected widget.
//!
//! Each state card renders on a `Palette::EDITOR_BG` canvas so widgets are
//! tested against the realistic surface they'll inhabit in the workbench.

use eframe::egui;
use egui::{Color32, FontId, Frame, Margin, RichText, Sense, Stroke, Vec2};
use vscode_rust::theme::{self, Palette};
use vscode_rust::vscode_widgets::testing::{Category, Story, StoryState, STORIES};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 740.0])
            .with_title("vscode-rust · widget showcase"),
        ..Default::default()
    };

    eframe::run_native(
        "widget_showcase",
        options,
        Box::new(|cc| {
            vscode_rust::icons::register_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(ShowcaseApp::default()))
        }),
    )
}

#[derive(Default)]
struct ShowcaseApp {
    selected_idx: usize,
}

impl eframe::App for ShowcaseApp {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // UI is registered via panels in `update`, not via the root Ui.
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top URL strip.
        egui::TopBottomPanel::top("url_strip")
            .exact_height(32.0)
            .frame(
                Frame::default()
                    .fill(Palette::PANEL_BG)
                    .inner_margin(Margin::symmetric(12, 4)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let upstream = STORIES
                        .get(self.selected_idx)
                        .map(|s| s.upstream_url)
                        .unwrap_or("https://vscode-elements.github.io/");
                    ui.label(
                        RichText::new("vscode-elements ▸ ")
                            .color(Palette::FG_DESCRIPTION)
                            .font(FontId::proportional(12.0)),
                    );
                    let label = STORIES
                        .get(self.selected_idx)
                        .map(|s| s.display)
                        .unwrap_or("(no widgets registered)");
                    ui.label(
                        RichText::new(label)
                            .color(Palette::FG)
                            .font(FontId::proportional(12.5)),
                    );
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(
                                RichText::new(upstream)
                                    .color(Palette::FG_DESCRIPTION)
                                    .font(FontId::monospace(11.5)),
                            );
                        },
                    );
                });
            });

        // Left rail (flat list, phases 0–4).
        egui::SidePanel::left("rail")
            .exact_width(240.0)
            .resizable(false)
            .frame(
                Frame::default()
                    .fill(Palette::SIDEBAR_BG)
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                draw_rail(ui, &mut self.selected_idx);
            });

        egui::CentralPanel::default()
            .frame(
                Frame::default()
                    .fill(Palette::EDITOR_BG)
                    .inner_margin(Margin::ZERO),
            )
            .show(ctx, |ui| {
                draw_state_cards(ui, STORIES.get(self.selected_idx));
            });
    }
}

fn draw_rail(ui: &mut egui::Ui, selected: &mut usize) {
    if STORIES.is_empty() {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new("No widgets registered yet.")
                    .color(Palette::FG_DESCRIPTION)
                    .size(12.0),
            );
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new(
                    "Phase 0 is scaffolding only. Widgets land starting Phase 1 \
                     (icon, divider, label).",
                )
                .color(Palette::FG_DESCRIPTION)
                .size(11.0),
            );
        });
        return;
    }

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let mut current_category: Option<Category> = None;
        for (idx, story) in STORIES.iter().enumerate() {
            if Some(story.category) != current_category {
                current_category = Some(story.category);
                ui.add_space(if idx == 0 { 8.0 } else { 14.0 });
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(story.category.as_str().to_uppercase())
                            .color(Palette::FG_DESCRIPTION)
                            .size(11.0)
                            .strong(),
                    );
                });
                ui.add_space(4.0);
            }
            draw_rail_item(ui, story, idx, selected);
        }
    });
}

fn draw_rail_item(ui: &mut egui::Ui, story: &Story, idx: usize, selected: &mut usize) {
    let height = 24.0;
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        Sense::click(),
    );
    let is_selected = *selected == idx;
    let bg = if is_selected {
        Palette::LIST_ACTIVE_SELECTION_BG
    } else if resp.hovered() {
        Palette::LIST_HOVER_BG
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 0.0, bg);
    ui.painter().text(
        rect.left_center() + egui::vec2(20.0, 0.0),
        egui::Align2::LEFT_CENTER,
        story.display,
        FontId::proportional(12.5),
        if is_selected { Palette::FG } else { Palette::FG },
    );
    if resp.clicked() {
        *selected = idx;
    }
}

fn draw_state_cards(ui: &mut egui::Ui, story: Option<&Story>) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(20.0);
            let Some(story) = story else {
                center_message(
                    ui,
                    "Select a widget on the left to see its documented states.",
                );
                return;
            };
            if story.states.is_empty() {
                center_message(ui, "This widget has no states registered yet.");
                return;
            }

            ui.horizontal(|ui| {
                ui.add_space(28.0);
                ui.label(
                    RichText::new(story.display)
                        .color(Palette::FG)
                        .size(20.0)
                        .strong(),
                );
            });
            ui.add_space(20.0);

            for state in story.states {
                draw_state_card(ui, state);
                ui.add_space(20.0);
            }
        });
}

fn draw_state_card(ui: &mut egui::Ui, state: &StoryState) {
    let max_w = (ui.available_width() - 56.0).max(320.0);
    ui.horizontal(|ui| {
        ui.add_space(28.0);
        ui.allocate_ui_with_layout(
            Vec2::new(max_w, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.label(
                    RichText::new(state.name)
                        .color(Palette::FG)
                        .size(13.5)
                        .strong(),
                );
                if !state.caption.is_empty() {
                    ui.label(
                        RichText::new(state.caption)
                            .color(Palette::FG_DESCRIPTION)
                            .size(11.5),
                    );
                }
                ui.add_space(8.0);

                let pad = 12.0_f32;
                let canvas_w = (state.size.0 + pad * 2.0).min(max_w);
                let canvas_h = state.size.1 + pad * 2.0;
                // Constrain the Frame width by allocating a fixed-width
                // child UI before drawing. Otherwise Frame stretches to the
                // parent's full width and the bordered card looks huge for
                // tiny widgets like the icon.
                ui.allocate_ui(Vec2::new(canvas_w, canvas_h), |ui| {
                    Frame::default()
                        .fill(Palette::EDITOR_BG)
                        .stroke(Stroke::new(1.0, Palette::BORDER))
                        .inner_margin(Margin::same(pad as i8))
                        .show(ui, |ui| {
                            let inner =
                                Vec2::new(canvas_w - pad * 2.0, canvas_h - pad * 2.0);
                            ui.allocate_ui_with_layout(
                                inner,
                                egui::Layout::top_down(egui::Align::Center),
                                |ui| {
                                    ui.set_min_size(inner);
                                    ui.set_max_size(inner);
                                    (state.draw)(ui);
                                },
                            );
                        });
                });
            },
        );
    });
}

fn center_message(ui: &mut egui::Ui, text: &str) {
    ui.add_space(80.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(text)
                .color(Palette::FG_DESCRIPTION)
                .size(12.5),
        );
    });
}
