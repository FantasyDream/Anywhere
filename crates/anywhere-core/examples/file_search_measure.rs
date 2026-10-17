//! Read-only traversal/continuous-result probe. No Spotlight, Flutter, GUI or content reads.
//! Run a release binary under an external RSS sampler; `--baseline` loads no query.
use anywhere_core::features::filesystem::scan::ScanStore;
use anywhere_platform::FileSearchOptions;
use anywhere_protocol::FileSearchFilter;
use std::{collections::HashSet, path::PathBuf, time::Instant};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let root = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: file_search_measure ROOT QUERY | --baseline"))?;
    if root == "--baseline" {
        println!("{{\"baseline\":true}}");
        return Ok(());
    }
    let query = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("query missing"))?;
    let started = Instant::now();
    let store = ScanStore::new(
        &query,
        FileSearchFilter::All,
        FileSearchOptions {
            roots: vec![PathBuf::from(root)],
            include_hidden: true,
            ..Default::default()
        },
    )?;
    let mut first_ms = None;
    store.run(&|| false, |page| {
        if !page.hits.is_empty() && first_ms.is_none() {
            first_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
        }
        Ok(())
    })?;
    let complete_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut displayed = HashSet::new();
    let mut batch_count = 0;
    let mut reading_ms = 0.0;
    let mut slowest_batch_ms: f64 = 0.0;
    loop {
        let started = Instant::now();
        let batch = store.more(&displayed, &|| false)?;
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        reading_ms += elapsed;
        slowest_batch_ms = slowest_batch_ms.max(elapsed);
        if batch.hits.is_empty() {
            break;
        }
        batch_count += 1;
        for hit in batch.hits {
            anyhow::ensure!(displayed.insert(hit.path), "duplicate continuous result");
        }
    }
    let disk_bytes: u64 = std::fs::read_dir(store.workspace())?
        .map(|entry| entry?.metadata().map(|m| m.len()))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .sum();
    let progress = store.progress();
    anyhow::ensure!(
        displayed.len() as u64 == progress.confirmed_matches,
        "continuous reading omitted matches"
    );
    println!(
        "{}",
        serde_json::json!({
            "first_batch_ms":first_ms, "complete_ms":complete_ms,
            "scanned_entries":progress.scanned_entries,"matches":progress.confirmed_matches,
            "issues":progress.issue_count,"batches":batch_count,"returned":displayed.len(),
            "all_batches_ms":reading_ms,"slowest_batch_ms":slowest_batch_ms,"temporary_bytes":disk_bytes
        })
    );
    Ok(())
}
