//! Disposable, indexed unread rows. No permanent catalog and no in-memory path list.
use super::super::scan::{read_record, NameQuery, RankedHit, ScanStore, PAGE_SIZE};
use anyhow::{bail, ensure, Context, Result};
use anywhere_platform::search_storage::{decode_path, encode_path};
use rusqlite::{params, Connection};
use std::{
    collections::HashSet,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    os::unix::ffi::OsStrExt,
    path::{Component, Path, PathBuf},
};

const TRANSACTION_ROWS: usize = 512;
const TRANSACTION_BYTES: u64 = 1024 * 1024;

pub(super) struct SqliteResults {
    pub(super) connection: Connection,
    boundary: u64,
    query_id: u64,
}

fn check(cancelled: &dyn Fn() -> bool) -> Result<()> {
    ensure!(!cancelled(), "search cancelled");
    Ok(())
}

// Unix Path::cmp compares components, not the bytes of the entire path. Escaping
// every byte lets a component terminator sort before any following byte, keeping
// /a/report before /a-b/report. Identity remains a separate, lossless raw BLOB.
pub(super) fn path_key(path: &Path) -> Vec<u8> {
    let mut key = Vec::new();
    for component in path.components() {
        key.push(match component {
            Component::Prefix(_) => unreachable!("Unix paths have no prefix"),
            Component::RootDir => 1,
            Component::CurDir => 2,
            Component::ParentDir => 3,
            Component::Normal(_) => 4,
        });
        for &byte in component.as_os_str().as_bytes() {
            key.extend_from_slice(&[1, byte]);
        }
        key.push(0);
    }
    key
}

impl SqliteResults {
    pub(super) fn new(store: &ScanStore) -> Result<Self> {
        let connection = Connection::open(store.workspace().join("probe.sqlite"))?;
        // Like the production temporary files, this is disposable and not fsynced.
        // A small SQLite page cache is a target, not a whole-process RSS limit.
        connection.execute_batch(
            "PRAGMA journal_mode=DELETE;
             PRAGMA synchronous=OFF;
             PRAGMA temp_store=FILE;
             PRAGMA cache_size=-512;
             PRAGMA mmap_size=0;
             CREATE TABLE unread (
                 path BLOB PRIMARY KEY,
                 directory INTEGER NOT NULL,
                 score INTEGER NOT NULL,
                 normalized TEXT NOT NULL,
                 path_key BLOB NOT NULL
             );
             CREATE INDEX unread_order ON unread(score DESC, normalized, path_key);",
        )?;
        Ok(Self {
            connection,
            boundary: 0,
            query_id: store.query_id,
        })
    }

    fn ingest(&mut self, store: &ScanStore, cancelled: &dyn Fn() -> bool) -> Result<()> {
        ensure!(self.query_id == store.query_id, "query changed");
        let boundary = store.progress.lock().unwrap().boundary;
        ensure!(boundary >= self.boundary, "source boundary moved backwards");
        let mut input = File::open(store.matches_path())?;
        input.seek(SeekFrom::Start(self.boundary))?;
        let mut input = BufReader::new(input.take(boundary - self.boundary));
        while self.boundary < boundary {
            check(cancelled)?;
            let transaction = self.connection.transaction()?;
            let mut bytes = 0;
            {
                let mut insert = transaction.prepare_cached(
                    "INSERT INTO unread(path,directory,score,normalized,path_key) VALUES(?1,?2,?3,?4,?5)
                     ON CONFLICT(path) DO NOTHING",
                )?;
                for _ in 0..TRANSACTION_ROWS {
                    check(cancelled)?;
                    let Some((path, directory, length)) = read_record(&mut input)? else {
                        bail!("source ended before committed boundary");
                    };
                    let hit = store
                        .query
                        .rank(&path, directory)
                        .context("invalid match record")?;
                    insert.execute(params![
                        encode_path(&path),
                        directory,
                        hit.score,
                        hit.normalized,
                        path_key(&path)
                    ])?;
                    bytes += length;
                    if self.boundary + bytes == boundary || bytes >= TRANSACTION_BYTES {
                        break;
                    }
                }
            }
            check(cancelled)?;
            transaction.commit()?;
            // Publish only a fully committed ingestion boundary. A failed batch
            // is rolled back and reread on retry, including cancellation halfway.
            self.boundary += bytes;
        }
        Ok(())
    }

    pub(super) fn more(
        &mut self,
        store: &ScanStore,
        displayed: &HashSet<PathBuf>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<RankedHit>> {
        self.ingest(store, cancelled)?;
        let transaction = self.connection.transaction()?;
        let mut hits = Vec::with_capacity(PAGE_SIZE);
        loop {
            check(cancelled)?;
            let rows = {
                let mut select = transaction.prepare_cached(
                    "SELECT path,directory FROM unread ORDER BY score DESC,normalized,path_key LIMIT ?1",
                )?;
                let rows = select
                    .query_map([PAGE_SIZE - hits.len()], |row| {
                        Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, bool>(1)?))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows
            };
            if rows.is_empty() {
                break;
            }
            let mut delete = transaction.prepare_cached("DELETE FROM unread WHERE path=?1")?;
            for (raw, directory) in rows {
                check(cancelled)?;
                let path = decode_path(raw.clone())?;
                delete.execute([raw])?;
                if !displayed.contains(&path) {
                    hits.push(rank(&store.query, &path, directory)?);
                }
            }
            if hits.len() == PAGE_SIZE {
                break;
            }
        }
        check(cancelled)?;
        transaction.commit()?;
        Ok(hits)
    }
}

fn rank(query: &NameQuery, path: &Path, directory: bool) -> Result<RankedHit> {
    query.rank(path, directory).context("invalid stored record")
}
