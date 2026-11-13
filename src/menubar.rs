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

/// The menu items that run a VS Code command, with the key equivalent each
/// was built with. Their accelerators follow keybindings.json
/// ([`MenuKeys::apply`]), as VS Code's menus show each command's
/// keybinding.
#[derive(Clone)]
pub struct MenuKeys {
    items: Vec<(MenuItem, &'static str, Option<Accelerator>)>,
}

impl MenuKeys {
    /// The VS Code command a menu item runs.
    pub fn command(&self, id: &MenuId) -> Option<&'static str> {
        self.items.iter().find(|(m, ..)| m.id() == id).map(|(_, c, _)| *c)
    }

    /// Sets each item's key equivalent: `choose(command)` returns `None` to
    /// keep the built-in one, or `Some(keybinding)` (keybindings.json
    /// spelling, `shift+cmd+s`; `None` inside for no key equivalent).
    pub fn apply(&self, choose: impl Fn(&str) -> Option<Option<String>>) {
        for (item, command, default) in &self.items {
            let accelerator = match choose(command) {
                None => *default,
                Some(key) => key.as_deref().and_then(keybinding_to_accelerator),
            };
            if accelerator.is_none() {
                // muda 0.15 leaves the old key equivalent in place for `None`.
                clear_key_equivalent(&item.text());
            }
            let _ = item.set_accelerator(accelerator);
        }
    }
}

/// Removes the key equivalent of every native menu item titled `title`.
#[cfg(target_os = "macos")]
fn clear_key_equivalent(title: &str) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu};
    use objc2_foundation::NSString;
    fn walk(menu: &NSMenu, title: &NSString) {
        for item in menu.itemArray().iter() {
            if &*item.title() == title {
                item.setKeyEquivalent(&NSString::from_str(""));
                item.setKeyEquivalentModifierMask(NSEventModifierFlags::empty());
            }
            if let Some(sub) = item.submenu() {
                walk(&sub, title);
            }
        }
    }
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    if let Some(menu) = app.mainMenu() {
        walk(&menu, &NSString::from_str(title));
    }
}

#[cfg(not(target_os = "macos"))]
fn clear_key_equivalent(_title: &str) {}

/// A muda accelerator for a single-chord keybinding (`shift+cmd+s`); chords
/// (`cmd+k cmd+t`) have none, as NSMenu can't show them.
pub fn keybinding_to_accelerator(key: &str) -> Option<Accelerator> {
    if key.contains(' ') || key.is_empty() {
        return None;
    }
    let lower = key.to_lowercase();
    let (mods, k) = match lower.rfind('+') {
        Some(i) if i + 1 < lower.len() => (&lower[..i], &lower[i + 1..]),
        _ => ("", lower.as_str()),
    };
    let mut parts: Vec<String> = Vec::new();
    for m in mods.split('+').filter(|p| !p.is_empty()) {
        parts.push(match m {
            "ctrl" => "CTRL",
            "shift" => "SHIFT",
            "alt" | "option" => "ALT",
            // Parsed "CMD" / "SUPER" become META, which macOS ignores;
            // "CMDORCTRL" is SUPER, the Command key.
            "cmd" | "meta" | "win" | "super" => "CMDORCTRL",
            _ => return None,
        }.to_string());
    }
    parts.push(k.to_uppercase());
    parts.join("+").parse().ok()
}

pub struct InstalledMenu {
    pub ids: MenuIds,
    pub keys: MenuKeys,
    // Keep the Menu alive — muda relies on its owned handles for the lifetime of the menubar.
    #[allow(dead_code)]
    menu: Menu,
}

