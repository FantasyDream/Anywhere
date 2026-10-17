//! Synthetic, isolated storage baseline. Never reads the user's clipboard or database.
use anyhow::{ensure, Result};
use anywhere_core::{
    features::clipboard::{
        history::{HistoryRepository, MAX_ENTRIES},
        service::HistoryService,
    },
    persistence::clipboard::SqliteHistoryRepository,
};
use serde_json::json;
use std::time::Instant;

fn main() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("history.sqlite3");
    let mut repository = SqliteHistoryRepository::open(&database)?;
    let start = Instant::now();
    for index in 0..MAX_ENTRIES {
        repository.record(
            &format!("entry-{index:04} 中文 rust fixture {}", "x".repeat(7900)),
            index as i64,
        )?;
    }
    let insert_ms = start.elapsed().as_millis();
    let entries = repository.search("")?;
    ensure!(
        entries.len() == MAX_ENTRIES,
        "fixture did not reach capacity"
    );
    let summary_bytes: usize = entries.iter().map(|row| row.summary.len()).sum();
    let selected = entries[0].id;
    drop(entries);
    let mut results = Vec::new();
    for query in ["", "entry", "中文 rust", "entry-0999", "no-such-entry"] {
        let mut samples = Vec::new();
        let mut matches = 0;
        for _ in 0..50 {
            let start = Instant::now();
            matches = repository.search(query)?.len();
            samples.push(start.elapsed().as_micros());
        }
        samples.sort_unstable();
        results.push(json!({
            "query": query, "matches": matches, "samples": samples.len(),
            "p50_us": samples[25], "p95_us": samples[47], "max_us": samples[49],
        }));
    }
    let service = HistoryService::new(Ok(Box::new(repository)));
    let start = Instant::now();
    for _ in 0..100 {
        ensure!(service.detail(selected)?.is_some());
    }
    let detail_mean_us = start.elapsed().as_micros() / 100;
    drop(service);
    let reopened = SqliteHistoryRepository::open(&database)?;
    ensure!(reopened.search("")?.len() == MAX_ENTRIES);
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "fixture": "1000 synthetic plain-text entries, approximately 8 KiB each",
            "rows": MAX_ENTRIES, "database_bytes": database.metadata()?.len(),
            "summary_utf8_bytes": summary_bytes, "insert_ms": insert_ms,
            "detail_mean_us": detail_mean_us, "queries": results,
            "scope": "Rust repository queries and service previews; excludes Flutter, bridge and OS clipboard",
        }))?
    );
    Ok(())
}
