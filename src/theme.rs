use egui::{Color32, Stroke, Visuals};

// VS Code "Dark Modern" theme (default since 2023) — exact values from
// vscode/extensions/theme-defaults/themes/dark_modern.json, with fallbacks
// resolved through dark_plus.json and platform color registry defaults.
pub struct Palette;

#[allow(dead_code)]
impl Palette {
    // Surfaces — values pulled from VS Code 1.119 default "2026 Dark"
    // (extensions/theme-defaults/themes/2026-dark.json).
    pub const EDITOR_BG: Color32 = rgb(0x12, 0x13, 0x14);
    pub const PANEL_BG: Color32 = rgb(0x19, 0x1A, 0x1B); // sidebar / activity bar / status bar / title bar / tabs strip
    // Inter-panel hairline. VS Code 2026 Dark uses `contrastBorder = null`
    // and lets neighbouring background colours do the separation; when an
    // explicit border is needed (tab strip, section header) the value is
    // ~#2B2B2B. Anything brighter looks "drawn" and breaks the look.
    pub const BORDER: Color32 = rgb(0x2B, 0x2B, 0x2B);
    pub const WIDGET_BORDER: Color32 = rgb(0x2B, 0x2B, 0x2B);
    pub const QUICK_INPUT_BG: Color32 = rgb(0x20, 0x21, 0x22);
    pub const MENU_BG: Color32 = rgb(0x20, 0x21, 0x22);
    pub const EDITOR_WIDGET_BG: Color32 = rgb(0x20, 0x21, 0x22);

    // Foreground
    pub const FG: Color32 = rgb(0xBF, 0xBF, 0xBF);
    pub const FG_BRIGHT: Color32 = rgb(0xFF, 0xFF, 0xFF);
    pub const FG_DESCRIPTION: Color32 = rgb(0x8C, 0x8C, 0x8C);
    pub const FG_INACTIVE: Color32 = rgb(0x8C, 0x8C, 0x8C);
    pub const ACTIVITY_FG: Color32 = rgb(0xBF, 0xBF, 0xBF);

    // Accent — 2026 Dark uses a desaturated teal-blue instead of Dark+ azure
    pub const ACCENT: Color32 = rgb(0x39, 0x94, 0xBC);
    pub const ACCENT_HOVER: Color32 = rgb(0x2B, 0x7D, 0xA3);
    pub const FOCUS_BORDER: Color32 = rgb(0x39, 0x94, 0xBC);
    pub const BUTTON_BG: Color32 = rgb(0x29, 0x7A, 0xA0);
    pub const BUTTON_HOVER: Color32 = rgb(0x2B, 0x7D, 0xA3);

    // States — VS Code uses translucent overlays for hovers
    pub const LIST_HOVER_BG: Color32 = rgba(0xFF, 0xFF, 0xFF, 0x14); // ~8% white
    pub const LIST_ACTIVE_SELECTION_BG: Color32 = rgb(0x04, 0x39, 0x5E);
    pub const LIST_INACTIVE_SELECTION_BG: Color32 = rgb(0x37, 0x37, 0x3D);
    pub const STATUS_ITEM_HOVER_BG: Color32 = rgba(0xF1, 0xF1, 0xF1, 0x33);

    // Inputs
    pub const INPUT_BG: Color32 = rgb(0x19, 0x1A, 0x1B);
    pub const INPUT_BORDER: Color32 = rgb(0x33, 0x35, 0x36);
    pub const INPUT_PLACEHOLDER: Color32 = rgb(0x8C, 0x8C, 0x8C);
    // (INPUT_OPTION_ACTIVE_* are defined below in the 2026 Dark block)

    // Tabs
    pub const TAB_ACTIVE_BG: Color32 = rgb(0x12, 0x13, 0x14); // = EDITOR_BG, kept literal so compiler can fold
    pub const TAB_INACTIVE_BG: Color32 = rgb(0x19, 0x1A, 0x1B);
    pub const TABS_STRIP_BG: Color32 = rgb(0x19, 0x1A, 0x1B);
    pub const TAB_ACTIVE_BORDER_TOP: Color32 = rgb(0x39, 0x94, 0xBC);

