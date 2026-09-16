//! Repeatable local CPU timings for the V0.2 hot path.
//!
//! Reports the three phases of a candidate build separately — action matching, application
//! matching and ranking — because the first two are matching work and only the third is
//! sorting. They used to be reported as one number under two names, which hid a 1.2s
//! regression in application matching behind the ranking label.
//!
//! Finder Automation, FFI, window rendering and RSS are not measurable here; use
//! `launcher.log` and the manual validation checklist for those.
use anywhere_core::{
    core::action::descriptor::CommandRegistry,
    core::search::engine::SearchEngine,
    features::applications::index::IndexedApplication,
    input::{text_context, InputEngine},
    provider::{repository_root, text_provider_contexts},
    query::Query,
    ranking::UsageStats,
    SessionFacts,
};
use std::time::Instant;

/// Percentiles of a sample vector in milliseconds.
fn percentiles(mut samples: Vec<u128>) -> (f64, f64) {
    samples.sort_unstable();
    let index = |percent: usize| {
        let position = (samples.len() * percent / 100).min(samples.len() - 1);
        samples[position] as f64 / 1e6
    };
    (index(50), index(95))
}

fn synthetic_catalog(count: usize) -> Vec<IndexedApplication> {
    (0..count)
        .map(|index| IndexedApplication {
            bundle_id: format!("com.example.application{index}"),
            name: format!("Example Application {index}"),
            path: format!("/Applications/Example Application {index}.app"),
            aliases: vec![format!("app{index}")],
            keywords: vec!["example".into()],
        })
        .collect()
}

fn main() {
    let registry = CommandRegistry::builtin();
    let catalog = synthetic_catalog(1000);
    let empty_usage = |_: &str, _: &str| UsageStats::default();
    let facts = SessionFacts {
        git_available: true,
        ..Default::default()
    };
    let engine = SearchEngine {
        registry: &registry,
        applications: &catalog,
        usage: &empty_usage,
        app_usage: &empty_usage,
        facts: &facts,
        now: 1000,
    };

    println!("samples: 100 warmup + 1000 measured, 1000 synthetic applications");
    for input in [
        "https://github.com",
        r#"{"name":"foo","age":18}"#,
        "Cargo.toml",
        ".",
        "hello world",
        "git sta",
        "chrome",
        &"x".repeat(1024 * 1024),
    ] {
        let label = if input.len() > 64 {
            format!("{} chars of text", input.len())
        } else {
            format!("{input:?}")
        };
        let mut recognize = Vec::new();
        let mut matcher_times = Vec::new();
        let mut apps_times = Vec::new();
        let mut ranking_times = Vec::new();
        for sample in 0..1100 {
            let started = Instant::now();
            let mut contexts = InputEngine.resolve(std::hint::black_box(input));
            if let Some(text) = text_context(input) {
                contexts.insert(0, text);
            }
            let recognized = started.elapsed();

            let query = Query::parse(input);
            let set = engine.search(&query, &contexts);

            if sample >= 100 {
                recognize.push(recognized.as_nanos());
                // Each phase is timed where it happens, so these three are additive parts
                // of `build` rather than one number reported under three names.
                matcher_times.push(set.matcher_us as u128 * 1000);
                apps_times.push(set.apps_us as u128 * 1000);
                ranking_times.push(set.ranking_us as u128 * 1000);
            }
        }
        let (recognize_p50, recognize_p95) = percentiles(recognize);
        let (matcher_p50, matcher_p95) = percentiles(matcher_times);
        let (apps_p50, apps_p95) = percentiles(apps_times);
        let (ranking_p50, ranking_p95) = percentiles(ranking_times);
        println!(
            "{label}: input_recognition p50={recognize_p50:.3}ms p95={recognize_p95:.3}ms; \
             matcher p50={matcher_p50:.3}ms p95={matcher_p95:.3}ms; \
             applications p50={apps_p50:.3}ms p95={apps_p95:.3}ms; \
             ranking p50={ranking_p50:.3}ms p95={ranking_p95:.3}ms"
        );
    }

    // Repository detection is metadata-only and must stay off the Git hot path.
    let repository = repository_root(&std::env::current_dir().unwrap());
    println!(
        "repository_root({:?}) = {repository:?}",
        std::env::current_dir().unwrap()
    );
    println!(
        "text provider contexts for \"hello world\": {}",
        text_provider_contexts("hello world").len()
    );
}
