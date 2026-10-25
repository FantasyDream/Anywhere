//! Synthetic source fixtures only; never opens an installed app's private history.
use anyhow::{ensure, Result};
use anywhere_core::{
    application::application_history::SourceCatalog,
    features::application_history::service::HistoryService,
    persistence::module_storage::ModuleStorageHost,
};
use serde_json::json;
use std::{path::Path, time::Instant};

fn fixture(directory: &Path) -> Result<HistoryService> {
    let sources = directory.join("sources");
    std::fs::create_dir_all(&sources)?;
    let mut xml = String::from("<application><component name=\"RecentProjectsManager\"><option name=\"additionalInfo\"><map>");
    let mut recent = Vec::new();
    for index in 0..500 {
        let project = sources.join(format!("project-{index:04}"));
        std::fs::create_dir_all(&project)?;
        let target = project
            .to_string_lossy()
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;");
        xml.push_str(&format!("<entry key=\"{target}\"><value><RecentProjectMetaInfo><option name=\"displayName\" value=\"配置验收 {index:04}\"/></RecentProjectMetaInfo></value></entry>"));
        recent.push(json!({"folderUri":url::Url::from_file_path(project).unwrap().as_str(),"label":format!("配置验收 {index:04}")}));
    }
    xml.push_str("</map></option></component></application>");
    let idea = sources.join("recentProjects.xml");
    std::fs::write(&idea, xml)?;
    let vscode = sources.join("state.vscdb");
    let db = rusqlite::Connection::open(&vscode)?;
    db.execute_batch("CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value TEXT)")?;
    db.execute(
        "INSERT INTO ItemTable VALUES (?1, ?2)",
        rusqlite::params![
            "history.recentlyOpenedPathsList",
            json!({"entries":recent}).to_string()
        ],
    )?;
    drop(db);
    let storage = ModuleStorageHost::open(&directory.join("extension-storage.sqlite3"))?
        .bind("application_history")?;
    let service = HistoryService::new(
        Ok(storage),
        SourceCatalog::bundled_with_config_directory(None)?,
    );
    service.refresh()?;
    service.configure("idea", Some(idea.to_str().unwrap()))?;
    service.configure("vscode", Some(vscode.to_str().unwrap()))?;
    Ok(service)
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 2 && args[0] == "--prepare" {
        let path = std::path::PathBuf::from(&args[1]);
        ensure!(
            path.is_absolute() && !path.exists(),
            "fixture directory must be new and absolute"
        );
        std::fs::create_dir(&path)?;
        fixture(&path)?;
        println!(
            "{}",
            json!({"fixture_prepared":true,"sources":2,"entries_per_source":500})
        );
        return Ok(());
    }
    ensure!(
        args.is_empty(),
        "usage: application_history_measure [--prepare NEW_ABSOLUTE_DIRECTORY]"
    );
    let directory = tempfile::tempdir()?;
    let service = fixture(directory.path())?;
    let owner = service.open_session()?;
    let mut output = Vec::new();
    for (query, expected) in [
        ("", 1000),
        ("配置", 1000),
        ("pzys", 1000),
        ("project-0042", 2),
        ("no-matching-history", 0),
    ] {
        let mut times = Vec::new();
        for _ in 0..30 {
            let started = Instant::now();
            let result = service.search(owner, query, "")?;
            ensure!(
                result.notice.is_none() && result.items.len() == expected,
                "synthetic source query mismatch"
            );
            times.push(started.elapsed().as_micros() as u64);
        }
        times.sort_unstable();
        output.push(json!({"query_kind":if query.is_empty(){"empty"}else if query=="配置"{"literal"}else if query=="pzys"{"pinyin"}else if expected==0{"unmatched"}else{"path"},"samples":times.len(),"rows":expected,"p50_us":times[times.len()/2],"p95_us":times[(times.len()*95).div_ceil(100)-1],"max_us":times[times.len()-1]}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"scope":"Rust service, source reads, QuickJS transforms and matching; excludes Bridge/Flutter/physical display", "synthetic_entries":1000,"private_history_used":false,"queries":output})
        )?
    );
    Ok(())
}
