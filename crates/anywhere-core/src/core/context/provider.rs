//! The context provider contract.
//!
//! A provider turns one context into further contexts — `core.text` into `core.url`, a
//! directory into a `git.repository`. It never knows about budgets, timeouts or the user
//! interface: the caller decides how long to wait and what to do with the result.
use crate::core::context::{ExecutionContext, ResolvedContext};
use anyhow::Result;
use anywhere_protocol::{ContextTypeId, ProviderId};
use async_trait::async_trait;

/// The performance metrics V0.2 must record.
///
/// The spec names seven metrics; this module lists the exact name each one appears under in
/// the log, so the contract is stated once and a test asserts every one of them is emitted.
/// Most are tracing *fields*; `context_resolution_total` is a log *message* name, because it
/// marks a completion rather than carrying a duration.
///
/// `first_snapshot_duration` is the `first_snapshot_duration_us` field of the first
/// `snapshot_committed` line of a revision (`snapshot_seq == 1`).
///
/// The emission sites spell these names inline: `tracing` requires literal field names, so a
/// site cannot reference a const. The test in `tests/mvp.rs` is what keeps the two ends
/// honest.
pub mod metrics {
    pub const PROVIDER_DURATION: &str = "provider_duration";
    pub const CONTEXT_RESOLUTION_TOTAL: &str = "context_resolution_total";
    pub const FIRST_SNAPSHOT_DURATION: &str = "first_snapshot_duration";
    pub const MATCHER_DURATION: &str = "matcher_duration";
    pub const RANKING_DURATION: &str = "ranking_duration";
    pub const HOTKEY_TO_VISIBLE: &str = "hotkey_to_visible_ms";
    pub const LATE_CONTEXT_COUNT: &str = "late_context_count";

    pub const ALL: [&str; 7] = [
        PROVIDER_DURATION,
        CONTEXT_RESOLUTION_TOTAL,
        FIRST_SNAPSHOT_DURATION,
        MATCHER_DURATION,
        RANKING_DURATION,
        HOTKEY_TO_VISIBLE,
        LATE_CONTEXT_COUNT,
    ];
}

/// What a provider declares about itself.
#[derive(Debug, Clone)]
pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub accepts: Vec<ContextTypeId>,
    pub produces: Vec<ContextTypeId>,
    pub priority: i32,
}

#[async_trait]
pub trait ContextProvider: Send + Sync {
    fn descriptor(&self) -> &ProviderDescriptor;
    /// Resolve `input` into further contexts.
    ///
    /// Called at most once per `(provider, input, session, revision)`: a caller that stops
    /// waiting keeps the started call and resumes it, so a provider never has to defend
    /// against being re-entered for work it has already begun.
    async fn resolve(
        &self,
        input: &ResolvedContext,
        execution: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>>;
}

pub enum StepOutcome {
    Completed {
        result: Result<Vec<ResolvedContext>>,
        duration_us: u64,
    },
    /// The caller abandoned this run before the provider was polled.
    Abandoned,
}
