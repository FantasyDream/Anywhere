//! Bounded file reads shared by project recognizers; no project command execution.
use anyhow::{bail, Context, Result};
use std::{io::Read, path::Path};
pub const MAX_MANIFEST_BYTES: usize = 256 * 1024;

pub fn read_manifest(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path).context("project manifest could not be read")?;
    if !file.metadata()?.is_file() {
        bail!("project manifest must be a regular file");
    }
    let mut bytes = Vec::new();
    file.take((MAX_MANIFEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_MANIFEST_BYTES {
        bail!("project manifest exceeds the 256 KiB limit");
    }
    String::from_utf8(bytes).context("project manifest must be UTF-8")
}

pub fn encoded(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
