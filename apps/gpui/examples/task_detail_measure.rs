//! Synthetic task-detail clone costs. Excludes GPUI layout, native work and RSS.
use anywhere_gpui::host::{ExecutionDetail, ExecutionRow};
use anywhere_protocol::ExecutionSessionStatus;
use serde_json::json;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

const STREAM_BYTES: usize = 128 * 1024;
const SAMPLES: usize = 1_000;
static REQUESTED: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

// Every allocation is delegated to System; the counter observes requested sizes only.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            REQUESTED.fetch_add(layout.size(), Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(pointer, layout, size) };
        if !next.is_null() {
            REQUESTED.fetch_add(size, Ordering::Relaxed);
        }
        next
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn main() {
    let detail = ExecutionDetail {
        row: ExecutionRow {
            id: "synthetic-task".into(),
            title: "Synthetic build".into(),
            directory: "/synthetic/project".into(),
            command: "synthetic build".into(),
            status: ExecutionSessionStatus::Running,
            output_revision: 1,
            exit_code: None,
            started_at_ms: 0,
            finished_at_ms: None,
            output_truncated: false,
            error: None,
        },
        stdout: "o".repeat(STREAM_BYTES).into(),
        stderr: "e".repeat(STREAM_BYTES).into(),
    };
    for _ in 0..100 {
        black_box(detail.clone());
    }
    let mut times = [0_u64; SAMPLES];
    let before = REQUESTED.load(Ordering::Relaxed);
    for sample in &mut times {
        let started = Instant::now();
        black_box(detail.clone());
        *sample = started.elapsed().as_nanos() as u64;
    }
    let bytes = REQUESTED.load(Ordering::Relaxed) - before;
    times.sort_unstable();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "scope": "Synthetic ExecutionDetail clone only; requested Rust allocation bytes, not RSS or physical footprint. Excludes Core reads, GPUI layout, text shaping, native presentation and clipboard writes.",
            "stream_bytes": STREAM_BYTES,
            "samples": SAMPLES,
            "requested_bytes_per_clone": bytes / SAMPLES,
            "p50_us": times[SAMPLES / 2] as f64 / 1000.0,
            "p95_us": times[SAMPLES * 95 / 100] as f64 / 1000.0,
            "max_us": times[SAMPLES - 1] as f64 / 1000.0,
        }))
        .expect("serialize task detail measurement")
    );
}
