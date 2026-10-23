//! Prism Rush library: all game modules. `main.rs` only parses the command
//! line and assembles the app; keeping the logic in a library makes it easy
//! to test with `cargo test`.

// Bevy systems naturally take many parameters and complex query types
// (`Query<(&A, &mut B), (With<C>, Without<D>)>`): these two lints fight the
// framework's idioms, so they are disabled crate-wide (common in Bevy projects).
#![allow(clippy::type_complexity, clippy::too_many_arguments)]

pub mod bot;
pub mod config;
pub mod game;
pub mod net;
pub mod render;
pub mod shared;
pub mod ui;
