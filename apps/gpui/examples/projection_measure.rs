//! Synthetic GPUI presentation projection benchmark. Reads no user data.
use anywhere_gpui::{l10n::Locale, launcher::LauncherModel};
use anywhere_protocol::{
    Availability, CandidateKind, CandidatePresentation, IconDescriptor, LauncherState,
};
use serde_json::json;
use std::{hint::black_box, sync::Arc, time::Instant};

const CANDIDATES: usize = 10_000;
const SAMPLES: usize = 100;

fn candidate(index: usize) -> CandidatePresentation {
    CandidatePresentation {
        id: format!("benchmark.action.{index}"),
        kind: CandidateKind::Action,
        is_menu: false,
        file_kind: None,
        title: format!("Operation {index}"),
        localized_titles: Default::default(),
        subtitle: Some("Synthetic projection fixture".into()),
        shortcut: None,
        icon: IconDescriptor {
            key: "command".into(),
        },
        score: 1.0,
        group_id: "benchmark".into(),
        availability: Availability::enabled(),
        command_id: Some(format!("benchmark.action.{index}")),
        bundle_id: None,
        owning_app: None,
        menu_path: Vec::new(),
        context_keys: Vec::new(),
        execution_context_id: None,
        execution_target_label: None,
        primary_action: false,
        section_label: None,
    }
}

fn state(candidates: Arc<Vec<CandidatePresentation>>, snapshot_seq: u32) -> LauncherState {
    LauncherState {
        input: "operation".into(),
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
    metadata_model.apply_shared(Arc::new(state(candidates.clone(), 0)));
    let metadata = timing(|snapshot_seq| {
        let state = state(candidates.clone(), snapshot_seq as u32 + 1);
        let result = metadata_model.apply_shared(Arc::new(state));
        assert!(!result.rows_changed);
        black_box(metadata_model.rows.first());
    });
    let rebuild = timing(|snapshot_seq| {
        let mut model = LauncherModel::preview(Locale::En);
        let result =
            model.apply_shared(Arc::new(state(candidates.clone(), snapshot_seq as u32 + 1)));
        assert!(result.rows_changed);
        assert!(!model.rows.is_empty());
        black_box(model.rows);
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "scope": "synthetic release projection fixture; excludes GPUI layout, rasterization, Core work and native presentation",
            "candidates": CANDIDATES,
            "metadata_only": metadata,
            "full_projection": rebuild,
        }))
        .expect("serialize projection measurement")
    );
}
