use crate::ranking::UsageStats;
use anyhow::Result;
use std::collections::HashMap;

pub type UsageSnapshot = HashMap<(String, String), UsageStats>;
pub trait UsageHistory: Send {
    fn load(&self, source_app: &str) -> Result<UsageSnapshot>;
    fn record(
        &mut self,
        command_id: &str,
        context_type: &str,
        source_app: &str,
        now: i64,
    ) -> Result<()>;
}

#[derive(Default)]
pub struct MemoryHistory(pub HashMap<(String, String, String), UsageStats>);
impl UsageHistory for MemoryHistory {
    fn load(&self, source_app: &str) -> Result<UsageSnapshot> {
        Ok(self
            .0
            .iter()
            .filter(|((_, _, app), _)| app == source_app)
            .map(|((command, kind, _), stats)| ((command.clone(), kind.clone()), *stats))
            .collect())
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
        Ok(rows.collect::<rusqlite::Result<_>>()?)
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
}
