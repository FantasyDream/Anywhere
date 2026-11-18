//! Installed-application index lifetime, independent of any launcher session.
use crate::features::applications::index::{self, ApplicationCatalog, ScanStatus};
use anyhow::{anyhow, Result};
use anywhere_platform::ApplicationCatalog as ApplicationCatalogHost;
use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::watch;

pub(super) struct ApplicationIndex {
    published: Mutex<PublishedIndex>,
    scans: Mutex<ScanQueue>,
    issued: AtomicU64,
    updates: watch::Sender<u64>,
}

enum ScanQueue {
    Idle,
    Running { pending: Option<ScanRequest> },
}

struct ScanRequest {
    generation: u64,
    platform: Arc<dyn ApplicationCatalogHost>,
}

struct PublishedIndex {
    completed_scan: u64,
    full: Arc<ApplicationCatalog>,
    visible: Arc<ApplicationCatalog>,
    excluded: BTreeSet<String>,
}

/// Immutable search input; a filter change invalidates it without a new scan.
pub(super) struct IndexSnapshot {
    pub catalog: Arc<ApplicationCatalog>,
    completed_scan: u64,
}

impl Default for ApplicationIndex {
    fn default() -> Self {
        let full = Arc::new(ApplicationCatalog::empty());
        Self {
            published: Mutex::new(PublishedIndex {
                completed_scan: 0,
                visible: full.clone(),
                full,
                excluded: BTreeSet::new(),
            }),
            scans: Mutex::new(ScanQueue::Idle),
            issued: AtomicU64::new(0),
            updates: watch::channel(0).0,
        }
    }
}

impl ApplicationIndex {
    pub fn snapshot(&self) -> Result<IndexSnapshot> {
        let state = self
            .published
            .lock()
            .map_err(|_| anyhow!("catalog lock poisoned"))?;
        Ok(IndexSnapshot {
            catalog: state.visible.clone(),
            completed_scan: state.completed_scan,
        })
    }

    pub fn is_current(&self, snapshot: &IndexSnapshot) -> Result<bool> {
        let state = self
            .published
            .lock()
            .map_err(|_| anyhow!("catalog lock poisoned"))?;
        Ok(state.completed_scan == snapshot.completed_scan
            && Arc::ptr_eq(&state.visible, &snapshot.catalog))
    }

    /// Configuration labels include applications excluded from search.
    pub fn full_catalog(&self) -> Result<Arc<ApplicationCatalog>> {
        self.published
            .lock()
            .map(|state| state.full.clone())
            .map_err(|_| anyhow!("application catalog lock poisoned"))
    }

    pub fn has_completed_scan(&self) -> bool {
        self.published
            .lock()
            .map(|state| state.completed_scan > 0)
            .unwrap_or(false)
    }

    pub fn seed(&self, report: index::ScanReport) {
        let generation = self.issue_scan();
        self.publish(generation, ScanStatus::Complete(report));
    }

    pub fn excluded(&self) -> Vec<String> {
        self.published
            .lock()
            .map(|state| state.excluded.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn set_excluded(&self, ids: Vec<String>) -> Result<bool> {
        let excluded = ids.into_iter().filter(|id| !id.trim().is_empty()).collect();
        let mut state = self
            .published
            .lock()
            .map_err(|_| anyhow!("catalog lock poisoned"))?;
        if state.excluded == excluded {
            return Ok(false);
        }
        // Preferences and the filtered view change together, including during scans.
        state.excluded = excluded;
        state.visible = filtered(&state.full, &state.excluded);
        Ok(true)
    }

    /// Keep at most one scan running and one follow-up for the latest activation.
    pub fn spawn_scan(self: &Arc<Self>, platform: Arc<dyn ApplicationCatalogHost>) -> u64 {
        let mut queue = self.scans.lock().expect("application scan queue poisoned");
        let generation = self.issue_scan();
        let request = ScanRequest {
            generation,
            platform,
        };
        if let ScanQueue::Running { pending } = &mut *queue {
            *pending = Some(request);
            return generation;
        }
        *queue = ScanQueue::Running { pending: None };
        drop(queue);

        let index = self.clone();
        tokio::spawn(index.run_scans(request));
        generation
    }

    async fn run_scans(self: Arc<Self>, mut request: ScanRequest) {
        loop {
            let scanned = index::scan_async(request.platform).await;
            self.publish(request.generation, scanned);

            let mut queue = self.scans.lock().expect("application scan queue poisoned");
            let ScanQueue::Running { pending } = &mut *queue else {
                unreachable!("scan worker owns the running queue");
            };
            let Some(next) = pending.take() else {
                *queue = ScanQueue::Idle;
                return;
            };
            request = next;
        }
    }

    fn issue_scan(&self) -> u64 {
        self.issued.fetch_add(1, Ordering::AcqRel) + 1
    }

    fn publish(&self, generation: u64, scanned: ScanStatus) {
        let Ok(mut state) = self.published.lock() else {
            return;
        };
        // An older completed scan cannot overwrite a newer success or failure.
        if generation < state.completed_scan {
            return;
        }
        state.completed_scan = generation;
        match scanned {
            ScanStatus::Complete(report) => {
                let skipped = report.skipped;
                state.full = Arc::new(ApplicationCatalog::new(report.applications));
                state.visible = filtered(&state.full, &state.excluded);
                let count = state.visible.applications().len();
                self.updates.send_replace(generation);
                tracing::info!(count, skipped, generation, "app_index_published");
            }
            ScanStatus::Failed => {
                self.updates.send_replace(generation);
                tracing::error!(generation, "app_index_kept");
            }
        }
    }

    /// Notifications carry scan completion, not filter changes or session revisions.
    pub fn updates(&self) -> watch::Receiver<u64> {
        self.updates.subscribe()
    }

    /// Wait for the scan issued at entry, including failures that retain the last index.
    pub async fn ready(&self) {
        let expected = self.issued.load(Ordering::Acquire);
        let mut updates = self.updates();
        while *updates.borrow_and_update() < expected {
            if !matches!(
                tokio::time::timeout(Duration::from_secs(10), updates.changed()).await,
                Ok(Ok(()))
            ) {
                return;
            }
        }
    }
}

fn filtered(
    full: &Arc<ApplicationCatalog>,
    excluded: &BTreeSet<String>,
) -> Arc<ApplicationCatalog> {
    if excluded.is_empty() {
        full.clone()
    } else {
        Arc::new(full.excluding(excluded))
    }
}

#[cfg(test)]
#[path = "application_index_tests.rs"]
mod tests;
