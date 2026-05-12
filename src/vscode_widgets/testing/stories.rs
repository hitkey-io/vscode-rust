//! Story definitions per widget. Aggregated into `STORIES` by
//! `catalogue.rs`. Each widget gets one `pub const STORY_NAME: Story`
//! constant whose `states` slice covers every documented variant from
//! the upstream `vscode-elements` docs page.

use super::catalogue::{Category, Story, StoryState};
use crate::icons;
use crate::vscode_widgets::forms::{
    checkbox, radio, textarea, textfield, CheckboxProps, CheckboxState, RadioProps, TextFieldProps,
    TextareaProps,
};
use crate::vscode_widgets::composite::{
    context_menu, multi_select, single_select, table, tabs, tree, ContextMenuItem,
    ContextMenuProps, MultiSelectProps, SingleSelectProps, Tab, TableProps, TabsProps, TreeItem,
    TreeProps,
};
use crate::vscode_widgets::forms::{
    form_container, form_group, form_helper, FormContainerProps, FormGroupProps, FormHelperProps,
};
use crate::vscode_widgets::layout::{
    collapsible, scrollable, split_layout, toolbar_container, CollapsibleProps, ScrollableProps,
    SplitLayoutProps, ToolbarContainerProps,
};
use crate::vscode_widgets::primitives::{
    badge, button, divider, icon, icon_button, label, progress_ring, BadgeProps, ButtonProps,
    DividerProps, IconButtonProps, IconProps, LabelProps, ProgressRingProps,
};
use egui::Ui;

// Helpers ----------------------------------------------------------------

fn persisted_string(ui: &mut Ui, key: &'static str, default: &str) -> String {
    let id = egui::Id::new(("vsce_story", key));
    ui.data(|d| d.get_temp::<String>(id).unwrap_or_else(|| default.to_string()))
}

fn write_string(ui: &mut Ui, key: &'static str, value: String) {
    let id = egui::Id::new(("vsce_story", key));
    ui.data_mut(|d| d.insert_temp(id, value));
}

fn persisted_check(ui: &mut Ui, key: &'static str, default: CheckboxState) -> CheckboxState {
    let id = egui::Id::new(("vsce_story", key));
    ui.data(|d| d.get_temp::<CheckboxState>(id).unwrap_or(default))
}

fn write_check(ui: &mut Ui, key: &'static str, value: CheckboxState) {
    let id = egui::Id::new(("vsce_story", key));
    ui.data_mut(|d| d.insert_temp(id, value));
}

fn persisted_bool(ui: &mut Ui, key: &'static str, default: bool) -> bool {
    let id = egui::Id::new(("vsce_story", key));
    ui.data(|d| d.get_temp::<bool>(id).unwrap_or(default))
}

fn write_bool(ui: &mut Ui, key: &'static str, value: bool) {
    let id = egui::Id::new(("vsce_story", key));
    ui.data_mut(|d| d.insert_temp(id, value));
}

fn persisted_f32(ui: &mut Ui, key: &'static str, default: f32) -> f32 {
    let id = egui::Id::new(("vsce_story", key));
    ui.data(|d| d.get_temp::<f32>(id).unwrap_or(default))
}

fn write_f32(ui: &mut Ui, key: &'static str, value: f32) {
    let id = egui::Id::new(("vsce_story", key));
    ui.data_mut(|d| d.insert_temp(id, value));
}

fn persisted_usize(ui: &mut Ui, key: &'static str, default: usize) -> usize {
    let id = egui::Id::new(("vsce_story", key));
    ui.data(|d| d.get_temp::<usize>(id).unwrap_or(default))
}

fn write_usize(ui: &mut Ui, key: &'static str, value: usize) {
    let id = egui::Id::new(("vsce_story", key));
    ui.data_mut(|d| d.insert_temp(id, value));
}

fn persisted<T: Clone + Send + Sync + 'static>(ui: &mut Ui, key: &'static str, default: T) -> T {
    let id = egui::Id::new(("vsce_story", key));
    ui.data(|d| d.get_temp::<T>(id).unwrap_or(default))
}

fn write_value<T: Clone + Send + Sync + 'static>(ui: &mut Ui, key: &'static str, value: T) {
    let id = egui::Id::new(("vsce_story", key));
    ui.data_mut(|d| d.insert_temp(id, value));
}

// ─── icon ────────────────────────────────────────────────────────────────

fn icon_default(ui: &mut Ui) {
    icon(ui, &IconProps::new(icons::SEARCH));
}
fn icon_large(ui: &mut Ui) {
    icon(ui, &IconProps::new(icons::SEARCH).size(32.0));
}
fn icon_action(ui: &mut Ui) {
    icon(ui, &IconProps::new(icons::SETTINGS_GEAR).action_icon());
}
fn icon_spin(ui: &mut Ui) {
    icon(ui, &IconProps::new(icons::SYNC).spin());
}

pub const ICON: Story = Story {
    widget: "icon",
    display: "Icon",
    upstream_url: "https://vscode-elements.github.io/components/icon/",
    category: Category::Primitives,
    states: &[
        StoryState {
            name: "default",
            caption: "16×16 codicon, default colour",
            size: (40.0, 40.0),
            draw: icon_default,
        },
        StoryState {
            name: "large",
            caption: "32×32 codicon",
            size: (60.0, 60.0),
            draw: icon_large,
        },
        StoryState {
            name: "action-icon",
            caption: "Toolbar action variant — hover adds a tinted background",
            size: (40.0, 40.0),
            draw: icon_action,
        },
        StoryState {
            name: "spinning",
            caption: "Spin animation — useful for sync/loading states",
            size: (40.0, 40.0),
            draw: icon_spin,
        },
    ],
};

// ─── divider ─────────────────────────────────────────────────────────────

fn divider_horizontal(ui: &mut Ui) {
    ui.add_space(8.0);
    divider(ui, &DividerProps::horizontal().length(200.0));
    ui.add_space(8.0);
}
fn divider_vertical(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        divider(ui, &DividerProps::vertical().length(40.0));
        ui.add_space(8.0);
    });
}

