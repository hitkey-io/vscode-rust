pub mod activity_bar;
pub mod breadcrumbs;
pub mod cards;
pub mod command_palette;
pub mod extension_details;
pub mod extensions_view;
pub mod language_ui;
pub mod notifications;
pub mod sidebar;
pub mod source_control;
pub mod status_bar;
pub mod tabs;
pub mod webviews;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActivityView {
    Explorer,
    Search,
    SourceControl,
    Extensions,
    /// A sidebar container contributed by an extension (`App` keeps
    /// which one).
    ExtensionContainer,
}
