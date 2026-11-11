//! Native menu bar (macOS NSMenu / Windows / Linux) via `muda`.
//!
//! VS Code on macOS uses the system menubar instead of an in-window one — see
//! vscode/src/vs/workbench/browser/parts/titlebar/titlebarPart.ts (`hasMenubar`).
//! We replicate that by installing a `muda::Menu` as the application menu
//! before eframe builds its event loop.

use muda::{
    accelerator::{Accelerator, Code, Modifiers},
    Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};

#[derive(Clone)]
pub struct MenuIds {
    pub new_file: MenuId,
    pub open_folder: MenuId,
    pub open_file: MenuId,
    pub close_folder: MenuId,
    pub save: MenuId,
    pub save_all: MenuId,
    pub close_editor: MenuId,
    pub close_all: MenuId,
    pub palette: MenuId,
    pub toggle_sidebar: MenuId,
    pub show_explorer: MenuId,
    pub show_search: MenuId,
    pub go_to_file: MenuId,
    pub welcome: MenuId,
    pub color_theme: MenuId,
    pub open_settings: MenuId,
    pub open_keybindings: MenuId,
    pub toggle_panel: MenuId,
    pub show_problems: MenuId,
    pub show_output: MenuId,
    pub show_debug_console: MenuId,
    pub show_terminal: MenuId,
    pub new_terminal: MenuId,
}

pub struct InstalledMenu {
    pub ids: MenuIds,
    // Keep the Menu alive — muda relies on its owned handles for the lifetime of the menubar.
    #[allow(dead_code)]
    menu: Menu,
}

