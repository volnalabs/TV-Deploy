//! TV Deploy core library.
//!
//! Everything that touches user input, user files or child processes lives here so it
//! can be unit-tested without a UI. The Tauri layer is a thin adapter on top.

pub mod connectivity;
pub mod diagnostics;
pub mod package;
pub mod samsung;
pub mod validate;
