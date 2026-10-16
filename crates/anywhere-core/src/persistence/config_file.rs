//! Shared bounded reads and single-file atomic replacement for configuration.
use anyhow::{ensure, Context, Result};
use std::{
    io::{Read, Write},
    path::Path,
};

pub(super) fn read_bounded(path: &Path, limit: usize) -> Result<Option<Vec<u8>>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("preferences unavailable"),
    };
    let mut raw = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut raw)?;
    Ok(Some(raw))
}

pub(super) fn write_json<T: serde::Serialize>(path: &Path, value: &T, limit: usize) -> Result<()> {
    prepare_json(path, value, limit)?
        .persist(path)
        .context("could not save preferences")?;
    Ok(())
}

/// Migration can create a new file, but cannot replace another instance's configuration.
pub(super) fn create_json<T: serde::Serialize>(
    path: &Path,
    value: &T,
    limit: usize,
) -> Result<bool> {
    match prepare_json(path, value, limit)?.persist_noclobber(path) {
        Ok(_) => Ok(true),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error).context("could not migrate preferences"),
    }
}

fn prepare_json<T: serde::Serialize>(
    path: &Path,
    value: &T,
    limit: usize,
) -> Result<tempfile::NamedTempFile> {
    let raw = serde_json::to_vec_pretty(value)?;
    ensure!(raw.len() <= limit, "preferences exceed {limit} bytes");
    let parent = path.parent().context("preferences need a directory")?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&raw)?;
    file.as_file().sync_all()?;
    Ok(file)
}
