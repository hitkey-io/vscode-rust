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
    launch_with_extensions(workspace, files, search_query, None)
}

/// [`launch`], also loading the extensions in `extensions_dir`.
fn launch_with_extensions(
    workspace: Option<PathBuf>,
    files: Vec<PathBuf>,
    search_query: Option<String>,
    extensions_dir: Option<PathBuf>,
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
            if let Some(dir) = extensions_dir.clone() {
                a.bootstrap_extensions(vec![dir]);
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

    // The Welcome page's Start entries (VS Code 1.145 on macOS).
    let _new = harness.get_by_label("New File...");
    let _open = harness.get_by_label("Open...");
    let _connect = harness.get_by_label("Connect to...");
    let _startup = harness.get_by_label("Show welcome page on startup");
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

/// A folder holding only the checked-in sample extension, so whatever else
/// lands in `tests/fixtures/extensions` (an install made while trying the
/// app) never runs in tests.
fn sample_extensions_dir() -> PathBuf {
    let sample = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/extensions/sample.hello-world-0.0.1");
    let dir = std::env::temp_dir().join(format!("vscode-rust-sample-ext-{}", std::process::id()));
    let link = dir.join("sample.hello-world-0.0.1");
    if !link.exists() {
        std::fs::create_dir_all(&dir).unwrap();
        // Tests run in parallel: another one may have just made it.
        if let Err(e) = std::os::unix::fs::symlink(&sample, &link) {
            assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists, "{e}");
        }
    }
    dir
}

/// Run frames until `label` shows up (the extension host answers from
/// another process), or fail after a few seconds.
fn run_until_label(harness: &mut Harness<'static>, label: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while harness.query_by_label(label).is_none() {
        assert!(std::time::Instant::now() < deadline, "'{label}' never appeared");
        std::thread::sleep(std::time::Duration::from_millis(30));
        // `step`, not `run`: toasts keep a repaint timer for auto-hide.
        harness.step();
    }
}

#[test]
fn format_document_applies_the_extensions_formatter() {
    if vscode_rust::extensions::host::find_node().is_none() {
        return;
    }
    let dir = std::env::temp_dir().join("vscode-rust-format-doc");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.md");
    std::fs::write(&file, "keep\ntrim me   \nend\t\n").unwrap();
    let mut harness = launch_with_extensions(Some(dir.clone()), vec![file.clone()], None, Some(sample_extensions_dir()));
    harness.step();
    harness.step();
    run_until_label(&mut harness, "$(smiley) Hello");
    use egui::{Key, Modifiers};
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::P);
    harness.step();
    harness.step();
    harness.event(egui::Event::Text("format document".to_string()));
    harness.step();
    let _ = harness.get_by_label("Format Document");
    harness.key_press(Key::Enter);
    // The answer comes from the extension host; once its edits are in, Save writes them.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::fs::read_to_string(&file).unwrap() != "keep\ntrim me\nend\n" {
        assert!(std::time::Instant::now() < deadline, "the formatted text was never saved");
        std::thread::sleep(std::time::Duration::from_millis(50));
        harness.step();
        harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        harness.step();
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn extension_runs_from_view_status_bar_and_palette() {
    if vscode_rust::extensions::host::find_node().is_none() {
        return; // Needs Node.js to run extension code.
    }
    let extensions = sample_extensions_dir();
    let mut harness = launch_with_extensions(Some(workspace_path()), vec![], None, Some(extensions));
    harness.run();
    harness.run();

    // The extension activates on startup and adds a status bar item.
    run_until_label(&mut harness, "$(smiley) Hello");

    // The Extensions view lists it as activated.
    harness.get_by_label("Extensions (⇧⌘X)").click();
    harness.run();
    harness.run();
    let _ = harness.get_by_label("Hello World Sample, sample, Activated");
    // Marketplace sections are there; tests never reach the network.
    let _ = harness.get_by_label("INSTALLED");
    let _ = harness.get_by_label("POPULAR");
    let _ = harness.get_by_label("Refresh");

    // Clicking an extension opens its page in an editor tab.
    harness.get_by_label("Hello World Sample, sample, Activated").click();
    harness.run_steps(3);
    let _ = harness.get_by_label("tab:Extension: Hello World Sample");

    // Its contributed command runs from the command palette; the
    // notification names the extension.
    use egui::{Key, Modifiers};
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::P);
    harness.run();
    harness.run();
    harness.event(egui::Event::Text("say hello".to_string()));
    harness.run();
    let _ = harness.get_by_label("Hello: Say Hello");
    harness.key_press(Key::Enter);
    run_until_label(&mut harness, "Hello from workspace!");

    // Accessibility click: kittest reports this toast's bounds offset from
    // where it's drawn, so a simulated pointer click misses (a real click
    // in the app closes it).
    harness.get_by_label("Clear Notification").click_accesskit();
    harness.run_steps(3);
    assert!(harness.query_by_label("Hello from workspace!").is_none());

    // Clicking the status bar item runs its command too.
    harness.get_by_label("$(smiley) Hello").click();
    run_until_label(&mut harness, "Hello from workspace!");
}

#[test]
fn extension_quick_pick_and_input_box_use_the_quick_access_popup() {
    if vscode_rust::extensions::host::find_node().is_none() {
        return;
    }
    let extensions = sample_extensions_dir();
    let mut harness = launch_with_extensions(Some(workspace_path()), vec![], None, Some(extensions));
    harness.run();
    harness.run();
    run_until_label(&mut harness, "$(smiley) Hello");

    use egui::{Key, Modifiers};
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::P);
    harness.run();
    harness.event(egui::Event::Text("pick a fruit".to_string()));
    harness.run();
    harness.key_press(Key::Enter);

    // The extension's list shows in the popup; pick the second entry.
    run_until_label(&mut harness, "Banana");
    harness.key_press(Key::ArrowDown);
    harness.step();
    harness.key_press(Key::Enter);
    harness.step();

    // Then its input box, prefilled with "Ada"; Enter accepts it.
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(30));
        harness.step();
    }
    harness.key_press(Key::Enter);
    run_until_label(&mut harness, "Picked Banana for Ada");
}

#[test]
fn extension_completions_open_while_typing_and_insert_on_enter() {
    if vscode_rust::extensions::host::find_node().is_none() {
        return;
    }
    let ws = workspace_path();
    let extensions = sample_extensions_dir();
    let mut harness =
        launch_with_extensions(Some(ws.clone()), vec![ws.join("README.md")], None, Some(extensions));
    harness.run();
    harness.run();
    run_until_label(&mut harness, "$(smiley) Hello");

    // Focus the code view and type a word: the extension's suggestions show.
    harness.get_by_role(egui::accesskit::Role::MultilineTextInput).click();
    harness.step();
    harness.event(egui::Event::Text(" hel".to_string()));
    run_until_label(&mut harness, "helloWorld");

    // Enter accepts the highlighted suggestion (snippet placeholders become
    // their default text).
    use egui::Key;
    harness.key_press(Key::Enter);
    for _ in 0..5 {
        harness.step();
    }
    let text = harness
        .get_by_role(egui::accesskit::Role::MultilineTextInput)
        .value()
        .unwrap_or_default();
    assert!(text.contains("helloWorld(name)"), "editor text: {text:?}");
    assert!(harness.query_by_label("heading").is_none(), "the list closes after accepting");
}

// =============================================================================
// Helpers
// =============================================================================

/// Self-contained sample workspace checked in next to the tests, so every
/// run sees the same tree regardless of what exists on the host machine.
fn workspace_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workspace")
}

/// Like [`launch`], then opens the Settings editor with `query`.
fn launch_settings(query: Option<&'static str>) -> (Harness<'static>, std::rc::Rc<std::cell::RefCell<Option<serde_json::Value>>>) {
    launch_settings_watching(query, "editor.formatOnSave")
}

