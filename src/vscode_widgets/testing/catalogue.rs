//! Single source of truth for both the interactive storybook
//! (`examples/widget_showcase.rs`) and the visual-parity test harness
//! (`tests/widget_parity.rs`).
//!
//! Each widget module exports a `pub const STORY: Story = ...`. The aggregate
//! `STORIES` slice composes them.

use egui::Ui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Primitives,
    Forms,
    Layout,
    Composite,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Primitives => "Primitives",
            Category::Forms => "Forms",
            Category::Layout => "Layout",
            Category::Composite => "Composite",
        }
    }
}

/// One documented state of a widget (e.g. "default", "hover", "disabled").
#[derive(Clone, Copy)]
pub struct StoryState {
    pub name: &'static str,
    /// One-line caption shown above the rendered widget in the storybook card.
    pub caption: &'static str,
    /// Suggested canvas size for this state (in logical points). The parity
    /// harness uses this for `Harness::builder().with_size(...)`.
    pub size: (f32, f32),
    /// Renderer for this state. The closure receives a `Ui` already painted on
    /// a `Palette::EDITOR_BG` (or `PANEL_BG`) background.
    pub draw: fn(&mut Ui),
}

/// One catalogue entry — a widget plus all of its documented states.
#[derive(Clone, Copy)]
pub struct Story {
    /// Component slug, used as folder name (e.g. `"button"`, `"tree-item"`).
    pub widget: &'static str,
    /// Human-readable name shown in the storybook left rail.
    pub display: &'static str,
    /// Upstream docs URL — shown in the storybook top strip + serves as the
    /// pointer to capture baseline screenshots from.
    pub upstream_url: &'static str,
    pub category: Category,
    pub states: &'static [StoryState],
}

/// Aggregate list of every widget that the storybook + parity tests know
/// about. Widget modules will be added to this list as they land.
pub const STORIES: &[Story] = &[
    super::stories::ICON,
    super::stories::DIVIDER,
    super::stories::LABEL,
    super::stories::BUTTON,
    super::stories::ICON_BUTTON,
    super::stories::BADGE,
    super::stories::PROGRESS_RING,
    super::stories::TEXTFIELD,
    super::stories::TEXTAREA,
    super::stories::CHECKBOX,
    super::stories::RADIO,
    super::stories::COLLAPSIBLE,
    super::stories::SCROLLABLE,
    super::stories::TOOLBAR,
    super::stories::SPLIT_LAYOUT,
    super::stories::TABS,
    super::stories::CONTEXT_MENU,
    super::stories::TREE,
    super::stories::SINGLE_SELECT,
    super::stories::MULTI_SELECT,
    super::stories::TABLE,
    super::stories::FORM_CONTAINER,
    super::stories::FORM_HELPER,
];
