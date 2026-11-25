//! The look of Sango Studio: a white theme, a few widgets, and the trait of a module.

pub mod focus;
pub mod game_text;
pub mod item_menu;
pub mod language;
pub mod module;
pub mod picker;
pub mod retired;
pub mod textures;
pub mod theme;
pub mod widget;

pub use eframe::egui;
pub use module::Module;
pub use sango_core::Session;
