use crate::ranking::UsageStats;
use anyhow::Result;
use anywhere_protocol::{JSON_DOCUMENT, JSON_LEGACY};
use std::collections::HashMap;

/// Key of an action usage row: `(command_id, canonical context_type)`.
pub type UsageKey = (String, String);
pub type UsageSnapshot = HashMap<UsageKey, UsageStats>;

/// Context type recorded for application-launch usage rows.
pub const APPLICATION_CONTEXT_TYPE: &str = "application";

/// Command id used for an application-launch usage row.
///
/// Both the recording and the ranking path must use this, from one place.
pub fn application_usage_id(bundle_id: &str) -> String {
    format!("app:{bundle_id}")
}

pub trait UsageHistory: Send {
    fn load(&self, source_app: &str) -> Result<UsageSnapshot>;
    /// Usage aggregated across every source application, for application candidates.
    fn load_global(&self) -> Result<UsageSnapshot>;
    fn record(
        &mut self,
        command_id: &str,
        context_type: &str,
        source_app: &str,
        now: i64,
    ) -> Result<()>;
}

/// Merge legacy `core.json` rows into the canonical `json.document` key.
///
/// Counts saturate and timestamps take the maximum, so repeated loads are
/// idempotent and no existing data has to be rewritten.
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

#[derive(Default)]
pub struct MemoryHistory(pub HashMap<(String, String, String), UsageStats>);
impl UsageHistory for MemoryHistory {
    fn load(&self, source_app: &str) -> Result<UsageSnapshot> {
        let mut snapshot = self
            .0
            .iter()
            .filter(|((_, _, app), _)| app == source_app)
            .map(|((command, kind, _), stats)| ((command.clone(), kind.clone()), *stats))
            .collect::<UsageSnapshot>();
        merge_json_aliases(&mut snapshot);
        Ok(snapshot)
    }
    fn load_global(&self) -> Result<UsageSnapshot> {
        let mut snapshot = UsageSnapshot::new();
        for ((command, kind, _), stats) in &self.0 {
            merge_usage(&mut snapshot, (command.clone(), kind.clone()), *stats);
        }
        merge_json_aliases(&mut snapshot);
        Ok(snapshot)
    }
    fn record(
        &mut self,
        command_id: &str,
        context_type: &str,
        source_app: &str,
        now: i64,
    ) -> Result<()> {
        let stats = self
            .0
            .entry((command_id.into(), context_type.into(), source_app.into()))
            .or_default();
        stats.execute_count = stats.execute_count.saturating_add(1);
        stats.last_executed_at = now;
        Ok(())
    }
}

