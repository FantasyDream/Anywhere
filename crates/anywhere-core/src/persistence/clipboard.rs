//! Bounded SQLite history. Lists contain metadata; representations are BLOBs.
use crate::features::clipboard::policy::HistoryPolicy;
use crate::features::clipboard::{content, history::*};
use anyhow::{ensure, Result};
use anywhere_platform::clipboard::{ClipboardFormat, ClipboardPayload};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::path::Path;

pub struct SqliteHistoryRepository(Connection);
impl SqliteHistoryRepository {
    pub fn open(path: &Path) -> Result<Self> {
        anywhere_platform::private_storage::prepare_file(path)?;
        Self::connection(Connection::open(path)?)
    }
    fn connection(mut connection: Connection) -> Result<Self> {
        connection.busy_timeout(std::time::Duration::from_secs(2))?;
        let version: u32 = connection.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        ensure!(version <= 3, "unsupported clipboard database version");
        connection.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA secure_delete=ON; PRAGMA cache_size=-512; PRAGMA temp_store=FILE; PRAGMA foreign_keys=ON;")?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if version == 1 {
            tx.execute_batch("ALTER TABLE clipboard_items RENAME TO clipboard_text_v1; DROP INDEX clipboard_recent;")?;
        }
        tx.execute_batch("CREATE TABLE IF NOT EXISTS clipboard_settings(id INTEGER PRIMARY KEY CHECK(id=1), recording INTEGER NOT NULL CHECK(recording IN(0,1)));
            INSERT OR IGNORE INTO clipboard_settings VALUES(1,0);
            CREATE TABLE IF NOT EXISTS clipboard_items(
                id INTEGER PRIMARY KEY AUTOINCREMENT, fingerprint BLOB NOT NULL UNIQUE,
                search_text TEXT NOT NULL, summary TEXT NOT NULL, kind TEXT NOT NULL,
                copied_at_ms INTEGER NOT NULL, sequence INTEGER NOT NULL, byte_count INTEGER NOT NULL, stored_bytes INTEGER NOT NULL);
            CREATE INDEX IF NOT EXISTS clipboard_recent ON clipboard_items(sequence DESC);
            CREATE TABLE IF NOT EXISTS clipboard_formats(item_id INTEGER NOT NULL REFERENCES clipboard_items(id) ON DELETE CASCADE,
                name TEXT NOT NULL, data BLOB NOT NULL, PRIMARY KEY(item_id,name));")?;
        if version == 1 {
            // Preserve identity, ordering and recording preference. No old clipboard reads.
            let mut statement = tx.prepare(
                "SELECT id,text,copied_at_ms,sequence FROM clipboard_text_v1 ORDER BY id",
            )?;
            let rows = statement.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?;
            for row in rows {
                let (id, text, time, sequence) = row?;
                let payload = ClipboardPayload::text(&text);
                payload.validate()?;
                insert(&tx, &payload, time, sequence, Some(id))?;
            }
            drop(statement);
            tx.execute_batch("DROP TABLE clipboard_text_v1;")?;
        }
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS clipboard_policy(
            id INTEGER PRIMARY KEY CHECK(id=1), paused_until_ms INTEGER,
            retention_days INTEGER NOT NULL CHECK(retention_days IN(0,1,7,30,90)));
            INSERT OR IGNORE INTO clipboard_policy VALUES(1,NULL,0);
            CREATE INDEX IF NOT EXISTS clipboard_expiry ON clipboard_items(copied_at_ms);
            PRAGMA user_version=3;",
        )?;
        tx.commit()?;
        Ok(Self(connection))
    }
}
fn fingerprint(payload: &ClipboardPayload) -> Vec<u8> {
    let mut hash = Sha256::new();
    let mut formats: Vec<_> = payload.formats.iter().collect();
    formats.sort_by(|a, b| a.name.cmp(&b.name));
    for format in formats {
        hash.update((format.name.len() as u64).to_le_bytes());
        hash.update(format.name.as_bytes());
        hash.update((format.data.len() as u64).to_le_bytes());
        hash.update(&format.data);
    }
    hash.finalize().to_vec()
}
fn insert(
    tx: &Transaction<'_>,
    payload: &ClipboardPayload,
    time: i64,
    sequence: i64,
    id: Option<i64>,
) -> Result<i64> {
    let (kind, summary, search) = content::metadata(payload);
    let stored_bytes = payload.byte_count()
        + summary.len()
        + search.len()
        + payload.formats.iter().map(|f| f.name.len()).sum::<usize>();
    tx.execute("INSERT INTO clipboard_items(id,fingerprint,search_text,summary,kind,copied_at_ms,sequence,byte_count,stored_bytes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![id,fingerprint(payload),search,summary,kind,time,sequence,payload.byte_count(),stored_bytes])?;
    let id = id.unwrap_or_else(|| tx.last_insert_rowid());
    for format in &payload.formats {
        tx.execute(
            "INSERT INTO clipboard_formats VALUES(?1,?2,?3)",
            params![id, format.name, format.data],
        )?;
    }
    Ok(id)
}
fn load(connection: &Connection, id: i64) -> Result<Option<ClipboardPayload>> {
    let (count, bytes): (usize, usize) = connection.query_row(
        "SELECT COUNT(*),COALESCE(SUM(length(data)),0) FROM clipboard_formats WHERE item_id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if count == 0 {
        return Ok(None);
    }
    ensure!(
        count <= anywhere_platform::clipboard::MAX_FORMATS && bytes <= MAX_ENTRY_BYTES,
        "clipboard record exceeds limits"
    );
    let mut statement = connection
        .prepare("SELECT name,data FROM clipboard_formats WHERE item_id=?1 ORDER BY name")?;
    let formats = statement
        .query_map([id], |r| {
            Ok(ClipboardFormat {
                name: r.get(0)?,
                data: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let payload = ClipboardPayload { formats };
    payload.validate()?;
    Ok(Some(payload))
}
impl HistoryRepository for SqliteHistoryRepository {
    fn policy(&self) -> Result<HistoryPolicy> {
        let policy = self.0.query_row(
            "SELECT paused_until_ms,retention_days FROM clipboard_policy WHERE id=1",
            [],
            |row| {
                Ok(HistoryPolicy {
                    paused_until_ms: row.get(0)?,
                    retention_days: row.get(1)?,
                })
            },
        )?;
        policy.validate()?;
        Ok(policy)
    }
    fn set_policy(&mut self, policy: HistoryPolicy) -> Result<()> {
        policy.validate()?;
        self.0.execute(
            "UPDATE clipboard_policy SET paused_until_ms=?1,retention_days=?2 WHERE id=1",
            params![policy.paused_until_ms, policy.retention_days],
        )?;
        Ok(())
    }
    fn expire(&mut self, cutoff_ms: i64) -> Result<()> {
        // Most reads have nothing to expire. Avoid a write transaction and its
        // lock contention on each search, status read and preview recheck.
        let due: bool = self.0.query_row(
            "SELECT EXISTS(SELECT 1 FROM clipboard_items WHERE copied_at_ms<=?1)",
            [cutoff_ms],
            |row| row.get(0),
        )?;
        if !due {
            return Ok(());
        }
        self.0.execute(
            "DELETE FROM clipboard_items WHERE copied_at_ms<=?1",
            [cutoff_ms],
        )?;
        Ok(())
    }
    fn usage(&self) -> Result<(u32, u32)> {
        Ok(self.0.query_row(
            "SELECT COUNT(*),COALESCE(SUM(stored_bytes),0) FROM clipboard_items",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    }
    fn recording_enabled(&self) -> Result<bool> {
        Ok(self.0.query_row(
            "SELECT recording FROM clipboard_settings WHERE id=1",
            [],
            |r| r.get(0),
        )?)
    }
    fn set_recording_enabled(&mut self, enabled: bool) -> Result<()> {
        let tx = self.0.transaction()?;
        tx.execute(
            "UPDATE clipboard_settings SET recording=?1 WHERE id=1",
            [enabled],
        )?;
        tx.execute(
            "UPDATE clipboard_policy SET paused_until_ms=NULL WHERE id=1",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
    fn record_payload(
        &mut self,
        payload: &ClipboardPayload,
        copied_at_ms: i64,
    ) -> Result<Option<i64>> {
        if payload.validate().is_err() {
            return Ok(None);
        }
        if payload.formats.len() == 1
            && payload
                .plain_text()
                .is_some_and(|text| text.trim().is_empty())
        {
            return Ok(None);
        }
        let tx = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence),0)+1 FROM clipboard_items",
            [],
            |r| r.get(0),
        )?;
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM clipboard_items WHERE fingerprint=?1",
                [fingerprint(payload)],
                |r| r.get(0),
            )
            .optional()?;
        let id = if let Some(id) = existing {
            let stored =
                load(&tx, id)?.ok_or_else(|| anyhow::anyhow!("clipboard record unavailable"))?;
            // Never merge different bodies even if a fingerprint collides or the DB is corrupt.
            ensure!(
                fingerprint(&stored) == fingerprint(payload) && equivalent(&stored, payload),
                "clipboard identity mismatch"
            );
            tx.execute(
                "UPDATE clipboard_items SET copied_at_ms=?1,sequence=?2 WHERE id=?3",
                params![copied_at_ms, sequence, id],
            )?;
            id
        } else {
            insert(&tx, payload, copied_at_ms, sequence, None)?
        };
        tx.execute("DELETE FROM clipboard_items WHERE id IN(SELECT id FROM(SELECT id,ROW_NUMBER() OVER(ORDER BY sequence DESC) position,SUM(stored_bytes) OVER(ORDER BY sequence DESC) total FROM clipboard_items) WHERE position>?1 OR total>?2)",params![MAX_ENTRIES,MAX_STORED_BYTES])?;
        tx.commit()?;
        Ok(Some(id))
    }
    fn search(&self, query: &str) -> Result<Vec<HistoryItem>> {
        ensure!(
            query.len() <= MAX_QUERY_BYTES,
            "clipboard query is too long"
        );
        let normalized = query.to_lowercase();
        let tokens: Vec<_> = normalized.split_whitespace().collect();
        let mut sql = format!(
            "SELECT id,substr(summary,1,{SUMMARY_CHARACTERS}),copied_at_ms,byte_count,kind FROM clipboard_items"
        );
        if !tokens.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&vec!["instr(search_text,?)>0"; tokens.len()].join(" AND "));
        }
        sql.push_str(&format!(" ORDER BY sequence DESC LIMIT {MAX_ENTRIES}"));
        let mut statement = self.0.prepare(&sql)?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(tokens), item)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }
    fn item(&self, id: i64) -> Result<Option<HistoryItem>> {
        Ok(self.0.query_row(&format!("SELECT id,substr(summary,1,{SUMMARY_CHARACTERS}),copied_at_ms,byte_count,kind FROM clipboard_items WHERE id=?1"),[id],item).optional()?)
    }
    fn payload(&self, id: i64) -> Result<Option<ClipboardPayload>> {
        load(&self.0, id)
    }
    fn delete(&mut self, id: i64) -> Result<()> {
        self.0
            .execute("DELETE FROM clipboard_items WHERE id=?1", [id])?;
        Ok(())
    }
    fn clear(&mut self) -> Result<()> {
        self.0.execute("DELETE FROM clipboard_items", [])?;
        Ok(())
    }
}
fn equivalent(a: &ClipboardPayload, b: &ClipboardPayload) -> bool {
    a.formats.len() == b.formats.len() && a.formats.iter().all(|f| b.formats.contains(f))
}
fn item(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryItem> {
    Ok(HistoryItem {
        id: row.get(0)?,
        summary: row.get(1)?,
        copied_at_ms: row.get(2)?,
        byte_count: row.get(3)?,
        kind: row.get(4)?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn memory() -> SqliteHistoryRepository {
        SqliteHistoryRepository::connection(Connection::open_in_memory().unwrap()).unwrap()
    }
    #[test]
    fn restart_dedup_unicode_literal_query_and_summary_only_list() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("clipboard.sqlite3");
        let id;
        {
            let mut repo = SqliteHistoryRepository::open(&path).unwrap();
            assert!(!repo.recording_enabled().unwrap());
            repo.set_recording_enabled(true).unwrap();
            id = repo.record("Hello 中文\n100%_safe", 1).unwrap().unwrap();
            repo.record("other", 2).unwrap();
            assert_eq!(repo.record("Hello 中文\n100%_safe", 3).unwrap(), Some(id));
        }
        let repo = SqliteHistoryRepository::open(&path).unwrap();
        assert!(repo.recording_enabled().unwrap());
        let rows = repo.search("中文 HELLO").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].copied_at_ms, 3);
        assert!(!rows[0].summary.contains('\n'));
        assert_eq!(repo.search("%_").unwrap().len(), 1);
        assert!(repo.search("' OR 1=1 --").unwrap().is_empty());
        assert!(repo.search("hello missing").unwrap().is_empty());
        assert_eq!(repo.search("").unwrap().len(), 2);
        assert_eq!(
            repo.payload(id).unwrap().unwrap().plain_text().unwrap(),
            "Hello 中文\n100%_safe"
        );
    }
    #[test]
    fn entry_count_bytes_and_query_limits_are_enforced() {
        let mut repo = memory();
        for input in [
            "".to_string(),
            "  \n".into(),
            "a\0b".into(),
            "x".repeat(MAX_ENTRY_BYTES + 1),
        ] {
            assert_eq!(repo.record(&input, 0).unwrap(), None);
        }
        for i in 0..MAX_ENTRIES + 5 {
            repo.record(&format!("entry-{i}"), i as i64).unwrap();
        }
        let rows = repo.search("").unwrap();
        assert_eq!(rows.len(), MAX_ENTRIES);
        assert_eq!(rows.last().unwrap().summary, "entry-5");
        assert!(repo.search(&"q".repeat(MAX_QUERY_BYTES + 1)).is_err());
        repo.clear().unwrap();
        for i in 0..12 {
            repo.record(&format!("{i:03}{}", "x".repeat(MAX_ENTRY_BYTES / 2 - 3)), i)
                .unwrap();
        }
        let (count, bytes): (usize, usize) = repo
            .0
            .query_row(
                "SELECT COUNT(*),SUM(stored_bytes) FROM clipboard_items",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(count < 12 && bytes <= MAX_STORED_BYTES);
        assert_eq!(
            repo.search("").unwrap()[0].summary.chars().count(),
            SUMMARY_CHARACTERS
        );
    }
    #[test]
    fn failed_transaction_preserves_history_and_delete_ids_are_not_reused() {
        let mut repo = memory();
        let first = repo.record("first", 1).unwrap().unwrap();
        repo.0.execute_batch("CREATE TRIGGER reject_delete BEFORE DELETE ON clipboard_items BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        for i in 1..MAX_ENTRIES {
            repo.record(&format!("item-{i}"), i as i64).unwrap();
        }
        assert!(repo.record("overflow", 9999).is_err());
        assert!(repo.search("overflow").unwrap().is_empty());
        assert!(repo.item(first).unwrap().is_some());
        repo.0.execute_batch("DROP TRIGGER reject_delete").unwrap();
        repo.delete(first).unwrap();
        assert!(repo.item(first).unwrap().is_none());
        repo.clear().unwrap();
        let next = repo.record("next", 5).unwrap().unwrap();
        assert!(next > first);
    }
    #[test]
    fn newer_database_is_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("clipboard.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA user_version=99").unwrap();
        drop(connection);
        assert!(SqliteHistoryRepository::open(&path).is_err());
        let version: u32 = Connection::open(path)
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 99);
    }
}

#[cfg(test)]
mod multiformat_tests {
    use super::*;
    use anywhere_platform::clipboard::{FILES, TEXT};
    fn memory() -> SqliteHistoryRepository {
        SqliteHistoryRepository::connection(Connection::open_in_memory().unwrap()).unwrap()
    }
    fn format(name: &str, data: &[u8]) -> ClipboardFormat {
        ClipboardFormat {
            name: name.into(),
            data: data.to_vec(),
        }
    }
    #[test]
    fn expiry_checks_are_read_only_until_a_record_is_due_and_delete_failure_is_not_ignored() {
        let mut repo = memory();
        repo.record("fresh", 100).unwrap();
        repo.0.execute_batch("PRAGMA query_only=ON").unwrap();
        repo.expire(99).unwrap();
        assert!(repo.expire(100).is_err());
    }
    #[test]
    fn v2_upgrade_preserves_history_and_policy_persists_without_enabling_recording() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history");
        let mut repo = SqliteHistoryRepository::open(&path).unwrap();
        let id = repo.record("preserved", 10).unwrap().unwrap();
        repo.0
            .execute_batch("DROP TABLE clipboard_policy; PRAGMA user_version=2;")
            .unwrap();
        drop(repo);
        let mut repo = SqliteHistoryRepository::open(&path).unwrap();
        assert_eq!(repo.policy().unwrap(), HistoryPolicy::default());
        assert!(!repo.recording_enabled().unwrap());
        assert_eq!(repo.search("").unwrap()[0].id, id);
        let policy = HistoryPolicy {
            paused_until_ms: Some(1000),
            retention_days: 7,
        };
        repo.set_policy(policy).unwrap();
        drop(repo);
        let mut repo = SqliteHistoryRepository::open(&path).unwrap();
        assert_eq!(repo.policy().unwrap(), policy);
        assert_eq!(
            repo.payload(id).unwrap().unwrap().plain_text(),
            Some("preserved")
        );
        repo.set_recording_enabled(false).unwrap();
        assert!(repo.policy().unwrap().paused_until_ms.is_none());
        assert_eq!(repo.policy().unwrap().retention_days, 7);
    }
    #[test]
    fn multiple_representations_are_one_identity_and_restore_original_bytes() {
        let mut repo = memory();
        let payload = ClipboardPayload {
            formats: vec![
                format(TEXT, b"Hello"),
                format("public.html", b"<b>Hello</b>"),
                format("public.rtf", b"{\\rtf1 Hello}"),
                format("com.example.custom", &[0, 1, 255]),
            ],
        };
        let id = repo.record_payload(&payload, 1).unwrap().unwrap();
        let mut reordered = payload.clone();
        reordered.formats.reverse();
        assert_eq!(repo.record_payload(&reordered, 3).unwrap(), Some(id));
        assert_eq!(repo.search("").unwrap().len(), 1);
        assert_eq!(repo.search("HELLO rich_text").unwrap()[0].kind, "rich_text");
        assert!(equivalent(&repo.payload(id).unwrap().unwrap(), &payload));
        let mut changed = payload.clone();
        changed.formats[1].data = b"<i>Hello</i>".to_vec();
        assert_ne!(repo.record_payload(&changed, 4).unwrap(), Some(id));
        repo.delete(id).unwrap();
        let orphans: usize = repo
            .0
            .query_row(
                "SELECT COUNT(*) FROM clipboard_formats WHERE item_id=?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 0);
    }
    #[test]
    fn file_paths_are_searchable_but_file_contents_are_not_read() {
        let mut repo = memory();
        let paths = vec![
            "/does not exist/中文项目/photo.png",
            "/does not exist/report.pdf",
        ];
        let payload = ClipboardPayload {
            formats: vec![format(FILES, &serde_json::to_vec(&paths).unwrap())],
        };
        let id = repo.record_payload(&payload, 1).unwrap().unwrap();
        assert_eq!(repo.search("中文项目").unwrap()[0].kind, "files");
        assert_eq!(repo.payload(id).unwrap().unwrap().files().unwrap(), paths);
    }
    #[test]
    fn image_and_binary_data_remain_intact() {
        let mut repo = memory();
        let mut image = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1024, 768)
            .write_to(&mut image, image::ImageFormat::Png)
            .unwrap();
        let bytes = image.into_inner();
        let payload = ClipboardPayload {
            formats: vec![format("public.png", &bytes)],
        };
        let id = repo.record_payload(&payload, 1).unwrap().unwrap();
        assert_eq!(repo.item(id).unwrap().unwrap().kind, "image");
        assert_eq!(repo.payload(id).unwrap().unwrap().formats[0].data, bytes);
        let binary = ClipboardPayload {
            formats: vec![format("com.example.binary", &[0, 255, 3, 4])],
        };
        let id = repo.record_payload(&binary, 2).unwrap().unwrap();
        assert_eq!(repo.item(id).unwrap().unwrap().kind, "binary");
        assert_eq!(repo.payload(id).unwrap().unwrap(), binary);
    }
    #[test]
    fn v1_migration_preserves_ids_order_and_recording_preference() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("history");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("CREATE TABLE clipboard_settings(id INTEGER PRIMARY KEY,recording INTEGER NOT NULL); INSERT INTO clipboard_settings VALUES(1,0);
            CREATE TABLE clipboard_items(id INTEGER PRIMARY KEY AUTOINCREMENT,text TEXT NOT NULL UNIQUE,search_text TEXT NOT NULL,summary TEXT NOT NULL,copied_at_ms INTEGER NOT NULL,sequence INTEGER NOT NULL,byte_count INTEGER NOT NULL,stored_bytes INTEGER NOT NULL);
            CREATE INDEX clipboard_recent ON clipboard_items(sequence DESC);
            INSERT INTO clipboard_items VALUES(41,'Hello 中文','hello 中文','Hello 中文',123,7,12,36);
            PRAGMA user_version=1;").unwrap();
        drop(connection);
        let mut repo = SqliteHistoryRepository::open(&path).unwrap();
        assert!(!repo.recording_enabled().unwrap());
        assert_eq!(repo.search("中文").unwrap()[0].id, 41);
        assert_eq!(
            repo.payload(41).unwrap().unwrap().plain_text(),
            Some("Hello 中文")
        );
        assert_eq!(repo.record("Hello 中文", 456).unwrap(), Some(41));
        let next = repo.record("new", 500).unwrap().unwrap();
        assert!(next > 41);
        drop(repo);
        let repo = SqliteHistoryRepository::open(&path).unwrap();
        assert_eq!(repo.search("").unwrap().len(), 2);
    }
    #[test]
    fn fingerprint_mismatch_is_rejected_instead_of_merging_different_content() {
        let mut repo = memory();
        let id = repo.record("a", 1).unwrap().unwrap();
        repo.0
            .execute(
                "UPDATE clipboard_items SET fingerprint=?1 WHERE id=?2",
                params![fingerprint(&ClipboardPayload::text("b")), id],
            )
            .unwrap();
        assert!(repo.record("b", 2).is_err());
        assert_eq!(repo.payload(id).unwrap().unwrap().plain_text(), Some("a"));
    }
    #[test]
    fn full_unicode_text_is_stored_without_truncation() {
        let mut repo = memory();
        let text = "中".repeat(MAX_PREVIEW_BYTES);
        let id = repo.record(&text, 1).unwrap().unwrap();
        assert_eq!(
            repo.payload(id).unwrap().unwrap().plain_text(),
            Some(text.as_str())
        );
    }
}
