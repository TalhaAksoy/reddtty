//! ReddTTY library crate.
//!
//! The binary in `main.rs` is intentionally thin; everything testable lives
//! here so integration tests can exercise the same code as the app.

pub mod app;
pub mod cli;
pub mod config;
pub mod error;
pub mod event;
pub mod library;
pub mod media;
pub mod models;
pub mod persistence;
pub mod providers;
pub mod ui;
pub mod utils;