pub const DIVIDER: Story = Story {
    widget: "divider",
    display: "Divider",
    upstream_url: "https://vscode-elements.github.io/components/divider/",
    category: Category::Primitives,
    states: &[
        StoryState {
            name: "horizontal",
            caption: "1px horizontal separator",
            size: (220.0, 24.0),
            draw: divider_horizontal,
        },
        StoryState {
            name: "vertical",
            caption: "1px vertical separator",
            size: (28.0, 56.0),
            draw: divider_vertical,
        },
    ],
};

// ─── label ───────────────────────────────────────────────────────────────

fn label_default(ui: &mut Ui) {
    label(ui, &LabelProps::new("Label"));
}
fn label_normal(ui: &mut Ui) {
    label(ui, &LabelProps::new("Label").normal());
}
fn label_required(ui: &mut Ui) {
    label(ui, &LabelProps::new("Field name").required());
}
fn label_description(ui: &mut Ui) {
    label(
        ui,
        &LabelProps::new("Helper text below an input").description().normal(),
    );
}

pub const LABEL: Story = Story {
    widget: "label",
    display: "Label",
    upstream_url: "https://vscode-elements.github.io/components/label/",
    category: Category::Primitives,
    states: &[
        StoryState {
            name: "default",
            caption: "Bold label foreground",
            size: (180.0, 24.0),
            draw: label_default,
        },
        StoryState {
            name: "normal",
            caption: "Non-bold variant",
            size: (180.0, 24.0),
            draw: label_normal,
        },
        StoryState {
            name: "required",
            caption: "Red asterisk for required form fields",
            size: (180.0, 24.0),
            draw: label_required,
        },
        StoryState {
            name: "description",
            caption: "Description tone — dimmer foreground",
            size: (260.0, 24.0),
            draw: label_description,
        },
    ],
};

// ─── button ──────────────────────────────────────────────────────────────

fn button_primary(ui: &mut Ui) {
    button(ui, &ButtonProps::new("Confirm"));
}
fn button_secondary(ui: &mut Ui) {
    button(ui, &ButtonProps::new("Cancel").secondary());
}
fn button_disabled(ui: &mut Ui) {
    button(ui, &ButtonProps::new("Confirm").disabled());
}
fn button_focused(ui: &mut Ui) {
    button(ui, &ButtonProps::new("Confirm").focused());
}
fn button_icon_before(ui: &mut Ui) {
    button(ui, &ButtonProps::new("Search").icon(icons::SEARCH));
}
fn button_icon_after(ui: &mut Ui) {
    button(ui, &ButtonProps::new("Continue").icon_after(icons::CHEVRON_RIGHT));
}
fn button_block(ui: &mut Ui) {
    button(ui, &ButtonProps::new("Full width action").block());
}
fn button_small(ui: &mut Ui) {
    button(ui, &ButtonProps::new("Small").small());
}

pub const BUTTON: Story = Story {
    widget: "button",
    display: "Button",
    upstream_url: "https://vscode-elements.github.io/components/button/",
    category: Category::Primitives,
    states: &[
        StoryState {
            name: "primary",
            caption: "Default filled button",
            size: (140.0, 32.0),
            draw: button_primary,
        },
        StoryState {
            name: "secondary",
            caption: "Outline variant for less-prominent actions",
            size: (140.0, 32.0),
            draw: button_secondary,
        },
        StoryState {
            name: "disabled",
            caption: "Disabled state — 0.4 opacity",
            size: (140.0, 32.0),
            draw: button_disabled,
        },
        StoryState {
            name: "focused",
            caption: "Focus ring (programmatic)",
            size: (160.0, 36.0),
            draw: button_focused,
        },
        StoryState {
            name: "icon-before",
            caption: "Leading codicon",
            size: (160.0, 32.0),
            draw: button_icon_before,
        },
        StoryState {
            name: "icon-after",
            caption: "Trailing codicon",
            size: (160.0, 32.0),
            draw: button_icon_after,
        },
        StoryState {
            name: "block",
            caption: "Block-level button fills the available width",
            size: (260.0, 32.0),
            draw: button_block,
        },
        StoryState {
            name: "small",
            caption: "Small size variant (11/14 instead of 12/16)",
            size: (110.0, 28.0),
            draw: button_small,
        },
    ],
};

// ─── icon-button ─────────────────────────────────────────────────────────

fn icon_button_default(ui: &mut Ui) {
    icon_button(ui, &IconButtonProps::new(icons::SETTINGS_GEAR));
}
fn icon_button_disabled(ui: &mut Ui) {
    icon_button(ui, &IconButtonProps::new(icons::SETTINGS_GEAR).disabled());
}
fn icon_button_activity_bar(ui: &mut Ui) {
    icon_button(
        ui,
        &IconButtonProps::new(icons::FILES).size(48.0).icon_size(24.0).active_stripe(),
    );
}

pub const ICON_BUTTON: Story = Story {
    widget: "icon-button",
    display: "IconButton",
    upstream_url: "https://vscode-elements.github.io/components/icon-button/",
    category: Category::Primitives,
    states: &[
        StoryState {
            name: "default",
            caption: "Toolbar action button (22×22)",
            size: (36.0, 36.0),
            draw: icon_button_default,
        },
        StoryState {
            name: "disabled",
            caption: "Disabled — icon dimmed to 0.4",
            size: (36.0, 36.0),
            draw: icon_button_disabled,
        },
        StoryState {
            name: "activity-bar",
            caption: "Activity-bar pattern: 48×48 with left active stripe",
            size: (60.0, 60.0),
            draw: icon_button_activity_bar,
        },
    ],
};

// ─── badge ───────────────────────────────────────────────────────────────

fn badge_default(ui: &mut Ui) {
    badge(ui, &BadgeProps::new("3"));
}
fn badge_counter(ui: &mut Ui) {
    badge(ui, &BadgeProps::counter("128 issues"));
}
fn badge_activity_bar(ui: &mut Ui) {
    badge(ui, &BadgeProps::activity_bar("9"));
}
fn badge_tab_header(ui: &mut Ui) {
    badge(ui, &BadgeProps::tab_header("M"));
}