    // Editor gutter
    pub const LINE_NUMBER_FG: Color32 = rgb(0x85, 0x88, 0x89);
    pub const LINE_NUMBER_ACTIVE_FG: Color32 = rgb(0xBB, 0xBE, 0xBF);

    // Selection
    pub const SELECTION_BG: Color32 = rgb(0x27, 0x67, 0x82);
    pub const FIND_MATCH_BG: Color32 = rgb(0x27, 0x67, 0x82);
    pub const SEARCH_MATCH_BG: Color32 = rgba(0x27, 0x67, 0x82, 0x80);
    pub const LINE_HIGHLIGHT_BG: Color32 = rgb(0x24, 0x25, 0x26);
    pub const BADGE_BG: Color32 = rgb(0x39, 0x94, 0xBC);

    // Input option active states
    pub const INPUT_OPTION_ACTIVE_BG: Color32 = rgba(0x39, 0x94, 0xBC, 0x60);
    pub const INPUT_OPTION_ACTIVE_BORDER: Color32 = rgb(0x39, 0x94, 0xBC);

    // Activity bar — 2026 Dark uses a neutral light-gray indicator, not the accent color
    pub const ACTIVITY_BAR_BG: Color32 = Self::PANEL_BG;
    pub const ACTIVITY_BAR_FG: Color32 = Self::ACTIVITY_FG;
    pub const ACTIVITY_BAR_INACTIVE_FG: Color32 = Self::FG_INACTIVE;
    pub const ACTIVITY_BAR_ACTIVE_BORDER: Color32 = rgb(0xBF, 0xBF, 0xBF);
    pub const ACTIVITY_BAR_BADGE_BG: Color32 = Self::ACCENT;
    pub const ACTIVITY_BAR_BADGE_FG: Color32 = Self::FG_BRIGHT;

    // Sidebar
    pub const SIDEBAR_BG: Color32 = Self::PANEL_BG;
    pub const SIDEBAR_FG: Color32 = Self::FG;
    pub const SIDEBAR_SECTION_HEADER_BG: Color32 = Self::PANEL_BG;
    pub const SIDEBAR_SECTION_HEADER_FG: Color32 = Self::FG;

    // Status bar
    pub const STATUS_BAR_BG: Color32 = Self::PANEL_BG;
    pub const STATUS_BAR_FG: Color32 = Self::FG;
    pub const STATUS_BAR_REMOTE_BG: Color32 = Self::ACCENT;

    // Title bar
    pub const TITLE_BAR_BG: Color32 = Self::PANEL_BG;
    pub const TITLE_BAR_FG: Color32 = Self::FG;
    pub const TITLE_BAR_INACTIVE_FG: Color32 = Self::FG_INACTIVE;

    // Errors / state
    pub const ERROR: Color32 = rgb(0xF8, 0x51, 0x49);
    pub const WARNING: Color32 = rgb(0xCC, 0xA7, 0x00);
    pub const SUCCESS: Color32 = rgb(0x2E, 0xA0, 0x43);
    pub const GIT_MODIFIED: Color32 = rgb(0xE2, 0xC0, 0x8D);

    // ─── vscode-elements bridge (VSCE_*) ──────────────────────────────────
    //
    // These constants mirror the CSS custom properties documented at
    // https://vscode-elements.github.io/. Each entry references the
    // upstream variable name + default value taken from
    // vscode-original/extensions/theme-defaults/themes/dark_modern.json
    // (with fallback to colorRegistry.ts defaults where the theme JSON
    // doesn't override the registration).

    // --vscode-button-* (from dark_modern.json:button.*)
    pub const VSCE_BUTTON_BG: Color32 = rgb(0x00, 0x78, 0xD4);
    pub const VSCE_BUTTON_FG: Color32 = rgb(0xFF, 0xFF, 0xFF);
    pub const VSCE_BUTTON_HOVER_BG: Color32 = rgb(0x02, 0x6E, 0xC1);
    pub const VSCE_BUTTON_BORDER: Color32 = rgba(0xFF, 0xFF, 0xFF, 0x1A);
    pub const VSCE_BUTTON_SECONDARY_BG: Color32 = rgba(0x00, 0x00, 0x00, 0x00);
    pub const VSCE_BUTTON_SECONDARY_FG: Color32 = rgb(0xCC, 0xCC, 0xCC);
    pub const VSCE_BUTTON_SECONDARY_HOVER_BG: Color32 = rgb(0x2B, 0x2B, 0x2B);

