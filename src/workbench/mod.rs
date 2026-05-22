pub mod activity_bar;
pub mod breadcrumbs;
pub mod command_palette;
pub mod sidebar;
pub mod source_control;
pub mod status_bar;
pub mod tabs;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActivityView {
    Explorer,
    Search,
    SourceControl,
}
