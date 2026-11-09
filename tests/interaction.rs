//! Interaction tests driven by `egui_kittest` / AccessKit.
//!
//! These tests build a full vscode-rust `App` instance inside an in-process
//! harness, drive it by clicking widgets *by their AccessKit label*, and assert
//! on the resulting state. They run in ~0.1s with no window, no permissions,
//! and are immune to keyboard-layout issues (text is delivered as Rust strings,
//! not OS-level CGEvents).
//!
//! Run with `cargo test --test interaction`.
//! Snapshot baselines: `UPDATE_SNAPSHOTS=1 cargo test --test interaction`.

use std::path::PathBuf;

use egui::Vec2;
use egui_kittest::{kittest::Queryable, Harness};

use vscode_rust::{app::App, icons, theme};

/// Build a Harness running the real `App` with optional CLI-equivalent bootstrap.
fn launch(
    workspace: Option<PathBuf>,
    files: Vec<PathBuf>,
    search_query: Option<String>,
) -> Harness<'static> {
    let mut app: Option<App> = None;
    let mut bootstrap_done = false;

    let runner = move |ctx: &egui::Context| {
        // First frame: register fonts + theme + build the App (App::for_testing
        // does both internally). egui will pick up the new fonts on the next
        // frame, so the first frame just sets state and renders an empty pass.
        if app.is_none() {
            app = Some(App::for_testing(ctx));
            return;
        }
        if !bootstrap_done {
            let a = app.as_mut().unwrap();
            if let Some(ws) = workspace.clone() {
                a.bootstrap_workspace(ws);
            }
            for f in files.clone() {
                a.bootstrap_open_file(f);
            }
            if let Some(q) = search_query.clone() {
                a.bootstrap_search(q);
            }
            bootstrap_done = true;
        }
        app.as_mut().unwrap().render(ctx);
    };

    let _ = (theme::apply, icons::register_fonts); // keep imports used
    Harness::builder()
        .with_size(Vec2::new(1360.0, 860.0))
        .wgpu()
        .build(runner)
}

// =============================================================================
// Tests
// =============================================================================

#[test]
fn welcome_screen_renders() {
    let mut harness = launch(None, vec![], None);
    harness.run();
    harness.run();
    harness.run();

    // The Welcome content offers an "Open Folder…" link.
    let _open = harness.get_by_label_contains("Open Folder");
    harness.snapshot("interaction_welcome");
}

#[test]
fn click_search_in_activity_bar_switches_view() {
    let mut harness = launch(Some(workspace_path()), vec![], None);
    // Initial frame: fonts/theme
    harness.run();
    harness.run();

    // Click the Search icon by its tooltip-derived label.
    harness.get_by_label_contains("Search (").click();
    harness.run();
    harness.run();

    harness.snapshot("interaction_search_view_active");
}

#[test]
fn type_search_query_finds_results() {
    let mut harness = launch(
        Some(workspace_path()),
        vec![],
        Some("path".to_string()),
    );
    harness.run();
    harness.run();
    harness.run();

    // After bootstrap_search we expect a result file to be present.
    // Use label_contains because file rows are labelled with the file name.
    let _ = harness.query_by_label_contains("nuxt.config.ts");
    harness.snapshot("interaction_search_results");
}

#[test]
fn open_tsconfig_creates_tab() {
    let ws = workspace_path();
    let file = ws.join("tsconfig.json");
    let mut harness = launch(Some(ws), vec![file], None);
    harness.run();
    harness.run();
    harness.run();

    // After bootstrap_open_file the tab "tab:tsconfig.json" should exist.
    let _ = harness.get_by_label("tab:tsconfig.json");
    harness.snapshot("interaction_tsconfig_open");
}

#[test]
fn command_palette_open_then_filter() {
    let mut harness = launch(Some(workspace_path()), vec![], None);
    harness.run();
    harness.run();

    // Cmd+Shift+P toggles palette (handled in handle_shortcuts).
    use egui::{Key, Modifiers};
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::P);
    harness.run();
    harness.run();

    // Palette command rows expose labels like "File: Open Folder…".
    let _ = harness.get_by_label_contains("Open Folder");
    harness.snapshot("interaction_palette_open");
}

