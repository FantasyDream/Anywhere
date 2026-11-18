//! First-batch projection and continuous, retryable result delivery.
use super::{query::insert_top, record::read_match_record, ScanPage, ScanStore, BUFFER, PAGE_SIZE};
use anyhow::{bail, Result};
use anywhere_protocol::FileSearchProgress;
use std::path::PathBuf;
use std::{
    fs::File,
    io::{BufReader, Read},
    sync::atomic::Ordering as AtomicOrdering,
};

impl ScanStore {
    pub(in crate::features::filesystem) fn page_progress(
        &self,
        read_boundary: u64,
        more: bool,
    ) -> FileSearchProgress {
        let (mut progress, committed_boundary) = self.progress_with_boundary();
        progress.needs_refresh = read_boundary != committed_boundary;
        // A newer committed boundary may contain unread rows even if this read
        // exhausted its own boundary. The cursor is a query-local UI marker.
        progress.next_cursor = (more || progress.needs_refresh).then(|| self.query_id.to_string());
        progress
    }

    pub fn first_page(&self, cancelled: &(dyn Fn() -> bool + Sync)) -> Result<ScanPage> {
        let boundary = self.committed_boundary();
        let mut reader =
            BufReader::with_capacity(BUFFER, File::open(self.matches_path())?.take(boundary));
        let mut top = Vec::with_capacity(PAGE_SIZE);
        let mut eligible = 0;
        while let Some((hit, _)) = read_match_record(&mut reader)? {
            if self.cancelled.load(AtomicOrdering::Acquire) || cancelled() {
                bail!("search cancelled");
            }
            eligible += 1;
            insert_top(&mut top, hit);
        }
        let progress = self.page_progress(boundary, eligible > PAGE_SIZE);
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
            continuous.invalidate(self);
        }
        result
    }
}