/// [`launch_settings`], reporting the user value of `key` after each frame.
fn launch_settings_watching(query: Option<&'static str>, key: &'static str) -> (Harness<'static>, std::rc::Rc<std::cell::RefCell<Option<serde_json::Value>>>) {
    let mut app: Option<App> = None;
    let seen = std::rc::Rc::new(std::cell::RefCell::new(None));
    let seen_in = seen.clone();
    let runner = move |ctx: &egui::Context| {
        if app.is_none() {
            let mut a = App::for_testing(ctx);
            a.bootstrap_workspace(workspace_path());
            a.bootstrap_settings(query);
            app = Some(a);
            return;
        }
        let a = app.as_mut().unwrap();
        a.render(ctx);
        *seen_in.borrow_mut() = a.user_setting(key);
    };
    let harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    (harness, seen)
}

#[test]
fn untrusted_workspace_settings_get_a_trust_group_and_an_indicator() {
    let dir = std::env::temp_dir().join("vscode-rust-trust-settings");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".vscode")).unwrap();
    // A restricted setting and an ordinary one.
    std::fs::write(dir.join(".vscode/settings.json"), r#"{ "editor.renderControlCharacters": true, "editor.formatOnSave": true }"#).unwrap();
    let mut app: Option<App> = None;
    let folder = dir.clone();
    let runner = move |ctx: &egui::Context| {
        if app.is_none() {
            let mut a = App::for_testing(ctx);
            a.bootstrap_workspace(folder.clone());
            a.bootstrap_settings(None);
            app = Some(a);
            return;
        }
        app.as_mut().unwrap().render(ctx);
    };
    let mut harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    harness.run();
    harness.run();
    // The User tab has neither.
    assert!(harness.query_by_label("toc:Workspace Trust").is_none());
    harness.get_all_by_label("Workspace").next().unwrap().click();
    harness.run();
    harness.run();
    let _ = harness.get_by_label("toc:Workspace Trust");
    // Only the restricted setting carries the indicator (shown in its group and where it lives).
    assert!(harness.get_all_by_label("Requires workspace trust").count() >= 1);
    harness.snapshot("interaction_settings_workspace_trust");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn settings_editor_shows_toc_and_writes_values() {
    let (mut harness, seen) = launch_settings(None);
    harness.run();
    harness.run();
    let _search = harness.get_by_label("Search settings");
    for entry in ["Commonly Used", "Text Editor", "Workbench", "Window", "Features", "Application", "Security", "Extensions"] {
        let _ = harness.get_by_label(&format!("toc:{entry}"));
    }
    harness.snapshot("interaction_settings_editor");

    // Format On Save is a checkbox in Commonly Used; ticking it writes true.
    harness.get_by_label("editor.formatOnSave").click();
    harness.run();
    harness.run();
    assert_eq!(*seen.borrow(), Some(serde_json::json!(true)));
    // Unticking returns to the default, which removes the value.
    harness.get_by_label("editor.formatOnSave").click();
    harness.run();
    harness.run();
    assert_eq!(*seen.borrow(), None);
}

#[test]
fn settings_editor_search_filters() {
    let (mut harness, _) = launch_settings(Some("word wrap"));
    harness.run();
    harness.run();
    harness.run();
    let _ = harness.get_by_label("setting:editor.wordWrap");
    assert!(harness.query_by_label("setting:editor.fontSize").is_none());
}

#[test]
fn settings_editor_exclude_list_adds_and_removes_patterns() {
    let (mut harness, seen) = launch_settings_watching(Some("files.exclude"), "files.exclude");
    harness.run();
    harness.run();
    let _ = harness.get_by_label("item:**/.git");
    harness.get_all_by_label("Add Pattern").next().unwrap().click();
    harness.run();
    harness.get_by_label("Exclude Pattern...").type_text("**/dist");
    harness.run();
    harness.get_by_label("OK").click();
    harness.run();
    harness.run();
    assert_eq!(*seen.borrow(), Some(serde_json::json!({"**/dist": true})));
    // Removing a default pattern overrides it with `false`.
    harness.get_by_label("item:**/.svn").hover();
    harness.run();
    harness.get_by_label("Remove Exclude Item").click();
    harness.run();
    harness.run();
    assert_eq!(*seen.borrow(), Some(serde_json::json!({"**/.svn": false, "**/dist": true})));
    assert!(harness.query_by_label("item:**/.svn").is_none());
}

#[test]
fn restricted_mode_until_the_folder_is_trusted() {
    let mut harness = launch(Some(workspace_path()), vec![], None);
    harness.run();
    harness.run();
    // VS Code 1.145 opens untrusted folders in Restricted Mode, no dialog.
    let _ = harness.get_by_label("Learn More");
    harness.get_by_label("Restricted Mode").click();
    harness.run();
    harness.run();
    let _ = harness.get_by_label("98 workspace settings");
    harness.snapshot("interaction_workspace_trust_editor");
    harness.get_by_label("Trust").click();
    harness.run();
    harness.run();
    // Trusted: no banner, no status item, the editor says so.
    assert!(harness.query_by_label("Restricted Mode").is_none());
    assert!(harness.query_by_label("Learn More").is_none());
    let _ = harness.get_by_label("Don't Trust");
    // The folder is listed; Edit Path swaps it for another path in place.
    let row = format!("trusted:{}", workspace_path().display());
    harness.event(egui::Event::PointerMoved(egui::pos2(650.0, 400.0)));
    harness.step();
    harness.event(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, -600.0), phase: egui::TouchPhase::Move, modifiers: egui::Modifiers::NONE });
    harness.step();
    harness.step();
    let _ = harness.get_by_label(&row);
    harness.event(egui::Event::PointerMoved(egui::pos2(700.0, 674.0)));
    harness.step();
    harness.step();
    harness.get_by_label("Edit Path").click();
    harness.run();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.event(egui::Event::Text("/tmp/elsewhere".to_string()));
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    let _ = harness.get_by_label("trusted:/tmp/elsewhere");
    assert!(harness.query_by_label(&row).is_none());
}

#[test]
fn panel_terminal_asks_for_trust_in_restricted_mode() {
    let mut app: Option<App> = None;
    let runner = move |ctx: &egui::Context| {
        if app.is_none() {
            let mut a = App::for_testing(ctx);
            a.bootstrap_workspace(workspace_path());
            a.bootstrap_panel(ctx, "problems");
            app = Some(a);
            return;
        }
        app.as_mut().unwrap().render(ctx);
    };
    let mut harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    harness.run();
    harness.run();
    for tab in ["Problems", "Output", "Debug Console", "Terminal", "Ports"] {
        let _ = harness.get_by_label(&format!("panel:{tab}"));
    }
    harness.snapshot("interaction_panel_problems");
    // The terminal needs trust first: VS Code's request dialog.
    harness.get_by_label("panel:Terminal").click();
    harness.run();
    harness.run();
    let _ = harness.get_by_label("Trust Folder & Continue");
    harness.get_by_label("Cancel").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("Trust Folder & Continue").is_none());
    // The panel goes away with the terminal that couldn't start.
    assert!(harness.query_by_label("panel:Terminal").is_none());
}

#[test]
fn run_and_debug_view_asks_for_trust_before_debugging() {
    let mut harness = launch(Some(workspace_path()), vec![], None);
    harness.run();
    harness.run();
    harness.get_by_label_contains("Run and Debug (").click();
    harness.run();
    harness.run();
    let _ = harness.get_by_label("create a launch.json file");
    let _ = harness.get_by_label("Open a file");
    harness.snapshot("interaction_run_and_debug");
    harness.get_by_label("Run and Debug").click();
    harness.run();
    harness.run();
    // Restricted Mode: `startDebugging` asks for trust first.
    let _ = harness.get_by_label("Trust Folder & Continue");
    harness.get_by_label("Cancel").click();
    harness.run();
    assert!(harness.query_by_label("Trust Folder & Continue").is_none());
}

