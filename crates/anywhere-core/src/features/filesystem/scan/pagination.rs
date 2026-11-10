//! Boundary-bound cursors and stable, retryable continuous result delivery.
use super::{
    query::{insert_before, insert_top},
    record::{read_record, MAX_RECORD},
    RankedHit, ScanPage, ScanStore, BUFFER, PAGE_SIZE,
};
use anyhow::{bail, Context, Result};
use anywhere_platform::search_storage::{decode_path, encode_path};
use anywhere_protocol::FileSearchProgress;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{
    cmp::Ordering,
    fs::File,
    io::{BufReader, Read},
    sync::atomic::Ordering as AtomicOrdering,
};

#[derive(Serialize, Deserialize)]
struct Cursor {
    query: u64,
    boundary: u64,
    score: u16,
    normalized: String,
    path: Vec<u8>,
    page: u32,
    #[serde(default)]
    backwards: bool,
}
impl ScanStore {
    pub(in crate::features::filesystem) fn page_progress(
        &self,
        top: &[RankedHit],
        boundary: u64,
        page: u32,
        more: bool,
    ) -> Result<FileSearchProgress> {
        let mut progress = self.progress();
        progress.page = page;
        progress.next_cursor = if more {
            let next_page = page.checked_add(1).context("invalid page number")?;
            top.last()
                .map(|hit| {
                    serde_json::to_string(&Cursor {
                        query: self.query_id,
                        boundary,
                        score: hit.score,
                        normalized: hit.normalized.clone(),
                        path: encode_path(&hit.path),
                        page: next_page,
                        backwards: false,
                    })
                })
                .transpose()?
        } else {
            None
        };
        progress.previous_cursor = if page > 0 {
            top.first()
                .map(|hit| {
                    serde_json::to_string(&Cursor {
                        query: self.query_id,
                        boundary,
                        score: hit.score,
                        normalized: hit.normalized.clone(),
                        path: encode_path(&hit.path),
                        page: page - 1,
                        backwards: true,
                    })
                })
                .transpose()?
        } else {
            None
        };
        Ok(progress)
    }
    pub fn page(
        &self,
        cursor: Option<&str>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<ScanPage> {
        let (boundary, after, page, backwards) = if let Some(cursor) = cursor {
            if cursor.len() > MAX_RECORD * 4 {
                bail!("invalid page cursor");
            }
            let cursor: Cursor = serde_json::from_str(cursor).context("invalid page cursor")?;
            let progress = self.progress.lock().unwrap();
            if cursor.query != self.query_id
                || cursor.boundary > progress.boundary
                || u64::from(cursor.page) > progress.matches / PAGE_SIZE as u64
            {
                bail!("stale page cursor");
            }
            let hit = RankedHit {
                path: decode_path(cursor.path)?,
                is_directory: false,
                normalized: cursor.normalized,
                score: cursor.score,
            };
            (cursor.boundary, Some(hit), cursor.page, cursor.backwards)
        } else {
            (self.progress.lock().unwrap().boundary, None, 0, false)
        };
        let mut reader =
            BufReader::with_capacity(BUFFER, File::open(self.matches_path())?.take(boundary));
        let mut top = Vec::with_capacity(PAGE_SIZE + 1);
        let mut eligible = 0;
        while let Some((path, directory, _bytes)) = read_record(&mut reader)? {
            if self.cancelled.load(AtomicOrdering::Acquire) || cancelled() {
                bail!("search cancelled");
            }
            let hit = self
                .query
                .rank(&path, directory)
                .context("invalid match record")?;
            if after.as_ref().is_some_and(|after| {
                hit.compare(after)
                    != if backwards {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
            }) {
                continue;
            }
            eligible += 1;
            if backwards {
                insert_before(&mut top, hit);
            } else {
                insert_top(&mut top, hit);
            }
        }
        if backwards {
            top.reverse();
        }
        let mut progress =
            self.page_progress(&top, boundary, page, backwards || eligible > PAGE_SIZE)?;
        progress.needs_refresh = boundary != self.progress.lock().unwrap().boundary;
        Ok(ScanPage {
            hits: top,
            progress,
        })
    }
    pub fn more(
        &self,
        displayed: &std::collections::HashSet<PathBuf>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<ScanPage> {
        let mut continuous = self.continuous.lock().unwrap();
        let result = continuous.more(self, displayed, &|| {
            self.cancelled.load(AtomicOrdering::Acquire) || cancelled()
        });
        if result.is_err() {
            continuous.invalidate();
        }
        result
    }
}