pub const BADGE: Story = Story {
    widget: "badge",
    display: "Badge",
    upstream_url: "https://vscode-elements.github.io/components/badge/",
    category: Category::Primitives,
    states: &[
        StoryState {
            name: "default",
            caption: "Pill — 18×18 minimum, badge-background fill",
            size: (40.0, 32.0),
            draw: badge_default,
        },
        StoryState {
            name: "counter",
            caption: "Counter variant — long label, square corners",
            size: (110.0, 24.0),
            draw: badge_counter,
        },
        StoryState {
            name: "activity-bar",
            caption: "Activity-bar accent — blue background",
            size: (40.0, 32.0),
            draw: badge_activity_bar,
        },
        StoryState {
            name: "tab-header",
            caption: "Compact tab-header counter",
            size: (40.0, 24.0),
            draw: badge_tab_header,
        },
    ],
};

// ─── progress-ring ───────────────────────────────────────────────────────

fn progress_ring_default(ui: &mut Ui) {
    progress_ring(ui, &ProgressRingProps::default());
}
fn progress_ring_large(ui: &mut Ui) {
    progress_ring(ui, &ProgressRingProps::default().size(32.0).thickness(3.0));
}

pub const PROGRESS_RING: Story = Story {
    widget: "progress-ring",
    display: "ProgressRing",
    upstream_url: "https://vscode-elements.github.io/components/progress-ring/",
    category: Category::Primitives,
    states: &[
        StoryState {
            name: "default",
            caption: "16×16 indeterminate spinner (one frame)",
            size: (32.0, 32.0),
            draw: progress_ring_default,
        },
        StoryState {
            name: "large",
            caption: "32×32 spinner with thicker stroke",
            size: (44.0, 44.0),
            draw: progress_ring_large,
        },
    ],
};

// ─── textfield ───────────────────────────────────────────────────────────

fn textfield_placeholder(ui: &mut Ui) {
    let mut v = persisted_string(ui, "tf_placeholder", "");
    textfield(ui, &TextFieldProps::new().placeholder("Type here…"), &mut v);
    write_string(ui, "tf_placeholder", v);
}
fn textfield_with_value(ui: &mut Ui) {
    let mut v = persisted_string(ui, "tf_value", "Hello world");
    textfield(ui, &TextFieldProps::new(), &mut v);
    write_string(ui, "tf_value", v);
}
fn textfield_focused(ui: &mut Ui) {
    let mut v = persisted_string(ui, "tf_focused", "Focused");
    textfield(ui, &TextFieldProps::new().focused(), &mut v);
    write_string(ui, "tf_focused", v);
}
fn textfield_disabled(ui: &mut Ui) {
    let mut v = persisted_string(ui, "tf_disabled", "Disabled");
    textfield(ui, &TextFieldProps::new().disabled(), &mut v);
    write_string(ui, "tf_disabled", v);
}
fn textfield_invalid(ui: &mut Ui) {
    let mut v = persisted_string(ui, "tf_invalid", "bad@");
    textfield(ui, &TextFieldProps::new().invalid(), &mut v);
    write_string(ui, "tf_invalid", v);
}
fn textfield_prefix(ui: &mut Ui) {
    let mut v = persisted_string(ui, "tf_prefix", "");
    textfield(
        ui,
        &TextFieldProps::new().placeholder("Search").prefix_icon(icons::SEARCH),
        &mut v,
    );
    write_string(ui, "tf_prefix", v);
}
fn textfield_suffix(ui: &mut Ui) {
    let mut v = persisted_string(ui, "tf_suffix", "secret");
    textfield(
        ui,
        &TextFieldProps::new().password().suffix_icon(icons::CLOSE),
        &mut v,
    );
    write_string(ui, "tf_suffix", v);
}

pub const TEXTFIELD: Story = Story {
    widget: "textfield",
    display: "TextField",
    upstream_url: "https://vscode-elements.github.io/components/textfield/",
    category: Category::Forms,
    states: &[
        StoryState {
            name: "placeholder",
            caption: "Empty value with placeholder text",
            size: (240.0, 36.0),
            draw: textfield_placeholder,
        },
        StoryState {
            name: "with-value",
            caption: "Pre-filled text input",
            size: (240.0, 36.0),
            draw: textfield_with_value,
        },
        StoryState {
            name: "focused",
            caption: "Programmatic focus — focus border accent",
            size: (240.0, 36.0),
            draw: textfield_focused,
        },
        StoryState {
            name: "disabled",
            caption: "Disabled state — 0.6 opacity, hover suppressed",
            size: (240.0, 36.0),
            draw: textfield_disabled,
        },
        StoryState {
            name: "invalid",
            caption: "Invalid state — validation error border",
            size: (240.0, 36.0),
            draw: textfield_invalid,
        },
        StoryState {
            name: "prefix-icon",
            caption: "Leading codicon slot",
            size: (240.0, 36.0),
            draw: textfield_prefix,
        },
        StoryState {
            name: "suffix-icon",
            caption: "Trailing codicon slot (password mask + reveal)",
            size: (240.0, 36.0),
            draw: textfield_suffix,
        },
    ],
};

// ─── textarea ────────────────────────────────────────────────────────────

fn textarea_placeholder(ui: &mut Ui) {
    let mut v = persisted_string(ui, "ta_placeholder", "");
    textarea(ui, &TextareaProps::new().placeholder("Write something…"), &mut v);
    write_string(ui, "ta_placeholder", v);
}
fn textarea_with_value(ui: &mut Ui) {
    let mut v = persisted_string(
        ui,
        "ta_value",
        "Line one\nLine two\nLine three\nLine four",
    );
    textarea(ui, &TextareaProps::new().rows(4), &mut v);
    write_string(ui, "ta_value", v);
}
fn textarea_focused(ui: &mut Ui) {
    let mut v = persisted_string(ui, "ta_focused", "Editing…");
    textarea(ui, &TextareaProps::new().rows(3).focused(), &mut v);
    write_string(ui, "ta_focused", v);
}
fn textarea_disabled(ui: &mut Ui) {
    let mut v = persisted_string(ui, "ta_disabled", "Cannot edit");
    textarea(ui, &TextareaProps::new().disabled(), &mut v);
    write_string(ui, "ta_disabled", v);
}

