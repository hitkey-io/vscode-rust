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
    pub welcome: MenuId,
}

pub struct InstalledMenu {
    pub ids: MenuIds,
    // Keep the Menu alive — muda relies on its owned handles for the lifetime of the menubar.
    #[allow(dead_code)]
    menu: Menu,
}

pub fn install() -> InstalledMenu {
    let menu = Menu::new();

    // macOS application submenu — Apple, About, Services, Hide/Show, Quit
    #[cfg(target_os = "macos")]
    {
        let app_m = Submenu::new("vscode-rust", true);
        let _ = menu.append(&app_m);
        let _ = app_m.append_items(&[
            &PredefinedMenuItem::about(Some("About vscode-rust"), None),
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

    // Edit menu — predefined items for standard text editing
    let edit_m = Submenu::new("Edit", true);
    let _ = edit_m.append_items(&[
        &PredefinedMenuItem::undo(None),
        &PredefinedMenuItem::redo(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::cut(None),
        &PredefinedMenuItem::copy(None),
        &PredefinedMenuItem::paste(None),
        &PredefinedMenuItem::select_all(None),
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
    let _ = view_m.append_items(&[
        &palette,
        &PredefinedMenuItem::separator(),
        &toggle_sidebar,
        &show_explorer,
        &show_search,
    ]);

    // Help menu
    let help_m = Submenu::new("Help", true);
    let welcome = MenuItem::new("Welcome", true, None);
    let _ = help_m.append_items(&[&welcome]);

    let _ = menu.append_items(&[&file_m, &edit_m, &view_m, &help_m]);

    #[cfg(target_os = "macos")]
    {
        let _ = menu.init_for_nsapp();
    }
    #[cfg(target_os = "windows")]
    {
        // Windows attaches per-HWND; we do that from the App when the window is ready.
    }

    let ids = MenuIds {
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
        welcome: welcome.id().clone(),
    };

    InstalledMenu { ids, menu }
}

pub fn poll_event() -> Option<MenuEvent> {
    MenuEvent::receiver().try_recv().ok()
}
