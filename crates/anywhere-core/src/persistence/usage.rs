//! SQLite-backed usage storage.
//!
//! The only module in the crate that knows about `rusqlite`. Everything above it depends on
//! the `UsageRepository` interface.
pub use crate::core::ranking::usage::UsageRepository;
use crate::core::ranking::usage::{UsageKey, UsageSnapshot, UsageStats};
use anyhow::Result;

pub struct SqliteUsageRepository(rusqlite::Connection);
impl SqliteUsageRepository {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let connection = rusqlite::Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(2))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS command_usage (
                command_id TEXT NOT NULL,
                context_type TEXT NOT NULL,
                source_app TEXT NOT NULL DEFAULT '',
                execute_count INTEGER NOT NULL DEFAULT 0,
                last_executed_at INTEGER NOT NULL,
                PRIMARY KEY (command_id, context_type, source_app)
            );
            CREATE INDEX IF NOT EXISTS command_usage_source_app ON command_usage(source_app);",
        )?;
        let has_legacy_json: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM command_usage WHERE context_type='core.json')",
            [],
            |row| row.get(0),
        )?;
        if has_legacy_json {
            connection.execute_batch("BEGIN IMMEDIATE;
                INSERT INTO command_usage(command_id,context_type,source_app,execute_count,last_executed_at)
                SELECT command_id,'json.document',source_app,execute_count,last_executed_at FROM command_usage WHERE context_type='core.json'
                ON CONFLICT(command_id,context_type,source_app) DO UPDATE SET execute_count=MIN(command_usage.execute_count+excluded.execute_count,4294967295),last_executed_at=MAX(command_usage.last_executed_at,excluded.last_executed_at);
                DELETE FROM command_usage WHERE context_type='core.json'; COMMIT;")?;
        }
        Ok(Self(connection))
    }
}
impl UsageRepository for SqliteUsageRepository {
    fn load(&self, source_app: &str) -> anyhow::Result<UsageSnapshot> {
        let mut query = self.0.prepare_cached("SELECT command_id, context_type, execute_count, last_executed_at FROM command_usage WHERE source_app = ?1")?;
        let rows = query.query_map([source_app], |row| {
            let count: i64 = row.get(2)?;
            Ok((
                UsageKey(row.get(0)?, row.get(1)?),
                UsageStats {
                    execute_count: count.clamp(0, u32::MAX as i64) as u32,
                    last_executed_at: row.get(3)?,
                },
            ))
        })?;
        let snapshot: UsageSnapshot = rows.collect::<rusqlite::Result<_>>()?;
        Ok(snapshot)
    }
    fn load_global(&self) -> anyhow::Result<UsageSnapshot> {
        let mut query = self.0.prepare_cached(
            "SELECT command_id, context_type, MIN(SUM(execute_count),4294967295), MAX(last_executed_at) FROM command_usage GROUP BY command_id,context_type",
        )?;
        let rows = query.query_map([], |row| {
            let count: i64 = row.get(2)?;
            Ok((
                UsageKey(row.get::<_, String>(0)?, row.get::<_, String>(1)?),
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
    ) -> anyhow::Result<()> {
        self.0
            .prepare_cached(
                "INSERT INTO command_usage (command_id, context_type, source_app, execute_count, last_executed_at)
                VALUES (?1, ?2, ?3, 1, ?4)
                ON CONFLICT(command_id, context_type, source_app) DO UPDATE SET
                    execute_count = MIN(execute_count + 1, 4294967295), last_executed_at = excluded.last_executed_at",
            )?
            .execute(rusqlite::params![command_id, context_type, source_app, now])?;
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
        let mut history = SqliteUsageRepository::open(&path).unwrap();
        for time in 10..13 {
            history.record("copy", "core.url", "", time).unwrap();
        }
        history
            .record("copy", "core.url", "com.apple.finder", 15)
            .unwrap();
        drop(history);
        let history = SqliteUsageRepository::open(&path).unwrap();
        let usage = history.load("").unwrap();
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[&UsageKey::new("copy", "core.url")].execute_count, 3);
        assert_eq!(
            usage[&UsageKey::new("copy", "core.url")].last_executed_at,
            12
        );
        assert_eq!(history.load("com.apple.finder").unwrap().len(), 1);
    }

    #[test]
    fn global_usage_aggregates_across_apps_and_stays_separate_from_app_scoped_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("global.db");
        let mut history = SqliteUsageRepository::open(&path).unwrap();
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
            global[&UsageKey::new("app:com.google.Chrome", "application")].execute_count,
            2
        );
        assert_eq!(
            global[&UsageKey::new("core.url.copy", "core.url")].execute_count,
            2
        );
        assert_eq!(
            global[&UsageKey::new("core.url.copy", "core.url")].last_executed_at,
            40
        );
        // The app-scoped load must not leak another application's rows.
        let finder = history.load("com.apple.finder").unwrap();
        assert_eq!(
            finder[&UsageKey::new("core.url.copy", "core.url")].execute_count,
            1
        );
        assert!(!finder.contains_key(&UsageKey::new("app:com.google.Chrome", "application")));
    }

    #[test]
    fn memory_history_aggregates_the_same_way_as_sqlite() {
        let mut history = crate::core::ranking::usage::MemoryUsage::default();
        history
            .record("app:com.example", "application", "", 5)
            .unwrap();
        history
            .record("app:com.example", "application", "", 9)
            .unwrap();
        let global = history.load_global().unwrap();
        let stats = global[&UsageKey::new("app:com.example", "application")];
        assert_eq!(stats.execute_count, 2);
        assert_eq!(stats.last_executed_at, 9);
    }
    #[test]
    fn sqlite_json_alias_migration_merges_each_app_once_across_reopens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage.db");
        {
            let history = SqliteUsageRepository::open(&path).unwrap();
            history.0.execute_batch("INSERT INTO command_usage VALUES ('copy','core.json','app',3,100),('copy','json.document','app',4,250),('copy','core.json','other',2,50),('saturated','core.json','app',4294967295,1),('saturated','json.document','app',4294967295,2);").unwrap();
        }
        for _ in 0..2 {
            let history = SqliteUsageRepository::open(&path).unwrap();
            let app = history.load("app").unwrap();
            let value = app.get(&UsageKey::new("copy", "json.document")).unwrap();
            assert_eq!(value.execute_count, 7);
            assert_eq!(
                app[&UsageKey::new("saturated", "json.document")].execute_count,
                u32::MAX
            );
            assert_eq!(value.last_executed_at, 250);
            assert!(!app.contains_key(&UsageKey::new("copy", "core.json")));
            assert_eq!(
                history.load_global().unwrap()[&UsageKey::new("copy", "json.document")]
                    .execute_count,
                9
            );
        }
    }
}
