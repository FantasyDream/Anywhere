//! Bounded metadata discovery under a known configuration root; no history reads.
use anyhow::{ensure, Result};
use std::{
    fs::{self, File},
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

const MAX_VISITS: usize = 512;

pub(crate) fn validate(patterns: &[String]) -> Result<()> {
    ensure!(patterns.len() <= 8, "too many source discovery patterns");
    for pattern in patterns {
        ensure!(
            !pattern.is_empty()
                && pattern.len() <= 512
                && !pattern.contains(['\\', '\0'])
                && !pattern.contains("**")
                && pattern
                    .split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
                && Path::new(pattern).components().count() <= 8
                && Path::new(pattern)
                    .components()
                    .all(|part| matches!(part, Component::Normal(_))),
            "source discovery path must stay below the configuration root"
        );
        glob::Pattern::new(pattern)?;
    }
    Ok(())
}

pub(crate) fn detect(root: &Path, patterns: &[String]) -> Option<PathBuf> {
    if !root.is_absolute() || validate(patterns).is_err() {
        return None;
    }
    let root = root.canonicalize().ok()?;
    let mut visits = 0;
    let mut files = Vec::new();
    for pattern in patterns {
        let mut paths = vec![root.clone()];
        for part in pattern.split('/') {
            let mut next = Vec::new();
            for parent in paths {
                if part.contains(['*', '?', '[']) {
                    let Ok(entries) = fs::read_dir(&parent) else {
                        continue;
                    };
                    let matcher = glob::Pattern::new(part).ok()?;
                    for entry in entries {
                        if visits >= MAX_VISITS {
                            break;
                        }
                        visits += 1;
                        let Ok(entry) = entry else { continue };
                        if matcher.matches(&entry.file_name().to_string_lossy()) {
                            next.push(entry.path());
                        }
                    }
                } else {
                    next.push(parent.join(part));
                }
            }
            // Canonical containment also prevents walking through escaping symlinks.
            paths = next
                .into_iter()
                .filter_map(|path| path.canonicalize().ok())
                .filter(|path| path.starts_with(&root))
                .collect();
        }
        for path in paths {
            if !fs::metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
                continue;
            }
            let Ok(file) = File::open(&path) else {
                continue;
            };
            let Ok(metadata) = file.metadata() else {
                continue;
            };
            if metadata.is_file() {
                files.push((metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH), path));
            }
        }
    }
    // Prefer the most recently used configuration; ties have a stable path order.
    files.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    files.into_iter().next().map(|(_, path)| path)
}