pub const TEXTAREA: Story = Story {
    widget: "textarea",
    display: "Textarea",
    upstream_url: "https://vscode-elements.github.io/components/textarea/",
    category: Category::Forms,
    states: &[
        StoryState {
            name: "placeholder",
            caption: "3 rows, empty value",
            size: (300.0, 80.0),
            draw: textarea_placeholder,
        },
        StoryState {
            name: "with-value",
            caption: "Pre-filled multi-line content (4 rows)",
            size: (300.0, 96.0),
            draw: textarea_with_value,
        },
        StoryState {
            name: "focused",
            caption: "Focus ring",
            size: (300.0, 80.0),
            draw: textarea_focused,
        },
        StoryState {
            name: "disabled",
            caption: "Disabled — 0.6 opacity",
            size: (300.0, 80.0),
            draw: textarea_disabled,
        },
    ],
};

// ─── checkbox ────────────────────────────────────────────────────────────

fn checkbox_unchecked(ui: &mut Ui) {
    let mut s = persisted_check(ui, "cb_unchecked", CheckboxState::Unchecked);
    checkbox(ui, &CheckboxProps::new().label("Unchecked"), &mut s);
    write_check(ui, "cb_unchecked", s);
}
fn checkbox_checked(ui: &mut Ui) {
    let mut s = persisted_check(ui, "cb_checked", CheckboxState::Checked);
    checkbox(ui, &CheckboxProps::new().label("Checked"), &mut s);
    write_check(ui, "cb_checked", s);
}
fn checkbox_indeterminate(ui: &mut Ui) {
    let mut s = persisted_check(ui, "cb_indeterminate", CheckboxState::Indeterminate);
    checkbox(ui, &CheckboxProps::new().label("Indeterminate"), &mut s);
    write_check(ui, "cb_indeterminate", s);
}
fn checkbox_disabled(ui: &mut Ui) {
    let mut s = persisted_check(ui, "cb_disabled", CheckboxState::Checked);
    checkbox(ui, &CheckboxProps::new().label("Disabled").disabled(), &mut s);
    write_check(ui, "cb_disabled", s);
}
fn checkbox_focused(ui: &mut Ui) {
    let mut s = persisted_check(ui, "cb_focused", CheckboxState::Unchecked);
    checkbox(ui, &CheckboxProps::new().label("Focused").focused(), &mut s);
    write_check(ui, "cb_focused", s);
}

pub const CHECKBOX: Story = Story {
    widget: "checkbox",
    display: "Checkbox",
    upstream_url: "https://vscode-elements.github.io/components/checkbox/",
    category: Category::Forms,
    states: &[
        StoryState {
            name: "unchecked",
            caption: "Empty 18×18 box",
            size: (160.0, 28.0),
            draw: checkbox_unchecked,
        },
        StoryState {
            name: "checked",
            caption: "Checked state with codicon tick",
            size: (160.0, 28.0),
            draw: checkbox_checked,
        },
        StoryState {
            name: "indeterminate",
            caption: "Indeterminate — horizontal bar",
            size: (160.0, 28.0),
            draw: checkbox_indeterminate,
        },
        StoryState {
            name: "disabled",
            caption: "Disabled checked state — 0.5 opacity",
            size: (160.0, 28.0),
            draw: checkbox_disabled,
        },
        StoryState {
            name: "focused",
            caption: "Focus ring uses focusBorder",
            size: (160.0, 28.0),
            draw: checkbox_focused,
        },
    ],
};

// ─── radio ───────────────────────────────────────────────────────────────

fn radio_inactive(ui: &mut Ui) {
    let mut s = persisted_bool(ui, "rd_inactive", false);
    radio(ui, &RadioProps::new().label("Option A"), &mut s);
    write_bool(ui, "rd_inactive", s);
}
fn radio_active(ui: &mut Ui) {
    let mut s = persisted_bool(ui, "rd_active", true);
    radio(ui, &RadioProps::new().label("Option B"), &mut s);
    write_bool(ui, "rd_active", s);
}
fn radio_disabled(ui: &mut Ui) {
    let mut s = persisted_bool(ui, "rd_disabled", true);
    radio(ui, &RadioProps::new().label("Disabled").disabled(), &mut s);
    write_bool(ui, "rd_disabled", s);
}
fn radio_focused(ui: &mut Ui) {
    let mut s = persisted_bool(ui, "rd_focused", false);
    radio(ui, &RadioProps::new().label("Focused").focused(), &mut s);
    write_bool(ui, "rd_focused", s);
}

pub const RADIO: Story = Story {
    widget: "radio",
    display: "Radio",
    upstream_url: "https://vscode-elements.github.io/components/radio/",
    category: Category::Forms,
    states: &[
        StoryState {
            name: "inactive",
            caption: "Empty 16×16 circle",
            size: (160.0, 24.0),
            draw: radio_inactive,
        },
        StoryState {
            name: "active",
            caption: "Selected — 4px accent dot",
            size: (160.0, 24.0),
            draw: radio_active,
        },
        StoryState {
            name: "disabled",
            caption: "Disabled active state — 0.5 opacity",
            size: (160.0, 24.0),
            draw: radio_disabled,
        },
        StoryState {
            name: "focused",
            caption: "Focus ring uses focusBorder",
            size: (160.0, 24.0),
            draw: radio_focused,
        },
    ],
};

// ─── collapsible ─────────────────────────────────────────────────────────

fn collapsible_collapsed(ui: &mut Ui) {
    let mut open = persisted_bool(ui, "co_collapsed", false);
    collapsible(ui, &CollapsibleProps::new("Explorer"), &mut open, |_| {});
    write_bool(ui, "co_collapsed", open);
}
fn collapsible_expanded(ui: &mut Ui) {
    let mut open = persisted_bool(ui, "co_expanded", true);
    collapsible(ui, &CollapsibleProps::new("Open editors"), &mut open, |ui| {
        ui.add_space(4.0);
        for name in ["main.rs", "lib.rs", "Cargo.toml"] {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.label(egui::RichText::new(name).size(12.0));
            });
        }
    });
    write_bool(ui, "co_expanded", open);
}
fn collapsible_with_description(ui: &mut Ui) {
    let mut open = persisted_bool(ui, "co_desc", true);
    collapsible(
        ui,
        &CollapsibleProps::new("Outline").description("3 symbols"),
        &mut open,
        |ui| {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("  fn main()").size(12.0));
            ui.label(egui::RichText::new("  fn helper()").size(12.0));
            ui.label(egui::RichText::new("  struct Config").size(12.0));
        },
    );
    write_bool(ui, "co_desc", open);
}

