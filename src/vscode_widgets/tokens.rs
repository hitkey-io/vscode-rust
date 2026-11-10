//! Bridge between vscode-elements CSS custom properties (`--vscode-*`) and
//! [`crate::theme::Palette`] constants.
//!
//! This file exists for two reasons:
//!
//! 1. **Documentation**: Each VS Code theme token corresponds to a Palette
//!    constant. The mapping lives here so reviewers can audit it in one place.
//! 2. **Re-export aliases**: Widget modules `use` short names from here so
//!    their bodies read naturally without scrolling through Palette names.
//!
//! Source: every `Palette::VSCE_*` constant added in Phase 0 corresponds 1:1
//! with a CSS variable used by vscode-elements. The constant doc-comments
//! point back to the upstream variable name. The values follow the active
//! colour theme, so these are functions.

#![allow(non_snake_case)]

use crate::theme::Palette;
use egui::Color32;

// Buttons -----------------------------------------------------------------
pub fn BUTTON_BG() -> Color32 {
    Palette::VSCE_BUTTON_BG()
}
pub fn BUTTON_FG() -> Color32 {
    Palette::VSCE_BUTTON_FG()
}
pub fn BUTTON_HOVER_BG() -> Color32 {
    Palette::VSCE_BUTTON_HOVER_BG()
}
pub fn BUTTON_BORDER() -> Color32 {
    Palette::VSCE_BUTTON_BORDER()
}
pub fn BUTTON_SECONDARY_BG() -> Color32 {
    Palette::VSCE_BUTTON_SECONDARY_BG()
}
pub fn BUTTON_SECONDARY_FG() -> Color32 {
    Palette::VSCE_BUTTON_SECONDARY_FG()
}
pub fn BUTTON_SECONDARY_HOVER_BG() -> Color32 {
    Palette::VSCE_BUTTON_SECONDARY_HOVER_BG()
}

// Inputs ------------------------------------------------------------------
pub fn INPUT_BG() -> Color32 {
    Palette::VSCE_INPUT_BG()
}
pub fn INPUT_FG() -> Color32 {
    Palette::VSCE_INPUT_FG()
}
pub fn INPUT_BORDER() -> Color32 {
    Palette::VSCE_INPUT_BORDER()
}
pub fn INPUT_PLACEHOLDER_FG() -> Color32 {
    Palette::VSCE_INPUT_PLACEHOLDER_FG()
}

// Badges ------------------------------------------------------------------
pub fn BADGE_BG() -> Color32 {
    Palette::VSCE_BADGE_BG()
}
pub fn BADGE_FG() -> Color32 {
    Palette::VSCE_BADGE_FG()
}

// Focus / borders ---------------------------------------------------------
pub fn FOCUS_BORDER() -> Color32 {
    Palette::VSCE_FOCUS_BORDER()
}

// Foreground tones --------------------------------------------------------
pub fn FG() -> Color32 {
    Palette::VSCE_FG()
}
pub fn FG_DESCRIPTION() -> Color32 {
    Palette::VSCE_FG_DESCRIPTION()
}
pub fn FG_DISABLED() -> Color32 {
    Palette::VSCE_FG_DISABLED()
}
pub fn FG_ERROR() -> Color32 {
    Palette::VSCE_FG_ERROR()
}

// Surfaces (used by storybook cards and component bodies) -----------------
pub fn SURFACE_EDITOR() -> Color32 {
    Palette::EDITOR_BG()
}
pub fn SURFACE_PANEL() -> Color32 {
    Palette::PANEL_BG()
}

// List / tree -------------------------------------------------------------
pub fn LIST_HOVER_BG() -> Color32 {
    Palette::VSCE_LIST_HOVER_BG()
}
pub fn LIST_ACTIVE_SELECTION_BG() -> Color32 {
    Palette::VSCE_LIST_ACTIVE_SELECTION_BG()
}
pub fn LIST_ACTIVE_SELECTION_FG() -> Color32 {
    Palette::VSCE_LIST_ACTIVE_SELECTION_FG()
}
pub fn LIST_HIGHLIGHT_FG() -> Color32 {
    Palette::VSCE_LIST_HIGHLIGHT_FG()
}

