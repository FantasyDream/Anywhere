//! UI-neutral launcher engine.
//!
//! The tree mirrors the model the launcher is built on rather than the order features were
//! added:
//!
//! ```text
//! context  → context provider → context set
//!          → action matching → availability → candidates
//!          → query matching  → ranking      → scopes → presentation
//! ```
//!
//! - [`core`] holds the framework concepts and knows nothing about a feature, a UI or a
//!   platform API.
//! - [`application`] orchestrates one launcher invocation from those concepts, one file per
//!   use case. Product assembly lives in [`application::builtin_registry`].
//! - [`features`] owns launcher, filesystem, application and text capabilities.
//! - Domain modules own recognizers, actions and sources;
//!   [`features::contribution`] assembles contributions and [`features::session_seeds`]
//!   seeds each session.
//! - [`persistence`] stores data behind repository interfaces.
pub mod application;
pub mod core;
pub mod features;
pub mod persistence;

pub use application::{Application, LauncherApplication};
