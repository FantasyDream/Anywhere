//! Bounded scan diagnostics and progress snapshots.
use super::ScanStore;
use anywhere_protocol::FileSearchProgress;
use std::path::Path;

#[derive(Default)]
pub(super) struct Progress {
    pub(super) scanned: u64,
    pub(super) matches: u64,
    pub(super) boundary: u64,
    pub(super) finished: bool,
    pub(super) issues: u64,
    pub(super) inaccessible: u64,
    pub(super) examples: Vec<String>,
}
impl ScanStore {
    /// Only flushed match records are visible to pagination and external sorting.
    pub(in crate::features::filesystem) fn committed_boundary(&self) -> u64 {
        self.progress.lock().unwrap().boundary
    }

    pub(super) fn issue(&self, message: impl Into<String>) {
        let mut p = self.progress.lock().expect("scan progress poisoned");
        p.issues += 1;
        if p.examples.len() < 8 {
            let message = message.into();
            // Keep diagnostics bounded just like the UI examples. Record the
            // cause, not the user's query, so an incomplete scan is diagnosable.
            tracing::warn!(query_id = self.query_id, issue = %message, "file_search_issue");
            p.examples.push(message);
        }
    }
    pub(super) fn io_issue(&self, path: &Path, error: &std::io::Error) {
        match error.kind() {
            std::io::ErrorKind::PermissionDenied => {
                self.progress
                    .lock()
                    .expect("scan progress poisoned")
                    .inaccessible += 1;
            }
            // A mutable tree is not a filesystem snapshot. Entries removed
            // between listing and reading have no remaining result to return.
            std::io::ErrorKind::NotFound => {}
            _ => self.issue(format!("{}: {error}", path.display())),
        }
    }
    pub(super) fn root_issue(&self, path: &Path, error: &std::io::Error) {
        if error.kind() == std::io::ErrorKind::NotFound {
            self.issue(format!("{}: Search root no longer exists", path.display()));
        } else {
            self.io_issue(path, error);
        }
    }
    pub fn fail(&self, error: &anyhow::Error) {
        self.issue(error.to_string());
        self.progress.lock().unwrap().finished = true;
    }
    pub fn progress(&self) -> FileSearchProgress {
        let p = self.progress.lock().unwrap();
        FileSearchProgress {
            query_id: self.query_id,
            roots: if self.scope.options.roots.is_empty() {
                vec!["Home".into()]
            } else {
                self.scope
                    .options
                    .roots
                    .iter()
                    .map(|root| root.display().to_string())
                    .collect()
            },
            scanned_entries: p.scanned,
            confirmed_matches: p.matches,
            finished: p.finished,
            issue_count: p.issues,
            inaccessible_count: p.inaccessible,
            issue_examples: p.examples.clone(),
            ..Default::default()
        }
    }
}