pub struct SqliteHistory(rusqlite::Connection);
impl SqliteHistory {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let connection = rusqlite::Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(2))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS command_usage (
                command_id TEXT NOT NULL,
                context_type TEXT NOT NULL,
                source_app TEXT NOT NULL DEFAULT '',
                execute_count INTEGER NOT NULL DEFAULT 0,
                last_executed_at INTEGER NOT NULL,
                PRIMARY KEY (command_id, context_type, source_app)
            );",
        )?;
        Ok(Self(connection))
    }
}
impl UsageHistory for SqliteHistory {
    fn load(&self, source_app: &str) -> Result<UsageSnapshot> {
        let mut query = self.0.prepare("SELECT command_id, context_type, execute_count, last_executed_at FROM command_usage WHERE source_app = ?1")?;
        let rows = query.query_map([source_app], |row| {
            let count: i64 = row.get(2)?;
            Ok((
                (row.get(0)?, row.get(1)?),
                UsageStats {
                    execute_count: count.clamp(0, u32::MAX as i64) as u32,
                    last_executed_at: row.get(3)?,
                },
            ))
        })?;
        let mut snapshot: UsageSnapshot = rows.collect::<rusqlite::Result<_>>()?;
        merge_json_aliases(&mut snapshot);
        Ok(snapshot)
    }
    fn load_global(&self) -> Result<UsageSnapshot> {
        let mut query = self.0.prepare(
            "SELECT command_id, context_type, execute_count, last_executed_at FROM command_usage",
        )?;
        let rows = query.query_map([], |row| {
            let count: i64 = row.get(2)?;
            Ok((
                (row.get::<_, String>(0)?, row.get::<_, String>(1)?),
                UsageStats {
                    execute_count: count.clamp(0, u32::MAX as i64) as u32,
                    last_executed_at: row.get(3)?,
                },
            ))
        })?;
        let mut snapshot = UsageSnapshot::new();
        for row in rows {
            let (key, stats) = row?;
            merge_usage(&mut snapshot, key, stats);
        }
        merge_json_aliases(&mut snapshot);
        Ok(snapshot)
    }
    fn record(
        &mut self,
        command_id: &str,
        context_type: &str,
        source_app: &str,
        now: i64,
    ) -> Result<()> {
        self.0.execute("INSERT INTO command_usage (command_id, context_type, source_app, execute_count, last_executed_at)
            VALUES (?1, ?2, ?3, 1, ?4)
            ON CONFLICT(command_id, context_type, source_app) DO UPDATE SET
                execute_count = MIN(execute_count + 1, 4294967295), last_executed_at = excluded.last_executed_at",
            rusqlite::params![command_id, context_type, source_app, now])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_row(count: u32, at: i64) -> UsageStats {
        UsageStats {
            execute_count: count,
            last_executed_at: at,
        }
    }

    #[test]
    fn sqlite_upserts_missing_app_and_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        let mut history = SqliteHistory::open(&path).unwrap();
        for time in 10..13 {
            history.record("copy", "core.url", "", time).unwrap();
        }
        history
            .record("copy", "core.url", "com.apple.finder", 15)
            .unwrap();
        drop(history);
        let history = SqliteHistory::open(&path).unwrap();
        let usage = history.load("").unwrap();
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[&("copy".into(), "core.url".into())].execute_count, 3);
        assert_eq!(
            usage[&("copy".into(), "core.url".into())].last_executed_at,
            12
        );
        assert_eq!(history.load("com.apple.finder").unwrap().len(), 1);
    }

    #[test]
    fn legacy_json_rows_merge_once_with_saturating_counts() {
        let mut snapshot = UsageSnapshot::new();
        snapshot.insert(
            ("core.json.copy".into(), JSON_LEGACY.into()),
            legacy_row(3, 100),
        );
        snapshot.insert(
            ("core.json.copy".into(), JSON_DOCUMENT.into()),
            legacy_row(4, 250),
        );
        merge_json_aliases(&mut snapshot);
        assert_eq!(snapshot.len(), 1);
        let merged = &snapshot[&("core.json.copy".into(), JSON_DOCUMENT.into())];
        assert_eq!(merged.execute_count, 7);
        assert_eq!(merged.last_executed_at, 250);
        // Merging again is idempotent: the legacy key is gone.
        merge_json_aliases(&mut snapshot);
        assert_eq!(
            snapshot[&("core.json.copy".into(), JSON_DOCUMENT.into())].execute_count,
            7
        );

        // Saturation never wraps.
        let mut saturated = UsageSnapshot::new();
        saturated.insert(("c".into(), JSON_LEGACY.into()), legacy_row(u32::MAX, 1));
        saturated.insert(("c".into(), JSON_DOCUMENT.into()), legacy_row(u32::MAX, 2));
        merge_json_aliases(&mut saturated);
        assert_eq!(
            saturated[&("c".into(), JSON_DOCUMENT.into())].execute_count,
            u32::MAX
        );
    }

    #[test]
    fn global_usage_aggregates_across_apps_and_stays_separate_from_app_scoped_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("global.db");
        let mut history = SqliteHistory::open(&path).unwrap();
        history
            .record("app:com.google.Chrome", "application", "", 10)
            .unwrap();
        history
            .record("app:com.google.Chrome", "application", "", 20)
            .unwrap();
        history
            .record("core.url.copy", "core.url", "com.apple.finder", 30)
            .unwrap();
        history
            .record("core.url.copy", "core.url", "test.editor", 40)
            .unwrap();

        let global = history.load_global().unwrap();
        assert_eq!(
            global[&("app:com.google.Chrome".into(), "application".into())].execute_count,
            2
        );
        assert_eq!(
            global[&("core.url.copy".into(), "core.url".into())].execute_count,
            2
        );
        assert_eq!(
            global[&("core.url.copy".into(), "core.url".into())].last_executed_at,
            40
        );
        // The app-scoped load must not leak another application's rows.
        let finder = history.load("com.apple.finder").unwrap();
        assert_eq!(
            finder[&("core.url.copy".into(), "core.url".into())].execute_count,
            1
        );
        assert!(!finder.contains_key(&("app:com.google.Chrome".into(), "application".into())));
    }

    #[test]
    fn memory_history_aggregates_the_same_way_as_sqlite() {
        let mut history = MemoryHistory::default();
        history
            .record("app:com.example", "application", "", 5)
            .unwrap();
        history
            .record("app:com.example", "application", "", 9)
            .unwrap();
        let global = history.load_global().unwrap();
        let stats = global[&("app:com.example".into(), "application".into())];
        assert_eq!(stats.execute_count, 2);
        assert_eq!(stats.last_executed_at, 9);
    }
}
