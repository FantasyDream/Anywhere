//! Persist verified local index matches for the same cursor and execution checks as traversal.
use super::{
    query::insert_top, record::write_match_record, ScanPage, ScanStore, BUFFER, PAGE_SIZE,
};
use anyhow::{ensure, Result};
use anywhere_platform::FileSearchIndex;
use std::{
    collections::HashSet,
    fs::OpenOptions,
    io::{BufWriter, Write},
    sync::atomic::Ordering,
};

impl ScanStore {
    pub(in crate::features::filesystem) fn run_index(
        &self,
        index: &dyn FileSearchIndex,
        text: &str,
        cancelled: &(dyn Fn() -> bool + Sync),
        mut publish: impl FnMut(ScanPage) -> Result<()>,
    ) -> Result<()> {
        let roots = self.collect_canonical_roots()?;
        let results = index.search_index(text.trim(), &roots, cancelled)?;
        ensure!(!cancelled(), "search cancelled");
        let mut records = BufWriter::with_capacity(
            BUFFER,
            self.writer(
                OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(self.matches_path())?,
            ),
        );
        let mut top = Vec::with_capacity(PAGE_SIZE + 1);
        let mut seen = HashSet::new();
        let mut boundary = 0;
        let mut match_count = 0;
        let scanned = results.hits.len() as u64;
        for hit in results.hits {
            ensure!(
                !cancelled() && !self.cancelled.load(Ordering::Acquire),
                "search cancelled"
            );
            if !seen.insert(hit.path.clone()) {
                continue;
            }
            if let Some(ranked) = self.rank_index_hit(&hit, &roots) {
                boundary += write_match_record(&mut records, &ranked)?;
                match_count += 1;
                insert_top(&mut top, ranked);
            }
        }
        records.flush()?;
        if results.truncated {
            self.issue("File index result limit reached. Narrow the search to see more matches.");
        }
        {
            let mut status =
                crate::core::sync::lock_for_revoke(&self.progress, "file_search_progress");
            status.boundary = boundary;
            status.scanned = scanned;
            status.matches = match_count;
            status.finished = true;
        }
        publish(ScanPage {
            hits: top,
            progress: self.page_progress(boundary, match_count > PAGE_SIZE as u64),
        })
    }
}