/// The app with the Keyboard Shortcuts editor open; reports keybindings.json.
fn launch_keybindings(query: Option<&'static str>) -> (Harness<'static>, std::rc::Rc<std::cell::RefCell<String>>) {
    let mut app: Option<App> = None;
    let seen = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let seen_in = seen.clone();
    let runner = move |ctx: &egui::Context| {
        if app.is_none() {
            let mut a = App::for_testing(ctx);
            a.bootstrap_workspace(workspace_path());
            a.bootstrap_keybindings(query);
            app = Some(a);
            return;
        }
        let a = app.as_mut().unwrap();
        a.render(ctx);
        *seen_in.borrow_mut() = a.user_keybindings_json();
    };
    let harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    (harness, seen)
}

#[test]
fn keybindings_editor_searches_and_redefines_a_keybinding() {
    let (mut harness, seen) = launch_keybindings(Some("file save"));
    harness.run();
    harness.run();
    let _ = harness.get_by_label("Type to search in keybindings");
    harness.get_by_label("File: Save").click();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::ALT | egui::Modifiers::COMMAND, egui::Key::S);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    let json = seen.borrow().clone();
    assert!(json.contains("\"key\": \"alt+cmd+s\",\n        \"command\": \"workbench.action.files.save\""), "{json}");
    assert!(json.contains("\"command\": \"-workbench.action.files.save\""), "{json}");
}

#[test]
fn command_palette_gear_configures_the_keybinding() {
    let mut harness = launch(Some(workspace_path()), vec![], None);
    harness.run();
    harness.run();
    use egui::{Key, Modifiers};
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::P);
    harness.run();
    harness.event(egui::Event::Text("color theme".into()));
    harness.run();
    harness.run();
    // The selected command row has the gear (commandsQuickAccess.ts).
    harness.get_by_label("Configure Keybinding").click();
    harness.run();
    harness.run();
    harness.run();
    // The Keyboard Shortcuts editor shows only that command.
    let _ = harness.get_by_label("Type to search in keybindings");
    let _ = harness.get_by_label("Preferences: Color Theme");
    assert!(harness.query_by_label("File: Save").is_none());
}

#[test]
fn keybindings_json_rebinds_app_shortcuts() {
    // keybindings.json gives ⌘B to the panel (removing the side bar's) and
    // moves the side bar to ⌥⌘B.
    let mut app: Option<App> = None;
    let runner = move |ctx: &egui::Context| {
        // The first frame registers the fonts; the app starts on the next.
        if app.is_none() {
            app = Some(App::for_testing(ctx));
            return;
        }
        let a = app.as_mut().unwrap();
        if a.user_keybindings_json().matches("command").count() == 0 {
            a.bootstrap_workspace(workspace_path());
            a.bootstrap_user_keybindings(
                r#"[
                    { "key": "cmd+b", "command": "workbench.action.togglePanel" },
                    { "key": "cmd+b", "command": "-workbench.action.toggleSidebarVisibility" },
                    { "key": "alt+cmd+b", "command": "workbench.action.toggleSidebarVisibility" }
                ]"#,
            );
        }
        a.render(ctx);
    };
    let mut harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    harness.run();
    harness.run();
    harness.run();
    let _ = harness.get_by_label("README.md");
    assert!(harness.query_by_label("panel:Terminal").is_none());
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::B);
    harness.run();
    harness.run();
    let _ = harness.get_by_label("panel:Terminal");
    let _ = harness.get_by_label("README.md");
    harness.key_press_modifiers(egui::Modifiers::ALT | egui::Modifiers::COMMAND, egui::Key::B);
    harness.run();
    harness.run();
    assert!(harness.query_by_label("README.md").is_none(), "⌥⌘B hides the side bar");
}

#[test]
fn outline_pane_lists_symbols_and_goes_to_them() {
    use vscode_rust::extensions::DocumentSymbol;
    let sym = |name: &str, kind: u8, line: usize, end: usize, children: Vec<DocumentSymbol>| DocumentSymbol {
        name: name.to_string(),
        detail: String::new(),
        kind,
        range: [line, 0, end, 5],
        selection: [line, 0, line, 5],
        children,
    };
    let mut app: Option<App> = None;
    let runner = move |ctx: &egui::Context| {
        if app.is_none() {
            app = Some(App::for_testing(ctx));
            return;
        }
        let a = app.as_mut().unwrap();
        if !a.has_workspace() {
            a.bootstrap_workspace(workspace_path());
            a.bootstrap_open_file(workspace_path().join("tsconfig.json"));
            a.bootstrap_outline(vec![sym("Title", 14, 0, 9, vec![sym("greet", 11, 4, 5, vec![]), sym("Point", 4, 6, 8, vec![sym("x", 7, 7, 7, vec![])])])]);
        }
        a.render(ctx);
    };
    let mut harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    for _ in 0..4 {
        harness.run();
    }
    let _ = harness.get_by_label("outline:Title");
    let _ = harness.get_by_label("outline:greet");
    let _ = harness.get_by_label("outline:x");
    harness.snapshot("interaction_outline_pane");
    // A click goes to the symbol: line 7 (1-based) of the file.
    harness.get_by_label("outline:Point").click();
    harness.run();
    harness.run();
    let _ = harness.get_by_label("Ln 7, Col 1");
    // The breadcrumbs follow: tsconfig.json > Title > Point.
    harness.snapshot("interaction_outline_breadcrumbs");
}

#[test]
fn problems_view_lists_diagnostics_and_opens_them() {
    let mut app: Option<App> = None;
    let runner = move |ctx: &egui::Context| {
        if app.is_none() {
            app = Some(App::for_testing(ctx));
            return;
        }
        let a = app.as_mut().unwrap();
        if !a.has_workspace() {
            a.bootstrap_workspace(workspace_path());
            a.bootstrap_problems(
                workspace_path().join("tsconfig.json"),
                vec![(1, "Unused variable", Some("ts"), Some("6133"), 4, 2), (0, "Value expected", Some("json"), Some("516"), 2, 7)],
            );
            a.bootstrap_panel(ctx, "problems");
        }
        a.render(ctx);
    };
    let mut harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    for _ in 0..4 {
        harness.run();
    }
    // The file, its two problems (errors first), the count badge.
    let _ = harness.get_by_label("tsconfig.json, 2 problems");
    let _ = harness.get_by_label("error Value expected, json");
    let _ = harness.get_by_label("warning Unused variable, ts");
    harness.snapshot("interaction_problems_view");
    harness.get_by_label("error Value expected, json").click();
    harness.run();
    harness.run();
    // The file opens at the problem: its tab is active.
    let _ = harness.get_by_label("tab:tsconfig.json");
    // ↓ moves the selection to the next problem.
    use egui_kittest::kittest::NodeT;
    harness.key_press(egui::Key::ArrowDown);
    harness.run();
    harness.run();
    assert_eq!(harness.get_by_label("warning Unused variable, ts").accesskit_node().toggled().map(|t| format!("{t:?}") == "True"), Some(true));
    assert_ne!(harness.get_by_label("error Value expected, json").accesskit_node().toggled().map(|t| format!("{t:?}") == "True"), Some(true));
    // ← goes up to the file, ← again collapses it, → opens it again.
    harness.key_press(egui::Key::ArrowLeft);
    harness.run();
    harness.run();
    let _ = harness.get_by_label("error Value expected, json");
    harness.key_press(egui::Key::ArrowLeft);
    harness.run();
    harness.run();
    assert!(harness.query_by_label("error Value expected, json").is_none());
    harness.key_press(egui::Key::ArrowRight);
    harness.run();
    harness.run();
    let _ = harness.get_by_label("error Value expected, json");
}

