//! Complete, cancellable filename traversal with bounded RAM and query-local storage.
//! Directory work and all matches are streamed to temporary files, never an index.
use anyhow::Result;
use anywhere_platform::FileSearchOptions;
use anywhere_protocol::{FileSearchFilter, FileSearchProgress};
use progress::Progress;
use scope::ScanScope;
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering},
        Arc, Mutex,
    },
};

mod index;
mod pagination;
mod progress;
mod query;
mod record;
mod scope;
mod storage;
#[cfg(test)]
mod tests;
mod traversal;

pub use query::{normalize, NameQuery, RankedHit};
pub(super) use record::{read_match_record, write_match_record};
#[cfg(test)]
use record::{read_record, write_record};

pub const PAGE_SIZE: usize = 64;
const BUFFER: usize = 64 * 1024;
static NEXT_QUERY: AtomicU64 = AtomicU64::new(1);

pub struct ScanPage {
    pub hits: Vec<RankedHit>,
    pub progress: FileSearchProgress,
}
pub struct ScanStore {
    // Release the lease before deleting the workspace (important on Windows).
    directory: anywhere_platform::search_storage::SearchWorkspace,
    pub query_id: u64,
    pub(super) query: NameQuery,
    scope: ScanScope,
    progress: Mutex<Progress>,
    continuous: Mutex<super::result_store::ContinuousResults>,
    storage_bytes: Arc<AtomicU64>,
    pub(super) projections: Mutex<
        std::collections::HashMap<(PathBuf, bool), crate::core::search::candidate::Candidate>,
    >,
    pub cancelled: AtomicBool,
}
impl ScanStore {
    pub fn new(text: &str, filter: FileSearchFilter, options: FileSearchOptions) -> Result<Self> {
        let (text, options, direct_only, exact_path) = scope::prepare_query(text, options)?;
        let directory = anywhere_platform::search_storage::SearchWorkspace::new()?;
        File::create(directory.path().join("matches"))?;
        Ok(Self {
            directory,
            query_id: NEXT_QUERY.fetch_add(1, AtomicOrdering::Relaxed),
            query: NameQuery::new(&text, filter),
            scope: ScanScope::new(options, direct_only, exact_path),
            progress: Mutex::new(Progress::default()),
            continuous: Mutex::new(Default::default()),
            storage_bytes: Arc::new(AtomicU64::new(0)),
            projections: Mutex::new(Default::default()),
            cancelled: AtomicBool::new(false),
        })
    }
    pub(super) fn writer<W>(&self, writer: W) -> storage::BudgetWriter<W> {
        storage::BudgetWriter::new(writer, self.storage_bytes.clone())
    }
    pub(super) fn remove_run(&self, path: &Path) -> Result<()> {
        let bytes = std::fs::metadata(path)?.len();
        std::fs::remove_file(path)?;
        let _ = self.storage_bytes.try_update(
            AtomicOrdering::AcqRel,
            AtomicOrdering::Acquire,
            |used| Some(used.saturating_sub(bytes)),
        );
        Ok(())
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, AtomicOrdering::Release);
    }
    pub fn is_path_query(&self) -> bool {
        self.scope.direct_only
    }
    pub fn matches_path(&self) -> PathBuf {
        self.directory.path().join("matches")
    }
    pub fn workspace(&self) -> &Path {
        self.directory.path()
    }
}
