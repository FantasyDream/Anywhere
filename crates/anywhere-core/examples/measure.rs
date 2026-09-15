//! Repeatable local CPU timings for the V0.2 hot path.
//!
//! Covers input recognition, candidate construction and ranking with a realistic
//! application catalog. Finder Automation, FFI, window rendering and RSS are not
//! measurable here; use `launcher.log` and the manual validation checklist for those.
use anywhere_core::{
    candidates::{IndexedApplication, Matcher, SessionFacts},
    command::CommandRegistry,
    input::{text_context, InputEngine},
    provider::{repository_root, text_provider_contexts},
    query::Query,
    ranking::UsageStats,
};
use std::time::Instant;

/// Ranking is timed on its own, so the sample vector append is excluded.
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
    let matcher = Matcher {
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
        let mut ranking_times = Vec::new();
        for sample in 0..1100 {
            let started = Instant::now();
            let mut contexts = InputEngine.resolve(std::hint::black_box(input));
            if let Some(text) = text_context(input) {
                contexts.insert(0, text);
            }
            let recognized = started.elapsed();

            let query = Query::parse(input);
            let started = Instant::now();
            let set = matcher.build(&query, &contexts);
            let matched = started.elapsed();
            let ranked = set.ranking_us;

            if sample >= 100 {
                recognize.push(recognized.as_nanos());
                matcher_times.push(matched.as_nanos());
                ranking_times.push(ranked as u128 * 1000);
            }
        }
        let (recognize_p50, recognize_p95) = percentiles(recognize);
        let (matcher_p50, matcher_p95) = percentiles(matcher_times);
        let (ranking_p50, ranking_p95) = percentiles(ranking_times);
        println!(
            "{label}: input_recognition p50={recognize_p50:.3}ms p95={recognize_p95:.3}ms; \
             matcher p50={matcher_p50:.3}ms p95={matcher_p95:.3}ms; \
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
