// Ratatui render functions take the view state they draw as explicit arguments,
// and scroll windows are clearer as index ranges than as iterator chains.
#![allow(clippy::too_many_arguments, clippy::needless_range_loop)]

pub mod cli;
pub mod config;
pub mod core;
pub mod git;
pub mod integration;
pub mod ui;
pub mod watcher;
pub mod update;
