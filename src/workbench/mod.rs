pub mod activity_bar;
pub mod breadcrumbs;
pub mod cards;
pub mod command_palette;
pub mod extension_details;
pub mod extensions_view;
pub mod language_ui;
pub mod modal;
pub mod notifications;
pub mod settings_editor;
pub mod sidebar;
pub mod source_control;
pub mod status_bar;
pub mod tabs;
pub mod webviews;
pub mod welcome;
pub mod banner;
pub mod dialog;
pub mod panel;
pub mod run_debug;
pub mod workspace_trust_editor;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActivityView {
    Explorer,
    Search,
    SourceControl,
    /// Run and Debug (⇧⌘D).
    RunAndDebug,
    Extensions,
    /// A sidebar container contributed by an extension (`App` keeps
    /// which one).
    ExtensionContainer,
}
