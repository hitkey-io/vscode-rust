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
//! point back to the upstream variable name.

use crate::theme::Palette;
use egui::Color32;

// Buttons -----------------------------------------------------------------
pub const BUTTON_BG: Color32 = Palette::VSCE_BUTTON_BG;
pub const BUTTON_FG: Color32 = Palette::VSCE_BUTTON_FG;
pub const BUTTON_HOVER_BG: Color32 = Palette::VSCE_BUTTON_HOVER_BG;
pub const BUTTON_BORDER: Color32 = Palette::VSCE_BUTTON_BORDER;
pub const BUTTON_SECONDARY_BG: Color32 = Palette::VSCE_BUTTON_SECONDARY_BG;
pub const BUTTON_SECONDARY_FG: Color32 = Palette::VSCE_BUTTON_SECONDARY_FG;
pub const BUTTON_SECONDARY_HOVER_BG: Color32 = Palette::VSCE_BUTTON_SECONDARY_HOVER_BG;

// Inputs ------------------------------------------------------------------
pub const INPUT_BG: Color32 = Palette::VSCE_INPUT_BG;
pub const INPUT_FG: Color32 = Palette::VSCE_INPUT_FG;
pub const INPUT_BORDER: Color32 = Palette::VSCE_INPUT_BORDER;
pub const INPUT_PLACEHOLDER_FG: Color32 = Palette::VSCE_INPUT_PLACEHOLDER_FG;

// Badges ------------------------------------------------------------------
pub const BADGE_BG: Color32 = Palette::VSCE_BADGE_BG;
pub const BADGE_FG: Color32 = Palette::VSCE_BADGE_FG;

// Focus / borders ---------------------------------------------------------
pub const FOCUS_BORDER: Color32 = Palette::VSCE_FOCUS_BORDER;

// Foreground tones --------------------------------------------------------
pub const FG: Color32 = Palette::VSCE_FG;
pub const FG_DESCRIPTION: Color32 = Palette::VSCE_FG_DESCRIPTION;
pub const FG_DISABLED: Color32 = Palette::VSCE_FG_DISABLED;
pub const FG_ERROR: Color32 = Palette::VSCE_FG_ERROR;

// Surfaces (used by storybook cards and component bodies) -----------------
pub const SURFACE_EDITOR: Color32 = Palette::EDITOR_BG;
pub const SURFACE_PANEL: Color32 = Palette::PANEL_BG;

// List / tree -------------------------------------------------------------
pub const LIST_HOVER_BG: Color32 = Palette::VSCE_LIST_HOVER_BG;
pub const LIST_ACTIVE_SELECTION_BG: Color32 = Palette::VSCE_LIST_ACTIVE_SELECTION_BG;
pub const LIST_ACTIVE_SELECTION_FG: Color32 = Palette::VSCE_LIST_ACTIVE_SELECTION_FG;

// Dropdown / context menu -------------------------------------------------
pub const DROPDOWN_BG: Color32 = Palette::VSCE_DROPDOWN_BG;
pub const DROPDOWN_BORDER: Color32 = Palette::VSCE_DROPDOWN_BORDER;
pub const DROPDOWN_FG: Color32 = Palette::VSCE_DROPDOWN_FG;

// Icon / divider / label --------------------------------------------------
pub const ICON_FG: Color32 = Palette::VSCE_ICON_FG;
pub const TEXT_SEPARATOR_FG: Color32 = Palette::VSCE_TEXT_SEPARATOR_FG;
pub const LABEL_FG: Color32 = Palette::VSCE_LABEL_FG;

// Checkbox / form options -------------------------------------------------
pub const CHECKBOX_BG: Color32 = Palette::VSCE_CHECKBOX_BG;
pub const CHECKBOX_BORDER: Color32 = Palette::VSCE_CHECKBOX_BORDER;
pub const INPUT_ERROR_BORDER: Color32 = Palette::VSCE_INPUT_ERROR_BORDER;
pub const INPUT_OPTION_HOVER_BG: Color32 = Palette::VSCE_INPUT_OPTION_HOVER_BG;

// Layout containers -------------------------------------------------------
pub const SECTION_HEADER_BG: Color32 = Palette::VSCE_SECTION_HEADER_BG;
pub const SECTION_HEADER_FG: Color32 = Palette::VSCE_SECTION_HEADER_FG;
pub const SECTION_HEADER_BORDER: Color32 = Palette::VSCE_SECTION_HEADER_BORDER;
pub const SCROLLBAR_SLIDER_BG: Color32 = Palette::VSCE_SCROLLBAR_SLIDER_BG;
pub const SCROLLBAR_SLIDER_HOVER: Color32 = Palette::VSCE_SCROLLBAR_SLIDER_HOVER;
pub const SCROLLBAR_SLIDER_ACTIVE: Color32 = Palette::VSCE_SCROLLBAR_SLIDER_ACTIVE;
pub const SASH_HOVER_BORDER: Color32 = Palette::VSCE_SASH_HOVER_BORDER;

// Tabs --------------------------------------------------------------------
pub const TAB_ACTIVE_BG: Color32 = Palette::VSCE_TAB_ACTIVE_BG;
pub const TAB_INACTIVE_BG: Color32 = Palette::VSCE_TAB_INACTIVE_BG;
pub const TAB_ACTIVE_FG: Color32 = Palette::VSCE_TAB_ACTIVE_FG;
pub const TAB_INACTIVE_FG: Color32 = Palette::VSCE_TAB_INACTIVE_FG;
pub const TAB_BORDER: Color32 = Palette::VSCE_TAB_BORDER;
pub const TAB_ACTIVE_BORDER_TOP: Color32 = Palette::VSCE_TAB_ACTIVE_BORDER_TOP;

// Tree --------------------------------------------------------------------
pub const TREE_INDENT_GUIDE: Color32 = Palette::VSCE_TREE_INDENT_GUIDE;

// Table -------------------------------------------------------------------
pub const TABLE_COLUMN_BORDER: Color32 = Palette::VSCE_TABLE_COLUMN_BORDER;
pub const TABLE_ODD_ROW_BG: Color32 = Palette::VSCE_TABLE_ODD_ROW_BG;