pub const COLLAPSIBLE: Story = Story {
    widget: "collapsible",
    display: "Collapsible",
    upstream_url: "https://vscode-elements.github.io/components/collapsible/",
    category: Category::Layout,
    states: &[
        StoryState {
            name: "collapsed",
            caption: "Header only, body hidden",
            size: (260.0, 32.0),
            draw: collapsible_collapsed,
        },
        StoryState {
            name: "expanded",
            caption: "Open section with three list items",
            size: (260.0, 110.0),
            draw: collapsible_expanded,
        },
        StoryState {
            name: "with-description",
            caption: "Description text rendered next to the title",
            size: (280.0, 110.0),
            draw: collapsible_with_description,
        },
    ],
};

// ─── scrollable ──────────────────────────────────────────────────────────

fn scrollable_vertical(ui: &mut Ui) {
    scrollable(ui, &ScrollableProps::vertical().max_height(80.0), |ui| {
        for i in 1..=12 {
            ui.label(egui::RichText::new(format!("Row #{i}")).size(12.0));
        }
    });
}
fn scrollable_horizontal(ui: &mut Ui) {
    scrollable(
        ui,
        &ScrollableProps::horizontal().max_width(220.0).max_height(40.0),
        |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(
                        "Long line that scrolls to the right — keep going, keep going, keep going…",
                    )
                    .size(12.0),
                );
            });
        },
    );
}

pub const SCROLLABLE: Story = Story {
    widget: "scrollable",
    display: "Scrollable",
    upstream_url: "https://vscode-elements.github.io/components/scrollable/",
    category: Category::Layout,
    states: &[
        StoryState {
            name: "vertical",
            caption: "10px slider, 0.4 alpha — vertical only",
            size: (220.0, 100.0),
            draw: scrollable_vertical,
        },
        StoryState {
            name: "horizontal",
            caption: "Horizontal scroll, no vertical bar",
            size: (240.0, 56.0),
            draw: scrollable_horizontal,
        },
    ],
};

// ─── toolbar ─────────────────────────────────────────────────────────────

fn toolbar_actions(ui: &mut Ui) {
    toolbar_container(ui, &ToolbarContainerProps::new(), |ui| {
        for glyph in [icons::REFRESH, icons::COLLAPSE_ALL, icons::NEW_FILE] {
            icon_button(ui, &IconButtonProps::new(glyph));
        }
    });
}
fn toolbar_with_title(ui: &mut Ui) {
    toolbar_container(
        ui,
        &ToolbarContainerProps::new().title("Explorer"),
        |ui| {
            for glyph in [icons::NEW_FILE, icons::NEW_FOLDER, icons::REFRESH, icons::COLLAPSE_ALL] {
                icon_button(ui, &IconButtonProps::new(glyph));
            }
        },
    );
}
fn toolbar_single(ui: &mut Ui) {
    toolbar_container(ui, &ToolbarContainerProps::new(), |ui| {
        icon_button(ui, &IconButtonProps::new(icons::SETTINGS_GEAR));
    });
}

pub const TOOLBAR: Story = Story {
    widget: "toolbar-container",
    display: "ToolbarContainer",
    upstream_url: "https://vscode-elements.github.io/components/toolbar-container/",
    category: Category::Layout,
    states: &[
        StoryState {
            name: "single",
            caption: "Single right-aligned icon button",
            size: (200.0, 32.0),
            draw: toolbar_single,
        },
        StoryState {
            name: "three-actions",
            caption: "Three right-aligned actions",
            size: (200.0, 32.0),
            draw: toolbar_actions,
        },
        StoryState {
            name: "with-title",
            caption: "Title slot + four-action cluster",
            size: (300.0, 32.0),
            draw: toolbar_with_title,
        },
    ],
};

// ─── split-layout ────────────────────────────────────────────────────────

fn split_layout_vertical(ui: &mut Ui) {
    let mut pos = persisted_f32(ui, "sl_vertical", 120.0);
    split_layout(
        ui,
        &SplitLayoutProps::vertical().min_size(40.0),
        &mut pos,
        |ui| {
            ui.painter().rect_filled(
                ui.available_rect_before_wrap(),
                0.0,
                crate::theme::Palette::SIDEBAR_BG,
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Left pane").size(12.0));
            });
        },
        |ui| {
            ui.painter().rect_filled(
                ui.available_rect_before_wrap(),
                0.0,
                crate::theme::Palette::EDITOR_BG,
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Right pane").size(12.0));
            });
        },
    );
    write_f32(ui, "sl_vertical", pos);
}
fn split_layout_horizontal(ui: &mut Ui) {
    let mut pos = persisted_f32(ui, "sl_horizontal", 60.0);
    split_layout(
        ui,
        &SplitLayoutProps::horizontal().min_size(30.0),
        &mut pos,
        |ui| {
            ui.painter().rect_filled(
                ui.available_rect_before_wrap(),
                0.0,
                crate::theme::Palette::EDITOR_BG,
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Top pane").size(12.0));
            });
        },
        |ui| {
            ui.painter().rect_filled(
                ui.available_rect_before_wrap(),
                0.0,
                crate::theme::Palette::PANEL_BG,
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Bottom pane (panel)").size(12.0));
            });
        },
    );
    write_f32(ui, "sl_horizontal", pos);
}

pub const SPLIT_LAYOUT: Story = Story {
    widget: "split-layout",
    display: "SplitLayout",
    upstream_url: "https://vscode-elements.github.io/components/split-layout/",
    category: Category::Layout,
    states: &[
        StoryState {
            name: "vertical",
            caption: "Side-by-side panes, vertical sash at 120 px",
            size: (320.0, 160.0),
            draw: split_layout_vertical,
        },
        StoryState {
            name: "horizontal",
            caption: "Stacked panes, horizontal sash at 60 px",
            size: (320.0, 160.0),
            draw: split_layout_horizontal,
        },
    ],
};

// ─── tabs ────────────────────────────────────────────────────────────────

