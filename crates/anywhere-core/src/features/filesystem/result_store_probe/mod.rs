//! R2 experiment only. Reuses the real scanner's records, comparator and batch size.
//! The SQLite backend does not participate in discovery, authorization or UI state.
mod sqlite;

use super::scan::{write_record, ScanStore};
use anywhere_platform::FileSearchOptions;
use anywhere_protocol::FileSearchFilter;
use std::{fs::OpenOptions, io::Write, path::PathBuf};

fn store() -> ScanStore {
    ScanStore::new(
        "report",
        FileSearchFilter::All,
        FileSearchOptions::default(),
    )
    .unwrap()
}

fn append(store: &ScanStore, paths: impl IntoIterator<Item = (PathBuf, bool)>) {
    let mut file = OpenOptions::new()
        .append(true)
        .open(store.matches_path())
        .unwrap();
    let mut bytes = 0;
    for (path, directory) in paths {
        bytes += write_record(&mut file, &path, directory).unwrap();
    }
    file.flush().unwrap();
    store.progress.lock().unwrap().boundary += bytes;
}

mod benchmark;
#[cfg(test)]
mod tests;
