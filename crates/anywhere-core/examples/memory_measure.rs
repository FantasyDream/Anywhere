//! Counts requested Rust heap bytes in isolated fixtures. Excludes C/SQLite, VM,
//! Flutter, GPU and allocator overhead; this is not process RSS or footprint.
use anyhow::{ensure, Result};
use anywhere_core::{
    features::{
        application_history::{model::*, service::HistoryService},
        clipboard::history::HistoryRepository,
    },
    persistence::{clipboard::SqliteHistoryRepository, module_storage::ModuleStorageHost},
};
use anywhere_platform::clipboard::{ClipboardFormat, ClipboardPayload};
use serde_json::json;
use std::{
    alloc::{GlobalAlloc, Layout, System},
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
        unsafe {
            System.dealloc(pointer, layout);
        }
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
struct Source;
impl SourceLoader for Source {
    fn descriptors(&self) -> Vec<SourceDescriptor> {
        vec![SourceDescriptor {
            id: "bench".into(),
            title: "Synthetic".into(),
            file_hint: String::new(),
            applications: vec!["bench.app".into()],
            action: SourceAction {
                title: "History".into(),
                localized_titles: Default::default(),
                aliases: vec![],
            },
            built_in: false,
        }]
    }
    fn detect(&self, _: &str) -> Option<PathBuf> {
        Some("/synthetic/source.json".into())
    }
    fn read(&self, _: &SourcePreference, _: Cancelled) -> Result<Vec<Entry>> {
        Ok((0..1024)
            .map(|i| Entry {
                key: i.to_string(),
                title: format!("Project {i}"),
                target: Target::Directory {
                    path: PathBuf::from(format!("/synthetic/{}/{i}", "x".repeat(1000))),
                },
                last_used_at: None,
            })
            .collect())
    }
}
fn main() -> Result<()> {
    let root = tempfile::tempdir()?;
    let mut repo = SqliteHistoryRepository::open(&root.path().join("clipboard.sqlite3"))?;
    let payload = ClipboardPayload {
        formats: vec![ClipboardFormat {
            name: "application/synthetic".into(),
            data: vec![42; 16 * 1024 * 1024],
        }],
    };
    let id = repo.record_payload(&payload, 1)?.unwrap();
    let base = start();
    ensure!(repo.record_payload(&payload, 2)? == Some(id));
    let clipboard_peak = peak(base);
    drop(repo);
    drop(payload);
    let storage = ModuleStorageHost::open(&root.path().join("modules.sqlite3"))?.bind("history")?;
    let service = HistoryService::new(Ok(storage), Arc::new(Source))
        .with_installed_applications(["bench.app".into()]);
    let owner = service.open_session()?;
    let base = start();
    let result = service.search(owner, "", "bench")?;
    let history_peak = peak(base);
    ensure!(result.items.len() == 1024);
    let history_live = LIVE.load(Ordering::Relaxed).saturating_sub(base);
    drop(result);
    service.cancel_session(owner);
    let history_idle = LIVE.load(Ordering::Relaxed).saturating_sub(base);
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "scope":"Requested Rust heap bytes only; excludes SQLite/C allocations, allocator overhead, VM, OS, Bridge, Flutter and GPU. Not RSS or physical footprint.",
            "clipboard_payload_bytes":16*1024*1024,"duplicate_clipboard_peak_extra_bytes":clipboard_peak,
            "history_entries":1024,"history_query_peak_extra_bytes":history_peak,
            "history_result_and_service_live_extra_bytes":history_live,"history_after_result_drop_and_cancel_extra_bytes":history_idle,
        }))?
    );
    Ok(())
}