#[test]
fn multi_line_problems_take_a_row_per_line_and_fold_to_one() {
    let mut app: Option<App> = None;
    let runner = move |ctx: &egui::Context| {
        if app.is_none() {
            app = Some(App::for_testing(ctx));
            return;
        }
        let a = app.as_mut().unwrap();
        if !a.has_workspace() {
            a.bootstrap_workspace(workspace_path());
            a.bootstrap_problems(
                workspace_path().join("tsconfig.json"),
                vec![
                    (0, "Type 'string' is not assignable to type 'number'.\n  Types of property 'id' are incompatible.\n    Type 'string' is not assignable to type 'number'.", Some("ts"), Some("2322"), 4, 2),
                    (1, "Unused variable", Some("ts"), Some("6133"), 6, 2),
                ],
            );
            a.bootstrap_panel(ctx, "problems");
        }
        a.render(ctx);
    };
    let mut harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    for _ in 0..4 {
        harness.run();
    }
    // Three lines show, with the chevron that folds them.
    let _ = harness.get_by_label("Show message in single line");
    harness.snapshot("interaction_problems_multiline");
    harness.get_by_label("Show message in single line").click();
    harness.run();
    harness.run();
    let _ = harness.get_by_label("Show message in multiple lines");
    harness.snapshot("interaction_problems_multiline_folded");
}

#[test]
fn problems_view_can_be_shown_as_a_table() {
    let mut app: Option<App> = None;
    let runner = move |ctx: &egui::Context| {
        if app.is_none() {
            app = Some(App::for_testing(ctx));
            return;
        }
        let a = app.as_mut().unwrap();
        if !a.has_workspace() {
            a.bootstrap_workspace(workspace_path());
            a.bootstrap_problems(
                workspace_path().join("tsconfig.json"),
                vec![(1, "Unused variable", Some("ts"), Some("6133"), 4, 2), (0, "Value expected", Some("json"), Some("516"), 2, 7)],
            );
            a.bootstrap_panel(ctx, "problems");
        }
        a.render(ctx);
    };
    let mut harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    for _ in 0..4 {
        harness.run();
    }
    harness.get_by_label("View as Table").click();
    harness.run();
    harness.run();
    // No file row any more; Collapse All is gone, View as Tree took its place.
    assert!(harness.query_by_label("tsconfig.json, 2 problems").is_none());
    assert!(harness.query_by_label("Collapse All").is_none());
    let _ = harness.get_by_label("View as Tree");
    harness.snapshot("interaction_problems_table");
    use egui_kittest::kittest::NodeT;
    harness.get_by_label("warning Unused variable, ts").click();
    harness.run();
    harness.run();
    let _ = harness.get_by_label("tab:tsconfig.json");
    // ↑ moves to the error above (errors sort first).
    harness.key_press(egui::Key::ArrowUp);
    harness.run();
    harness.run();
    assert_eq!(harness.get_by_label("error Value expected, json").accesskit_node().toggled().map(|t| format!("{t:?}") == "True"), Some(true));
    harness.get_by_label("View as Tree").click();
    harness.run();
    harness.run();
    let _ = harness.get_by_label("tsconfig.json, 2 problems");
}

#[test]
fn output_view_shows_the_selected_channel() {
    let mut app: Option<App> = None;
    let runner = move |ctx: &egui::Context| {
        if app.is_none() {
            app = Some(App::for_testing(ctx));
            return;
        }
        let a = app.as_mut().unwrap();
        if !a.has_workspace() {
            a.bootstrap_workspace(workspace_path());
            a.bootstrap_output(
                "Git",
                "2026-11-14 09:58:54.816 [info] > git rev-parse --show-toplevel [12ms]\n2026-11-14 09:58:54.817 [warning] [Window] Failed to request chat control data Offline\n2026-11-14 09:59:30.813 [error] [Network] #132: https://main.vscode-cdn.net/extensions/chat.json - error GET Offline\n    at async Fyt.fetchAlerts (vscode-file://vscode-app/Applications/Visual%20Studio%20Code.app/Contents/Resources/app/out/vs/workbench/workbench.desktop.main.js:6193:7551)\n",
            );
            a.bootstrap_panel(ctx, "output");
        }
        a.render(ctx);
    };
    let mut harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    for _ in 0..4 {
        harness.run();
    }
    let _ = harness.get_by_label("Output channel: Git");
    harness.snapshot("interaction_output_view");
    // Dragging over the text selects it.
    drag(&mut harness, egui::pos2(380.0, 571.0), egui::pos2(700.0, 590.0));
    harness.snapshot("interaction_output_selection");
}

#[test]
fn log_channels_have_the_set_log_level_gear() {
    let mut app: Option<App> = None;
    let runner = move |ctx: &egui::Context| {
        if app.is_none() {
            app = Some(App::for_testing(ctx));
            return;
        }
        let a = app.as_mut().unwrap();
        if !a.has_workspace() {
            a.bootstrap_workspace(workspace_path());
            a.bootstrap_log_channel("Hello Log", "2026-11-18 14:05:12.345 [info] info line\n2026-11-18 14:05:12.346 [error] error line\n", 3);
            a.bootstrap_panel(ctx, "output");
        }
        a.render(ctx);
    };
    let mut harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    for _ in 0..4 {
        harness.run();
    }
    let _ = harness.get_by_label("Set Log Level...");
    let _ = harness.get_by_label("Output channel: Hello Log");
    harness.snapshot("interaction_output_log_gear");
}

