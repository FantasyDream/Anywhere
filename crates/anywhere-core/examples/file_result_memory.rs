//! Isolated retained-cost probe for the file-result execution map.
//!
//! Counts requested Rust heap bytes only. SQLite/C, allocator overhead, Bridge,
//! Flutter, OS page state and GPU resources are excluded; this is not RSS.
use anyhow::{ensure, Result};
use anywhere_core::{
    application::builtin_registry, core::search::candidate::Candidate,
    features::filesystem::search::file_candidates,
};
use anywhere_platform::FileSearchHit;
use serde_json::json;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

struct CountingAllocator;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn added(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            added(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(pointer, layout, size) };
        if !next.is_null() {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            added(size);
        }
        next
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn start() -> usize {
    let live = LIVE.load(Ordering::Relaxed);
    PEAK.store(live, Ordering::Relaxed);
    live
}

fn peak(base: usize) -> usize {
    PEAK.load(Ordering::Relaxed).saturating_sub(base)
}

fn live(base: usize) -> usize {
    LIVE.load(Ordering::Relaxed).saturating_sub(base)
}

fn rows(count: usize) -> Vec<Candidate> {
    let registry = builtin_registry();
    let row = file_candidates(
        vec![FileSearchHit {
            path: PathBuf::from(format!("/synthetic/{}/report.txt", "x".repeat(48))),
            is_directory: false,
        }],
        "",
        &registry,
    )
    .into_iter()
    .next()
    .expect("built-in file target");
    (0..count)
        .map(|index| {
            let mut candidate = row.clone();
            candidate.presentation.id = format!("file-{index}");
            candidate
        })
        .collect()
}

fn main() -> Result<()> {
    let count = match std::env::args().nth(1) {
        Some(raw) => raw.parse::<usize>()?,
        None => 10_000,
    };
    ensure!(
        (1..=1_000_000).contains(&count),
        "row count must be between 1 and 1,000,000"
    );

    let rows_base = start();
    let rows = Arc::new(rows(count));
    let rows_peak = peak(rows_base);
    let rows_live = live(rows_base);

    let presentations_base = start();
    let presentations: Arc<Vec<_>> = Arc::new(
        rows.iter()
            .map(|candidate| candidate.presentation.clone())
            .collect(),
    );
    let presentations_peak = peak(presentations_base);
    let presentations_live = live(presentations_base);
    ensure!(presentations.len() == count);

    let base = start();
    let full_candidate_copy = rows.as_ref().clone();
    let old_peak = peak(base);
    let old_live = live(base);
    drop(full_candidate_copy);

    let base = start();
    let shared_arc = rows.clone();
    let arc_peak = peak(base);
    ensure!(Arc::ptr_eq(&rows, &shared_arc));
    drop(shared_arc);

    let base = start();
    let indices: HashMap<_, _> = rows
        .iter()
        .enumerate()
        .map(|(index, candidate)| (candidate.presentation.id.clone(), index))
        .collect();
    let index_peak = peak(base);
    let index_live = live(base);
    ensure!(indices.len() == count);
    drop(indices);

    let bounded_base = start();
    let window_rows: Arc<Vec<_>> = Arc::new(
        rows.iter()
            .take(anywhere_core::features::filesystem::scan::PAGE_SIZE)
            .cloned()
            .collect(),
    );
    let window_presentations: Arc<Vec<_>> = Arc::new(
        window_rows
            .iter()
            .map(|candidate| candidate.presentation.clone())
            .collect(),
    );
    let window_indices: HashMap<_, _> = window_rows
        .iter()
        .enumerate()
        .map(|(index, candidate)| (candidate.presentation.id.clone(), index))
        .collect();
    let bounded_peak = peak(bounded_base);
    let bounded_live = live(bounded_base);
    ensure!(window_rows.len() == count.min(anywhere_core::features::filesystem::scan::PAGE_SIZE));
    ensure!(window_presentations.len() == window_rows.len());
    ensure!(window_indices.len() == window_rows.len());
    drop(window_indices);
    drop(window_presentations);
    drop(window_rows);

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "scope": "Requested Rust heap bytes; excludes allocator overhead, C/SQLite, Bridge, Flutter, OS and GPU. Not RSS or footprint.",
            "file_rows": count,
            "presentation_rows": presentations.len(),
            "candidate_rows_peak_bytes": rows_peak,
            "candidate_rows_live_bytes": rows_live,
            "presentation_rows_peak_bytes": presentations_peak,
            "presentation_rows_live_bytes": presentations_live,
            "candidate_array_copy_peak_bytes": old_peak,
            "candidate_array_copy_live_bytes": old_live,
            "arc_share_peak_bytes": arc_peak,
            "execution_index_peak_bytes": index_peak,
            "execution_index_live_bytes": index_live,
            "bounded_window_rows": window_rows_len(count),
            "bounded_state_peak_bytes": bounded_peak,
            "bounded_state_live_bytes": bounded_live,
        }))?
    );
    Ok(())
}

fn window_rows_len(count: usize) -> usize {
    count.min(anywhere_core::features::filesystem::scan::PAGE_SIZE)
}
