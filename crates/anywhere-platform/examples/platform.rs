//! Read-only by default. Explicit subcommands opt into OS side effects.
use anyhow::{bail, Result};
use anywhere_platform::{native_platform, ApplicationInfo};
use std::path::Path;

fn main() -> Result<()> {
    let platform = native_platform()?;
    let args: Vec<String> = std::env::args().collect();
    let value = args.get(2).map(String::as_str).unwrap_or("");
    match args.get(1).map(String::as_str).unwrap_or("capture") {
        "capture" => {
            let app = platform.frontmost_application()?;
            println!("frontmost={app:?}");
            if let Some(app) = app {
                println!("directory={:?}", platform.application_directory(&app)?);
            }
        }
        "finder" => println!(
            "{:?}",
            platform.application_directory(&ApplicationInfo {
                bundle_id: "com.apple.finder".into(),
                name: "Finder".into(),
                pid: 0,
                instance_id: None,
            })?
        ),
        "url" => platform.open_url(value)?,
        "file" => platform.open_file(Path::new(value))?,
        "reveal" => platform.reveal_file(Path::new(value))?,
        "directory" => platform.open_directory(Path::new(value))?,
        "terminal" => platform.open_terminal(Path::new(value))?,
        "copy" => platform.copy_text(value)?,
        "search-files" | "search-files-benchmark" => {
            let options = anywhere_platform::FileSearchOptions {
                restrict_to_roots: false,
                roots: args.get(3).map(|p| vec![p.into()]).unwrap_or_default(),
                excluded_directory_names: args.get(4).map(|n| vec![n.clone()]).unwrap_or_default(),
                engine: match args.get(5).map(String::as_str) {
                    Some("ripgrep") => anywhere_platform::FileSearchEngine::Ripgrep,
                    Some("combined") => anywhere_platform::FileSearchEngine::Combined,
                    _ => anywhere_platform::FileSearchEngine::SystemIndex,
                },
                include_hidden: args.get(6).is_some_and(|v| v == "hidden"),
            };
            let benchmark = args[1] == "search-files-benchmark";
            let runs = if benchmark { 12 } else { 1 };
            let mut elapsed = Vec::new();
            let mut counts = Vec::new();
            for _ in 0..runs {
                let started = std::time::Instant::now();
                let hits = platform.search_files_with_options(value, 64, &options, &|| false)?;
                elapsed.push(started.elapsed().as_secs_f64() * 1000.0);
                counts.push(hits.len());
                assert!(hits.len() <= 64 && hits.iter().all(|hit| hit.path.is_absolute() && hit.path.exists()));
            }
            if benchmark {
                let first_ms = elapsed[0];
                let mut warm = elapsed[1..].to_vec();
                warm.sort_by(f64::total_cmp);
                println!("{}", serde_json::json!({
                    "runs": runs, "first_ms": first_ms,
                    "warm_median_ms": warm[warm.len() / 2], "warm_p95_ms": warm[warm.len() - 1],
                    "hit_counts": counts, "default_open_invoked": false,
                }));
            } else {
                println!("file_search_hits={}", counts[0]);
            }
        }
        _ => bail!("capture | finder | search-files | search-files-benchmark | url | file | reveal | directory | terminal | copy [value]"),
    }
    Ok(())
}