fn tabs_default(ui: &mut Ui) {
    let mut active = persisted_usize(ui, "tabs_default", 0);
    tabs(
        ui,
        &TabsProps::default(),
        &[
            Tab::new("Welcome").icon(icons::VSCODE),
            Tab::new("main.rs").icon(icons::FILE),
            Tab::new("Cargo.toml").icon(icons::FILE),
        ],
        &mut active,
    );
    write_usize(ui, "tabs_default", active);
}
fn tabs_dirty(ui: &mut Ui) {
    let mut active = persisted_usize(ui, "tabs_dirty", 1);
    tabs(
        ui,
        &TabsProps::default(),
        &[
            Tab::new("README.md").icon(icons::FILE),
            Tab::new("lib.rs").icon(icons::FILE).dirty(),
            Tab::new("Cargo.lock").icon(icons::FILE),
        ],
        &mut active,
    );
    write_usize(ui, "tabs_dirty", active);
}
fn tabs_with_disabled(ui: &mut Ui) {
    let mut active = persisted_usize(ui, "tabs_disabled", 0);
    tabs(
        ui,
        &TabsProps::default(),
        &[
            Tab::new("Editor").icon(icons::FILE),
            Tab::new("Output").icon(icons::TERMINAL_ICON),
            Tab::new("Debug").icon(icons::DEBUG_ALT).disabled(),
        ],
        &mut active,
    );
    write_usize(ui, "tabs_disabled", active);
}

pub const TABS: Story = Story {
    widget: "tabs",
    display: "Tabs",
    upstream_url: "https://vscode-elements.github.io/components/tabs/",
    category: Category::Composite,
    states: &[
        StoryState {
            name: "default",
            caption: "Three tabs with codicons, first one active",
            size: (440.0, 56.0),
            draw: tabs_default,
        },
        StoryState {
            name: "with-dirty",
            caption: "Dirty (unsaved) middle tab shows filled circle",
            size: (440.0, 56.0),
            draw: tabs_dirty,
        },
        StoryState {
            name: "with-disabled",
            caption: "Last tab disabled — dimmed and non-interactive",
            size: (440.0, 56.0),
            draw: tabs_with_disabled,
        },
    ],
};

// ─── context-menu ────────────────────────────────────────────────────────

fn context_menu_default(ui: &mut Ui) {
    context_menu(
        ui,
        &ContextMenuProps::default(),
        &[
            ContextMenuItem::new("New File").shortcut("Cmd+N"),
            ContextMenuItem::new("New Folder").shortcut("Cmd+Shift+N"),
            ContextMenuItem::new("Open Folder…").shortcut("Cmd+O"),
        ],
    );
}
fn context_menu_with_icons(ui: &mut Ui) {
    context_menu(
        ui,
        &ContextMenuProps::default(),
        &[
            ContextMenuItem::new("Cut").icon(icons::CLOSE).shortcut("Cmd+X"),
            ContextMenuItem::new("Copy").icon(icons::FILE).shortcut("Cmd+C"),
            ContextMenuItem::new("Paste").icon(icons::CHEVRON_DOWN).shortcut("Cmd+V"),
        ],
    );
}
fn context_menu_with_separator(ui: &mut Ui) {
    context_menu(
        ui,
        &ContextMenuProps::default(),
        &[
            ContextMenuItem::new("Close").shortcut("Cmd+W"),
            ContextMenuItem::new("Close Others"),
            ContextMenuItem::new("Close All").shortcut("Cmd+K W"),
            ContextMenuItem::separator(),
            ContextMenuItem::new("Pin").icon(icons::CIRCLE_FILLED),
        ],
    );
}
fn context_menu_disabled(ui: &mut Ui) {
    context_menu(
        ui,
        &ContextMenuProps::default(),
        &[
            ContextMenuItem::new("Undo").shortcut("Cmd+Z"),
            ContextMenuItem::new("Redo").shortcut("Cmd+Shift+Z").disabled(),
            ContextMenuItem::separator(),
            ContextMenuItem::new("Select All").shortcut("Cmd+A"),
        ],
    );
}

pub const CONTEXT_MENU: Story = Story {
    widget: "context-menu",
    display: "ContextMenu",
    upstream_url: "https://vscode-elements.github.io/components/context-menu/",
    category: Category::Composite,
    states: &[
        StoryState {
            name: "default",
            caption: "Three plain items with shortcut hints",
            size: (260.0, 120.0),
            draw: context_menu_default,
        },
        StoryState {
            name: "with-icons",
            caption: "Items render a codicon glyph before the label",
            size: (260.0, 120.0),
            draw: context_menu_with_icons,
        },
        StoryState {
            name: "with-separator",
            caption: "Group divider between item clusters",
            size: (260.0, 160.0),
            draw: context_menu_with_separator,
        },
        StoryState {
            name: "with-disabled",
            caption: "Disabled item — dimmed, non-interactive",
            size: (260.0, 140.0),
            draw: context_menu_disabled,
        },
    ],
};

// ─── tree ────────────────────────────────────────────────────────────────

fn sample_tree() -> Vec<TreeItem> {
    vec![
        TreeItem::folder(
            "src",
            Some(icons::FOLDER_OPENED),
            vec![
                TreeItem::leaf("main.rs", Some(icons::FILE)),
                TreeItem::folder(
                    "vscode_widgets",
                    Some(icons::FOLDER),
                    vec![
                        TreeItem::leaf("button.rs", Some(icons::FILE)),
                        TreeItem::leaf("tree.rs", Some(icons::FILE)),
                    ],
                ),
            ],
        )
        .open(),
        TreeItem::folder(
            "tests",
            Some(icons::FOLDER),
            vec![TreeItem::leaf("widget_parity.rs", Some(icons::FILE))],
        ),
        TreeItem::leaf("Cargo.toml", Some(icons::FILE)),
    ]
}

