//! In-memory diagnostics contain only a predefined category and manifest entrypoint.
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};

use super::background_runtime::BackgroundFailure;

const MAX_PER_PACKAGE: usize = 10;

#[derive(Debug, Clone, Default)]
pub(super) struct Diagnostics(Arc<Mutex<BTreeMap<String, VecDeque<String>>>>);

impl Diagnostics {
    pub(super) fn record(&self, package: &str, entrypoint: &str, error: BackgroundFailure) {
        let Ok(mut packages) = self.0.lock() else {
            return;
        };
        // Package IDs originate only from the verified catalog. Limit the history
        // across repeated installs as well as that of each individual package.
        if packages.len() >= 128 && !packages.contains_key(package) {
            return;
        }
        let entries = packages.entry(package.to_owned()).or_default();
        if entries.len() == MAX_PER_PACKAGE {
            entries.pop_front();
        }
        entries.push_back(format!("{entrypoint}: {error}"));
    }

    pub(super) fn entries(&self, package: &str) -> Vec<String> {
        self.0
            .lock()
            .ok()
            .and_then(|packages| packages.get(package).cloned())
            .map_or_else(Vec::new, |entries| entries.into_iter().rev().collect())
    }

    pub(super) fn clear(&self, package: &str) {
        if let Ok(mut packages) = self.0.lock() {
            packages.remove(package);
        }
    }
}