pub fn install() -> InstalledMenu {
    let menu = Menu::new();
    // Code ▸ Settings ▸ Themes ▸ Color Theme (⌘K ⌘T is a chord, which a
    // native menu can't show, as in VS Code).
    let color_theme = item("Color Theme", None);
    // Code ▸ Settings ▸ Settings (⌘,).
    let open_settings = item("Settings", Some(Accelerator::new(Some(Modifiers::SUPER), Code::Comma)));
    // Code ▸ Settings ▸ Keyboard Shortcuts (⌘K ⌘S, a chord).
    let open_keybindings = item("Keyboard Shortcuts", None);

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
    let new_file = item("New Text File", Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyN)),
    );
    let open_folder = item("Open Folder…", None);
    let open_file = item("Open File…", None);
    let close_folder = item("Close Folder", None);
    let save = item("Save", Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyS)),
    );
    let save_all = item("Save All", Some(Accelerator::new(
            Some(Modifiers::SUPER | Modifiers::ALT),
            Code::KeyS,
        )),
    );
    let close_editor = item("Close Editor", Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyW)),
    );
    let close_all = item("Close All Editors", None);
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
    let palette = item("Command Palette…", Some(Accelerator::new(
            Some(Modifiers::SUPER | Modifiers::SHIFT),
            Code::KeyP,
        )),
    );
    let toggle_sidebar = item("Toggle Sidebar Visibility", Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyB)),
    );
    let show_explorer = item("Explorer", Some(Accelerator::new(
            Some(Modifiers::SUPER | Modifiers::SHIFT),
            Code::KeyE,
        )),
    );
    let show_search = item("Search", Some(Accelerator::new(
            Some(Modifiers::SUPER | Modifiers::SHIFT),
            Code::KeyF,
        )),
    );
    // View ▸ Problems / Output / Debug Console / Terminal and the panel
    // toggle (⌘J), as VS Code's View menu.
    let toggle_panel = item("Toggle Panel", Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyJ)));
    let show_problems = item("Problems", Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyM)));
    let show_output = item("Output", Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyU)));
    let show_debug_console = item("Debug Console", Some(Accelerator::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyY)));
    let show_terminal = item("Terminal", Some(Accelerator::new(Some(Modifiers::CONTROL), Code::Backquote)));
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
    let new_terminal = item("New Terminal", Some(Accelerator::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Backquote)));
    let _ = terminal_m.append(&new_terminal);

    // Go menu
    let go_m = Submenu::new("Go", true);
    let go_to_file = item("Go to File…", Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyP)),
    );
    let _ = go_m.append_items(&[&go_to_file]);

    // Help menu
    let help_m = Submenu::new("Help", true);
    let welcome = item("Welcome", None);
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

    let keys = MenuKeys {
        items: [
            (&new_file, "workbench.action.files.newUntitledFile"),
            (&open_folder, "workbench.action.files.openFolder"),
            (&open_file, "workbench.action.files.openFile"),
            (&close_folder, "workbench.action.closeFolder"),
            (&save, "workbench.action.files.save"),
            (&save_all, "workbench.action.files.saveAll"),
            (&close_editor, "workbench.action.closeActiveEditor"),
            (&close_all, "workbench.action.closeAllEditors"),
            (&palette, "workbench.action.showCommands"),
            (&toggle_sidebar, "workbench.action.toggleSidebarVisibility"),
            (&show_explorer, "workbench.view.explorer"),
            (&show_search, "workbench.view.search"),
            (&go_to_file, "workbench.action.quickOpen"),
            (&welcome, "workbench.action.openWalkthrough"),
            (&color_theme, "workbench.action.selectTheme"),
            (&open_settings, "workbench.action.openSettings"),
            (&open_keybindings, "workbench.action.openGlobalKeybindings"),
            (&toggle_panel, "workbench.action.togglePanel"),
            (&show_problems, "workbench.actions.view.problems"),
            (&show_output, "workbench.action.output.toggleOutput"),
            (&show_debug_console, "workbench.debug.action.toggleRepl"),
            (&show_terminal, "workbench.action.terminal.toggleTerminal"),
            (&new_terminal, "workbench.action.terminal.new"),
        ]
        .into_iter()
        .map(|(item, command)| ((*item).clone(), command, item_accelerator(item)))
        .collect(),
    };

    InstalledMenu { ids, keys, menu }
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

/// The accelerator an item was built with (muda has no getter, so it is read
/// back from the platform item's title-independent state we set here).
fn item_accelerator(item: &MenuItem) -> Option<Accelerator> {
    BUILT_WITH.with(|m| m.borrow().iter().find(|(id, _)| id == item.id()).and_then(|(_, a)| *a))
}

thread_local! {
    static BUILT_WITH: std::cell::RefCell<Vec<(MenuId, Option<Accelerator>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// `MenuItem::new` that remembers the accelerator for [`MenuKeys`].
fn item(text: &str, accelerator: Option<Accelerator>) -> MenuItem {
    let m = MenuItem::new(text, true, accelerator);
    BUILT_WITH.with(|b| b.borrow_mut().push((m.id().clone(), accelerator)));
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keybindings_become_accelerators() {
        assert_eq!(keybinding_to_accelerator("shift+cmd+s"), Some(Accelerator::new(Some(Modifiers::SHIFT | Modifiers::SUPER), Code::KeyS)));
        assert_eq!(keybinding_to_accelerator("ctrl+`"), Some(Accelerator::new(Some(Modifiers::CONTROL), Code::Backquote)));
        assert_eq!(keybinding_to_accelerator("alt+cmd+up"), Some(Accelerator::new(Some(Modifiers::ALT | Modifiers::SUPER), Code::ArrowUp)));
        assert_eq!(keybinding_to_accelerator("f5"), Some(Accelerator::new(None, Code::F5)));
        assert_eq!(keybinding_to_accelerator("cmd+k cmd+t"), None);
    }
}
