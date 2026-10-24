//! Only bundled, reviewed pure transforms run here; no Host functions or loaders.
use crate::features::application_history::model::*;
use anyhow::{ensure, Result};
use rquickjs::{Context, Function, Module, Runtime};
use std::time::{Duration, Instant};

pub(super) fn transform(
    source: &str,
    input: &serde_json::Value,
    cancelled: Cancelled,
) -> Result<Vec<Entry>> {
    ensure!(!cancelled(), "transform cancelled");
    let input = serde_json::to_string(input)?;
    ensure!(input.len() <= MAX_INPUT, "transform input too large");
    let runtime = Runtime::new().map_err(|_| anyhow::anyhow!("script runtime unavailable"))?;
    runtime.set_memory_limit(32 * 1024 * 1024);
    runtime.set_max_stack_size(256 * 1024);
    let started = Instant::now();
    let interrupt = cancelled.clone();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        interrupt() || started.elapsed() > Duration::from_millis(250)
    })));
    let context =
        Context::full(&runtime).map_err(|_| anyhow::anyhow!("script context unavailable"))?;
    let output = context
        .with(|ctx| -> rquickjs::Result<String> {
            let helpers = rquickjs::Object::new(ctx.clone())?;
            helpers.set(
                "localPath",
                Function::new(ctx.clone(), |value: String| -> Option<String> {
                    let path = if value.starts_with("file:") {
                        url::Url::parse(&value).ok()?.to_file_path().ok()?
                    } else {
                        anywhere_platform::paths::expand_home(&value)?
                    };
                    path.is_absolute()
                        .then(|| path.to_string_lossy().into_owned())
                })?,
            )?;
            ctx.globals().set("history", helpers)?;
            let (module, ready) = Module::declare(ctx.clone(), "transform", source)?.eval()?;
            ready.finish::<()>()?;
            let transform: Function = module.get("transform")?;
            let bridge: Function =
                ctx.eval("(transform, input) => JSON.stringify(transform(JSON.parse(input)))")?;
            bridge.call((transform, input))
        })
        .map_err(|_| anyhow::anyhow!("source transform failed or exceeded its budget"))?;
    ensure!(
        output.len() <= MAX_OUTPUT && !cancelled(),
        "transform cancelled or output too large"
    );
    let entries: Vec<Entry> = serde_json::from_str(&output)?;
    ensure!(entries.len() <= MAX_ENTRIES, "too many history entries");
    let mut keys = std::collections::BTreeSet::new();
    for entry in &entries {
        ensure!(
            !entry.key.is_empty()
                && entry.key.len() <= 16 * 1024
                && !entry.title.trim().is_empty()
                && entry.title.len() <= 4096
                && entry.target.validate()
                && keys.insert(&entry.key),
            "invalid or duplicate history entry"
        );
        ensure!(
            entry
                .last_used_at
                .is_none_or(|time| (0..=253402300799000).contains(&time)),
            "invalid source timestamp"
        );
    }
    Ok(entries)
}
