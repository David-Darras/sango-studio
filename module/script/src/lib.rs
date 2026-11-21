//! The script editor.

mod amx;
#[cfg(test)]
mod api_tests;
mod assembly;
mod create;
mod create_model;
mod decompile;
mod files;
mod highlight;
mod natives;
mod opcodes;
mod page;

pub use page::ScriptPage;
