//! The look of Sango Studio: a white theme, a few widgets, and the trait of a module.

pub mod module;
pub mod theme;
pub mod widget;

pub use eframe::egui;
pub use module::Module;
pub use sango_core::Session;