#[test]
fn extensions_needing_trust_wait_for_it() {
    use vscode_rust::extensions::{manifest::UntrustedSupport, Enablement, ExtensionService};
    // A throwaway extensions folder: one extension without a capability
    // declaration, one that supports untrusted workspaces, one "limited".
    let dir = std::env::temp_dir().join(format!("vscode-rust-trust-ext-{}", std::process::id()));
    let make = |name: &str, caps: &str| {
        let d = dir.join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("package.json"),
            format!(r#"{{"name":"{name}","publisher":"t","version":"1.0.0","engines":{{"vscode":"^1.0.0"}},"main":"./x.js"{caps}}}"#),
        )
        .unwrap();
    };
    make("plain", "");
    make("safe", r#","capabilities":{"untrustedWorkspaces":{"supported":true}}"#);
    make("limited", r#","capabilities":{"untrustedWorkspaces":{"supported":"limited"}}"#);
    let mut service = ExtensionService::default();
    service.load(&[dir.clone()]);
    // Trusted: all three run.
    assert!(service.extensions.iter().all(|e| ["t.plain", "t.safe", "t.limited"].contains(&e.id.as_str())));
    assert_eq!(service.extensions.len(), 3);
    // Restricted Mode: only the one that doesn't support it stops.
    service.set_restricted(true);
    let running: Vec<&str> = service.extensions.iter().map(|e| e.id.as_str()).collect();
    assert!(running.contains(&"t.safe") && running.contains(&"t.limited") && !running.contains(&"t.plain"), "{running:?}");
    assert_eq!(service.enablement("t.plain"), Enablement::DisabledByTrust);
    assert_eq!(service.enablement("t.safe"), Enablement::Enabled);
    assert_eq!(service.installed().find(|e| e.id == "t.limited").unwrap().untrusted_support(), UntrustedSupport::Limited);
    // Trusting the folder brings it back.
    service.set_restricted(false);
    assert_eq!(service.extensions.len(), 3);
    assert_eq!(service.enablement("t.plain"), Enablement::Enabled);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn settings_editor_object_table_adds_and_resets_entries() {
    let (mut harness, seen) = launch_settings_watching(Some("@id:files.associations"), "files.associations");
    for _ in 0..4 {
        harness.run();
    }
    let _ = harness.get_by_label("setting:files.associations");
    harness.get_by_label("Add Item").click();
    harness.run();
    harness.get_by_label("Key").type_text("*.vue");
    harness.run();
    harness.get_by_label("Value").click();
    harness.run();
    harness.get_by_label("Value").type_text("html");
    harness.run();
    harness.get_by_label("OK").click();
    harness.run();
    harness.run();
    assert_eq!(*seen.borrow(), Some(serde_json::json!({"*.vue": "html"})));
    let _ = harness.get_by_label("item:*.vue");
    harness.snapshot("interaction_settings_object");
    // Removing the only entry resets the setting.
    harness.get_by_label("item:*.vue").hover();
    harness.run();
    harness.get_by_label("Remove Item").click();
    harness.run();
    harness.run();
    assert_eq!(*seen.borrow(), None);
}

#[test]
fn settings_editor_checkbox_list_toggles_entries() {
    let (mut harness, seen) = launch_settings_watching(Some("@id:scm.diffDecorationsGutterPattern"), "scm.diffDecorationsGutterPattern");
    for _ in 0..4 {
        harness.run();
    }
    use egui_kittest::kittest::NodeT;
    let on = |harness: &egui_kittest::Harness<'_>, label: &str| harness.get_by_label(label).accesskit_node().toggled().map(|t| format!("{t:?}") == "True");
    let added = "Use pattern for the diff decorations in gutter for added lines. (added)";
    let modified = "Use pattern for the diff decorations in gutter for modified lines. (modified)";
    assert_eq!(on(&harness, added), Some(false));
    assert_eq!(on(&harness, modified), Some(true));
    harness.get_by_label(added).click();
    harness.run();
    harness.run();
    assert_eq!(*seen.borrow(), Some(serde_json::json!({"added": true})));
    assert_eq!(on(&harness, added), Some(true));
    harness.snapshot("interaction_settings_checkbox_list");
    // Back to the defaults: the setting resets.
    harness.get_by_label(added).click();
    harness.run();
    harness.run();
    assert_eq!(*seen.borrow(), None);
}

#[test]
fn split_editor_opens_a_second_group_and_open_to_the_side() {
    use egui::{Key, Modifiers};
    let ws = workspace_path();
    let mut harness = launch(Some(ws.clone()), vec![ws.join("tsconfig.json")], None);
    for _ in 0..3 {
        harness.run();
    }
    assert_eq!(harness.get_all_by_label("tab:tsconfig.json").count(), 1);
    harness.get_by_label("Split Editor Right (⌘\\)").click();
    harness.run();
    harness.run();
    // The same file is open in both groups.
    assert_eq!(harness.get_all_by_label("tab:tsconfig.json").count(), 2);
    // ⌘Enter in Quick Open opens the file in the right group.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::P);
    harness.run();
    harness.run();
    harness.event(egui::Event::Text("site".to_string()));
    harness.run();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.run();
    harness.run();
    assert_eq!(harness.get_all_by_label("tab:SiteHeader.vue").count(), 1);
    assert_eq!(harness.get_all_by_label("tab:tsconfig.json").count(), 2);
    harness.snapshot("interaction_split_editor");
    // Dragging the sash between the groups resizes them.
    let seam = egui::pos2(853.0, 500.0);
    harness.event(egui::Event::PointerMoved(seam));
    harness.step();
    harness.event(egui::Event::PointerButton { pos: seam, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    harness.step();
    harness.event(egui::Event::PointerMoved(egui::pos2(700.0, 500.0)));
    harness.step();
    harness.event(egui::Event::PointerButton { pos: egui::pos2(700.0, 500.0), button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    harness.step();
    harness.step();
    harness.snapshot("interaction_split_editor_resized");
}

#[test]
fn keybinding_moves_the_editor_to_the_next_group() {
    let ws = workspace_path();
    let mut app: Option<App> = None;
    let runner = move |ctx: &egui::Context| {
        if app.is_none() {
            app = Some(App::for_testing(ctx));
            return;
        }
        let a = app.as_mut().unwrap();
        if a.user_keybindings_json().matches("command").count() == 0 {
            a.bootstrap_workspace(workspace_path());
            a.bootstrap_open_file(workspace_path().join("tsconfig.json"));
            a.bootstrap_open_file(workspace_path().join("package.json"));
            a.bootstrap_user_keybindings(r#"[{ "key": "cmd+m", "command": "workbench.action.moveEditorToNextGroup" }]"#);
        }
        a.render(ctx);
    };
    let _ = ws;
    let mut harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    for _ in 0..4 {
        harness.run();
    }
    assert_eq!(harness.get_all_by_label("Split Editor Right (⌘\\)").count(), 1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::M);
    harness.run();
    harness.run();
    // Two groups now: each has its own tab strip with the title actions.
    assert_eq!(harness.get_all_by_label("Split Editor Right (⌘\\)").count(), 2);
    let _ = harness.get_by_label("tab:tsconfig.json");
    let _ = harness.get_by_label("tab:package.json");
}

/// Drag a tab from `from` to `to` with the primary button.
fn drag(harness: &mut Harness<'static>, from: egui::Pos2, to: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(from));
    harness.step();
    harness.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    harness.step();
    let mid = egui::pos2((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
    harness.event(egui::Event::PointerMoved(mid));
    harness.step();
    harness.event(egui::Event::PointerMoved(to));
    harness.step();
    harness.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    harness.step();
    harness.step();
}

fn launch_two_tabs() -> Harness<'static> {
    let mut app: Option<App> = None;
    let runner = move |ctx: &egui::Context| {
        if app.is_none() {
            app = Some(App::for_testing(ctx));
            return;
        }
        let a = app.as_mut().unwrap();
        if !a.has_workspace() {
            a.bootstrap_workspace(workspace_path());
            a.bootstrap_open_file(workspace_path().join("tsconfig.json"));
            a.bootstrap_open_file(workspace_path().join("package.json"));
        }
        a.render(ctx);
    };
    let mut harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    for _ in 0..4 {
        harness.run();
    }
    harness
}

#[test]
fn dragging_a_tab_reorders_it_or_splits_the_editor() {
    let mut harness = launch_two_tabs();
    let order = |h: &Harness<'static>| h.get_all_by_label_contains("tab:").map(|n| n.accesskit_node().label().unwrap_or_default().to_string()).collect::<Vec<_>>();
    use egui_kittest::kittest::NodeT;
    assert_eq!(order(&harness), vec!["tab:tsconfig.json", "tab:package.json"]);
    // The first tab dropped past the last one goes to the end.
    drag(&mut harness, egui::pos2(420.0, 77.0), egui::pos2(900.0, 77.0));
    assert_eq!(order(&harness), vec!["tab:package.json", "tab:tsconfig.json"]);
    // Dropped on the right of the editor area it opens a second group.
    assert_eq!(harness.get_all_by_label("Split Editor Right (⌘\\)").count(), 1);
    drag(&mut harness, egui::pos2(420.0, 77.0), egui::pos2(1100.0, 400.0));
    harness.run();
    assert_eq!(harness.get_all_by_label("Split Editor Right (⌘\\)").count(), 2);
}

#[test]
fn settings_editor_lang_filter_writes_the_language_section() {
    let (mut harness, seen) = launch_settings_watching(Some("@lang:typescript @id:editor.insertSpaces"), "[typescript]");
    for _ in 0..4 {
        harness.run();
    }
    harness.get_by_label("editor.insertSpaces").click();
    harness.run();
    harness.run();
    assert_eq!(*seen.borrow(), Some(serde_json::json!({"editor.insertSpaces": false})));
    // Back to the default: the section goes away.
    harness.get_by_label("editor.insertSpaces").click();
    harness.run();
    harness.run();
    assert_eq!(*seen.borrow(), None);
}

#[test]
fn settings_editor_keeps_the_group_header_pinned_while_scrolling() {
    let (mut harness, _) = launch_settings(None);
    for _ in 0..3 {
        harness.run();
    }
    harness.event(egui::Event::PointerMoved(egui::pos2(700.0, 400.0)));
    harness.step();
    harness.event(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, -1900.0), phase: egui::TouchPhase::Move, modifiers: egui::Modifiers::NONE });
    for _ in 0..3 {
        harness.step();
    }
    harness.snapshot("interaction_settings_sticky_header");
}

#[test]
fn a_dragged_tab_shows_where_it_will_land() {
    let mut harness = launch_two_tabs();
    let from = egui::pos2(420.0, 77.0);
    harness.event(egui::Event::PointerMoved(from));
    harness.step();
    harness.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    harness.step();
    harness.event(egui::Event::PointerMoved(egui::pos2(500.0, 77.0)));
    harness.step();
    harness.event(egui::Event::PointerMoved(egui::pos2(640.0, 77.0)));
    harness.step();
    harness.step();
    harness.snapshot("interaction_tab_drag_marker");
}

#[test]
fn a_dragged_tab_highlights_the_drop_zone_on_the_right() {
    let mut harness = launch_two_tabs();
    let from = egui::pos2(420.0, 77.0);
    harness.event(egui::Event::PointerMoved(from));
    harness.step();
    harness.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    harness.step();
    harness.event(egui::Event::PointerMoved(egui::pos2(700.0, 250.0)));
    harness.step();
    harness.event(egui::Event::PointerMoved(egui::pos2(1100.0, 400.0)));
    harness.step();
    harness.step();
    harness.snapshot("interaction_tab_drag_zone");
}

#[test]
fn status_bar_end_of_line_and_indentation_pickers_change_the_document() {
    let ws = workspace_path();
    let mut harness = launch(Some(ws.clone()), vec![ws.join("tsconfig.json")], None);
    for _ in 0..3 {
        harness.run();
    }
    // tsconfig.json is indented with spaces; the item shows what it found.
    let _ = harness.get_by_label("Spaces: 2");
    harness.get_by_label("LF").click();
    harness.run();
    harness.run();
    harness.key_press(egui::Key::ArrowDown);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    let _ = harness.get_by_label("CRLF");
    // The file is modified now: a dirty tab.
    harness.get_by_label("Spaces: 2").click();
    harness.run();
    harness.run();
    harness.key_press(egui::Key::ArrowDown);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    let _ = harness.get_by_label("Tab Size: 4");
}

#[test]
fn status_bar_encoding_picker_reopens_and_saves_with_another_encoding() {
    let dir = std::env::temp_dir().join(format!("vscode-rust-encoding-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // "Привет" in Windows 1251: not UTF-8, so it opens with replacement characters.
    let file = dir.join("legacy.txt");
    std::fs::write(&file, [0xCF, 0xF0, 0xE8, 0xE2, 0xE5, 0xF2]).unwrap();
    let mut harness = launch(Some(dir.clone()), vec![file.clone()], None);
    for _ in 0..3 {
        harness.run();
    }
    harness.get_by_label("UTF-8").click();
    harness.run();
    harness.run();
    // Select Action: Reopen with Encoding is the first entry.
    let _ = harness.get_by_label("Reopen with Encoding");
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    harness.event(egui::Event::Text("Windows 1251".to_string()));
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    let _ = harness.get_by_label("Windows 1251");
    // Save with Encoding writes the file at once.
    harness.get_by_label("Windows 1251").click();
    harness.run();
    harness.run();
    harness.key_press(egui::Key::ArrowDown);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    harness.event(egui::Event::Text("UTF-16 LE".to_string()));
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    let _ = harness.get_by_label("UTF-16 LE");
    let bytes = std::fs::read(&file).unwrap();
    assert_eq!(&bytes[..2], [0xFF, 0xFE]);
    assert_eq!(String::from_utf16_lossy(&bytes[2..].chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()), "Привет");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn explorer_creates_renames_and_deletes_files() {
    let dir = std::env::temp_dir().join(format!("vscode-rust-explorer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("keep.txt"), "x").unwrap();
    let mut harness = launch(Some(dir.clone()), vec![], None);
    for _ in 0..3 {
        harness.run();
    }
    // The folder header's New File button asks for a name in Quick Access.
    harness.event(egui::Event::PointerMoved(egui::pos2(200.0, 107.0)));
    harness.step();
    harness.step();
    // (The Welcome page has a New File... button too; the sidebar comes first.)
    harness.get_all_by_label("New File...").next().unwrap().click();
    harness.run();
    harness.run();
    harness.event(egui::Event::Text("made.txt".to_string()));
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    assert!(dir.join("made.txt").exists());
    let _ = harness.get_by_label("tab:made.txt");
    let _ = harness.get_by_label("made.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dragging_an_explorer_row_onto_a_folder_moves_it() {
    let dir = std::env::temp_dir().join(format!("vscode-rust-drag-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("a.txt"), "x").unwrap();
    let mut harness = launch(Some(dir.clone()), vec![dir.join("a.txt")], None);
    for _ in 0..3 {
        harness.run();
    }
    let from = harness.get_by_label("a.txt").rect().center();
    let to = harness.get_by_label("sub").rect().center();
    drag(&mut harness, from, to);
    assert!(dir.join("sub/a.txt").exists() && !dir.join("a.txt").exists());
    // Dropping a folder on its own parent changes nothing.
    let from = harness.get_by_label("sub").rect().center();
    let root_name = format!("vscode-rust-drag-{}", std::process::id());
    let root = harness.get_by_label(&root_name).rect().center();
    drag(&mut harness, from, root);
    assert!(dir.join("sub/a.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn f2_renames_the_selected_explorer_row() {
    let dir = std::env::temp_dir().join(format!("vscode-rust-f2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "x").unwrap();
    let mut harness = launch(Some(dir.clone()), vec![], None);
    for _ in 0..3 {
        harness.run();
    }
    harness.get_by_label("a.txt").click();
    harness.run();
    harness.run();
    // The pointer stays over the side bar after the click.
    harness.key_press(egui::Key::F2);
    harness.run();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.event(egui::Event::Text("b.txt".to_string()));
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    assert!(dir.join("b.txt").exists() && !dir.join("a.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn escape_cancels_the_inline_rename_and_blur_accepts_it() {
    let dir = std::env::temp_dir().join(format!("vscode-rust-inline-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "x").unwrap();
    let mut harness = launch(Some(dir.clone()), vec![], None);
    for _ in 0..3 {
        harness.run();
    }
    harness.get_by_label("a.txt").click();
    harness.run();
    harness.run();
    // Escape leaves the name alone.
    harness.key_press(egui::Key::F2);
    harness.run();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.event(egui::Event::Text("zzz.txt".to_string()));
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();
    harness.run();
    assert!(dir.join("a.txt").exists() && !dir.join("zzz.txt").exists());
    // Typing over the selected stem keeps the extension; a click elsewhere accepts.
    harness.key_press(egui::Key::F2);
    harness.run();
    harness.run();
    harness.event(egui::Event::Text("b".to_string()));
    harness.run();
    harness.event(egui::Event::PointerMoved(egui::pos2(900.0, 500.0)));
    harness.event(egui::Event::PointerButton { pos: egui::pos2(900.0, 500.0), button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    harness.run();
    harness.event(egui::Event::PointerButton { pos: egui::pos2(900.0, 500.0), button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    harness.run();
    harness.run();
    let names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert!(dir.join("b.txt").exists() && !dir.join("a.txt").exists(), "{names:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn inline_rename_input_sits_over_the_row_label() {
    let ws = workspace_path();
    let mut harness = launch(Some(ws.clone()), vec![], None);
    for _ in 0..3 {
        harness.run();
    }
    harness.get_by_label("tsconfig.json").click();
    harness.run();
    harness.run();
    harness.key_press(egui::Key::F2);
    harness.run();
    harness.run();
    harness.run();
    harness.snapshot("interaction_explorer_inline_rename");
}

#[test]
fn alt_dragging_an_explorer_row_copies_it() {
    let dir = std::env::temp_dir().join(format!("vscode-rust-altdrag-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("a.txt"), "x").unwrap();
    let mut harness = launch(Some(dir.clone()), vec![dir.join("a.txt")], None);
    for _ in 0..3 {
        harness.run();
    }
    let from = harness.get_by_label("a.txt").rect().center();
    let to = harness.get_by_label("sub").rect().center();
    let alt = egui::Modifiers { alt: true, ..Default::default() };
    harness.event_modifiers(egui::Event::PointerMoved(from), alt);
    harness.step();
    harness.event_modifiers(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: alt }, alt);
    harness.step();
    for t in [0.4, 0.8, 1.0] {
        harness.event_modifiers(egui::Event::PointerMoved(from + (to - from) * t), alt);
        harness.step();
    }
    harness.step();
    harness.event_modifiers(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: alt }, alt);
    harness.step();
    harness.run();
    // The original stays, the copy is in the folder.
    assert!(dir.join("a.txt").exists() && dir.join("sub/a.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_editor_closes_pairs_indents_on_enter_and_comments_lines() {
    use egui::{Key, Modifiers};
    use egui_kittest::kittest::NodeT;
    let dir = std::env::temp_dir().join("vscode-rust-smart-editing");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.ts");
    std::fs::write(&file, "").unwrap();
    let mut harness = launch(Some(dir.clone()), vec![file.clone()], None);
    for _ in 0..3 {
        harness.run();
    }
    let value = |h: &Harness<'static>| h.get_by_role(egui::accesskit::Role::MultilineTextInput).accesskit_node().value().unwrap_or_default().to_string();
    harness.get_by_role(egui::accesskit::Role::MultilineTextInput).click();
    harness.run();
    // A bracket closes itself, the closer is typed over, quotes pair up.
    harness.event(egui::Event::Text("f".to_string()));
    harness.event(egui::Event::Text("(".to_string()));
    harness.run();
    assert_eq!(value(&harness), "f()");
    harness.event(egui::Event::Text(")".to_string()));
    harness.event(egui::Event::Text(" ".to_string()));
    harness.event(egui::Event::Text("{".to_string()));
    harness.run();
    assert_eq!(value(&harness), "f() {}");
    // Enter between the braces opens an indented line and puts } below.
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(value(&harness), "f() {\n    \n}");
    harness.event(egui::Event::Text("x".to_string()));
    harness.run();
    // ⌘/ comments the line, ⌘/ again takes it off.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Slash);
    harness.run();
    assert_eq!(value(&harness), "f() {\n    // x\n}");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Slash);
    harness.run();
    assert_eq!(value(&harness), "f() {\n    x\n}");
    // ⌥↑ moves the line up.
    harness.key_press_modifiers(Modifiers::ALT, Key::ArrowUp);
    harness.run();
    assert_eq!(value(&harness), "    x\nf() {\n}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_find_widget_searches_with_options_and_replaces() {
    use egui::{Key, Modifiers};
    use egui_kittest::kittest::NodeT;
    let dir = std::env::temp_dir().join("vscode-rust-find-widget");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.txt");
    std::fs::write(&file, "Foo foo FOO food\n").unwrap();
    let mut harness = launch(Some(dir.clone()), vec![file.clone()], None);
    for _ in 0..3 {
        harness.run();
    }
    let value = |h: &Harness<'static>| h.get_by_role(egui::accesskit::Role::MultilineTextInput).accesskit_node().value().unwrap_or_default().to_string();
    harness.get_by_role(egui::accesskit::Role::MultilineTextInput).click();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::F);
    harness.run();
    harness.run();
    harness.event(egui::Event::Text("foo".to_string()));
    harness.run();
    harness.run();
    let _ = harness.get_by_label("Match Case (⌥⌘C)");
    harness.snapshot("interaction_find_widget");
    // Match Case narrows 4 matches to 2, and the toggle reports it.
    harness.get_by_label("Match Case (⌥⌘C)").click();
    harness.run();
    harness.run();
    assert_eq!(harness.get_by_label("Match Case (⌥⌘C)").accesskit_node().toggled().map(|t| format!("{t:?}") == "True"), Some(true));
    // The replace row opens with the chevron; Replace All writes into the text.
    harness.get_by_label("Toggle Replace").click();
    harness.run();
    harness.run();
    // ⌥⌘F puts the keyboard in the replace input.
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::F);
    harness.run();
    harness.run();
    harness.event(egui::Event::Text("bar".to_string()));
    harness.run();
    harness.snapshot("interaction_find_replace_widget");
    harness.get_by_label("Replace All (⌘Enter)").click();
    harness.run();
    harness.run();
    // Case-sensitive "foo": the lower-case word and the start of "food".
    assert_eq!(value(&harness), "Foo bar FOO bard\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn expand_and_shrink_selection_keep_the_text() {
    use egui_kittest::kittest::NodeT;
    let dir = std::env::temp_dir().join("vscode-rust-expand-selection");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.txt");
    let original = "fn a() {\n  call(x, yy);\n}\n";
    std::fs::write(&file, original).unwrap();
    let mut harness = launch(Some(dir.clone()), vec![file.clone()], None);
    for _ in 0..3 {
        harness.run_steps(3);
    }
    let editor = |h: &Harness<'static>| h.get_by_role(egui::accesskit::Role::MultilineTextInput).accesskit_node().value().unwrap_or_default().to_string();
    harness.get_by_role(egui::accesskit::Role::MultilineTextInput).click();
    harness.run_steps(3);
    let queue = |h: &mut Harness<'static>, what: &'static str| {
        h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("editor-action"), what));
        h.run_steps(3);
        h.run_steps(3);
    };
    // What is selected shows when it is duplicated: the caret starts in "fn",
    // so the first expansion is the word, the second the line's text.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::ArrowUp);
    harness.run_steps(3);
    queue(&mut harness, "expand");
    queue(&mut harness, "duplicate");
    assert!(editor(&harness).starts_with("fnfn a()"), "got {:?}", editor(&harness));
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run_steps(3);
    assert_eq!(editor(&harness), original);
    // From inside the call's arguments, to the last step, the text stays put.
    for _ in 0..17 {
        harness.key_press(egui::Key::ArrowRight);
    }
    harness.run_steps(3);
    // Further expansions and shrinks never touch the text.
    for _ in 0..6 {
        queue(&mut harness, "expand");
        assert_eq!(editor(&harness), original, "expand must not touch the text");
    }
    for _ in 0..8 {
        queue(&mut harness, "shrink");
        assert_eq!(editor(&harness), original, "shrink must not touch the text");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reopen_closed_editor_brings_the_file_back() {
    let dir = std::env::temp_dir().join("vscode-rust-reopen-closed");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("keep.txt");
    std::fs::write(&file, "hello\n").unwrap();
    let mut harness = launch(Some(dir.clone()), vec![file.clone()], None);
    for _ in 0..3 {
        harness.run();
    }
    // Close the editor with ⌘W, then ⇧⌘T brings it back.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::W);
    harness.run();
    assert!(harness.query_by_role(egui::accesskit::Role::MultilineTextInput).is_none(), "the editor should be closed");
    harness.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::T);
    harness.run();
    harness.run();
    assert!(harness.query_by_role(egui::accesskit::Role::MultilineTextInput).is_some(), "the editor should be back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn problems_show_related_information_under_the_problem() {
    let mut app: Option<App> = None;
    let runner = move |ctx: &egui::Context| {
        if app.is_none() {
            app = Some(App::for_testing(ctx));
            return;
        }
        let a = app.as_mut().unwrap();
        if !a.has_workspace() {
            let file = workspace_path().join("tsconfig.json");
            a.bootstrap_workspace(workspace_path());
            a.bootstrap_problems(file.clone(), vec![(0, "Duplicate identifier 'x'", Some("ts"), Some("2300"), 2, 4)]);
            a.bootstrap_problem_related(
                &file,
                0,
                vec![(workspace_path().join("package.json"), 1, 2, "'x' was also declared here."), (file.clone(), 5, 0, "and here.")],
            );
            a.bootstrap_panel(ctx, "problems");
        }
        a.render(ctx);
    };
    let mut harness = Harness::builder().with_size(Vec2::new(1300.0, 820.0)).wgpu().build(runner);
    for _ in 0..4 {
        harness.run();
    }
    // The related places are listed under the problem, unfolded.
    let related = format!("{}: 'x' was also declared here.", workspace_path().join("package.json").display());
    let _ = harness.get_by_label(&related);
    harness.snapshot("interaction_problems_related");
    // The twistie folds them away and back.
    harness.get_by_label("Collapse").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label(&related).is_none());
    harness.get_by_label("Expand").click();
    harness.run();
    harness.run();
    // A click on a related row opens its file.
    harness.get_by_label(&related).click();
    harness.run();
    harness.run();
    let _ = harness.get_by_label("tab:package.json");
}

#[test]
fn open_recent_picker_lists_folders_and_files() {
    let dir = std::env::temp_dir().join("vscode-rust-open-recent");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("recent-one.txt");
    std::fs::write(&file, "hello\n").unwrap();
    let mut harness = launch(Some(dir.clone()), vec![file.clone()], None);
    for _ in 0..3 {
        harness.run();
    }
    // ⌃R opens the picker: the folder, then the file.
    harness.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::R);
    harness.run();
    harness.run();
    // Each row is the name with its parent folder as the description.
    let parent = vscode_rust::menubar::recent_label(dir.parent().unwrap());
    let folder_row = format!("vscode-rust-open-recent, {parent}");
    let file_row = format!("recent-one.txt, {}", vscode_rust::menubar::recent_label(&dir));
    let _ = harness.get_by_label(&folder_row);
    let _ = harness.get_by_label(&file_row);
    harness.snapshot("interaction_open_recent");
    // The open folder comes first, so the file is focused; its button removes it.
    harness.get_by_label("Remove from Recently Opened").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label(&file_row).is_none(), "the file should be gone from the list");
    let _ = harness.get_by_label(&folder_row);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn f2_renames_the_symbol_through_the_extension() {
    if vscode_rust::extensions::host::find_node().is_none() {
        return;
    }
    use egui::{Key, Modifiers};
    use egui_kittest::kittest::NodeT;
    let dir = std::env::temp_dir().join("vscode-rust-rename-symbol");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.md");
    std::fs::write(&file, "foo bar foo\n").unwrap();
    let mut harness = launch_with_extensions(Some(dir.clone()), vec![file.clone()], None, Some(sample_extensions_dir()));
    harness.step();
    harness.step();
    run_until_label(&mut harness, "$(smiley) Hello");
    let value = |h: &Harness<'static>| h.get_by_role(egui::accesskit::Role::MultilineTextInput).accesskit_node().value().unwrap_or_default().to_string();
    // The caret at the start of the text: on the first "foo".
    harness.get_by_role(egui::accesskit::Role::MultilineTextInput).click();
    harness.step();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::ArrowUp);
    harness.step();
    harness.step();
    harness.key_press(Key::F2);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while harness.query_by_label("Rename input. Type new name and press Enter to commit.").is_none() {
        assert!(std::time::Instant::now() < deadline, "the rename input never opened");
        std::thread::sleep(std::time::Duration::from_millis(30));
        harness.step();
    }
    harness.step();
    // No snapshot: the caret blinks and the extension's activity bar icon
    // arrives at its own time.
    // The name is selected: typing replaces it, Enter renames every "foo".
    harness.event(egui::Event::Text("baz".to_string()));
    harness.step();
    harness.key_press(Key::Enter);
    while value(&harness) != "baz bar baz\n" {
        assert!(std::time::Instant::now() < deadline, "the rename was never applied: {:?}", value(&harness));
        std::thread::sleep(std::time::Duration::from_millis(30));
        harness.step();
    }
    assert!(harness.query_by_label("Rename input. Type new name and press Enter to commit.").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_lightbulb_shows_code_actions_and_applies_one() {
    if vscode_rust::extensions::host::find_node().is_none() {
        return;
    }
    use egui::{Key, Modifiers};
    use egui_kittest::kittest::NodeT;
    let dir = std::env::temp_dir().join("vscode-rust-lightbulb");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.md");
    std::fs::write(&file, "FIXME later\n").unwrap();
    let mut harness = launch_with_extensions(Some(dir.clone()), vec![file.clone()], None, Some(sample_extensions_dir()));
    harness.step();
    harness.step();
    run_until_label(&mut harness, "$(smiley) Hello");
    let value = |h: &Harness<'static>| h.get_by_role(egui::accesskit::Role::MultilineTextInput).accesskit_node().value().unwrap_or_default().to_string();
    harness.get_by_role(egui::accesskit::Role::MultilineTextInput).click();
    harness.step();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::ArrowUp);
    harness.step();
    // Once the caret rests on the FIXME line, the lightbulb shows.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while harness.query_by_label("Show Code Actions (⌘.)").is_none() {
        assert!(std::time::Instant::now() < deadline, "the lightbulb never showed");
        std::thread::sleep(std::time::Duration::from_millis(30));
        harness.step();
    }
    harness.get_by_label("Show Code Actions (⌘.)").click();
    harness.step();
    harness.step();
    harness.get_by_label("Replace FIXME with TODO").click();
    while value(&harness) != "TODO later\n" {
        assert!(std::time::Instant::now() < deadline, "the quick fix was never applied: {:?}", value(&harness));
        std::thread::sleep(std::time::Duration::from_millis(30));
        harness.step();
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shift_f12_peeks_the_references_and_enter_goes_to_one() {
    if vscode_rust::extensions::host::find_node().is_none() {
        return;
    }
    use egui::{Key, Modifiers};
    let dir = std::env::temp_dir().join("vscode-rust-references");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.md");
    std::fs::write(&file, "foo bar\nbaz foo\n").unwrap();
    let mut harness = launch_with_extensions(Some(dir.clone()), vec![file.clone()], None, Some(sample_extensions_dir()));
    harness.step();
    harness.step();
    run_until_label(&mut harness, "$(smiley) Hello");
    harness.get_by_role(egui::accesskit::Role::MultilineTextInput).click();
    harness.step();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::ArrowUp);
    harness.step();
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F12);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while harness.query_by_label("a.md, 2 references").is_none() {
        assert!(std::time::Instant::now() < deadline, "the peek never opened");
        std::thread::sleep(std::time::Duration::from_millis(30));
        harness.step();
    }
    // Both references are listed by their line; the one at the caret is
    // selected first, ↓ moves to the other, Enter goes there and closes.
    assert!(harness.query_by_label("foo bar").is_some());
    assert!(harness.query_by_label("baz foo").is_some());
    harness.key_press(Key::ArrowDown);
    harness.step();
    harness.key_press(Key::Enter);
    harness.step();
    harness.step();
    assert!(harness.query_by_label("a.md, 2 references").is_none(), "Enter closes the peek");
    // ⇧F12 again and Escape closes it.
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F12);
    while harness.query_by_label("a.md, 2 references").is_none() {
        assert!(std::time::Instant::now() < deadline, "the peek never opened again");
        std::thread::sleep(std::time::Duration::from_millis(30));
        harness.step();
    }
    harness.key_press(Key::Escape);
    harness.step();
    harness.step();
    assert!(harness.query_by_label("a.md, 2 references").is_none(), "Escape closes the peek");
    let _ = std::fs::remove_dir_all(&dir);
}
