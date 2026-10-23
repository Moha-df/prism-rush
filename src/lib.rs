//! Prism Rush library: all game modules. `main.rs` only parses the command
//! line and assembles the app; keeping the logic in a library makes it easy
//! to test with `cargo test`.

pub mod bot;
pub mod config;
pub mod game;
pub mod net;
pub mod render;
pub mod shared;
pub mod ui;