    // --vscode-input-* (from dark_modern.json:input.*)
    pub const VSCE_INPUT_BG: Color32 = rgb(0x31, 0x31, 0x31);
    pub const VSCE_INPUT_FG: Color32 = rgb(0xCC, 0xCC, 0xCC);
    pub const VSCE_INPUT_BORDER: Color32 = rgb(0x3C, 0x3C, 0x3C);
    pub const VSCE_INPUT_PLACEHOLDER_FG: Color32 = rgb(0x98, 0x98, 0x98);

    // --vscode-badge-* (from dark_modern.json:badge.*)
    pub const VSCE_BADGE_BG: Color32 = rgb(0x61, 0x61, 0x61);
    pub const VSCE_BADGE_FG: Color32 = rgb(0xF8, 0xF8, 0xF8);

    // --vscode-focusBorder (from dark_modern.json:focusBorder)
    pub const VSCE_FOCUS_BORDER: Color32 = rgb(0x00, 0x78, 0xD4);

    // --vscode-foreground / --vscode-descriptionForeground / etc.
    pub const VSCE_FG: Color32 = rgb(0xCC, 0xCC, 0xCC);
    pub const VSCE_FG_DESCRIPTION: Color32 = rgb(0x9D, 0x9D, 0x9D);
    pub const VSCE_FG_DISABLED: Color32 = rgb(0x55, 0x55, 0x55);
    pub const VSCE_FG_ERROR: Color32 = rgb(0xF8, 0x51, 0x49);

    // --vscode-list-* (from colorRegistry.ts list.* defaults — theme JSON inherits)
    pub const VSCE_LIST_HOVER_BG: Color32 = rgb(0x2A, 0x2D, 0x2E);
    pub const VSCE_LIST_ACTIVE_SELECTION_BG: Color32 = rgb(0x04, 0x39, 0x5E);
    pub const VSCE_LIST_ACTIVE_SELECTION_FG: Color32 = rgb(0xFF, 0xFF, 0xFF);

    // --vscode-dropdown-* (from dark_modern.json:dropdown.*)
    pub const VSCE_DROPDOWN_BG: Color32 = rgb(0x31, 0x31, 0x31);
    pub const VSCE_DROPDOWN_BORDER: Color32 = rgb(0x3C, 0x3C, 0x3C);
    pub const VSCE_DROPDOWN_FG: Color32 = rgb(0xCC, 0xCC, 0xCC);

    // --vscode-checkbox-* (from dark_modern.json:checkbox.*)
    pub const VSCE_CHECKBOX_BG: Color32 = rgb(0x31, 0x31, 0x31);
    pub const VSCE_CHECKBOX_BORDER: Color32 = rgb(0x3C, 0x3C, 0x3C);

    // --vscode-progressBar-background
    pub const VSCE_PROGRESS_BG: Color32 = rgb(0x00, 0x78, 0xD4);

    // --vscode-icon-foreground (from dark_modern.json:icon.foreground)
    pub const VSCE_ICON_FG: Color32 = rgb(0xCC, 0xCC, 0xCC);

    // --vscode-textSeparator-foreground (baseColors.ts: rgba(255,255,255,0.18))
    pub const VSCE_TEXT_SEPARATOR_FG: Color32 = rgba(0xFF, 0xFF, 0xFF, 0x2E);

    // --vscode-settings-headerForeground (label.foreground analogue)
    pub const VSCE_LABEL_FG: Color32 = rgb(0xE7, 0xE7, 0xE7);

    // --vscode-inputValidation-errorBorder
    pub const VSCE_INPUT_ERROR_BORDER: Color32 = rgb(0xBE, 0x11, 0x00);

    // --vscode-inputOption-hoverBackground (checkbox / toggle hover)
    pub const VSCE_INPUT_OPTION_HOVER_BG: Color32 = rgba(0xFF, 0xFF, 0xFF, 0x14);

