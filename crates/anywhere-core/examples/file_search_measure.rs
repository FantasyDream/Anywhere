//! Read-only traversal/page probe. No Spotlight, Flutter, GUI or file content reads.
//! Run a release binary under an external RSS sampler; `--baseline` loads no query.
use anywhere_core::features::filesystem::scan::ScanStore;
use anywhere_platform::FileSearchOptions;
use anywhere_protocol::FileSearchFilter;
use std::{path::PathBuf, time::Instant};

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
    let disk_bytes: u64 = std::fs::read_dir(store.workspace())?
        .map(|entry| entry?.metadata().map(|m| m.len()))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .sum();
    let mut page = store.page(None, &|| false)?;
    let mut returned = page.hits.len();
    let mut page_count = 1;
    let mut paging_ms = 0.0;
    let mut slowest_page_ms: f64 = 0.0;
    while let Some(cursor) = page.progress.next_cursor {
        let started = Instant::now();
        page = store.page(Some(&cursor), &|| false)?;
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;
        paging_ms += elapsed;
        slowest_page_ms = slowest_page_ms.max(elapsed);
        returned += page.hits.len();
        page_count += 1;
    }
    let progress = store.progress();
    anyhow::ensure!(
        returned as u64 == progress.confirmed_matches,
        "pagination omitted matches"
    );
    println!(
        "{}",
        serde_json::json!({
            "first_batch_ms":first_ms, "complete_ms":complete_ms,
            "scanned_entries":progress.scanned_entries,"matches":progress.confirmed_matches,
            "issues":progress.issue_count,"pages":page_count,"returned":returned,
            "all_pages_ms":paging_ms,"slowest_next_page_ms":slowest_page_ms,"temporary_bytes":disk_bytes
        })
    );
    Ok(())
}