// Dropdown / context menu -------------------------------------------------
pub fn DROPDOWN_BG() -> Color32 {
    Palette::VSCE_DROPDOWN_BG()
}
pub fn DROPDOWN_BORDER() -> Color32 {
    Palette::VSCE_DROPDOWN_BORDER()
}
pub fn DROPDOWN_FG() -> Color32 {
    Palette::VSCE_DROPDOWN_FG()
}

// Icon / divider / label --------------------------------------------------
pub fn ICON_FG() -> Color32 {
    Palette::VSCE_ICON_FG()
}
pub fn TEXT_SEPARATOR_FG() -> Color32 {
    Palette::VSCE_TEXT_SEPARATOR_FG()
}
pub fn LABEL_FG() -> Color32 {
    Palette::VSCE_LABEL_FG()
}

// Checkbox / form options -------------------------------------------------
pub fn CHECKBOX_BG() -> Color32 {
    Palette::VSCE_CHECKBOX_BG()
}
pub fn CHECKBOX_BORDER() -> Color32 {
    Palette::VSCE_CHECKBOX_BORDER()
}
pub fn INPUT_ERROR_BORDER() -> Color32 {
    Palette::VSCE_INPUT_ERROR_BORDER()
}
pub fn INPUT_OPTION_HOVER_BG() -> Color32 {
    Palette::VSCE_INPUT_OPTION_HOVER_BG()
}

// Layout containers -------------------------------------------------------
pub fn SECTION_HEADER_BG() -> Color32 {
    Palette::VSCE_SECTION_HEADER_BG()
}
pub fn SECTION_HEADER_FG() -> Color32 {
    Palette::VSCE_SECTION_HEADER_FG()
}
pub fn SECTION_HEADER_BORDER() -> Color32 {
    Palette::VSCE_SECTION_HEADER_BORDER()
}
pub fn SCROLLBAR_SLIDER_BG() -> Color32 {
    Palette::VSCE_SCROLLBAR_SLIDER_BG()
}
pub fn SCROLLBAR_SLIDER_HOVER() -> Color32 {
    Palette::VSCE_SCROLLBAR_SLIDER_HOVER()
}
pub fn SCROLLBAR_SLIDER_ACTIVE() -> Color32 {
    Palette::VSCE_SCROLLBAR_SLIDER_ACTIVE()
}
pub fn SASH_HOVER_BORDER() -> Color32 {
    Palette::VSCE_SASH_HOVER_BORDER()
}

// Tabs --------------------------------------------------------------------
pub fn TAB_ACTIVE_BG() -> Color32 {
    Palette::VSCE_TAB_ACTIVE_BG()
}
pub fn TAB_INACTIVE_BG() -> Color32 {
    Palette::VSCE_TAB_INACTIVE_BG()
}
pub fn TAB_ACTIVE_FG() -> Color32 {
    Palette::VSCE_TAB_ACTIVE_FG()
}
pub fn TAB_INACTIVE_FG() -> Color32 {
    Palette::VSCE_TAB_INACTIVE_FG()
}
pub fn TAB_BORDER() -> Color32 {
    Palette::VSCE_TAB_BORDER()
}
pub fn TAB_ACTIVE_BORDER_TOP() -> Color32 {
    Palette::VSCE_TAB_ACTIVE_BORDER_TOP()
}

// Tree --------------------------------------------------------------------
pub fn TREE_INDENT_GUIDE() -> Color32 {
    Palette::VSCE_TREE_INDENT_GUIDE()
}

// Table -------------------------------------------------------------------
pub fn TABLE_COLUMN_BORDER() -> Color32 {
    Palette::VSCE_TABLE_COLUMN_BORDER()
}
pub fn TABLE_ODD_ROW_BG() -> Color32 {
    Palette::VSCE_TABLE_ODD_ROW_BG()
}
