//! Launcher use cases: activate, update the query, execute a candidate.
//!
//! Each use case is one file. They assemble core algorithms with first-party features
//! registered in [`crate::features`]. Recognition, matching, ranking and storage live
//! elsewhere; this layer owns session lifecycle and snapshot publication.
pub mod activate;
pub mod execute;
pub mod launcher_service;
mod legacy_plugins;
mod native_features;
pub mod settings_service;
pub mod update_query;

pub use launcher_service::{Application, LauncherApplication, SnapshotWait};

pub(crate) mod composition;
mod context_capture;
mod file_preview;
mod search_sources;
pub use crate::features::context_semantic;
pub use composition::builtin_registry;

mod session_model;
mod session_runtime;
mod snapshot;

mod app_actions;
mod shortcuts;
pub use shortcuts::{BindingResolution, ShortcutAction, ShortcutApplication, ShortcutRoute};

mod object_actions;

pub mod input_policy;

mod file_workspace;
mod source_runtime;

pub mod context_projection;

#[cfg(test)]
mod source_catalog_tests;

#[cfg(test)]
mod feature_policy_tests;