pub fn install() -> InstalledMenu {
    let menu = Menu::new();
    // Code ▸ Settings ▸ Themes ▸ Color Theme (⌘K ⌘T is a chord, which a
    // native menu can't show, as in VS Code).
    let color_theme = MenuItem::new("Color Theme", true, None);
    // Code ▸ Settings ▸ Settings (⌘,).
    let open_settings = MenuItem::new("Settings", true, Some(Accelerator::new(Some(Modifiers::SUPER), Code::Comma)));
    // Code ▸ Settings ▸ Keyboard Shortcuts (⌘K ⌘S, a chord).
    let open_keybindings = MenuItem::new("Keyboard Shortcuts", true, None);

    // macOS application submenu — Apple, About, Services, Hide/Show, Quit
    #[cfg(target_os = "macos")]
    {
        let app_m = Submenu::new("vscode-rust", true);
        let themes_m = Submenu::new("Themes", true);
        let _ = themes_m.append(&color_theme);
        let settings_m = Submenu::new("Settings", true);
        let _ = settings_m.append_items(&[&open_settings, &open_keybindings, &PredefinedMenuItem::separator(), &themes_m]);
        let _ = menu.append(&app_m);
        let _ = app_m.append_items(&[
            &PredefinedMenuItem::about(Some("About vscode-rust"), None),
            &PredefinedMenuItem::separator(),
            &settings_m,
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::services(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ]);
    }

    // File menu
    let file_m = Submenu::new("File", true);
    let new_file = MenuItem::new(
        "New Text File",
        true,
        Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyN)),
    );
    let open_folder = MenuItem::new("Open Folder…", true, None);
    let open_file = MenuItem::new("Open File…", true, None);
    let close_folder = MenuItem::new("Close Folder", true, None);
    let save = MenuItem::new(
        "Save",
        true,
        Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyS)),
    );
    let save_all = MenuItem::new(
        "Save All",
        true,
        Some(Accelerator::new(
            Some(Modifiers::SUPER | Modifiers::ALT),
            Code::KeyS,
        )),
    );
    let close_editor = MenuItem::new(
        "Close Editor",
        true,
        Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyW)),
    );
    let close_all = MenuItem::new("Close All Editors", true, None);
    let _ = file_m.append_items(&[
        &new_file,
        &PredefinedMenuItem::separator(),
        &open_folder,
        &open_file,
        &PredefinedMenuItem::separator(),
        &save,
        &save_all,
        &PredefinedMenuItem::separator(),
        &close_editor,
        &close_all,
        &close_folder,
    ]);

    // Edit menu. NOTE: we deliberately do NOT use PredefinedMenuItem here.
    // Predefined cut/copy/paste/selectAll bind the native Cmd+X/C/V/A
    // accelerators and route through the NSResponder chain, which cannot reach
    // egui's custom-drawn text — so they would swallow the shortcuts and do
    // nothing. Plain items without accelerators keep the menu present for
    // parity while letting egui's editor handle the shortcuts directly.
    let edit_m = Submenu::new("Edit", true);
    let _ = edit_m.append_items(&[
        &MenuItem::new("Undo", true, None),
        &MenuItem::new("Redo", true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::new("Cut", true, None),
        &MenuItem::new("Copy", true, None),
        &MenuItem::new("Paste", true, None),
        &MenuItem::new("Select All", true, None),
    ]);

    // View menu
    let view_m = Submenu::new("View", true);
    let palette = MenuItem::new(
        "Command Palette…",
        true,
        Some(Accelerator::new(
            Some(Modifiers::SUPER | Modifiers::SHIFT),
            Code::KeyP,
        )),
    );
    let toggle_sidebar = MenuItem::new(
        "Toggle Sidebar Visibility",
        true,
        Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyB)),
    );
    let show_explorer = MenuItem::new(
        "Explorer",
        true,
        Some(Accelerator::new(
            Some(Modifiers::SUPER | Modifiers::SHIFT),
            Code::KeyE,
        )),
    );
    let show_search = MenuItem::new(
        "Search",
        true,
        Some(Accelerator::new(
            Some(Modifiers::SUPER | Modifiers::SHIFT),
            Code::KeyF,
        )),
    );
    // View ▸ Problems / Output / Debug Console / Terminal and the panel
    // toggle (⌘J), as VS Code's View menu.
    let toggle_panel = MenuItem::new("Toggle Panel", true, Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyJ)));
    let show_problems = MenuItem::new("Problems", true, Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyM)));
    let show_output = MenuItem::new("Output", true, Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyU)));
    let show_debug_console = MenuItem::new("Debug Console", true, Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyY)));
    let show_terminal = MenuItem::new("Terminal", true, Some(Accelerator::new(Some(Modifiers::CONTROL), Code::Backquote)));
    let _ = view_m.append_items(&[
        &palette,
        &PredefinedMenuItem::separator(),
        &toggle_sidebar,
        &toggle_panel,
        &show_explorer,
        &show_search,
        &PredefinedMenuItem::separator(),
        &show_problems,
        &show_output,
        &show_debug_console,
        &show_terminal,
    ]);

    // Terminal menu
    let terminal_m = Submenu::new("Terminal", true);
    let new_terminal = MenuItem::new("New Terminal", true, Some(Accelerator::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Backquote)));
    let _ = terminal_m.append(&new_terminal);

    // Go menu
    let go_m = Submenu::new("Go", true);
    let go_to_file = MenuItem::new(
        "Go to File…",
        true,
        Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyP)),
    );
    let _ = go_m.append_items(&[&go_to_file]);

    // Help menu
    let help_m = Submenu::new("Help", true);
    let welcome = MenuItem::new("Welcome", true, None);
    let _ = help_m.append_items(&[&welcome]);

    let _ = menu.append_items(&[&file_m, &edit_m, &view_m, &go_m, &terminal_m, &help_m]);

    #[cfg(target_os = "macos")]
    {
        let _ = menu.init_for_nsapp();
    }
    #[cfg(target_os = "windows")]
    {
        // Windows attaches per-HWND; we do that from the App when the window is ready.
    }

    let ids = MenuIds {
        new_file: new_file.id().clone(),
        open_folder: open_folder.id().clone(),
        open_file: open_file.id().clone(),
        close_folder: close_folder.id().clone(),
        save: save.id().clone(),
        save_all: save_all.id().clone(),
        close_editor: close_editor.id().clone(),
        close_all: close_all.id().clone(),
        palette: palette.id().clone(),
        toggle_sidebar: toggle_sidebar.id().clone(),
        show_explorer: show_explorer.id().clone(),
        show_search: show_search.id().clone(),
        go_to_file: go_to_file.id().clone(),
        welcome: welcome.id().clone(),
        color_theme: color_theme.id().clone(),
        open_settings: open_settings.id().clone(),
        open_keybindings: open_keybindings.id().clone(),
        toggle_panel: toggle_panel.id().clone(),
        show_problems: show_problems.id().clone(),
        show_output: show_output.id().clone(),
        show_debug_console: show_debug_console.id().clone(),
        show_terminal: show_terminal.id().clone(),
        new_terminal: new_terminal.id().clone(),
    };

    InstalledMenu { ids, menu }
}

/// Menu events forwarded by [`repaint_on_event`].
static FORWARDED: std::sync::OnceLock<std::sync::Mutex<std::sync::mpsc::Receiver<MenuEvent>>> =
    std::sync::OnceLock::new();

/// Wake the UI when a menu item fires. A native menu click (or its key
/// equivalent, e.g. ⌘P) produces no window event, so without this egui would
/// not run another frame and the result would only show on the next mouse
/// move.
pub fn repaint_on_event(ctx: &egui::Context) {
    let (tx, rx) = std::sync::mpsc::channel();
    if FORWARDED.set(std::sync::Mutex::new(rx)).is_err() {
        return;
    }
    let ctx = ctx.clone();
    MenuEvent::set_event_handler(Some(move |ev| {
        let _ = tx.send(ev);
        ctx.request_repaint();
    }));
}

pub fn poll_event() -> Option<MenuEvent> {
    match FORWARDED.get() {
        Some(rx) => rx.lock().ok()?.try_recv().ok(),
        None => MenuEvent::receiver().try_recv().ok(),
    }
}