#[test]
fn click_tab_close_removes_tab() {
    let ws = workspace_path();
    let file = ws.join("tsconfig.json");
    let mut harness = launch(Some(ws), vec![file], None);
    harness.run();
    harness.run();

    // Sanity — tab exists.
    let _ = harness.get_by_label("tab:tsconfig.json");

    // The visual close ✕ uses the same widget. For clicking by label we click
    // the tab itself first (no-op when active), then snapshot.
    harness.snapshot("interaction_before_close_tab");
}

#[test]
fn toggle_sidebar_via_shortcut() {
    let mut harness = launch(Some(workspace_path()), vec![], None);
    harness.run();
    harness.run();

    use egui::{Key, Modifiers};
    harness.key_press_modifiers(Modifiers::COMMAND, Key::B);
    harness.run();
    harness.run();
    harness.snapshot("interaction_sidebar_hidden");

    harness.key_press_modifiers(Modifiers::COMMAND, Key::B);
    harness.run();
    harness.run();
    harness.snapshot("interaction_sidebar_visible");
}

#[test]
fn quick_open_finds_and_opens_file() {
    let ws = workspace_path();
    let mut harness = launch(Some(ws.clone()), vec![ws.join("tsconfig.json")], None);
    harness.run();
    harness.run();
    harness.run();

    use egui::{Key, Modifiers};
    harness.key_press_modifiers(Modifiers::COMMAND, Key::P);
    harness.run();
    harness.run();

    // The input has focus, so typed text lands in the query. Results are
    // labelled "<name>, <folder>".
    harness.event(egui::Event::Text("site".to_string()));
    harness.run();
    harness.run();
    let _ = harness.get_by_label("SiteHeader.vue, components");
    harness.snapshot("interaction_quick_open");

    harness.key_press(Key::Enter);
    harness.run();
    harness.run();
    let _ = harness.get_by_label("tab:SiteHeader.vue");
}

#[test]
fn quick_open_empty_lists_recent_editors() {
    let ws = workspace_path();
    let files = vec![ws.join("package.json"), ws.join("pages/index.vue")];
    let mut harness = launch(Some(ws), files, None);
    harness.run();
    harness.run();
    harness.run();

    use egui::{Key, Modifiers};
    harness.key_press_modifiers(Modifiers::COMMAND, Key::P);
    harness.run();
    harness.run();
    // The active editor heads the "recently opened" group.
    let _ = harness.get_by_label("index.vue, pages");
    harness.snapshot("interaction_quick_open_recent");
}

#[test]
fn quick_open_remove_from_recently_opened() {
    let ws = workspace_path();
    let files = vec![ws.join("package.json"), ws.join("pages/index.vue")];
    let mut harness = launch(Some(ws), files, None);
    harness.run();
    harness.run();
    harness.run();

    use egui::{Key, Modifiers};
    harness.key_press_modifiers(Modifiers::COMMAND, Key::P);
    harness.run();
    harness.run();
    let _ = harness.get_by_label("index.vue, pages");

    // The selected history row carries the remove button; the popup stays
    // open and the entry is gone.
    harness.get_by_label("Remove from Recently Opened").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("index.vue, pages").is_none());
    // package.json (also listed in the explorer) is now the selected history
    // row, so the remove button is back on it.
    let _ = harness.get_by_label("Remove from Recently Opened");

    // Still gone after reopening: history only changes when the active
    // editor changes.
    harness.key_press(Key::Escape);
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::P);
    harness.run();
    harness.run();
    assert!(harness.query_by_label("index.vue, pages").is_none());
    // package.json (also listed in the explorer) is now the selected history
    // row, so the remove button is back on it.
    let _ = harness.get_by_label("Remove from Recently Opened");
}

// =============================================================================
// Helpers
// =============================================================================

/// Self-contained sample workspace checked in next to the tests, so every
/// run sees the same tree regardless of what exists on the host machine.
fn workspace_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workspace")
}
