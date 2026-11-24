//! Usage history: the ranking domain's view of what the user has done before.
//!
//! The domain depends on this interface, never on SQLite. `UsageStats` is the only shape
//! ranking sees; how the numbers are stored is a persistence concern.
use std::{
    borrow::Borrow,
    collections::HashMap,
    hash::{Hash, Hasher},
};

/// What the ranking engine reads about past use.
#[derive(Debug, Default, Clone, Copy)]
pub struct UsageStats {
    pub execute_count: u32,
    pub last_executed_at: i64,
}

/// Key of an action usage row: `(action_id, canonical context_type)`.
///
/// Supports zero-allocation lookups from borrowed `(&str, &str)` pairs through
/// [`UsageKeyRef`] and [`lookup_usage`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageKey(pub String, pub String);

impl UsageKey {
    pub fn new(action_id: impl Into<String>, context_type: impl Into<String>) -> Self {
        Self(action_id.into(), context_type.into())
    }
}

impl From<(String, String)> for UsageKey {
    fn from((action_id, context_type): (String, String)) -> Self {
        Self(action_id, context_type)
    }
}

pub trait UsageKeyRef {
    fn as_pair(&self) -> (&str, &str);
}

impl UsageKeyRef for UsageKey {
    fn as_pair(&self) -> (&str, &str) {
        (&self.0, &self.1)
    }
}

impl UsageKeyRef for (&str, &str) {
    fn as_pair(&self) -> (&str, &str) {
        (self.0, self.1)
    }
}

impl<'a> Borrow<dyn UsageKeyRef + 'a> for UsageKey {
    fn borrow(&self) -> &(dyn UsageKeyRef + 'a) {
        self
    }
}

impl PartialEq for dyn UsageKeyRef + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.as_pair() == other.as_pair()
    }
}

impl Eq for dyn UsageKeyRef + '_ {}

impl Hash for dyn UsageKeyRef + '_ {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_pair().hash(state);
    }
}

impl Hash for UsageKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_pair().hash(state);
    }
}

pub type UsageSnapshot = HashMap<UsageKey, UsageStats>;

/// Zero-allocation lookup by `(action_id, context_type)`.
pub fn lookup_usage(snapshot: &UsageSnapshot, action_id: &str, context_type: &str) -> UsageStats {
    let key = (action_id, context_type);
    snapshot
        .get(&key as &dyn UsageKeyRef)
        .copied()
        .unwrap_or_default()
}

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
        Ok(self
            .0
            .iter()
            .filter(|((_, _, app), _)| app == source_app)
            .map(|((action, kind, _), stats)| (UsageKey(action.clone(), kind.clone()), *stats))
            .collect())
    }
    fn load_global(&self) -> anyhow::Result<UsageSnapshot> {
        let mut snapshot = UsageSnapshot::new();
        for ((action, kind, _), stats) in &self.0 {
            merge_usage(
                &mut snapshot,
                UsageKey(action.clone(), kind.clone()),
                *stats,
            );
        }
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
