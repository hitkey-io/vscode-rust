//! `vscode_widgets` — egui port of the official [vscode-elements] Web Component
//! library, adapted to immediate-mode rendering.
//!
//! Each submodule mirrors one component from the upstream library, with a
//! header doc-comment pointing to the canonical source URL, the docs page, the
//! analogous code in the real VS Code source tree, and the CSS variables it
//! consumes (mapped to entries in [`crate::theme::Palette`]).
//!
//! [vscode-elements]: https://vscode-elements.github.io/
//!
//! ## API convention
//!
//! Every widget exposes:
//! - A `*Props` struct (visual configuration, passed by reference)
//! - An optional `*State` struct (caller-owned, `Default::default()` for fresh use)
//! - A `pub fn show(ui, props, state) -> *Response` function
//!
//! `*Response` is `egui::Response` for trivial widgets, or `{ inner: Response,
//! /* domain events */ }` for stateful composites (Tabs, Tree, ContextMenu).
//! Every widget calls `response.widget_info(...)` so AccessKit / kittest
//! queries continue to work.
//!
//! ## Storybook and parity harness
//!
//! The single source of truth for both consumers is `testing::catalogue::STORIES`.
//! - Interactive storybook: `cargo run --example widget_showcase`
//! - Visual-parity tests: `cargo test --test widget_parity`

pub mod composite;
pub mod forms;
pub mod layout;
pub mod primitives;
pub mod testing;
pub mod tokens;

pub mod prelude {
    //! Re-exports for ergonomic `use vscode_rust::vscode_widgets::prelude::*;`.
    //!
    //! Populated phase-by-phase as widgets land.
}
