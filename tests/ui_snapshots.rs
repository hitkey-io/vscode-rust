//! Regression snapshot tests for the workbench components.
//!
//! Run `UPDATE_SNAPSHOTS=1 cargo test --test ui_snapshots` once to generate
//! baseline PNGs under `../test-artifacts/snapshots/` (path configured in
//! `kittest.toml`). Subsequent `cargo test` runs compare freshly rendered
//! output against those baselines and fail on diff.

use std::path::PathBuf;

use egui::{CentralPanel, FontId, Vec2};
use egui_kittest::Harness;

use vscode_rust::app::welcome_screen;
use vscode_rust::editor::Document;
use vscode_rust::fs::FileNode;
use vscode_rust::search::SearchState;
use vscode_rust::workbench::command_palette::CommandPaletteState;
use vscode_rust::workbench::{
    activity_bar, command_palette, sidebar, status_bar, tabs, ActivityView,
};
use vscode_rust::{icons, theme};

fn setup(ctx: &egui::Context) {
    icons::register_fonts(ctx);
    theme::apply(ctx);
}

/// Wrap a draw closure so the first frame only registers fonts/theme
/// (egui applies new fonts on the *next* frame), and the actual UI is
/// drawn from frame 2 onwards. Combine with `harness.run_steps(3)`.
fn with_setup<F: FnMut(&egui::Context) + 'static>(
    mut draw: F,
) -> impl FnMut(&egui::Context) + 'static {
    let mut initialized = false;
    move |ctx: &egui::Context| {
        if !initialized {
            setup(ctx);
            initialized = true;
            return;
        }
        draw(ctx);
    }
}

fn make_doc(name: &str, contents: &str, dirty: bool) -> Document {
    let path = PathBuf::from(format!("/tmp/{name}"));
    let mut doc = Document {
        saved_text: contents.to_string(),
        text: contents.to_string(),
        dirty,
        language: "rs",
        path,
        cursor_line: 1,
        cursor_col: 1,
        pending_nav: None,
        pinned: false,
        folded: std::collections::BTreeSet::new(),
        diff_base: None,
        diff_title: None,
    };
    if dirty {
        doc.text.push_str("\n// edited");
        doc.check_dirty();
    }
    doc
}

fn mock_tree() -> Option<FileNode> {
    let mut root = FileNode {
        path: PathBuf::from("/tmp/my-project"),
        name: "my-project".into(),
        is_dir: true,
        expanded: true,
        children: Some(Vec::new()),
    };
    let src = FileNode {
        path: PathBuf::from("/tmp/my-project/src"),
        name: "src".into(),
        is_dir: true,
        expanded: true,
        children: Some(vec![
            FileNode {
                path: PathBuf::from("/tmp/my-project/src/main.rs"),
                name: "main.rs".into(),
                is_dir: false,
                expanded: false,
                children: None,
            },
            FileNode {
                path: PathBuf::from("/tmp/my-project/src/lib.rs"),
                name: "lib.rs".into(),
                is_dir: false,
                expanded: false,
                children: None,
            },
        ]),
    };
    let readme = FileNode {
        path: PathBuf::from("/tmp/my-project/README.md"),
        name: "README.md".into(),
        is_dir: false,
        expanded: false,
        children: None,
    };
    let cargo = FileNode {
        path: PathBuf::from("/tmp/my-project/Cargo.toml"),
        name: "Cargo.toml".into(),
        is_dir: false,
        expanded: false,
        children: None,
    };
    if let Some(children) = root.children.as_mut() {
        children.push(src);
        children.push(cargo);
        children.push(readme);
    }
    Some(root)
}

