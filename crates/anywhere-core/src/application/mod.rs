//! Launcher use cases: activate, update the query, execute a candidate.
//!
//! Each use case is one file and one entry point. They coordinate the core concepts —
//! context resolution, search, ranking, execution — and own nothing themselves: no URL
//! parsing, no file detection, no matching, no sorting, no storage access.
pub mod activate;
pub mod execute;
pub mod launcher_service;
pub mod update_query;

pub use launcher_service::{Application, LauncherApplication};

pub mod composition;
mod session_model;
mod session_runtime;

mod app_actions;
