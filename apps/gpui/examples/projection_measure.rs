//! Synthetic GPUI presentation projection benchmark. Reads no user data.
use anywhere_gpui::{l10n::Locale, launcher::LauncherModel};
use anywhere_protocol::{
    Availability, CandidateKind, CandidatePresentation, IconDescriptor, LauncherState,
};
use serde_json::json;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

// This example runs on a single thread with no GPUI runtime. Observe the live
// requested Rust heap across projection, excluding the prebuilt Core snapshot.
struct ProjectionAllocator;
static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for ProjectionAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: ProjectionAllocator = ProjectionAllocator;

const CANDIDATES: usize = 10_000;
const SAMPLES: usize = 100;

fn candidate(index: usize) -> CandidatePresentation {
    CandidatePresentation {
        id: format!("benchmark.file.{index}"),
        kind: CandidateKind::File,
        is_menu: false,
        file_kind: Some(anywhere_protocol::FileObjectKind::File),
        title: format!("Report {index}.pdf"),
        localized_titles: Default::default(),
        subtitle: Some(format!("/synthetic/project/Report {index}.pdf")),
        shortcut: None,
        icon: IconDescriptor { key: "file".into() },
        score: 1.0,
        group_id: "benchmark".into(),
        availability: Availability::enabled(),
        command_id: Some("core.file.open".into()),
        bundle_id: None,
        owning_app: None,
        menu_path: Vec::new(),
        context_keys: Vec::new(),
        execution_context_id: Some(format!("file:{index}")),
        execution_target_label: None,
        primary_action: true,
        section_label: None,
    }
}

fn state(candidates: Arc<Vec<CandidatePresentation>>, snapshot_seq: u32) -> LauncherState {
    LauncherState {
        input: "operation".into(),
        file_search: Some(Default::default()),
        candidates,
        snapshot_seq,
        ..Default::default()
    }
}

fn timing(mut operation: impl FnMut(usize)) -> serde_json::Value {
    for index in 0..10 {
        operation(index + SAMPLES);
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    for index in 0..SAMPLES {
        let started = Instant::now();
        operation(index);
        samples.push(started.elapsed().as_nanos() as u64);
    }
    samples.sort_unstable();
    json!({
        "samples": SAMPLES,
        "p50_us": samples[SAMPLES / 2] as f64 / 1000.0,
        "p95_us": samples[SAMPLES * 95 / 100] as f64 / 1000.0,
        "max_us": samples[SAMPLES - 1] as f64 / 1000.0,
    })
}

fn main() {
    let candidates = Arc::new((0..CANDIDATES).map(candidate).collect::<Vec<_>>());
    let mut metadata_model = LauncherModel::preview(Locale::En);
    let initial = Arc::new(state(candidates.clone(), 0));
    let base = LIVE.load(Ordering::Relaxed);
    metadata_model.apply_shared(initial);
    let retained = LIVE.load(Ordering::Relaxed) as i64 - base as i64;
    metadata_model.select_row(CANDIDATES - 1);
    let metadata = timing(|snapshot_seq| {
        let state = state(candidates.clone(), snapshot_seq as u32 + 1);
        let result = metadata_model.apply_shared(Arc::new(state));
        assert!(!result.rows_changed);
        black_box(metadata_model.selection_id());
    });
    let rebuild = timing(|snapshot_seq| {
        let mut model = LauncherModel::preview(Locale::En);
        let result =
            model.apply_shared(Arc::new(state(candidates.clone(), snapshot_seq as u32 + 1)));
        assert!(result.rows_changed);
        assert_eq!(model.rows.len(), CANDIDATES);
        for index in 0..8 {
            black_box(model.rows.get(index).unwrap());
        }
        black_box(model);
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "scope": "synthetic release projection fixture; excludes GPUI layout, rasterization, Core work and native presentation",
            "candidates": CANDIDATES,
            "metadata_only": metadata,
            "snapshot_and_eight_rows": rebuild,
            "projection_retained_bytes_delta": retained,
        }))
        .expect("serialize projection measurement")
    );
}