fn tree_collapsed(ui: &mut Ui) {
    let mut items = persisted::<Vec<TreeItem>>(ui, "tree_collapsed", {
        let mut v = sample_tree();
        // force the first folder closed for this state
        v[0].open = false;
        v
    });
    let mut sel = persisted::<Option<Vec<usize>>>(ui, "tree_collapsed_sel", None);
    tree(ui, &TreeProps::default(), &mut items, &mut sel);
    write_value(ui, "tree_collapsed", items);
    write_value(ui, "tree_collapsed_sel", sel);
}
fn tree_expanded(ui: &mut Ui) {
    let mut items = persisted::<Vec<TreeItem>>(ui, "tree_expanded", {
        let mut v = sample_tree();
        v[0].open = true;
        if let Some(f) = v[0].children.iter_mut().find(|c| c.label == "vscode_widgets") {
            f.open = true;
        }
        v
    });
    let mut sel = persisted::<Option<Vec<usize>>>(ui, "tree_expanded_sel", None);
    tree(ui, &TreeProps::default(), &mut items, &mut sel);
    write_value(ui, "tree_expanded", items);
    write_value(ui, "tree_expanded_sel", sel);
}
fn tree_selected(ui: &mut Ui) {
    let mut items = persisted::<Vec<TreeItem>>(ui, "tree_selected", {
        let mut v = sample_tree();
        v[0].open = true;
        v
    });
    let mut sel = persisted::<Option<Vec<usize>>>(ui, "tree_selected_sel", Some(vec![0, 0]));
    tree(ui, &TreeProps::default(), &mut items, &mut sel);
    write_value(ui, "tree_selected", items);
    write_value(ui, "tree_selected_sel", sel);
}

pub const TREE: Story = Story {
    widget: "tree",
    display: "Tree",
    upstream_url: "https://vscode-elements.github.io/components/tree/",
    category: Category::Composite,
    states: &[
        StoryState {
            name: "collapsed",
            caption: "Three root nodes, all folders closed",
            size: (260.0, 90.0),
            draw: tree_collapsed,
        },
        StoryState {
            name: "expanded",
            caption: "Nested folder open — indent guides visible",
            size: (260.0, 150.0),
            draw: tree_expanded,
        },
        StoryState {
            name: "selected",
            caption: "Selected leaf — list-active-selection background",
            size: (260.0, 120.0),
            draw: tree_selected,
        },
    ],
};

// ─── single-select ───────────────────────────────────────────────────────

const LANG_OPTIONS: &[&str] = &["Rust", "TypeScript", "Python", "Go", "Swift"];

fn single_select_closed(ui: &mut Ui) {
    let mut sel = persisted::<Option<usize>>(ui, "ss_closed_sel", None);
    let mut open = persisted_bool(ui, "ss_closed_open", false);
    single_select(ui, &SingleSelectProps::new(LANG_OPTIONS).placeholder("Choose language…"), &mut sel, &mut open);
    write_value(ui, "ss_closed_sel", sel);
    write_bool(ui, "ss_closed_open", open);
}
fn single_select_with_value(ui: &mut Ui) {
    let mut sel = persisted::<Option<usize>>(ui, "ss_value_sel", Some(0));
    let mut open = persisted_bool(ui, "ss_value_open", false);
    single_select(ui, &SingleSelectProps::new(LANG_OPTIONS), &mut sel, &mut open);
    write_value(ui, "ss_value_sel", sel);
    write_bool(ui, "ss_value_open", open);
}
fn single_select_open(ui: &mut Ui) {
    let mut sel = persisted::<Option<usize>>(ui, "ss_open_sel", Some(2));
    let mut open = persisted_bool(ui, "ss_open_open", true);
    single_select(ui, &SingleSelectProps::new(LANG_OPTIONS), &mut sel, &mut open);
    write_value(ui, "ss_open_sel", sel);
    write_bool(ui, "ss_open_open", open);
}
fn single_select_disabled(ui: &mut Ui) {
    let mut sel = persisted::<Option<usize>>(ui, "ss_disabled_sel", Some(1));
    let mut open = persisted_bool(ui, "ss_disabled_open", false);
    single_select(ui, &SingleSelectProps::new(LANG_OPTIONS).disabled(), &mut sel, &mut open);
    write_value(ui, "ss_disabled_sel", sel);
    write_bool(ui, "ss_disabled_open", open);
}

pub const SINGLE_SELECT: Story = Story {
    widget: "single-select",
    display: "SingleSelect",
    upstream_url: "https://vscode-elements.github.io/components/single-select/",
    category: Category::Composite,
    states: &[
        StoryState {
            name: "closed",
            caption: "Trigger only, placeholder text",
            size: (260.0, 40.0),
            draw: single_select_closed,
        },
        StoryState {
            name: "with-value",
            caption: "Trigger shows the selected option label",
            size: (260.0, 40.0),
            draw: single_select_with_value,
        },
        StoryState {
            name: "open",
            caption: "Popup expanded — 22 px items, accent on selected",
            size: (260.0, 170.0),
            draw: single_select_open,
        },
        StoryState {
            name: "disabled",
            caption: "Trigger dimmed — clicks suppressed",
            size: (260.0, 40.0),
            draw: single_select_disabled,
        },
    ],
};

// ─── multi-select ────────────────────────────────────────────────────────

fn multi_select_closed(ui: &mut Ui) {
    let mut sel = persisted::<Vec<usize>>(ui, "ms_closed_sel", Vec::new());
    let mut open = persisted_bool(ui, "ms_closed_open", false);
    multi_select(ui, &MultiSelectProps::new(LANG_OPTIONS).placeholder("Pick languages…"), &mut sel, &mut open);
    write_value(ui, "ms_closed_sel", sel);
    write_bool(ui, "ms_closed_open", open);
}
fn multi_select_with_values(ui: &mut Ui) {
    let mut sel = persisted::<Vec<usize>>(ui, "ms_values_sel", vec![0, 2]);
    let mut open = persisted_bool(ui, "ms_values_open", false);
    multi_select(ui, &MultiSelectProps::new(LANG_OPTIONS), &mut sel, &mut open);
    write_value(ui, "ms_values_sel", sel);
    write_bool(ui, "ms_values_open", open);
}
fn multi_select_open(ui: &mut Ui) {
    let mut sel = persisted::<Vec<usize>>(ui, "ms_open_sel", vec![1, 3]);
    let mut open = persisted_bool(ui, "ms_open_open", true);
    multi_select(ui, &MultiSelectProps::new(LANG_OPTIONS), &mut sel, &mut open);
    write_value(ui, "ms_open_sel", sel);
    write_bool(ui, "ms_open_open", open);
}

