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
//! - [`application`] orchestrates one launcher invocation from those concepts.
//! - [`features`] holds first-party product capabilities; each owns its providers, its
//!   actions and its candidate sources.
//! - [`persistence`] stores data behind repository interfaces.
//!
//! The old flat paths are still exported below so the structure migration can land phase by
//! phase without a single unreviewable rewrite. They are removed once every caller has
//! moved; see the V2.0 structure plan.
pub mod application;
pub mod core;
pub mod features;
pub mod persistence;

// ---- Paths kept while the structure migration lands ----
pub use crate::application::launcher_service;
pub use crate::core::action::executor as execution;
pub use crate::core::action::registry as command;
pub use crate::core::context::context as context;
pub use crate::core::context::provider as provider;
pub use crate::core::context::resolvers as input;
pub use crate::core::ranking::score as ranking;
pub use crate::core::search::candidate as candidates;
pub use crate::core::search::query as query;
pub use crate::core::session::manager as resolver;
pub use crate::features::applications::index as applications;
pub use crate::persistence::usage as history;

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
