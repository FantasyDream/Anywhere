//! Synthetic, isolated storage baseline. Never reads the user's clipboard or database.
use anyhow::{ensure, Context, Result};
use anywhere_core::{
    features::clipboard::{
        history::{HistoryRepository, MAX_ENTRIES},
        service::HistoryService,
    },
    persistence::clipboard::SqliteHistoryRepository,
};
use anywhere_platform::{
    clipboard::{ClipboardFormat, ClipboardPayload},
    ClipboardHost,
};
use serde_json::json;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Instant;

struct SyntheticClipboardHost {
    generation: AtomicI64,
}

impl ClipboardHost for SyntheticClipboardHost {
    fn copy_text(&self, _text: &str) -> Result<()> {
        Ok(())
    }
    fn clipboard_generation(&self) -> Result<i64> {
        Ok(self.generation.load(Ordering::Relaxed))
    }
    fn request_clipboard_history_access(&self) -> Result<()> {
        Ok(())
    }
    fn clipboard_history_payload(&self, generation: i64) -> Result<Option<ClipboardPayload>> {
        let mut bytes = vec![0x89u8; 2 * 1024 * 1024];
        bytes[..8].copy_from_slice(&generation.to_le_bytes());
        Ok(Some(ClipboardPayload {
            formats: vec![ClipboardFormat {
                name: "public.png".to_owned(),
                data: bytes,
            }],
        }))
    }
}

fn main() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("history.sqlite3");
    let mut repository = SqliteHistoryRepository::open(&database)?;
    let copied_at = anywhere_core::features::clipboard::policy::now_ms();
    let start = Instant::now();
    for index in 0..MAX_ENTRIES {
        repository.record(
            &format!("entry-{index:04} 中文 rust fixture {}", "x".repeat(7900)),
            copied_at + index as i64,
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
    let mut previews = anywhere_core::features::preview::PreviewStore::default();
    previews.configure(directory.path().join("previews"));
    let start = Instant::now();
    for _ in 0..100 {
        let resource = service
            .inspect_payload(selected, |item, payload| {
                previews.clipboard(&payload, item.summary)
            })?
            .context("fixture disappeared")?;
        previews.release(&resource.id);
    }
    let prepare_preview_mean_us = start.elapsed().as_micros() / 100;
    let mut policy_queries = Vec::new();
    for days in [0, 7] {
        service.set_retention_days(days)?;
        for query in ["", "中文 rust", "entry-0999", "no-such-entry"] {
            let mut samples = Vec::new();
            let mut matches = 0;
            for _ in 0..50 {
                let start = Instant::now();
                matches = service.search(query)?.len();
                samples.push(start.elapsed().as_micros());
            }
            samples.sort_unstable();
            policy_queries.push(json!({
                "retention_days": days, "query": query, "matches": matches,
                "samples": samples.len(), "p50_us": samples[25], "p95_us": samples[47],
            }));
        }
    }
    drop(service);
    let reopened = SqliteHistoryRepository::open(&database)?;
    ensure!(reopened.search("")?.len() == MAX_ENTRIES);

    // Continuous large-copy contention probe: 20 x 2 MiB image copies concurrently with UI status/snapshot.
    let service = HistoryService::new(Ok(Box::new(reopened)));
    let host = SyntheticClipboardHost {
        generation: AtomicI64::new(1),
    };
    service.request_recording(true, &host)?;
    let mut status_ms = Vec::new();
    let mut snapshot_ms = Vec::new();
    let mut poll_ms = std::thread::scope(|scope| -> Result<Vec<f64>> {
        let writer = scope.spawn(|| -> Result<Vec<f64>> {
            let mut times = Vec::with_capacity(20);
            for step in 2..=21 {
                host.generation.store(step, Ordering::Relaxed);
                let t0 = Instant::now();
                service.poll(&host, copied_at + 10_000 + step)?;
                times.push(t0.elapsed().as_secs_f64() * 1000.0);
            }
            Ok(times)
        });
        while !writer.is_finished() {
            let t0 = Instant::now();
            let _ = service.status()?;
            status_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
            let t1 = Instant::now();
            let _ = service.snapshot("", None)?;
            snapshot_ms.push(t1.elapsed().as_secs_f64() * 1000.0);
        }
        writer
            .join()
            .map_err(|_| anyhow::anyhow!("clipboard benchmark writer panicked"))?
    })?;
    poll_ms.sort_by(f64::total_cmp);
    status_ms.sort_by(f64::total_cmp);
    snapshot_ms.sort_by(f64::total_cmp);
    let pct = |samples: &[f64], percentile: usize| {
        samples
            .get((samples.len() * percentile / 100).min(samples.len().saturating_sub(1)))
            .copied()
    };
    let over_5ms = |samples: &[f64]| samples.iter().filter(|&&ms| ms > 5.0).count();

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "fixture": "1000 synthetic plain-text entries, approximately 8 KiB each",
            "rows": MAX_ENTRIES, "database_bytes": database.metadata()?.len(),
            "summary_utf8_bytes": summary_bytes, "insert_ms": insert_ms,
            "prepare_preview_mean_us": prepare_preview_mean_us, "queries": results,
            "service_policy_queries": policy_queries,
            "continuous_2mib_image_contention": {
                "poll_samples": poll_ms.len(),
                "poll_p50_ms": pct(&poll_ms, 50),
                "poll_p95_ms": pct(&poll_ms, 95),
                "poll_max_ms": poll_ms.last().copied(),
                "poll_over_5ms": over_5ms(&poll_ms),
                "status_samples": status_ms.len(),
                "status_p50_ms": pct(&status_ms, 50),
                "status_p95_ms": pct(&status_ms, 95),
                "status_max_ms": status_ms.last().copied(),
                "status_over_5ms": over_5ms(&status_ms),
                "snapshot_samples": snapshot_ms.len(),
                "snapshot_p50_ms": pct(&snapshot_ms, 50),
                "snapshot_p95_ms": pct(&snapshot_ms, 95),
                "snapshot_max_ms": snapshot_ms.last().copied(),
                "snapshot_over_5ms": over_5ms(&snapshot_ms),
            },
            "scope": "Rust repository queries and service previews; excludes GPUI and OS clipboard",
        }))?
    );
    Ok(())
}