#[test]
fn activity_bar_explorer_selected() {
    let mut current = ActivityView::Explorer;
    let mut visible = true;

    let app = with_setup(move |ctx| {
        CentralPanel::default()
            .frame(egui::Frame::default().fill(theme::Palette::ACTIVITY_BAR_BG))
            .show(ctx, |ui| {
                activity_bar::show(ui, &mut current, &mut visible, 0);
            });
    });
    let mut harness = Harness::builder()
        .with_size(Vec2::new(48.0, 400.0))
        .wgpu()
        .build(app);
    harness.run_steps(3);
    harness.snapshot("activity_bar_explorer");
}

#[test]
fn activity_bar_search_selected() {
    let mut current = ActivityView::Search;
    let mut visible = true;

    let app = with_setup(move |ctx| {
        CentralPanel::default()
            .frame(egui::Frame::default().fill(theme::Palette::ACTIVITY_BAR_BG))
            .show(ctx, |ui| {
                activity_bar::show(ui, &mut current, &mut visible, 0);
            });
    });
    let mut harness = Harness::builder()
        .with_size(Vec2::new(48.0, 400.0))
        .wgpu()
        .build(app);
    harness.run_steps(3);
    harness.snapshot("activity_bar_search");
}

#[test]
fn sidebar_explorer_with_tree() {
    let mut tree = mock_tree();
    let workspace = Some(PathBuf::from("/tmp/my-project"));
    let mut search = SearchState::default();

    let app = with_setup(move |ctx| {
        CentralPanel::default()
            .frame(egui::Frame::default().fill(theme::Palette::SIDEBAR_BG))
            .show(ctx, |ui| {
                let _ = sidebar::show(
                    ui,
                    ActivityView::Explorer,
                    &workspace,
                    &mut tree,
                    &mut search,
                    &vscode_rust::git::Model::default(),
                    &[],
                    None,
                    &mut vscode_rust::workbench::source_control::ScmUiState::default(),
                    &std::collections::BTreeMap::new(),
                    None,
                );
            });
    });
    let mut harness = Harness::builder()
        .with_size(Vec2::new(260.0, 500.0))
        .wgpu()
        .build(app);
    harness.run_steps(3);
    harness.snapshot("sidebar_explorer");
}

#[test]
fn sidebar_explorer_no_workspace() {
    let mut tree: Option<FileNode> = None;
    let workspace: Option<PathBuf> = None;
    let mut search = SearchState::default();

    let app = with_setup(move |ctx| {
        CentralPanel::default()
            .frame(egui::Frame::default().fill(theme::Palette::SIDEBAR_BG))
            .show(ctx, |ui| {
                let _ = sidebar::show(
                    ui,
                    ActivityView::Explorer,
                    &workspace,
                    &mut tree,
                    &mut search,
                    &vscode_rust::git::Model::default(),
                    &[],
                    None,
                    &mut vscode_rust::workbench::source_control::ScmUiState::default(),
                    &std::collections::BTreeMap::new(),
                    None,
                );
            });
    });
    let mut harness = Harness::builder()
        .with_size(Vec2::new(260.0, 500.0))
        .wgpu()
        .build(app);
    harness.run_steps(3);
    harness.snapshot("sidebar_no_workspace");
}

#[test]
fn tabs_strip() {
    let docs = vec![
        make_doc("main.rs", "fn main() {}", false),
        make_doc("lib.rs", "pub fn hello() {}", true),
        make_doc("README.md", "# Project", false),
    ];
    let active = Some(0);

    let app = with_setup(move |ctx| {
        CentralPanel::default()
            .frame(egui::Frame::default().fill(theme::Palette::TABS_STRIP_BG))
            .show(ctx, |ui| {
                let _ = tabs::show(ui, &docs, active, false);
            });
    });
    let mut harness = Harness::builder()
        .with_size(Vec2::new(900.0, 36.0))
        .wgpu()
        .build(app);
    harness.run_steps(3);
    harness.snapshot("tabs_strip");
}

