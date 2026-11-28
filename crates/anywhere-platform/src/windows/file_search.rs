//! Windows file search orchestration across Everything, Windows Search, Ripgrep, and Direct Scan.
use super::{everything::EverythingBackend, windows_search::WindowsSearchBackend};
use crate::{
    file_search::{self, RipgrepBackend},
    FileIndexResults, FileSearchBackend, FileSearchEngine, FileSearchOptions, FileSearchQuery,
};
use anyhow::Result;
use std::{path::PathBuf, time::Duration};

pub(crate) const SUPPORTED_ENGINES: &[FileSearchEngine] = &[
    FileSearchEngine::Combined,
    FileSearchEngine::Everything,
    FileSearchEngine::SystemIndex,
    FileSearchEngine::Ripgrep,
    FileSearchEngine::DirectScan,
];

pub(crate) const DEFAULT_ENGINE: FileSearchEngine = FileSearchEngine::Combined;

const DEFAULT_LIMIT: usize = 64;
const INDEX_TIMEOUT: Duration = Duration::from_millis(1500);
const COMBINED_RIPGREP_TIMEOUT: Duration = Duration::from_millis(1200);

pub(super) fn search_provisional(
    query: &str,
    roots: &[PathBuf],
    options: &FileSearchOptions,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<FileIndexResults> {
    if cancelled() || roots.is_empty() || options.engine == FileSearchEngine::DirectScan {
        return Ok(FileIndexResults::default());
    }
    let request = FileSearchQuery {
        text: query,
        roots,
        limit: DEFAULT_LIMIT,
        options,
        timeout: INDEX_TIMEOUT,
    };
    match options.engine {
        FileSearchEngine::DirectScan => Ok(FileIndexResults::default()),
        FileSearchEngine::Everything => EverythingBackend.search(&request, cancelled),
        FileSearchEngine::SystemIndex => WindowsSearchBackend.search(&request, cancelled),
        FileSearchEngine::Ripgrep => RipgrepBackend.search(&request, cancelled),
        FileSearchEngine::Combined => search_combined(&request, cancelled),
    }
}

fn search_combined(
    request: &FileSearchQuery<'_>,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<FileIndexResults> {
    // 1. Prefer Everything when running: instant NTFS index covering arbitrary scopes.
    if EverythingBackend.is_available() {
        if let Ok(results) = EverythingBackend.search(request, cancelled) {
            if !results.hits.is_empty() || cancelled() {
                return Ok(results);
            }
        }
    }
    if cancelled() {
        return Ok(FileIndexResults::default());
    }
    // 2. Query native Windows Search index and Ripgrep concurrently when available.
    let has_rg = RipgrepBackend.is_available();
    if !has_rg {
        return WindowsSearchBackend.search(request, cancelled);
    }
    let (index_result, rg_result) = std::thread::scope(|scope| {
        let index = scope.spawn(|| WindowsSearchBackend.search(request, cancelled));
        let rg_request = FileSearchQuery {
            timeout: COMBINED_RIPGREP_TIMEOUT,
            ..*request
        };
        let rg = scope.spawn(move || RipgrepBackend.search(&rg_request, cancelled));
        (
            index
                .join()
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Windows Search worker failed")))
                .map(|r| r.hits),
            rg.join()
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Ripgrep worker failed")))
                .map(|r| r.hits),
        )
    });
    if cancelled() {
        return Ok(FileIndexResults::default());
    }
    let hits = file_search::merge(index_result, rg_result, request.limit)?;
    Ok(FileIndexResults {
        truncated: hits.len() >= request.limit,
        hits,
    })
}
