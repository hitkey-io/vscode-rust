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
    /// The title of the item running `id`'s command (without a `[⌘K …]` chord).
    pub fn title(&self, id: &MenuId) -> Option<String> {
        let item = self.items.iter().find(|(m, ..)| m.id() == id).map(|(m, ..)| m.text())?;
        Some(item.split(" [").next().unwrap_or(&item).to_string())
    }

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

/// One entry of the menu bar table.
enum E {
    Sep,
    /// Title, VS Code command id, default keybinding (keybindings.json
    /// spelling; chords are shown in the title like VS Code's `[⌘K ⌘S]`).
    Item(&'static str, &'static str, &'static str),
    /// The same, greyed out (nothing here can run it).
    Off(&'static str, &'static str, &'static str),
    Sub(&'static str, &'static [E]),
}
use E::{Item, Off, Sep, Sub};

/// Commands the app runs (`App::run_vscode_command`): only their menu items
/// bind a key equivalent, so a key that the editor handles itself isn't
/// swallowed by a menu item that does nothing.
const IMPLEMENTED: &[&str] = &[
    "editor.action.goToReferences",
    "workbench.action.files.newUntitledFile",
    "workbench.action.files.openFolder",
    "workbench.action.files.openFileFolder",
    "workbench.action.closeFolder",
    "workbench.action.files.save",
    "workbench.action.files.saveAll",
    "workbench.action.closeActiveEditor",
    "workbench.action.closeAllEditors",
    "workbench.action.closeWindow",
    "workbench.action.showCommands",
    "workbench.action.toggleSidebarVisibility",
    "workbench.view.explorer",
    "workbench.view.search",
    "workbench.view.scm",
    "workbench.view.debug",
    "workbench.view.extensions",
    "workbench.action.quickOpen",
    "workbench.action.gotoLine",
    "editor.action.jumpToBracket",
    "editor.action.duplicateSelection",
    "editor.action.smartSelect.expand",
    "editor.action.smartSelect.shrink",
    "workbench.action.clearRecentFiles",
    "workbench.action.reopenClosedEditor",
    "workbench.action.openRecent",
    "workbench.action.openWalkthrough",
    "workbench.action.selectTheme",
    "workbench.action.openSettings",
    "workbench.action.openGlobalKeybindings",
    "workbench.action.togglePanel",
    "workbench.actions.view.problems",
    "workbench.action.output.toggleOutput",
    "workbench.debug.action.toggleRepl",
    "workbench.action.terminal.toggleTerminal",
    "workbench.action.terminal.new",
    "workbench.action.terminal.split",
    "workbench.action.splitEditor",
    "workbench.action.nextEditor",
    "workbench.action.previousEditor",
    "workbench.action.toggleFullScreen",
    "workbench.action.zoomIn",
    "workbench.action.zoomOut",
    // The clipboard and undo items hand egui the events its text widgets
    // react to (the editor can't be reached through the responder chain).
    "undo",
    "redo",
    "editor.action.clipboardCutAction",
    "editor.action.clipboardCopyAction",
    "editor.action.clipboardPasteAction",
    "editor.action.selectAll",
    "editor.action.moveLinesUpAction",
    "editor.action.moveLinesDownAction",
    "editor.action.copyLinesUpAction",
    "editor.action.copyLinesDownAction",
    "editor.action.commentLine",
    "editor.action.blockComment",
];

const PREFERENCES: &[E] = &[
    Item("Profiles", "workbench.profiles.actions.manageProfiles", ""),
    Item("Settings", "workbench.action.openSettings", "cmd+,"),
    Item("Extensions", "workbench.view.extensions", "shift+cmd+x"),
    Item("Keyboard Shortcuts [⌘K ⌘S]", "workbench.action.openGlobalKeybindings", ""),
    Item("Configure Snippets", "workbench.action.openSnippets", ""),
    Item("Tasks", "workbench.action.tasks.openUserTasks", ""),
    Sub("Themes", &[
        Item("Color Theme [⌘K ⌘T]", "workbench.action.selectTheme", ""),
        Item("File Icon Theme", "workbench.action.selectIconTheme", ""),
        Item("Product Icon Theme", "workbench.action.selectProductIconTheme", ""),
    ]),
    Sep,
    Item("Backup and Sync Settings…", "workbench.userDataSync.actions.turnOn", ""),
    Sep,
    Item("Online Services Settings", "workbench.action.openApplicationSettings", ""),
];

const FILE: &[E] = &[
    Item("New Text File", "workbench.action.files.newUntitledFile", "cmd+n"),
    Item("New File…", "welcome.showNewFileEntries", "ctrl+alt+cmd+n"),
    Item("New Window", "workbench.action.newWindow", "shift+cmd+n"),
    Sub("New Window with Profile", &[Item("New Profile…", "workbench.profiles.actions.createProfile", "")]),
    Sep,
    Item("Open…", "workbench.action.files.openFileFolder", "cmd+o"),
    Item("Open Folder…", "workbench.action.files.openFolder", ""),
    Item("Open Workspace from File…", "workbench.action.openWorkspace", ""),
    Sub("Open Recent", &[
        Item("Reopen Closed Editor", "workbench.action.reopenClosedEditor", "shift+cmd+t"),
        Sep,
        Item("More… [⌃R]", "workbench.action.openRecent", ""),
        Sep,
        Item("Clear Recently Opened…", "workbench.action.clearRecentFiles", ""),
    ]),
    Sep,
    Item("Add Folder to Workspace…", "workbench.action.addRootFolder", ""),
    Item("Save Workspace As…", "workbench.action.saveWorkspaceAs", ""),
    Item("Duplicate Workspace", "workbench.action.duplicateWorkspaceInNewWindow", ""),
    Sep,
    Item("Save", "workbench.action.files.save", "cmd+s"),
    Item("Save As…", "workbench.action.files.saveAs", "shift+cmd+s"),
    Item("Save All", "workbench.action.files.saveAll", "alt+cmd+s"),
    Sep,
    Sub("Share", &[Item("Export Profile (Default)…", "workbench.profiles.actions.exportProfile", "")]),
    Sep,
    Item("Auto Save", "workbench.action.toggleAutoSave", ""),
    Sep,
    Item("Revert File", "workbench.action.files.revert", ""),
    Item("Close Editor", "workbench.action.closeActiveEditor", "cmd+w"),
    Item("Close Folder [⌘K F]", "workbench.action.closeFolder", ""),
    Item("Close Window", "workbench.action.closeWindow", "shift+cmd+w"),
];

const EDIT: &[E] = &[
    Item("Undo", "undo", "cmd+z"),
    Item("Redo", "redo", "shift+cmd+z"),
    Sep,
    Item("Cut", "editor.action.clipboardCutAction", "cmd+x"),
    Item("Copy", "editor.action.clipboardCopyAction", "cmd+c"),
    Item("Paste", "editor.action.clipboardPasteAction", "cmd+v"),
    Sep,
    Item("Find", "actions.find", "cmd+f"),
    Item("Replace", "editor.action.startFindReplace", "alt+cmd+f"),
    Sep,
    Item("Find in Files", "workbench.action.findInFiles", "shift+cmd+f"),
    Item("Replace in Files", "workbench.action.replaceInFiles", "shift+cmd+h"),
    Sep,
    Item("Toggle Line Comment", "editor.action.commentLine", "cmd+/"),
    Item("Toggle Block Comment", "editor.action.blockComment", "alt+shift+a"),
    Item("Emmet: Expand Abbreviation", "editor.emmet.action.expandAbbreviation", ""),
];

const SELECTION: &[E] = &[
    Item("Select All", "editor.action.selectAll", "cmd+a"),
    Item("Expand Selection", "editor.action.smartSelect.expand", "ctrl+shift+cmd+right"),
    Item("Shrink Selection", "editor.action.smartSelect.shrink", "ctrl+shift+cmd+left"),
    Sep,
    Item("Copy Line Up", "editor.action.copyLinesUpAction", "alt+shift+up"),
    Item("Copy Line Down", "editor.action.copyLinesDownAction", "alt+shift+down"),
    Item("Move Line Up", "editor.action.moveLinesUpAction", "alt+up"),
    Item("Move Line Down", "editor.action.moveLinesDownAction", "alt+down"),
    Item("Duplicate Selection", "editor.action.duplicateSelection", ""),
    Sep,
    Item("Add Cursor Above", "editor.action.insertCursorAbove", "alt+cmd+up"),
    Item("Add Cursor Below", "editor.action.insertCursorBelow", "alt+cmd+down"),
    Item("Add Cursors to Line Ends", "editor.action.insertCursorAtEndOfEachLineSelected", "alt+shift+i"),
    Item("Add Next Occurrence", "editor.action.addSelectionToNextFindMatch", "cmd+d"),
    Item("Add Previous Occurrence", "editor.action.addSelectionToPreviousFindMatch", ""),
    Item("Select All Occurrences", "editor.action.selectHighlights", "shift+cmd+l"),
    Sep,
    Item("Switch to Cmd+Click for Multi-Cursor", "workbench.action.toggleMultiCursorModifier", ""),
    Item("Column Selection Mode", "editor.action.toggleColumnSelection", ""),
];

const APPEARANCE: &[E] = &[
    Item("Full Screen", "workbench.action.toggleFullScreen", "ctrl+cmd+f"),
    Item("Zen Mode [⌘K Z]", "workbench.action.toggleZenMode", ""),
    Item("Centered Layout", "workbench.action.toggleCenteredLayout", ""),
    Sep,
    Item("Primary Side Bar", "workbench.action.toggleSidebarVisibility", "cmd+b"),
    Item("Secondary Side Bar", "workbench.action.toggleAuxiliaryBar", "alt+cmd+b"),
    Item("Status Bar", "workbench.action.toggleStatusbarVisibility", ""),
    Item("Panel", "workbench.action.togglePanel", "cmd+j"),
    Sep,
    Item("Move Primary Side Bar Right", "workbench.action.moveSideBarRight", ""),
    Sub("Activity Bar Position", &[
        Item("Default", "workbench.action.activityBarLocation.default", ""),
        Item("Top", "workbench.action.activityBarLocation.top", ""),
        Item("Bottom", "workbench.action.activityBarLocation.bottom", ""),
        Item("Hidden", "workbench.action.activityBarLocation.hide", ""),
    ]),
    Sub("Panel Position", &[
        Item("Top", "workbench.action.positionPanelTop", ""),
        Item("Left", "workbench.action.positionPanelLeft", ""),
        Item("Right", "workbench.action.positionPanelRight", ""),
        Item("Bottom", "workbench.action.positionPanelBottom", ""),
    ]),
    Sub("Align Panel", &[
        Item("Center", "workbench.action.alignPanelCenter", ""),
        Item("Justify", "workbench.action.alignPanelJustify", ""),
        Item("Left", "workbench.action.alignPanelLeft", ""),
        Item("Right", "workbench.action.alignPanelRight", ""),
    ]),
    Sub("Tab Bar", &[
        Item("Multiple Tabs", "workbench.action.showMultipleEditorTabs", ""),
        Item("Single Tab", "workbench.action.showEditorTab", ""),
        Item("Hidden", "workbench.action.hideEditorTabs", ""),
    ]),
    Sub("Editor Actions Position", &[
        Item("Tab Bar", "workbench.action.editorActionsDefault", ""),
        Item("Title Bar", "workbench.action.editorActionsTitleBar", ""),
        Item("Hidden", "workbench.action.editorActionsHidden", ""),
    ]),
    Sep,
    Item("Minimap", "editor.action.toggleMinimap", ""),
    Item("Breadcrumbs", "breadcrumbs.toggle", ""),
    Item("Sticky Scroll", "editor.action.toggleStickyScroll", ""),
    Sub("Render Whitespace", &[
        Item("Set Render Whitespace to None", "editor.action.renderWhitespace.none", ""),
        Item("Set Render Whitespace to Boundary", "editor.action.renderWhitespace.boundary", ""),
        Item("Set Render Whitespace to Selection", "editor.action.renderWhitespace.selection", ""),
        Item("Set Render Whitespace to Trailing", "editor.action.renderWhitespace.trailing", ""),
        Item("Set Render Whitespace to All", "editor.action.renderWhitespace.all", ""),
    ]),
    Item("Render Control Characters", "editor.action.toggleRenderControlCharacter", ""),
    Sep,
    Item("Zoom In", "workbench.action.zoomIn", "cmd+="),
    Item("Zoom Out", "workbench.action.zoomOut", "cmd+-"),
    Item("Reset Zoom", "workbench.action.zoomReset", ""),
];

const EDITOR_LAYOUT: &[E] = &[
    Item("Split Up [⌘K ⌘\\]", "workbench.action.splitEditorUp", ""),
    Item("Split Down", "workbench.action.splitEditorDown", ""),
    Item("Split Left", "workbench.action.splitEditorLeft", ""),
    Item("Split Right", "workbench.action.splitEditor", ""),
    Sep,
    Item("Move Editor into New Window", "workbench.action.moveEditorToNewWindow", ""),
    Item("Copy Editor into New Window [⌘K O]", "workbench.action.copyEditorToNewWindow", ""),
    Sep,
    Item("Single", "workbench.action.editorLayoutSingle", ""),
    Item("Two Columns", "workbench.action.editorLayoutTwoColumns", ""),
    Item("Three Columns", "workbench.action.editorLayoutThreeColumns", ""),
    Item("Two Rows", "workbench.action.editorLayoutTwoRows", ""),
    Item("Three Rows", "workbench.action.editorLayoutThreeRows", ""),
    Item("Grid (2x2)", "workbench.action.editorLayoutTwoByTwoGrid", ""),
    Item("Two Rows Right", "workbench.action.editorLayoutTwoRowsRight", ""),
    Item("Two Columns Bottom", "workbench.action.editorLayoutTwoColumnsBottom", ""),
    Sep,
    Item("Flip Layout", "workbench.action.flipEditorLayout", "alt+cmd+0"),
];

const VIEW: &[E] = &[
    Item("Command Palette…", "workbench.action.showCommands", "shift+cmd+p"),
    Item("Open View…", "workbench.action.openView", ""),
    Sep,
    Sub("Appearance", APPEARANCE),
    Sub("Editor Layout", EDITOR_LAYOUT),
    Sep,
    Item("Explorer", "workbench.view.explorer", "shift+cmd+e"),
    Item("Search", "workbench.view.search", "shift+cmd+f"),
    Item("Source Control", "workbench.view.scm", "ctrl+shift+g"),
    Item("Run", "workbench.view.debug", "shift+cmd+d"),
    Item("Extensions", "workbench.view.extensions", "shift+cmd+x"),
    Sep,
    Item("Browser", "workbench.action.browser.open", "alt+cmd+/"),
    Sep,
    Item("Problems", "workbench.actions.view.problems", "shift+cmd+m"),
    Item("Output", "workbench.action.output.toggleOutput", "shift+cmd+u"),
    Item("Debug Console", "workbench.debug.action.toggleRepl", "shift+cmd+y"),
    Item("Terminal", "workbench.action.terminal.toggleTerminal", "ctrl+`"),
    Sep,
    Off("Word Wrap", "editor.action.toggleWordWrap", "alt+z"),
];

const SWITCH_EDITOR: &[E] = &[
    Item("Next Editor", "workbench.action.nextEditor", "alt+cmd+right"),
    Item("Previous Editor", "workbench.action.previousEditor", "alt+cmd+left"),
    Sep,
    Item("Next Used Editor", "workbench.action.openNextRecentlyUsedEditor", ""),
    Item("Previous Used Editor", "workbench.action.openPreviousRecentlyUsedEditor", ""),
    Sep,
    Item("Next Editor in Group [⌘K ⌥⌘→]", "workbench.action.nextEditorInGroup", ""),
    Item("Previous Editor in Group [⌘K ⌥⌘←]", "workbench.action.previousEditorInGroup", ""),
    Sep,
    Item("Next Used Editor in Group", "workbench.action.openNextRecentlyUsedEditorInGroup", ""),
    Item("Previous Used Editor in Group", "workbench.action.openPreviousRecentlyUsedEditorInGroup", ""),
];

const SWITCH_GROUP: &[E] = &[
    Item("Group 1", "workbench.action.focusFirstEditorGroup", "cmd+1"),
    Item("Group 2", "workbench.action.focusSecondEditorGroup", "cmd+2"),
    Off("Group 3", "workbench.action.focusThirdEditorGroup", "cmd+3"),
    Off("Group 4", "workbench.action.focusFourthEditorGroup", "cmd+4"),
    Off("Group 5", "workbench.action.focusFifthEditorGroup", "cmd+5"),
    Sep,
    Item("Next Group", "workbench.action.focusNextGroup", ""),
    Item("Previous Group", "workbench.action.focusPreviousGroup", ""),
    Sep,
    Item("Group Left [⌘K ⌘←]", "workbench.action.focusLeftGroup", ""),
    Item("Group Right [⌘K ⌘→]", "workbench.action.focusRightGroup", ""),
    Item("Group Above [⌘K ⌘↑]", "workbench.action.focusAboveGroup", ""),
    Item("Group Below [⌘K ⌘↓]", "workbench.action.focusBelowGroup", ""),
];

const GO: &[E] = &[
    Off("Back", "workbench.action.navigateBack", "ctrl+-"),
    Off("Forward", "workbench.action.navigateForward", "ctrl+shift+-"),
    Off("Last Edit Location [⌘K ⌘Q]", "workbench.action.navigateToLastEditLocation", ""),
    Sep,
    Sub("Switch Editor", SWITCH_EDITOR),
    Sub("Switch Group", SWITCH_GROUP),
    Sep,
    Item("Go to File…", "workbench.action.quickOpen", "cmd+p"),
    Item("Go to Symbol in Workspace…", "workbench.action.showAllSymbols", "cmd+t"),
    Sep,
    Item("Go to Symbol in Editor…", "workbench.action.gotoSymbol", "shift+cmd+o"),
    Item("Go to Definition", "editor.action.revealDefinition", "f12"),
    Item("Go to Declaration", "editor.action.revealDeclaration", ""),
    Item("Go to Type Definition", "editor.action.goToTypeDefinition", ""),
    Item("Go to Implementations", "editor.action.goToImplementation", "cmd+f12"),
    Item("Go to References", "editor.action.goToReferences", "shift+f12"),
    Sep,
    Item("Go to Line/Column…", "workbench.action.gotoLine", "ctrl+g"),
    Item("Go to Bracket", "editor.action.jumpToBracket", "shift+cmd+\\"),
    Sep,
    Item("Next Problem", "editor.action.marker.nextInFiles", "f8"),
    Item("Previous Problem", "editor.action.marker.prevInFiles", "shift+f8"),
    Sep,
    Item("Next Change", "workbench.action.editor.nextChange", "alt+f3"),
    Item("Previous Change", "workbench.action.editor.previousChange", "alt+shift+f3"),
];

const RUN: &[E] = &[
    Item("Start Debugging", "workbench.action.debug.start", "f5"),
    Item("Run Without Debugging", "workbench.action.debug.run", "ctrl+f5"),
    Off("Stop Debugging", "workbench.action.debug.stop", "shift+f5"),
    Off("Restart Debugging", "workbench.action.debug.restart", "shift+cmd+f5"),
    Sep,
    Off("Open Configurations", "workbench.action.debug.configure", ""),
    Item("Add Configuration…", "debug.addConfiguration", ""),
    Sep,
    Off("Step Over", "workbench.action.debug.stepOver", "f10"),
    Off("Step Into", "workbench.action.debug.stepInto", "f11"),
    Off("Step Out", "workbench.action.debug.stepOut", "shift+f11"),
    Off("Continue", "workbench.action.debug.continue", "f5"),
    Sep,
    Item("Toggle Breakpoint", "editor.debug.action.toggleBreakpoint", "f9"),
    Sub("New Breakpoint", &[
        Item("Conditional Breakpoint…", "editor.debug.action.conditionalBreakpoint", ""),
        Item("Edit Breakpoint", "editor.debug.action.editBreakpoint", ""),
        Item("Inline Breakpoint", "editor.debug.action.toggleInlineBreakpoint", "shift+f9"),
        Item("Function Breakpoint…", "workbench.debug.viewlet.action.addFunctionBreakpointAction", ""),
        Item("Logpoint…", "editor.debug.action.addLogPoint", ""),
        Item("Triggered Breakpoint…", "editor.debug.action.triggerByBreakpoint", ""),
    ]),
    Sep,
    Item("Enable All Breakpoints", "workbench.debug.viewlet.action.enableAllBreakpoints", ""),
    Item("Disable All Breakpoints", "workbench.debug.viewlet.action.disableAllBreakpoints", ""),
    Item("Remove All Breakpoints", "workbench.debug.viewlet.action.removeAllBreakpoints", ""),
    Sep,
    Item("Install Additional Debuggers…", "debug.installAdditionalDebuggers", ""),
];

const TERMINAL: &[E] = &[
    Item("New Terminal", "workbench.action.terminal.new", "ctrl+shift+`"),
    Item("Split Terminal", "workbench.action.terminal.split", "cmd+\\"),
    Off("New Terminal Window", "workbench.action.terminal.newInNewWindow", "ctrl+alt+shift+`"),
    Sep,
    Item("Run Task…", "workbench.action.tasks.runTask", ""),
    Item("Run Build Task…", "workbench.action.tasks.build", "shift+cmd+b"),
    Item("Run Active File", "workbench.action.terminal.runActiveFile", ""),
    Item("Run Selected Text", "workbench.action.terminal.runSelectedText", ""),
    Sep,
    Off("Show Running Tasks…", "workbench.action.tasks.showTasks", ""),
    Off("Restart Running Task…", "workbench.action.tasks.restartTask", ""),
    Off("Terminate Task…", "workbench.action.tasks.terminate", ""),
    Sep,
    Item("Configure Tasks…", "workbench.action.tasks.configureTaskRunner", ""),
    Item("Configure Default Build Task…", "workbench.action.tasks.configureDefaultBuildTask", ""),
];

const WINDOW: &[E] = &[
    Item("Switch Window…", "workbench.action.quickSwitchWindow", ""),
];

const HELP: &[E] = &[
    Item("Welcome", "workbench.action.openWalkthrough", ""),
    Item("Show All Commands", "workbench.action.showCommands", "shift+cmd+p"),
    Item("Documentation", "workbench.action.openDocumentationUrl", ""),
    Item("Editor Playground", "workbench.action.showInteractivePlayground", ""),
    Item("Open Walkthrough…", "workbench.action.openWalkthrough.pick", ""),
    Item("Show Release Notes", "update.showCurrentReleaseNotes", ""),
    Item("Get Started with Accessibility Features", "workbench.action.openAccessibilityHelp", ""),
    Sep,
    Item("Keyboard Shortcuts Reference [⌘K ⌘R]", "workbench.action.keybindingsReference", ""),
    Item("Video Tutorials", "workbench.action.openVideoTutorialsUrl", ""),
    Item("Tips and Tricks", "workbench.action.openTipsAndTricksUrl", ""),
    Sep,
    Item("Join Us on YouTube", "workbench.action.openYouTubeUrl", ""),
    Item("Search Feature Requests", "workbench.action.openRequestFeatureUrl", ""),
    Item("Report Issue", "workbench.action.openIssueReporter", ""),
    Sep,
    Item("View License", "workbench.action.openLicenseUrl", ""),
    Item("Privacy Statement", "workbench.action.openPrivacyStatementUrl", ""),
    Sep,
    Item("Toggle Developer Tools", "workbench.action.toggleDevTools", ""),
    Item("Open Process Explorer", "workbench.action.openProcessExplorer", ""),
];

type Built = Vec<(MenuItem, &'static str, Option<Accelerator>)>;

/// Builds `entries` into `parent` (a `Menu` or `Submenu`), collecting the
/// command items.
fn build(entries: &[E], parent: &dyn muda::ContextMenu, append: &dyn Fn(&dyn muda::IsMenuItem), built: &mut Built) {
    let _ = parent;
    for e in entries {
        match e {
            Sep => append(&PredefinedMenuItem::separator()),
            Item(title, command, key) | Off(title, command, key) => {
                let enabled = matches!(e, Item(..));
                let accelerator = if IMPLEMENTED.contains(command) { keybinding_to_accelerator(key) } else { None };
                let m = MenuItem::new(*title, enabled, accelerator);
                built.push((m.clone(), command, accelerator));
                append(&m);
            }
            Sub(title, children) => {
                let sub = Submenu::new(*title, true);
                let first = built.len();
                build(children, &sub, &|i| {
                    let _ = sub.append(i);
                }, built);
                if *title == "Open Recent" {
                    let item = |command: &str| built[first..].iter().find(|(_, c, _)| *c == command).map(|(m, ..)| m.clone());
                    RECENT.with(|r| {
                        *r.borrow_mut() = Some(RecentMenu {
                            sub: sub.clone(),
                            reopen: item("workbench.action.reopenClosedEditor"),
                            more: item("workbench.action.openRecent"),
                            clear: item("workbench.action.clearRecentFiles"),
                            dynamic: Vec::new(),
                        });
                    });
                }
                append(&sub);
            }
        }
    }
}

pub fn install() -> InstalledMenu {
    // macOS adds "Enter Full Screen" to every View menu unless told not to
    // (Electron does the same): VS Code's own Full Screen item is in
    // View ▸ Appearance.
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSString, NSUserDefaults};
        NSUserDefaults::standardUserDefaults().setBool_forKey(false, &NSString::from_str("NSFullScreenMenuItemEverywhere"));
    }
    let menu = Menu::new();
    let mut built: Built = Vec::new();

    // macOS application menu: About, Check for Updates…, Preferences ▸,
    // Services ▸, Hide, Hide Others, Show All, Quit (names the app).
    #[cfg(target_os = "macos")]
    {
        let app_m = Submenu::new("vscode-rust", true);
        let _ = menu.append(&app_m);
        let updates = MenuItem::new("Check for Updates…", true, None);
        built.push((updates.clone(), "update.checkForUpdate", None));
        let preferences = Submenu::new("Preferences", true);
        build(PREFERENCES, &preferences, &|i| {
            let _ = preferences.append(i);
        }, &mut built);
        let _ = app_m.append_items(&[
            &PredefinedMenuItem::about(Some("About vscode-rust"), None),
            &updates,
            &PredefinedMenuItem::separator(),
            &preferences,
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

    // Edit: plain items (not the predefined cut/copy/paste that route through
    // the responder chain, which can't reach egui's custom-drawn text). Only
    // implemented commands bind a key equivalent, so ⌘C and friends still
    // reach the editor.
    let top: [(&str, &[E]); 7] = [("File", FILE), ("Edit", EDIT), ("Selection", SELECTION), ("View", VIEW), ("Go", GO), ("Run", RUN), ("Terminal", TERMINAL)];
    for (title, entries) in top {
        let sub = Submenu::new(title, true);
        build(entries, &sub, &|i| {
            let _ = sub.append(i);
        }, &mut built);
        let _ = menu.append(&sub);
    }

    // Window: Minimize / Zoom, the switcher, and (macOS) the system's window list.
    let window_m = Submenu::new("Window", true);
    let _ = window_m.append_items(&[&PredefinedMenuItem::minimize(None), &PredefinedMenuItem::maximize(None), &PredefinedMenuItem::separator()]);
    build(WINDOW, &window_m, &|i| {
        let _ = window_m.append(i);
    }, &mut built);
    let _ = window_m.append_items(&[&PredefinedMenuItem::separator(), &PredefinedMenuItem::bring_all_to_front(None)]);
    let _ = menu.append(&window_m);

    let help_m = Submenu::new("Help", true);
    build(HELP, &help_m, &|i| {
        let _ = help_m.append(i);
    }, &mut built);
    let _ = menu.append(&help_m);

    #[cfg(target_os = "macos")]
    {
        let _ = menu.init_for_nsapp();
        let _ = window_m.set_as_windows_menu_for_nsapp();
        let _ = help_m.set_as_help_menu_for_nsapp();
    }
    #[cfg(target_os = "windows")]
    {
        // Windows attaches per-HWND; we do that from the App when the window is ready.
    }

    // The ids the app keeps for its own items (by command); a command that is
    // in the table twice (Settings, Extensions…) takes its first item.
    let id_of = |command: &str| built.iter().find(|(_, c, _)| *c == command).map(|(m, ..)| m.id().clone()).unwrap_or_else(|| MenuId::new(command));
    let ids = MenuIds {
        new_file: id_of("workbench.action.files.newUntitledFile"),
        open_folder: id_of("workbench.action.files.openFolder"),
        open_file: id_of("workbench.action.files.openFileFolder"),
        close_folder: id_of("workbench.action.closeFolder"),
        save: id_of("workbench.action.files.save"),
        save_all: id_of("workbench.action.files.saveAll"),
        close_editor: id_of("workbench.action.closeActiveEditor"),
        close_all: id_of("workbench.action.closeAllEditors"),
        palette: id_of("workbench.action.showCommands"),
        toggle_sidebar: id_of("workbench.action.toggleSidebarVisibility"),
        show_explorer: id_of("workbench.view.explorer"),
        show_search: id_of("workbench.view.search"),
        go_to_file: id_of("workbench.action.quickOpen"),
        welcome: id_of("workbench.action.openWalkthrough"),
        color_theme: id_of("workbench.action.selectTheme"),
        open_settings: id_of("workbench.action.openSettings"),
        open_keybindings: id_of("workbench.action.openGlobalKeybindings"),
        toggle_panel: id_of("workbench.action.togglePanel"),
        show_problems: id_of("workbench.actions.view.problems"),
        show_output: id_of("workbench.action.output.toggleOutput"),
        show_debug_console: id_of("workbench.debug.action.toggleRepl"),
        show_terminal: id_of("workbench.action.terminal.toggleTerminal"),
        new_terminal: id_of("workbench.action.terminal.new"),
    };
    let keys = MenuKeys { items: built };

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

/// File ▸ Open Recent: the submenu, its fixed items and the entries put in
/// for the recent folders and files (ids `recent:<path>`).
struct RecentMenu {
    sub: Submenu,
    reopen: Option<MenuItem>,
    more: Option<MenuItem>,
    clear: Option<MenuItem>,
    dynamic: Vec<MenuItem>,
}

thread_local! {
    static RECENT: std::cell::RefCell<Option<RecentMenu>> = const { std::cell::RefCell::new(None) };
}

/// The label of a recent entry: the home folder as `~`.
pub fn recent_label(path: &std::path::Path) -> String {
    let full = path.to_string_lossy().to_string();
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && full.starts_with(&format!("{home}/")) => format!("~{}", &full[home.len()..]),
        _ => full,
    }
}

/// Puts the recent folders and files into File ▸ Open Recent, in VS Code's
/// order: Reopen Closed Editor, folders, files, More…, Clear Recently Opened….
/// An entry's id is `recent:<path>`.
pub fn set_recent(folders: &[std::path::PathBuf], files: &[std::path::PathBuf]) {
    RECENT.with(|r| {
        let mut guard = r.borrow_mut();
        let Some(m) = guard.as_mut() else { return };
        // Everything out, then back in order.
        for child in m.sub.items() {
            match &child {
                muda::MenuItemKind::MenuItem(i) => drop(m.sub.remove(i)),
                muda::MenuItemKind::Predefined(i) => drop(m.sub.remove(i)),
                muda::MenuItemKind::Submenu(i) => drop(m.sub.remove(i)),
                muda::MenuItemKind::Check(i) => drop(m.sub.remove(i)),
                muda::MenuItemKind::Icon(i) => drop(m.sub.remove(i)),
            }
        }
        m.dynamic.clear();
        let sep = || PredefinedMenuItem::separator();
        if let Some(i) = &m.reopen {
            let _ = m.sub.append(i);
            let _ = m.sub.append(&sep());
        }
        for (list, kind) in [(folders, "folder"), (files, "file")] {
            let _ = kind;
            if list.is_empty() {
                continue;
            }
            for path in list.iter().take(10) {
                let item = MenuItem::with_id(MenuId::new(format!("recent:{}", path.display())), recent_label(path), true, None);
                let _ = m.sub.append(&item);
                m.dynamic.push(item);
            }
            let _ = m.sub.append(&sep());
        }
        if let Some(i) = &m.more {
            let _ = m.sub.append(i);
            let _ = m.sub.append(&sep());
        }
        if let Some(i) = &m.clear {
            let _ = m.sub.append(i);
        }
    });
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

    /// Every entry of the table: (title, command, key).
    fn walk(entries: &[E], out: &mut Vec<(&'static str, &'static str, &'static str)>) {
        for e in entries {
            match e {
                Sep => {}
                Item(t, c, k) | Off(t, c, k) => out.push((t, c, k)),
                Sub(_, children) => walk(children, out),
            }
        }
    }

    fn all_entries() -> Vec<(&'static str, &'static str, &'static str)> {
        let mut out = Vec::new();
        for table in [PREFERENCES, FILE, EDIT, SELECTION, VIEW, GO, RUN, TERMINAL, WINDOW, HELP] {
            walk(table, &mut out);
        }
        out
    }

    #[test]
    fn the_menu_table_matches_what_the_app_runs() {
        let all = all_entries();
        // The key of every item that runs a command the app implements parses
        // (an item without a key is fine; so is a command with no menu item).
        for command in IMPLEMENTED {
            for (title, _, key) in all.iter().filter(|(_, c, _)| c == command) {
                if !key.is_empty() {
                    assert!(keybinding_to_accelerator(key).is_some(), "{title}: key {key:?} doesn't parse");
                }
            }
        }
        // Chords are only in titles (a native menu can't show them), never as keys.
        for (title, _, key) in &all {
            assert!(!key.contains(' '), "{title}: chord as a key");
        }
        // The count VS Code's menus have (minus the AI items, the open recent list, the system ones).
        assert!(all.len() > 200, "{} entries", all.len());
    }

    #[test]
    fn keybindings_become_accelerators() {
        assert_eq!(keybinding_to_accelerator("shift+cmd+s"), Some(Accelerator::new(Some(Modifiers::SHIFT | Modifiers::SUPER), Code::KeyS)));
        assert_eq!(keybinding_to_accelerator("ctrl+`"), Some(Accelerator::new(Some(Modifiers::CONTROL), Code::Backquote)));
        assert_eq!(keybinding_to_accelerator("alt+cmd+up"), Some(Accelerator::new(Some(Modifiers::ALT | Modifiers::SUPER), Code::ArrowUp)));
        assert_eq!(keybinding_to_accelerator("f5"), Some(Accelerator::new(None, Code::F5)));
        assert_eq!(keybinding_to_accelerator("cmd+k cmd+t"), None);
    }
}