#[test]
fn status_bar_with_doc() {
    let doc = make_doc("main.rs", "fn main() {}", false);
    let docs = vec![doc];

    let app = with_setup(move |ctx| {
        CentralPanel::default()
            .frame(egui::Frame::default().fill(theme::Palette::STATUS_BAR_BG))
            .show(ctx, |ui| {
                let _ = status_bar::show(ui, Some(&docs[0]), "Ready", false, None, 0, (0, 0), false);
            });
    });
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 22.0))
        .wgpu()
        .build(app);
    harness.run_steps(3);
    harness.snapshot("status_bar_with_doc");
}

#[test]
fn status_bar_no_doc() {
    let app = with_setup(move |ctx| {
        CentralPanel::default()
            .frame(egui::Frame::default().fill(theme::Palette::STATUS_BAR_BG))
            .show(ctx, |ui| {
                let _ = status_bar::show(ui, None, "", false, None, 0, (0, 0), false);
            });
    });
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 22.0))
        .wgpu()
        .build(app);
    harness.run_steps(3);
    harness.snapshot("status_bar_no_doc");
}

#[test]
fn command_palette_empty_query() {
    let mut state = CommandPaletteState::default();
    state.open();

    let app = with_setup(move |ctx| {
        CentralPanel::default()
            .frame(egui::Frame::default().fill(theme::Palette::EDITOR_BG))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("editor background")
                        .color(theme::Palette::FG_DESCRIPTION),
                );
            });
        let _ = command_palette::show(ctx, &mut state);
    });
    let mut harness = Harness::builder()
        .with_size(Vec2::new(960.0, 540.0))
        .wgpu()
        .build(app);
    harness.run_steps(4);
    harness.snapshot("command_palette_empty");
}

#[test]
fn full_app_welcome() {
    // Compose the whole workbench layout in welcome state (no workspace, no docs).
    let mut active_view = ActivityView::Explorer;
    let mut sidebar_visible = true;
    let mut tree: Option<FileNode> = None;
    let mut search = SearchState::default();
    let workspace: Option<PathBuf> = None;

    let app = with_setup(move |ctx| {
        use egui::{Frame, Margin, SidePanel, TopBottomPanel};

        // Title bar
        TopBottomPanel::top("test_title_bar")
            .exact_height(35.0)
            .frame(Frame::default().fill(theme::Palette::TITLE_BAR_BG).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let bottom_border = egui::Rect::from_min_size(
                    egui::pos2(rect.left(), rect.bottom() - 1.0),
                    egui::vec2(rect.width(), 1.0),
                );
                ui.painter().rect_filled(bottom_border, 0.0, theme::Palette::BORDER);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "vscode-rust",
                    egui::FontId::proportional(13.0),
                    theme::Palette::TITLE_BAR_FG,
                );
            });

        // Status bar
        TopBottomPanel::bottom("test_status_bar")
            .exact_height(22.0)
            .frame(Frame::default().fill(theme::Palette::STATUS_BAR_BG).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                let _ = status_bar::show(ui, None, "", false, None, 0, (0, 0), false);
            });

        // Activity bar
        SidePanel::left("test_activity_bar")
            .exact_width(48.0)
            .resizable(false)
            .frame(Frame::default().fill(theme::Palette::ACTIVITY_BAR_BG).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                activity_bar::show(ui, &mut active_view, &mut sidebar_visible, 0);
            });

        // Sidebar
        SidePanel::left("test_sidebar")
            .resizable(true)
            .min_width(170.0)
            .default_width(260.0)
            .frame(Frame::default().fill(theme::Palette::SIDEBAR_BG).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                let _ = sidebar::show(
                    ui,
                    active_view,
                    &workspace,
                    &mut tree,
                    &mut search,
                    &vscode_rust::git::Model::default(),
                    &[],
                    None,
                    &mut vscode_rust::workbench::source_control::ScmUiState::default(),
                    &std::collections::BTreeMap::new(),
                    None,
                );
            });

        // Central — welcome
        egui::CentralPanel::default()
            .frame(Frame::default().fill(theme::Palette::EDITOR_BG).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                let _ = welcome_screen(ui);
            });
    });
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 800.0))
        .wgpu()
        .build(app);
    harness.run_steps(4);
    harness.snapshot("full_app_welcome");
}

