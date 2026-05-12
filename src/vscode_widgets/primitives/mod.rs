//! Primitive widgets — building blocks for everything else.
//!
//! Populated in Phases 1–2:
//! - Phase 1: icon, divider, label
//! - Phase 2: button, icon_button, badge, progress_ring

pub mod badge;
pub mod button;
pub mod divider;
pub mod icon;
pub mod icon_button;
pub mod label;
pub mod progress_ring;

pub use badge::{badge, BadgeProps, BadgeVariant};
pub use button::{button, ButtonProps, ButtonSize, ButtonVariant};
pub use divider::{divider, DividerOrientation, DividerProps};
pub use icon::{icon, IconProps};
pub use icon_button::{icon_button, IconButtonProps};
pub use label::{label, LabelProps};
pub use progress_ring::{progress_ring, ProgressRingProps};
