//! Verify a provisional filename result batch against the shared query and scope.
use super::{RankedHit, ScanStore};
use anyhow::Result;
use anywhere_platform::{FileSearchHost, FileSearchOptions};

impl ScanStore {
    pub(in crate::features::filesystem) fn provisional_hits(
        &self,
        platform: &dyn FileSearchHost,
        text: &str,
        options: &FileSearchOptions,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<RankedHit>> {
        let roots: Vec<_> = self
            .scope
            .configured_roots()?
            .into_iter()
            .filter_map(|root| root.canonicalize().ok().filter(|path| path.is_dir()))
            .collect();
        if roots.is_empty() {
            return Ok(Vec::new());
        }
        let results = platform.search_provisional(text.trim(), &roots, options, cancelled)?;
        Ok(self.verify_index_hits(results.hits, cancelled))
    }

    #[cfg(test)]
    pub(in crate::features::filesystem) fn provisional_index_hits(
        &self,
        index: &dyn anywhere_platform::FileSearchIndex,
        text: &str,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<RankedHit>> {
        let roots: Vec<_> = self
            .scope
            .configured_roots()?
            .into_iter()
            .filter_map(|root| root.canonicalize().ok().filter(|path| path.is_dir()))
            .collect();
        if roots.is_empty() {
            return Ok(Vec::new());
        }
        let results = index.search_index(text.trim(), &roots, cancelled)?;
        Ok(self.verify_index_hits(results.hits, cancelled))
    }
}