#[test]
fn full_app_with_doc_and_palette() {
    let mut active_view = ActivityView::Explorer;
    let mut sidebar_visible = true;
    let mut tree = mock_tree();
    let mut search = SearchState::default();
    let workspace = Some(PathBuf::from("/tmp/my-project"));

    let docs = vec![
        make_doc("main.rs", "fn main() {\n    println!(\"hello\");\n}\n", false),
        make_doc("lib.rs", "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n", true),
    ];
    let active = Some(0usize);

    let mut palette = CommandPaletteState::default();
    palette.open();
    palette.query = "open".to_string();

    let app = with_setup(move |ctx| {
        use egui::{Frame, Margin, SidePanel, TopBottomPanel};

        TopBottomPanel::top("title")
            .exact_height(35.0)
            .frame(Frame::default().fill(theme::Palette::TITLE_BAR_BG).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "main.rs — my-project — vscode-rust",
                    egui::FontId::proportional(13.0),
                    theme::Palette::TITLE_BAR_FG,
                );
            });

        TopBottomPanel::bottom("status")
            .exact_height(22.0)
            .frame(Frame::default().fill(theme::Palette::STATUS_BAR_BG).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                let _ = status_bar::show(ui, Some(&docs[0]), "", true, None, 0, (0, 0), false);
            });

        SidePanel::left("activity")
            .exact_width(48.0)
            .resizable(false)
            .frame(Frame::default().fill(theme::Palette::ACTIVITY_BAR_BG).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                activity_bar::show(ui, &mut active_view, &mut sidebar_visible, 0);
            });

        SidePanel::left("sidebar")
            .resizable(true)
            .default_width(260.0)
            .frame(Frame::default().fill(theme::Palette::SIDEBAR_BG).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                let _ = sidebar::show(ui, active_view, &workspace, &mut tree, &mut search, &vscode_rust::git::Model::default(), &[], None, &mut vscode_rust::workbench::source_control::ScmUiState::default(), &std::collections::BTreeMap::new(), None);
            });

        egui::CentralPanel::default()
            .frame(Frame::default().fill(theme::Palette::EDITOR_BG).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    let _ = ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 35.0),
                        egui::Layout::left_to_right(egui::Align::Min),
                        |ui| tabs::show(ui, &docs, active, false),
                    );
                    if let Some(idx) = active {
                        let doc = &docs[idx];
                        ui.painter().text(
                            ui.cursor().left_top() + egui::vec2(12.0, 12.0),
                            egui::Align2::LEFT_TOP,
                            &doc.text,
                            egui::FontId::monospace(13.5),
                            theme::Palette::FG,
                        );
                    }
                });
            });

        let _ = command_palette::show(ctx, &mut palette);
    });
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 800.0))
        .wgpu()
        .build(app);
    harness.run_steps(4);
    harness.snapshot("full_app_doc_palette");
}

#[test]
fn command_palette_filtered() {
    let mut state = CommandPaletteState::default();
    state.open();
    state.query = "save".to_string();

    let app = with_setup(move |ctx| {
        CentralPanel::default()
            .frame(egui::Frame::default().fill(theme::Palette::EDITOR_BG))
            .show(ctx, |ui| {
                let _ = ui.label(
                    egui::RichText::new("editor background")
                        .font(FontId::proportional(14.0))
                        .color(theme::Palette::FG_DESCRIPTION),
                );
            });
        let _ = command_palette::show(ctx, &mut state);
    });
    let mut harness = Harness::builder()
        .with_size(Vec2::new(960.0, 540.0))
        .wgpu()
        .build(app);
    harness.run_steps(4);
    harness.snapshot("command_palette_save");
}
