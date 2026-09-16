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
//!   use case.
//! - [`features`] holds first-party product capabilities; each owns its contexts, its actions
//!   and its candidate sources, and registers them through [`features::builtin_providers`]
//!   and [`features::builtin_action_bundles`].
//! - [`persistence`] stores data behind repository interfaces.
pub mod application;
pub mod core;
pub mod features;
pub mod persistence;

pub use application::{Application, LauncherApplication};

pub fn ping() -> &'static str {
    "pong"
}

#[cfg(test)]
mod tests {
    #[test]
    fn core_runs_without_gui() {
        assert_eq!(super::ping(), "pong");
    }
}
