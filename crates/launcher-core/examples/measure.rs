//! Repeatable local CPU timings; Finder/visible-window timings are in launcher.log.
use launcher_core::{
    command::CommandRegistry,
    input::InputEngine,
    ranking::{rank, UsageStats},
};
use std::time::Instant;
fn main() {
    let registry = CommandRegistry::builtin();
    for input in [
        "https://github.com",
        r#"{"name":"foo","age":18}"#,
        "Cargo.toml",
        ".",
        "hello world",
    ] {
        let mut resolve_times = Vec::new();
        let mut ranking_times = Vec::new();
        for sample in 0..1100 {
            let started = Instant::now();
            let contexts = InputEngine.resolve(std::hint::black_box(input));
            let resolved = started.elapsed();
            let started = Instant::now();
            std::hint::black_box(rank(&registry, &contexts, 1000, |_, _| {
                UsageStats::default()
            }));
            if sample >= 100 {
                resolve_times.push(resolved.as_nanos());
                ranking_times.push(started.elapsed().as_nanos());
            }
        }
        resolve_times.sort_unstable();
        ranking_times.sort_unstable();
        println!(
            "{input:?}: resolver p50={:.3}ms p95={:.3}ms; ranking p50={:.3}ms p95={:.3}ms",
            resolve_times[500] as f64 / 1e6,
            resolve_times[950] as f64 / 1e6,
            ranking_times[500] as f64 / 1e6,
            ranking_times[950] as f64 / 1e6
        );
    }
}
