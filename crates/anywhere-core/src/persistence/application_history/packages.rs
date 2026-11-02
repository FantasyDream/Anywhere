//! Read user-owned source definitions at startup. Never execute their scripts here.
use anyhow::{ensure, Result};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

pub(crate) struct SourceFiles {
    pub manifest: String,
    pub script: Option<String>,
}

fn text(root: &Path, path: &Path) -> Result<String> {
    let path = path.canonicalize()?;
    ensure!(path.starts_with(root), "source file escapes its directory");
    let metadata = fs::metadata(&path)?;
    ensure!(
        metadata.is_file() && metadata.len() <= 64 * 1024,
        "source definition too large or not a file"
    );
    let mut file = File::open(path)?;
    let mut output = String::new();
    (&mut file)
        .take(64 * 1024 + 1)
        .read_to_string(&mut output)?;
    let after = file.metadata()?;
    ensure!(
        output.len() <= 64 * 1024
            && metadata.len() == after.len()
            && metadata.modified()? == after.modified()?,
        "source definition changed while reading"
    );
    Ok(output)
}

pub(crate) fn read(root: &Path) -> Result<Vec<SourceFiles>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(error.into()),
    };
    let root = root.canonicalize()?;
    let mut directories = Vec::new();
    for (index, entry) in entries.enumerate() {
        ensure!(index < 64, "too many user source directory entries");
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with('.') || !entry.file_type()?.is_dir() {
            continue;
        }
        directories.push(entry.path());
        ensure!(directories.len() <= 32, "too many user source packages");
    }
    directories.sort();
    let mut total = 0;
    let mut files = Vec::new();
    for directory in directories {
        let manifest = text(&root, &directory.join("source.json"))?;
        let script_path = directory.join("transform.js");
        let script = match fs::symlink_metadata(&script_path) {
            Ok(_) => Some(text(&root, &script_path)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        total += manifest.len() + script.as_ref().map_or(0, String::len);
        ensure!(
            total <= 1024 * 1024,
            "user source definitions exceed their budget"
        );
        files.push(SourceFiles { manifest, script });
    }
    Ok(files)
}
