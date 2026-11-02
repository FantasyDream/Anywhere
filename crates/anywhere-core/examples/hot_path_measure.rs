//! Isolated CPU/storage fixtures; never reads native menus or the user's clipboard.
use anyhow::Result;
use anywhere_core::{
    core::{
        action::{
            availability::AvailabilityRule,
            descriptor::{ActionDefinition, ActionExecutor, ActionRegistry},
            matcher::ActionMatcher,
        },
        context::{ContextValue, ResolvedContext},
        search::query::Query,
        session::facts::SessionFacts,
    },
    features::clipboard::history::HistoryRepository,
    persistence::clipboard::SqliteHistoryRepository,
};
use anywhere_platform::clipboard::{ClipboardFormat, ClipboardPayload};
use anywhere_protocol::{ContextMatcher, ContextSource, DeclarativeAction, APPLICATION};
use serde_json::{json, Value};
use std::time::Instant;

fn measure(mut operation: impl FnMut() -> Result<()>) -> Result<Value> {
    let mut samples = Vec::new();
    for index in 0..110 {
        let started = Instant::now();
        operation()?;
        if index >= 10 {
            samples.push(started.elapsed().as_nanos() as u64);
        }
    }
    samples.sort_unstable();
    Ok(json!({"samples":100,"p50_us":samples[50] as f64/1000.,"p95_us":samples[94] as f64/1000.}))
}
fn main() -> Result<()> {
    let mut registry = ActionRegistry::default();
    for index in 0..1000 {
        registry.register(ActionDefinition {
            id: format!("bench.menu.{index}"),
            title: format!("Operation {index}"),
            localized_titles: Default::default(),
            subtitle: None,
            shortcut: None,
            accepts: vec![ContextMatcher {
                type_id: APPLICATION.into(),
            }],
            executor: ActionExecutor::Declarative(DeclarativeAction::Copy {
                text: String::new(),
            }),
            availability: AvailabilityRule::Enabled,
            base_priority: 0,
            icon: "menu".into(),
            group_id: "bench".into(),
            aliases: vec![],
            keywords: vec![],
            owning_app: Some("dev.example.editor".into()),
            menu_path: vec![],
            primary_action: true,
            search_title: None,
        })?;
    }
    let contexts = [ResolvedContext::new(
        APPLICATION,
        ContextValue::Application {
            bundle_id: "dev.example.editor".into(),
            name: "Example Editor".into(),
        },
        ContextSource::ForegroundApplication,
        "synthetic",
    )];
    let facts = SessionFacts::default();
    let usage = |_: &str, _: &str| Default::default();
    let matcher = ActionMatcher::new(&registry, &usage, &facts, 1000, |_| 0);
    let mut matching = Vec::new();
    for text in ["example operation", "operation", "missing"] {
        let query = Query::parse(text);
        let expected = if text == "missing" { 0 } else { 1000 };
        let timing = measure(|| {
            let rows = matcher.match_actions(std::hint::black_box(&query), &contexts);
            anyhow::ensure!(rows.len() == expected, "unexpected matching results");
            std::hint::black_box(rows);
            Ok(())
        })?;
        matching.push(json!({"query":text,"timing":timing}));
    }
    let root = tempfile::tempdir()?;
    let mut history = SqliteHistoryRepository::open(&root.path().join("fixture.sqlite3"))?;
    let payload = ClipboardPayload {
        formats: vec![ClipboardFormat {
            name: "public.png".into(),
            data: vec![42; 2 * 1024 * 1024],
        }],
    };
    let id = history.record_payload(&payload, 1)?.unwrap();
    let duplicate = measure(|| {
        anyhow::ensure!(history.record_payload(std::hint::black_box(&payload), 2)? == Some(id));
        Ok(())
    })?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "scope":"Release Rust matcher and isolated SQLite duplicate capture; excludes OS, Bridge, Flutter, GPU and process memory",
            "os":std::env::consts::OS,"arch":std::env::consts::ARCH,
            "menu_actions":1000,"clipboard_bytes":2*1024*1024,
            "matching":matching,"duplicate_clipboard":duplicate,
        }))?
    );
    Ok(())
}