pub const MULTI_SELECT: Story = Story {
    widget: "multi-select",
    display: "MultiSelect",
    upstream_url: "https://vscode-elements.github.io/components/multi-select/",
    category: Category::Composite,
    states: &[
        StoryState {
            name: "closed",
            caption: "Empty selection, placeholder shown",
            size: (300.0, 40.0),
            draw: multi_select_closed,
        },
        StoryState {
            name: "with-values",
            caption: "Count badge + summary of selected labels",
            size: (300.0, 40.0),
            draw: multi_select_with_values,
        },
        StoryState {
            name: "open",
            caption: "Popup with 14×14 checkboxes per option",
            size: (300.0, 180.0),
            draw: multi_select_open,
        },
    ],
};

// ─── table ───────────────────────────────────────────────────────────────

fn table_default(ui: &mut Ui) {
    let mut sel = persisted::<Option<usize>>(ui, "tb_default_sel", None);
    let widths = [120.0, 80.0, 100.0];
    table(
        ui,
        &TableProps::new(&["File", "Status", "Size"]).column_widths(&widths),
        &[
            &["main.rs", "Modified", "12 KB"],
            &["lib.rs", "Untracked", "4 KB"],
            &["Cargo.toml", "Clean", "1 KB"],
        ],
        &mut sel,
    );
    write_value(ui, "tb_default_sel", sel);
}
fn table_striped(ui: &mut Ui) {
    let mut sel = persisted::<Option<usize>>(ui, "tb_striped_sel", None);
    let widths = [120.0, 80.0, 100.0];
    table(
        ui,
        &TableProps::new(&["File", "Status", "Size"]).column_widths(&widths).striped(),
        &[
            &["main.rs", "Modified", "12 KB"],
            &["lib.rs", "Untracked", "4 KB"],
            &["Cargo.toml", "Clean", "1 KB"],
            &["README.md", "Modified", "2 KB"],
            &["LICENSE", "Clean", "1 KB"],
        ],
        &mut sel,
    );
    write_value(ui, "tb_striped_sel", sel);
}
fn table_borders(ui: &mut Ui) {
    let mut sel = persisted::<Option<usize>>(ui, "tb_borders_sel", Some(1));
    let widths = [120.0, 80.0, 100.0];
    table(
        ui,
        &TableProps::new(&["File", "Status", "Size"]).column_widths(&widths).border_columns(),
        &[
            &["main.rs", "Modified", "12 KB"],
            &["lib.rs", "Untracked", "4 KB"],
            &["Cargo.toml", "Clean", "1 KB"],
        ],
        &mut sel,
    );
    write_value(ui, "tb_borders_sel", sel);
}

pub const TABLE: Story = Story {
    widget: "table",
    display: "Table",
    upstream_url: "https://vscode-elements.github.io/components/table/",
    category: Category::Composite,
    states: &[
        StoryState {
            name: "default",
            caption: "3 columns × 3 rows, no decoration",
            size: (340.0, 130.0),
            draw: table_default,
        },
        StoryState {
            name: "striped",
            caption: "Alternating row background (4% white overlay)",
            size: (340.0, 170.0),
            draw: table_striped,
        },
        StoryState {
            name: "with-borders",
            caption: "Vertical column dividers + row selection",
            size: (340.0, 130.0),
            draw: table_borders,
        },
    ],
};

// ─── form-container ──────────────────────────────────────────────────────

fn form_container_horizontal(ui: &mut Ui) {
    form_container(ui, &FormContainerProps::new(), |ctx, ui| {
        for (idx, (label, value)) in [
            ("Project name", "vscode-rust"),
            ("Default theme", "Dark Modern 2026"),
            ("Auto-save", "After delay"),
        ]
        .iter()
        .enumerate()
        {
            ctx.separator(ui);
            form_group(ui, &FormGroupProps::new(label), |ui| {
                let key = match idx {
                    0 => "fc_h0",
                    1 => "fc_h1",
                    _ => "fc_h2",
                };
                let mut s = persisted_string(ui, key, value);
                crate::vscode_widgets::forms::textfield(
                    ui,
                    &crate::vscode_widgets::forms::TextFieldProps::new().width(180.0),
                    &mut s,
                );
                write_string(ui, key, s);
            });
        }
    });
}
fn form_container_vertical(ui: &mut Ui) {
    form_container(ui, &FormContainerProps::new().row_gap(10.0), |ctx, ui| {
        for (idx, (label, value)) in [
            ("Workspace name", "Phase 6 demo"),
            ("Description", "Form rows stacked vertically"),
        ]
        .iter()
        .enumerate()
        {
            ctx.separator(ui);
            form_group(ui, &FormGroupProps::new(label).vertical(), |ui| {
                let key = match idx {
                    0 => "fc_v0",
                    _ => "fc_v1",
                };
                let mut s = persisted_string(ui, key, value);
                crate::vscode_widgets::forms::textfield(
                    ui,
                    &crate::vscode_widgets::forms::TextFieldProps::new().width(220.0),
                    &mut s,
                );
                write_string(ui, key, s);
            });
        }
    });
}

pub const FORM_CONTAINER: Story = Story {
    widget: "form-container",
    display: "FormContainer",
    upstream_url: "https://vscode-elements.github.io/components/form-container/",
    category: Category::Forms,
    states: &[
        StoryState {
            name: "horizontal",
            caption: "Label + control on same row, 140 px label width",
            size: (380.0, 160.0),
            draw: form_container_horizontal,
        },
        StoryState {
            name: "vertical",
            caption: "Label above control, 10 px row gap",
            size: (380.0, 160.0),
            draw: form_container_vertical,
        },
    ],
};

// ─── form-helper ─────────────────────────────────────────────────────────

fn form_helper_description(ui: &mut Ui) {
    form_helper(
        ui,
        &FormHelperProps::new("This name will appear in the explorer header"),
    );
}
fn form_helper_error(ui: &mut Ui) {
    form_helper(
        ui,
        &FormHelperProps::new("The path must be absolute and exist on disk").error(),
    );
}

pub const FORM_HELPER: Story = Story {
    widget: "form-helper",
    display: "FormHelper",
    upstream_url: "https://vscode-elements.github.io/components/form-helper/",
    category: Category::Forms,
    states: &[
        StoryState {
            name: "description",
            caption: "Dim helper text under a form field",
            size: (340.0, 24.0),
            draw: form_helper_description,
        },
        StoryState {
            name: "error",
            caption: "Error variant — errorForeground colour",
            size: (340.0, 24.0),
            draw: form_helper_error,
        },
    ],
};