    // Layout: section header / scrollbar / sash --------------------------
    pub const VSCE_SECTION_HEADER_BG: Color32 = rgb(0x18, 0x18, 0x18);
    pub const VSCE_SECTION_HEADER_FG: Color32 = rgb(0xCC, 0xCC, 0xCC);
    pub const VSCE_SECTION_HEADER_BORDER: Color32 = rgb(0x2B, 0x2B, 0x2B);
    pub const VSCE_SCROLLBAR_SLIDER_BG: Color32 = rgba(0x79, 0x79, 0x79, 0x66);
    pub const VSCE_SCROLLBAR_SLIDER_HOVER: Color32 = rgba(0x64, 0x64, 0x64, 0xB2);
    pub const VSCE_SCROLLBAR_SLIDER_ACTIVE: Color32 = rgba(0xBF, 0xBF, 0xBF, 0x66);
    pub const VSCE_SASH_HOVER_BORDER: Color32 = rgb(0x00, 0x78, 0xD4);

    // Tabs ----------------------------------------------------------------
    pub const VSCE_TAB_ACTIVE_BG: Color32 = rgb(0x1F, 0x1F, 0x1F);
    pub const VSCE_TAB_INACTIVE_BG: Color32 = rgb(0x18, 0x18, 0x18);
    pub const VSCE_TAB_ACTIVE_FG: Color32 = rgb(0xFF, 0xFF, 0xFF);
    pub const VSCE_TAB_INACTIVE_FG: Color32 = rgb(0x9D, 0x9D, 0x9D);
    pub const VSCE_TAB_BORDER: Color32 = rgb(0x2B, 0x2B, 0x2B);
    pub const VSCE_TAB_ACTIVE_BORDER_TOP: Color32 = rgb(0x00, 0x78, 0xD4);

    // Tree indent guides --------------------------------------------------
    // VS Code applies the stroke colour at ~50% alpha for inactive lines
    // (workbench.tree.indentGuidesStroke is #585858 nominally, but the
    // tree renderer dims it for non-hovered rows).
    pub const VSCE_TREE_INDENT_GUIDE: Color32 = rgba(0x58, 0x58, 0x58, 0x88);

    // Tables --------------------------------------------------------------
    pub const VSCE_TABLE_COLUMN_BORDER: Color32 = rgba(0xCC, 0xCC, 0xCC, 0x20);
    pub const VSCE_TABLE_ODD_ROW_BG: Color32 = rgba(0xCC, 0xCC, 0xCC, 0x0A);
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(
        ((r as u16 * a as u16) / 255) as u8,
        ((g as u16 * a as u16) / 255) as u8,
        ((b as u16 * a as u16) / 255) as u8,
        a,
    )
}

pub fn apply(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(Palette::FG);
    visuals.panel_fill = Palette::EDITOR_BG;
    visuals.window_fill = Palette::MENU_BG;
    visuals.extreme_bg_color = Palette::INPUT_BG;
    visuals.faint_bg_color = Palette::PANEL_BG;
    visuals.code_bg_color = Palette::EDITOR_BG;

    visuals.widgets.noninteractive.bg_fill = Palette::EDITOR_BG;
    visuals.widgets.noninteractive.weak_bg_fill = Palette::EDITOR_BG;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Palette::BORDER);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Palette::FG);

    visuals.widgets.inactive.bg_fill = Palette::INPUT_BG;
    visuals.widgets.inactive.weak_bg_fill = Palette::INPUT_BG;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Palette::INPUT_BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Palette::FG);

    visuals.widgets.hovered.bg_fill = Palette::LIST_HOVER_BG;
    visuals.widgets.hovered.weak_bg_fill = Palette::LIST_HOVER_BG;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Palette::INPUT_BORDER);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Palette::FG_BRIGHT);

    visuals.widgets.active.bg_fill = Palette::ACCENT;
    visuals.widgets.active.weak_bg_fill = Palette::ACCENT;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, Palette::ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Palette::FG_BRIGHT);

    visuals.widgets.open.bg_fill = Palette::LIST_HOVER_BG;
    visuals.widgets.open.weak_bg_fill = Palette::LIST_HOVER_BG;

    visuals.selection.bg_fill = Palette::SELECTION_BG;
    visuals.selection.stroke = Stroke::new(1.0, Palette::FG);

    visuals.hyperlink_color = rgb(0x3B, 0x8E, 0xEA);
    visuals.window_stroke = Stroke::new(1.0, Palette::BORDER);
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
