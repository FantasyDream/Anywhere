//! In-memory diagnostics contain only a predefined category and manifest entrypoint.
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};

use super::background_runtime::{BackgroundError, BackgroundFailure};

const MAX_PER_PACKAGE: usize = 10;

#[derive(Debug, Clone, Default)]
pub(super) struct Diagnostics(Arc<Mutex<BTreeMap<String, VecDeque<String>>>>);

impl Diagnostics {
    pub(super) fn record(&self, package: &str, entrypoint: &str, error: BackgroundFailure) {
        self.record_detail(package, entrypoint, error, None);
    }

    pub(super) fn record_error(&self, package: &str, entrypoint: &str, error: &BackgroundError) {
        self.record_detail(package, entrypoint, error.failure, error.stack.as_deref());
    }

    fn record_detail(
        &self,
        package: &str,
        entrypoint: &str,
        error: BackgroundFailure,
        stack: Option<&str>,
    ) {
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
        let line = match stack.filter(|value| !value.is_empty()) {
            Some(stack) => format!("{entrypoint}: {error} ({stack})"),
            None => format!("{entrypoint}: {error}"),
        };
        entries.push_back(line);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_keeps_latest_ten_and_clears_by_package() {
        let diagnostics = Diagnostics::default();
        for index in 0..12 {
            diagnostics.record(
                "dev.example.pkg",
                &format!("entry_{index}"),
                BackgroundFailure::ScriptException,
            );
        }
        let entries = diagnostics.entries("dev.example.pkg");
        assert_eq!(entries.len(), MAX_PER_PACKAGE);
        assert_eq!(entries[0], "entry_11: SCRIPT_EXCEPTION");
        assert_eq!(entries[9], "entry_2: SCRIPT_EXCEPTION");

        diagnostics.clear("dev.example.pkg");
        assert!(diagnostics.entries("dev.example.pkg").is_empty());
    }
}
