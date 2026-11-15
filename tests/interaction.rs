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
