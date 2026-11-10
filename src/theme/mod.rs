pub mod color;
pub mod registry;
pub mod themes;

use egui::{Color32, Stroke, Visuals};
use registry::ColorSource;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
pub use themes::ColorTheme;

/// Workbench colours, looked up in the active colour theme. Each entry
/// names the VS Code colour id it reads (`--vscode-<id>` in CSS); the
/// value follows `workbench.colorTheme` and `workbench.colorCustomizations`.
pub struct Palette;

/// Where a palette entry takes its colour from.
#[derive(Clone, Copy)]
enum Src {
    /// A colour id, resolved like `IColorTheme.getColor` (theme, then the
    /// registry default for the theme's type).
    Id(&'static str),
    /// The first id that resolves to a colour.
    OneOf(&'static [&'static str]),
    /// `opaque(fg, bg)`: a translucent colour flattened onto a background,
    /// where VS Code layers two elements and we paint one.
    Opaque(&'static str, &'static str),
    /// A constant per theme type (dark / light), used where VS Code's CSS
    /// hard-codes a colour.
    Kind(Color32, Color32),
}

macro_rules! palette {
    ($( $(#[$doc:meta])* $name:ident = $src:expr; )*) => {
        #[allow(non_camel_case_types, clippy::upper_case_acronyms)]
        #[repr(usize)]
        enum Slot { $($name,)* Count }

        const SOURCES: [(&str, Src); Slot::Count as usize] = [$((stringify!($name), $src),)*];

        static SLOTS: [AtomicU32; Slot::Count as usize] =
            [const { AtomicU32::new(0) }; Slot::Count as usize];

        #[allow(non_snake_case, dead_code)]
        impl Palette {
            $(
                $(#[$doc])*
                #[inline]
                pub fn $name() -> Color32 {
                    slot(Slot::$name as usize)
                }
            )*
        }
    };
}

use Src::*;

palette! {
    // ── Surfaces ──────────────────────────────────────────────────────────
    EDITOR_BG = Id("editor.background");
    /// The side bar, activity bar, status bar, title bar and tab strip
    /// share it in the default theme.
    PANEL_BG = Id("sideBar.background");
    /// Hairline between workbench parts.
    BORDER = OneOf(&["sideBar.border", "contrastBorder"]);
    ACTIVITY_BAR_BORDER = OneOf(&["activityBar.border", "contrastBorder"]);
    SIDEBAR_BORDER = OneOf(&["sideBar.border", "contrastBorder"]);
    STATUS_BAR_BORDER = OneOf(&["statusBar.border", "contrastBorder"]);
    TITLE_BAR_BORDER = OneOf(&["titleBar.border", "contrastBorder"]);
    PANEL_BORDER = Id("panel.border");
    EDITOR_WIDGET_BG = Id("editorWidget.background");
    MENU_BG = Id("menu.background");
    MENU_FG = Id("menu.foreground");
    MENU_BORDER = Id("menu.border");
    MENU_SELECTION_BG = Id("menu.selectionBackground");
    MENU_SELECTION_FG = Id("menu.selectionForeground");
    MENU_SEPARATOR = Id("menu.separatorBackground");

    // ── Modern UI (`workbench.experimental.modernUI`, on by default) ───────
    /// Behind the floating cards (floatingPanels.css).
    SHELL_BG = OneOf(&["modernUI.shellBackground", "titleBar.activeBackground"]);
    /// The side bar card.
    SURFACE_BG = OneOf(&["surface.background", "sideBar.background"]);
    SURFACE_BORDER = OneOf(&["surface.border", "widget.border"]);
    /// The editor card's outline (editorBorder.css).
    EDITOR_CARD_BORDER = OneOf(&["editor.border", "surface.border", "widget.border"]);
    MODERN_ACTIVITY_BAR_BG = OneOf(&["modernActivityBar.background", "activityBar.background"]);
    MODERN_ACTIVITY_BAR_BORDER = OneOf(&["modernActivityBar.border", "surface.border", "widget.border"]);
    /// A theme's `activityBar.activeBackground` wins over the modern colour
    /// (activitybarPart.ts theming participant).
    MODERN_ACTIVITY_ITEM_ACTIVE_BG = OneOf(&["activityBar.activeBackground", "modernActivityBarItem.activeBackground"]);
    MODERN_ACTIVITY_ITEM_ACTIVE_FG = OneOf(&["modernActivityBarItem.activeForeground", "activityBar.foreground"]);
    MODERN_ACTIVITY_ITEM_HOVER_BG = Id("modernActivityBarItem.hoverBackground");
    MODERN_ACTIVITY_ITEM_HOVER_FG = OneOf(&["modernActivityBarItem.hoverForeground", "activityBar.foreground"]);
    /// The tab strip behind connected editor tabs.
    CONNECTED_TABS_BG = OneOf(&["editorGroupHeader.connectedTabsBackground", "editorGroupHeader.tabsBackground"]);

    // ── Welcome page ──────────────────────────────────────────────────────
    WELCOME_TILE_BG = Id("welcomePage.tileBackground");
    WELCOME_TILE_HOVER_BG = Id("welcomePage.tileHoverBackground");
    WELCOME_PROGRESS_BG = Id("welcomePage.progress.background");
    WELCOME_PROGRESS_FG = Id("welcomePage.progress.foreground");

    // ── Foreground ────────────────────────────────────────────────────────
    FG = Id("foreground");
    /// Emphasised text and icons on hover.
    FG_BRIGHT = Kind(Color32::WHITE, Color32::BLACK);
    FG_DESCRIPTION = Id("descriptionForeground");
    BREADCRUMB_FG = Id("breadcrumb.foreground");
    EDITOR_FG = Id("editor.foreground");
    TEXT_LINK_FG = Id("textLink.foreground");
    TEXT_LINK_ACTIVE_FG = Id("textLink.activeForeground");
    ERROR_FG = Id("errorForeground");

    // ── Accent ────────────────────────────────────────────────────────────
    ACCENT = Id("textLink.foreground");
    FOCUS_BORDER = Id("focusBorder");
    BUTTON_BG = Id("button.background");
    BUTTON_FG = Id("button.foreground");
    BUTTON_HOVER = Id("button.hoverBackground");

    // ── Lists ─────────────────────────────────────────────────────────────
    LIST_HOVER_BG = Id("list.hoverBackground");
    LIST_ACTIVE_SELECTION_BG = Id("list.activeSelectionBackground");
    LIST_INACTIVE_SELECTION_BG = Id("list.inactiveSelectionBackground");

    // ── Inputs ────────────────────────────────────────────────────────────
    INPUT_BG = Id("input.background");
    INPUT_FG = Id("input.foreground");
    INPUT_BORDER = OneOf(&["input.border", "contrastBorder"]);
    INPUT_PLACEHOLDER = Id("input.placeholderForeground");
    INPUT_OPTION_ACTIVE_BG = Id("inputOption.activeBackground");
    INPUT_OPTION_ACTIVE_BORDER = Id("inputOption.activeBorder");
    INPUT_OPTION_ACTIVE_FG = Id("inputOption.activeForeground");

    // ── Tabs ──────────────────────────────────────────────────────────────
    TABS_STRIP_BG = Id("editorGroupHeader.tabsBackground");
    TABS_STRIP_BORDER = Id("editorGroupHeader.tabsBorder");

    // ── Editor ────────────────────────────────────────────────────────────
    LINE_NUMBER_FG = Id("editorLineNumber.foreground");
    LINE_NUMBER_ACTIVE_FG = Id("editorLineNumber.activeForeground");
    SELECTION_BG = Id("editor.selectionBackground");
    INACTIVE_SELECTION_BG = Id("editor.inactiveSelectionBackground");
    FIND_MATCH_BG = Id("editor.findMatchBackground");
    /// `editor.findMatchHighlightBackground` — the non-current matches.
    FIND_MATCH_HL_BG = Id("editor.findMatchHighlightBackground");
    SEARCH_MATCH_BG = Id("editor.findMatchHighlightBackground");
    LINE_HIGHLIGHT_BG = Id("editor.lineHighlightBackground");
    LINE_HIGHLIGHT_BORDER = Id("editor.lineHighlightBorder");
    CURSOR_FG = Id("editorCursor.foreground");
    WHITESPACE_FG = Id("editorWhitespace.foreground");
    INDENT_GUIDE = Id("editorIndentGuide.background1");
    INDENT_GUIDE_ACTIVE = Id("editorIndentGuide.activeBackground1");
    RULER_FG = Id("editorRuler.foreground");
    BADGE_BG = Id("badge.background");
    BADGE_FG = Id("badge.foreground");
    EDITOR_WIDGET_BORDER = OneOf(&["editorWidget.border", "contrastBorder"]);
    EDITOR_WIDGET_FG = Id("editorWidget.foreground");
    BRACKET_1 = Id("editorBracketHighlight.foreground1");
    BRACKET_2 = Id("editorBracketHighlight.foreground2");
    BRACKET_3 = Id("editorBracketHighlight.foreground3");
    BRACKET_4 = Id("editorBracketHighlight.foreground4");
    BRACKET_5 = Id("editorBracketHighlight.foreground5");
    BRACKET_6 = Id("editorBracketHighlight.foreground6");
    BRACKET_UNEXPECTED = Id("editorBracketHighlight.unexpectedBracket.foreground");
    MINIMAP_BG = OneOf(&["minimap.background", "editor.background"]);
    MINIMAP_SLIDER_BG = Id("minimapSlider.background");
    MINIMAP_SLIDER_HOVER = Id("minimapSlider.hoverBackground");
    MINIMAP_SLIDER_ACTIVE = Id("minimapSlider.activeBackground");
    OVERVIEW_RULER_BORDER = Id("editorOverviewRuler.border");

    // ── Activity bar ──────────────────────────────────────────────────────
    ACTIVITY_BAR_BG = Id("activityBar.background");
    ACTIVITY_BAR_FG = Id("activityBar.foreground");
    ACTIVITY_BAR_INACTIVE_FG = Id("activityBar.inactiveForeground");
    ACTIVITY_BAR_ACTIVE_BORDER = Id("activityBar.activeBorder");
    ACTIVITY_BAR_ACTIVE_BG = Id("activityBar.activeBackground");
    ACTIVITY_BAR_BADGE_BG = Id("activityBarBadge.background");
    ACTIVITY_BAR_BADGE_FG = Id("activityBarBadge.foreground");

    // ── Side bar ──────────────────────────────────────────────────────────
    SIDEBAR_BG = Id("sideBar.background");
    SIDEBAR_FG = OneOf(&["sideBar.foreground", "foreground"]);
    SIDEBAR_TITLE_FG = OneOf(&["sideBarTitle.foreground", "foreground"]);
    SIDEBAR_SECTION_HEADER_BG = Id("sideBarSectionHeader.background");
    SIDEBAR_SECTION_HEADER_FG = OneOf(&["sideBarSectionHeader.foreground", "foreground"]);

    // ── Status bar ────────────────────────────────────────────────────────
    STATUS_BAR_BG = Id("statusBar.background");
    STATUS_BAR_FG = Id("statusBar.foreground");
    STATUS_BAR_NO_FOLDER_BG = Id("statusBar.noFolderBackground");
    STATUS_BAR_NO_FOLDER_FG = Id("statusBar.noFolderForeground");
    STATUS_BAR_REMOTE_BG = Id("statusBarItem.remoteBackground");
    STATUS_BAR_REMOTE_FG = Id("statusBarItem.remoteForeground");
    /// `statusBarItem.hoverBackground` over the status bar.
    STATUS_BAR_ITEM_HOVER_BG = Opaque("statusBarItem.hoverBackground", "statusBar.background");
    STATUS_BAR_ITEM_HOVER_FG = OneOf(&["statusBarItem.hoverForeground", "statusBar.foreground"]);

    // ── Git ───────────────────────────────────────────────────────────────
    GIT_MODIFIED_FG = Id("gitDecoration.modifiedResourceForeground");
    GIT_ADDED_FG = Id("gitDecoration.addedResourceForeground");
    GIT_UNTRACKED_FG = Id("gitDecoration.untrackedResourceForeground");
    GIT_RENAMED_FG = Id("gitDecoration.renamedResourceForeground");
    GIT_DELETED_FG = Id("gitDecoration.deletedResourceForeground");
    GIT_CONFLICT_FG = Id("gitDecoration.conflictingResourceForeground");
    GIT_IGNORED_FG = Id("gitDecoration.ignoredResourceForeground");
    GIT_SUBMODULE_FG = Id("gitDecoration.submoduleResourceForeground");
    GIT_STAGE_MODIFIED_FG = Id("gitDecoration.stageModifiedResourceForeground");
    GIT_STAGE_DELETED_FG = Id("gitDecoration.stageDeletedResourceForeground");
    GIT_GUTTER_MODIFIED = Id("editorGutter.modifiedBackground");
    GIT_GUTTER_ADDED = Id("editorGutter.addedBackground");
    GIT_GUTTER_DELETED = Id("editorGutter.deletedBackground");

    // ── Diff editor ───────────────────────────────────────────────────────
    DIFF_INSERTED_BG = Id("diffEditor.insertedLineBackground");
    DIFF_REMOVED_BG = Id("diffEditor.removedLineBackground");
    DIFF_INSERTED_TEXT_BG = Id("diffEditor.insertedTextBackground");
    DIFF_REMOVED_TEXT_BG = Id("diffEditor.removedTextBackground");
    DIFF_DIAGONAL_FILL = Id("diffEditor.diagonalFill");
    /// The line between the diff editor's two sides and its overview ruler.
    DIFF_DIVIDER = OneOf(&["diffEditor.border", "editorWidget.border", "contrastBorder"]);
    /// The overview ruler's view slider.
    DIFF_RULER_SLIDER = Id("scrollbarSlider.background");

    // ── SCM graph ─────────────────────────────────────────────────────────
    SCM_GRAPH_1 = Id("scmGraph.foreground1");
    SCM_GRAPH_2 = Id("scmGraph.foreground2");
    SCM_GRAPH_3 = Id("scmGraph.foreground3");
    SCM_GRAPH_4 = Id("scmGraph.foreground4");
    SCM_GRAPH_5 = Id("scmGraph.foreground5");
    SCM_REF_LOCAL = Id("scmGraph.historyItemRefColor");
    SCM_REF_REMOTE = Id("scmGraph.historyItemRemoteRefColor");
    SCM_REF_BASE = Id("scmGraph.historyItemBaseRefColor");
    SCM_HOVER_LABEL_FG = Id("scmGraph.historyItemHoverLabelForeground");

    // ── Title bar ─────────────────────────────────────────────────────────
    TITLE_BAR_BG = Id("titleBar.activeBackground");
    TITLE_BAR_FG = Id("titleBar.activeForeground");
    TITLE_BAR_INACTIVE_BG = Id("titleBar.inactiveBackground");
    TITLE_BAR_INACTIVE_FG = Id("titleBar.inactiveForeground");
    COMMAND_CENTER_BG = Id("commandCenter.background");
    COMMAND_CENTER_ACTIVE_BG = Id("commandCenter.activeBackground");
    COMMAND_CENTER_BORDER = Id("commandCenter.border");
    COMMAND_CENTER_ACTIVE_BORDER = Id("commandCenter.activeBorder");
    COMMAND_CENTER_FG = Id("commandCenter.foreground");
    COMMAND_CENTER_ACTIVE_FG = Id("commandCenter.activeForeground");

    // ── Panel ─────────────────────────────────────────────────────────────
    PANEL_BG2 = Id("panel.background");
    PANEL_TITLE_ACTIVE_FG = Id("panelTitle.activeForeground");
    PANEL_TITLE_INACTIVE_FG = Id("panelTitle.inactiveForeground");
    PANEL_TITLE_ACTIVE_BORDER = Id("panelTitle.activeBorder");

    // ── Problems / notifications ──────────────────────────────────────────
    WARNING_FG = Id("editorWarning.foreground");

    // ── vscode-elements bridge (`--vscode-*` variables) ───────────────────
    VSCE_BUTTON_BG = Id("button.background");
    VSCE_BUTTON_FG = Id("button.foreground");
    VSCE_BUTTON_HOVER_BG = Id("button.hoverBackground");
    VSCE_BUTTON_BORDER = OneOf(&["button.border", "contrastBorder"]);
    VSCE_BUTTON_SECONDARY_BG = Id("button.secondaryBackground");
    VSCE_BUTTON_SECONDARY_FG = Id("button.secondaryForeground");
    VSCE_BUTTON_SECONDARY_HOVER_BG = Id("button.secondaryHoverBackground");
    VSCE_BUTTON_SECONDARY_BORDER = OneOf(&["button.secondaryBorder", "button.border", "contrastBorder"]);
    VSCE_INPUT_BG = Id("input.background");
    VSCE_INPUT_FG = Id("input.foreground");
    VSCE_INPUT_BORDER = OneOf(&["input.border", "contrastBorder"]);
    VSCE_INPUT_PLACEHOLDER_FG = Id("input.placeholderForeground");
    VSCE_BADGE_BG = Id("badge.background");
    VSCE_BADGE_FG = Id("badge.foreground");
    VSCE_FOCUS_BORDER = Id("focusBorder");
    VSCE_FG = Id("foreground");
    VSCE_FG_DESCRIPTION = Id("descriptionForeground");
    VSCE_FG_DISABLED = Id("disabledForeground");
    VSCE_FG_ERROR = Id("errorForeground");
    VSCE_LIST_HOVER_BG = Id("list.hoverBackground");
    VSCE_LIST_ACTIVE_SELECTION_BG = Id("list.activeSelectionBackground");
    VSCE_LIST_ACTIVE_SELECTION_FG = Id("list.activeSelectionForeground");
    VSCE_LIST_HIGHLIGHT_FG = Id("list.highlightForeground");
    VSCE_QUICK_INPUT_BG = Id("quickInput.background");
    VSCE_QUICK_INPUT_FG = Id("quickInput.foreground");
    VSCE_QUICK_INPUT_LIST_FOCUS_BG = Id("quickInputList.focusBackground");
    VSCE_QUICK_INPUT_LIST_FOCUS_FG = Id("quickInputList.focusForeground");
    VSCE_QUICK_INPUT_LIST_FOCUS_HIGHLIGHT_FG = Id("quickInputList.focusHighlightForeground");
    VSCE_QUICK_INPUT_LIST_HOVER_BG = OneOf(&["quickInputList.hoverBackground", "list.hoverBackground"]);
    VSCE_PICKER_GROUP_FG = Id("pickerGroup.foreground");
    VSCE_PICKER_GROUP_BORDER = Id("pickerGroup.border");
    VSCE_WIDGET_BORDER = OneOf(&["widget.border", "contrastBorder"]);
    VSCE_QUICK_INPUT_FOCUS_BORDER = Id("focusBorder");
    VSCE_QUICK_INPUT_BOX_BG = Id("input.background");
    VSCE_QUICK_INPUT_BOX_FG = Id("input.foreground");
    VSCE_QUICK_INPUT_PLACEHOLDER_FG = Id("input.placeholderForeground");
    VSCE_KEYBINDING_LABEL_BG = Id("keybindingLabel.background");
    VSCE_KEYBINDING_LABEL_FG = Id("keybindingLabel.foreground");
    VSCE_KEYBINDING_LABEL_BORDER = Id("keybindingLabel.border");
    VSCE_KEYBINDING_LABEL_BOTTOM_BORDER = Id("keybindingLabel.bottomBorder");
    VSCE_NOTIFICATIONS_BG = Id("notifications.background");
    VSCE_NOTIFICATIONS_FG = Id("notifications.foreground");
    VSCE_NOTIFICATION_TOAST_BORDER = OneOf(&["notificationToast.border", "contrastBorder"]);
    VSCE_NOTIFICATIONS_INFO_ICON = Id("notificationsInfoIcon.foreground");
    VSCE_NOTIFICATIONS_WARNING_ICON = Id("notificationsWarningIcon.foreground");
    VSCE_NOTIFICATIONS_ERROR_ICON = Id("notificationsErrorIcon.foreground");
    VSCE_BADGE_2026_BG = Id("badge.background");
    VSCE_BADGE_2026_FG = Id("badge.foreground");
    VSCE_EXTENSION_BUTTON_PROMINENT_BG = Id("extensionButton.prominentBackground");
    VSCE_EXTENSION_BUTTON_PROMINENT_FG = Id("extensionButton.prominentForeground");
    VSCE_EXTENSION_BUTTON_PROMINENT_HOVER_BG = Id("extensionButton.prominentHoverBackground");
    VSCE_EXTENSION_STAR_FG = Id("extensionIcon.starForeground");
    VSCE_EXTENSION_VERIFIED_FG = Id("extensionIcon.verifiedForeground");
    VSCE_EDITOR_ERROR_FG = Id("editorError.foreground");
    VSCE_EDITOR_WARNING_FG = Id("editorWarning.foreground");
    VSCE_EDITOR_INFO_FG = Id("editorInfo.foreground");
    VSCE_EDITOR_WIDGET_BG = Id("editorSuggestWidget.background");
    VSCE_EDITOR_WIDGET_BORDER = Id("editorSuggestWidget.border");
    VSCE_SUGGEST_SELECTED_BG = Id("editorSuggestWidget.selectedBackground");
    VSCE_SUGGEST_FG = Id("editorSuggestWidget.foreground");
    VSCE_SUGGEST_HIGHLIGHT_FG = Id("editorSuggestWidget.highlightForeground");
    VSCE_HOVER_BG = Id("editorHoverWidget.background");
    VSCE_HOVER_BORDER = Id("editorHoverWidget.border");
    VSCE_HOVER_FG = Id("editorHoverWidget.foreground");
    VSCE_BUTTON_2026_BG = Id("button.background");
    VSCE_BUTTON_2026_HOVER_BG = Id("button.hoverBackground");
    VSCE_BUTTON_2026_BORDER = OneOf(&["button.border", "contrastBorder"]);
    VSCE_BUTTON_2026_SECONDARY_HOVER_BG = Id("button.secondaryHoverBackground");
    VSCE_QUICK_INPUT_SEPARATOR_FG = Id("descriptionForeground");
    VSCE_QUICK_INPUT_ACTION_HOVER_BG = Id("toolbar.hoverBackground");
    VSCE_SIDEBAR_SECTION_HEADER_BG = Id("sideBarSectionHeader.background");
    VSCE_SIDEBAR_SECTION_HEADER_FG = OneOf(&["sideBarSectionHeader.foreground", "foreground"]);
    VSCE_SIDEBAR_SECTION_HEADER_BORDER = OneOf(&["sideBarSectionHeader.border", "contrastBorder"]);
    VSCE_ICON_2026_FG = Id("icon.foreground");
    VSCE_LIST_HOVER_2026_BG = Id("list.hoverBackground");
    VSCE_INPUT_2026_BG = Id("input.background");
    VSCE_INPUT_2026_PLACEHOLDER_FG = Id("input.placeholderForeground");
    VSCE_LIST_ACTIVE_SELECTION_2026_BG = Id("list.activeSelectionBackground");
    VSCE_LIST_INACTIVE_SELECTION_2026_BG = Id("list.inactiveSelectionBackground");
    VSCE_LIST_FOCUS_OUTLINE_2026 = Id("list.focusOutline");
    VSCE_TOOLBAR_HOVER_BG = Id("toolbar.hoverBackground");
    VSCE_DISABLED_FG = Id("disabledForeground");
    VSCE_DROPDOWN_BG = Id("dropdown.background");
    VSCE_DROPDOWN_BORDER = Id("dropdown.border");
    VSCE_DROPDOWN_FG = Id("dropdown.foreground");
    VSCE_CHECKBOX_BG = Id("checkbox.background");
    VSCE_CHECKBOX_BORDER = Id("checkbox.border");
    VSCE_CHECKBOX_FG = Id("checkbox.foreground");
    VSCE_PROGRESS_BG = Id("progressBar.background");
    VSCE_ICON_FG = Id("icon.foreground");
    VSCE_TEXT_SEPARATOR_FG = Id("textSeparator.foreground");
    VSCE_LABEL_FG = Id("foreground");
    VSCE_INPUT_ERROR_BORDER = Id("inputValidation.errorBorder");
    VSCE_INPUT_OPTION_HOVER_BG = Id("inputOption.hoverBackground");
    VSCE_SECTION_HEADER_BG = Id("sideBarSectionHeader.background");
    VSCE_SECTION_HEADER_FG = OneOf(&["sideBarSectionHeader.foreground", "foreground"]);
    VSCE_SECTION_HEADER_BORDER = OneOf(&["sideBarSectionHeader.border", "contrastBorder"]);
    VSCE_SCROLLBAR_SLIDER_BG = Id("scrollbarSlider.background");
    VSCE_SCROLLBAR_SLIDER_HOVER = Id("scrollbarSlider.hoverBackground");
    VSCE_SCROLLBAR_SLIDER_ACTIVE = Id("scrollbarSlider.activeBackground");
    VSCE_SCROLLBAR_SHADOW = Id("scrollbar.shadow");
    VSCE_SASH_HOVER_BORDER = Id("sash.hoverBorder");
    VSCE_TAB_ACTIVE_BG = Id("tab.activeBackground");
    VSCE_TAB_INACTIVE_BG = Id("tab.inactiveBackground");
    VSCE_TAB_ACTIVE_FG = Id("tab.activeForeground");
    VSCE_TAB_INACTIVE_FG = Id("tab.inactiveForeground");
    VSCE_TAB_HOVER_BG = Id("tab.hoverBackground");
    VSCE_TAB_HOVER_FG = Id("tab.hoverForeground");
    VSCE_TAB_BORDER = Id("tab.border");
    VSCE_TAB_ACTIVE_BORDER_TOP = Id("tab.activeBorderTop");
    VSCE_TAB_ACTIVE_BORDER = Id("tab.activeBorder");
    /// Indent guides of rows that aren't under the pointer.
    VSCE_TREE_INDENT_GUIDE = Id("tree.inactiveIndentGuidesStroke");
    VSCE_TREE_ACTIVE_INDENT_GUIDE = Id("tree.indentGuidesStroke");
    VSCE_TREE_INACTIVE_INDENT_GUIDE = Id("tree.inactiveIndentGuidesStroke");
    VSCE_TABLE_COLUMN_BORDER = Id("tree.tableColumnsBorder");
    VSCE_TABLE_ODD_ROW_BG = Id("tree.tableOddRowsBackground");
    VSCE_WIDGET_SHADOW = Id("widget.shadow");

    // ── Settings editor (settingsEditorColorRegistry.ts) ──────────────────
    SETTINGS_HEADER_FG = Id("settings.headerForeground");
    SETTINGS_HEADER_HOVER_FG = Id("settings.settingsHeaderHoverForeground");
    SETTINGS_MODIFIED_INDICATOR = Id("settings.modifiedItemIndicator");
    SETTINGS_SASH_BORDER = Id("settings.sashBorder");
    SETTINGS_DROPDOWN_BG = Id("settings.dropdownBackground");
    SETTINGS_DROPDOWN_FG = Id("settings.dropdownForeground");
    SETTINGS_DROPDOWN_BORDER = Id("settings.dropdownBorder");
    SETTINGS_DROPDOWN_LIST_BORDER = Id("settings.dropdownListBorder");
    SETTINGS_CHECKBOX_BG = Id("settings.checkboxBackground");
    SETTINGS_CHECKBOX_FG = Id("settings.checkboxForeground");
    SETTINGS_CHECKBOX_BORDER = Id("settings.checkboxBorder");
    SETTINGS_TEXT_INPUT_BG = Id("settings.textInputBackground");
    SETTINGS_TEXT_INPUT_FG = Id("settings.textInputForeground");
    SETTINGS_TEXT_INPUT_BORDER = Id("settings.textInputBorder");
    SETTINGS_NUMBER_INPUT_BG = Id("settings.numberInputBackground");
    SETTINGS_NUMBER_INPUT_FG = Id("settings.numberInputForeground");
    SETTINGS_NUMBER_INPUT_BORDER = Id("settings.numberInputBorder");
    SETTINGS_FOCUSED_ROW_BG = Id("settings.focusedRowBackground");
    SETTINGS_FOCUSED_ROW_BORDER = Id("settings.focusedRowBorder");
    SETTINGS_ROW_HOVER_BG = Id("settings.rowHoverBackground");
    TEXT_PREFORMAT_FG = Id("textPreformat.foreground");
    TEXT_PREFORMAT_BG = Id("textPreformat.background");
    TEXT_PREFORMAT_BORDER = Id("textPreformat.border");
    MODERN_TAB_ACTIVE_BG = Id("modernTab.activeBackground");
    MODERN_TAB_ACTIVE_FG = Id("modernTab.activeForeground");
    MODERN_TAB_HOVER_BG = Id("modernTab.hoverBackground");
    MODERN_TAB_HOVER_FG = Id("modernTab.hoverForeground");
    INPUT_PLACEHOLDER_FG = Id("input.placeholderForeground");
    INPUT_VALIDATION_ERROR_BG = Id("inputValidation.errorBackground");
    STATUS_BAR_PROMINENT_BG = Id("statusBarItem.prominentBackground");
    STATUS_BAR_PROMINENT_FG = Id("statusBarItem.prominentForeground");
    STATUS_BAR_PROMINENT_HOVER_BG = Id("statusBarItem.prominentHoverBackground");
    BANNER_BG = Id("banner.background");
    BANNER_FG = Id("banner.foreground");
    BANNER_ICON_FG = Id("banner.iconForeground");
    DEBUG_ICON_START_FG = Id("debugIcon.startForeground");
    EDITOR_ERROR_FG = Id("editorError.foreground");
    LIST_ACTIVE_SELECTION_FG = OneOf(&["list.activeSelectionForeground", "foreground"]);
}

impl Palette {
    /// `scmGraph.foreground1-5`, the graph's lane colours.
    #[allow(non_snake_case)]
    pub fn SCM_GRAPH_LANES() -> [Color32; 5] {
        [Self::SCM_GRAPH_1(), Self::SCM_GRAPH_2(), Self::SCM_GRAPH_3(), Self::SCM_GRAPH_4(), Self::SCM_GRAPH_5()]
    }
}

static INIT: std::sync::Once = std::sync::Once::new();

#[inline]
fn slot(i: usize) -> Color32 {
    INIT.call_once(|| install(Arc::new(ColorTheme::default_theme())));
    let v = SLOTS[i].load(Ordering::Relaxed);
    let [r, g, b, a] = v.to_be_bytes();
    Color32::from_rgba_premultiplied(r, g, b, a)
}

fn evaluate(theme: &ColorTheme, src: Src) -> Color32 {
    let c = match src {
        Id(id) => theme.color(id),
        OneOf(ids) => ids.iter().find_map(|id| theme.color(id)),
        Opaque(fg, bg) => match (theme.color(fg), theme.color(bg)) {
            (Some(f), Some(b)) => Some(f.make_opaque(&b)),
            (f, _) => f,
        },
        Kind(dark, light) => return if theme.kind.is_dark() { dark } else { light },
    };
    c.map(|c| c.to_color32()).unwrap_or(Color32::TRANSPARENT)
}

static CURRENT: RwLock<Option<Arc<ColorTheme>>> = RwLock::new(None);

/// Makes `theme` the active colour theme: every `Palette` entry, the
/// syntax colours and `theme::color` now read from it.
fn install(theme: Arc<ColorTheme>) {
    for (i, (_, src)) in SOURCES.iter().enumerate() {
        let c = evaluate(&theme, *src);
        SLOTS[i].store(u32::from_be_bytes(c.to_array()), Ordering::Relaxed);
    }
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = Some(theme);
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

static GENERATION: AtomicU32 = AtomicU32::new(0);

/// Changes whenever the colour theme changes, for caches keyed on colours.
pub fn generation() -> u32 {
    GENERATION.load(Ordering::Relaxed)
}

/// Switches the active colour theme.
pub fn set_theme(theme: ColorTheme) {
    INIT.call_once(|| {});
    install(Arc::new(theme));
}

/// The active colour theme.
pub fn current() -> Arc<ColorTheme> {
    INIT.call_once(|| install(Arc::new(ColorTheme::default_theme())));
    CURRENT.read().unwrap_or_else(|e| e.into_inner()).clone().expect("theme installed")
}

/// Every colour of the active theme as CSS variables (`--vscode-<id>`
/// with dots as dashes), the way the workbench hands them to webviews.
pub fn webview_css() -> String {
    let theme = current();
    let mut ids: Vec<String> = registry::ids().cloned().collect();
    ids.extend(theme.defined_ids().cloned());
    ids.sort();
    ids.dedup();
    let mut css = String::from(":root {\n");
    for id in &ids {
        if let Some(c) = theme.color(id) {
            css.push_str(&format!("  --vscode-{}: {};\n", id.replace('.', "-"), c.to_hex()));
        }
    }
    css.push_str("}\n");
    css
}

/// The body class webviews get for the theme's type.
pub fn webview_theme_class() -> &'static str {
    match current().kind {
        registry::ThemeKind::Dark => "vscode-dark",
        registry::ThemeKind::Light => "vscode-light",
        registry::ThemeKind::HcDark => "vscode-high-contrast",
        registry::ThemeKind::HcLight => "vscode-high-contrast-light",
    }
}

/// Any colour id in the active theme (`--vscode-<id>`), or `None` when
/// the theme and the registry leave it unset.
pub fn color(id: &str) -> Option<Color32> {
    current().color(id).map(|c| c.to_color32())
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}


pub fn apply(ctx: &egui::Context) {
    let mut visuals = if current().kind.is_dark() { Visuals::dark() } else { Visuals::light() };

    visuals.override_text_color = Some(Palette::FG());
    // The window behind the floating cards.
    visuals.panel_fill = Palette::SHELL_BG();
    visuals.window_fill = Palette::MENU_BG();
    visuals.extreme_bg_color = Palette::INPUT_BG();
    visuals.faint_bg_color = Palette::PANEL_BG();
    visuals.code_bg_color = Palette::EDITOR_BG();

    visuals.widgets.noninteractive.bg_fill = Palette::EDITOR_BG();
    visuals.widgets.noninteractive.weak_bg_fill = Palette::EDITOR_BG();
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Palette::BORDER());
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Palette::FG());

    visuals.widgets.inactive.bg_fill = Palette::INPUT_BG();
    visuals.widgets.inactive.weak_bg_fill = Palette::INPUT_BG();
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Palette::INPUT_BORDER());
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Palette::FG());

    visuals.widgets.hovered.bg_fill = Palette::LIST_HOVER_BG();
    visuals.widgets.hovered.weak_bg_fill = Palette::LIST_HOVER_BG();
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Palette::INPUT_BORDER());
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Palette::FG_BRIGHT());

    visuals.widgets.active.bg_fill = Palette::ACCENT();
    visuals.widgets.active.weak_bg_fill = Palette::ACCENT();
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, Palette::ACCENT());
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Palette::FG_BRIGHT());

    visuals.widgets.open.bg_fill = Palette::LIST_HOVER_BG();
    visuals.widgets.open.weak_bg_fill = Palette::LIST_HOVER_BG();

    visuals.selection.bg_fill = Palette::SELECTION_BG();
    visuals.selection.stroke = Stroke::new(1.0, Palette::FG());

    visuals.hyperlink_color = Palette::TEXT_LINK_FG();
    visuals.window_stroke = Stroke::new(1.0, Palette::BORDER());
    visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(160),
    };

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(4.0, 3.0);
    style.spacing.window_margin = egui::Margin::same(0);
    style.spacing.menu_margin = egui::Margin::same(4);
    style.spacing.button_padding = egui::vec2(6.0, 2.0);
    style.spacing.indent = 14.0;
    style.spacing.scroll.bar_width = 10.0;
    style.spacing.scroll.handle_min_length = 24.0;
    ctx.set_style(style);
}
