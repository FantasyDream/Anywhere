//! Usage history: the ranking domain's view of what the user has done before.
//!
//! The domain depends on this interface, never on SQLite. `UsageStats` is the only shape
//! ranking sees; how the numbers are stored is a persistence concern.
use anywhere_protocol::{JSON_DOCUMENT, JSON_LEGACY};
use std::collections::HashMap;

/// What the ranking engine reads about past use.
#[derive(Debug, Default, Clone, Copy)]
pub struct UsageStats {
    pub execute_count: u32,
    pub last_executed_at: i64,
}

/// Key of an action usage row: `(action_id, canonical context_type)`.
pub type UsageKey = (String, String);
pub type UsageSnapshot = HashMap<UsageKey, UsageStats>;

/// Context type recorded for application-launch usage rows.
pub const APPLICATION_CONTEXT_TYPE: &str = "application";

/// Action id used for an application-launch usage row.
///
/// Both the recording and the ranking path must use this, from one place.
pub fn application_usage_id(bundle_id: &str) -> String {
    format!("app:{bundle_id}")
}

/// Read and record usage. The ranking engine only reads.
pub trait UsageRepository: Send {
    fn load(&self, source_app: &str) -> anyhow::Result<UsageSnapshot>;
    /// Usage aggregated across every source application, for application candidates.
    fn load_global(&self) -> anyhow::Result<UsageSnapshot>;
    fn record(
        &mut self,
        action_id: &str,
        context_type: &str,
        source_app: &str,
        now: i64,
    ) -> anyhow::Result<()>;
}

/// Merge legacy `core.json` rows into the canonical `json.document` key.
///
/// Counts saturate and timestamps take the maximum, so repeated loads are idempotent and no
/// existing data has to be rewritten.
pub fn merge_json_aliases(snapshot: &mut UsageSnapshot) {
    let legacy_prefix = JSON_LEGACY;
    let legacy = snapshot
        .iter()
        .filter(|((_, kind), _)| kind == legacy_prefix)
        .map(|((command, _), stats)| (command.clone(), *stats))
        .collect::<Vec<_>>();
    if legacy.is_empty() {
        return;
    }
    for (command, stats) in legacy {
        merge_usage(snapshot, (command.clone(), JSON_DOCUMENT.to_owned()), stats);
        snapshot.remove(&(command, JSON_LEGACY.to_owned()));
    }
}

/// Fold one usage row into a snapshot: counts saturate, timestamps keep the latest.
pub fn merge_usage(snapshot: &mut UsageSnapshot, key: UsageKey, stats: UsageStats) {
    let entry = snapshot.entry(key).or_default();
    entry.execute_count = entry.execute_count.saturating_add(stats.execute_count);
    entry.last_executed_at = entry.last_executed_at.max(stats.last_executed_at);
}

/// In-memory repository, for tests and for a run without persistent storage.
#[derive(Default)]
pub struct MemoryUsage(pub HashMap<(String, String, String), UsageStats>);
impl UsageRepository for MemoryUsage {
    fn load(&self, source_app: &str) -> anyhow::Result<UsageSnapshot> {
        let mut snapshot = self
            .0
            .iter()
            .filter(|((_, _, app), _)| app == source_app)
            .map(|((command, kind, _), stats)| ((command.clone(), kind.clone()), *stats))
            .collect::<UsageSnapshot>();
        merge_json_aliases(&mut snapshot);
        Ok(snapshot)
    }
    fn load_global(&self) -> anyhow::Result<UsageSnapshot> {
        let mut snapshot = UsageSnapshot::new();
        for ((command, kind, _), stats) in &self.0 {
            merge_usage(&mut snapshot, (command.clone(), kind.clone()), *stats);
        }
        merge_json_aliases(&mut snapshot);
        Ok(snapshot)
    }
    fn record(
        &mut self,
        action_id: &str,
        context_type: &str,
        source_app: &str,
        now: i64,
    ) -> anyhow::Result<()> {
        let stats = self
            .0
            .entry((action_id.into(), context_type.into(), source_app.into()))
            .or_default();
        stats.execute_count = stats.execute_count.saturating_add(1);
        stats.last_executed_at = now;
        Ok(())
    }
}
