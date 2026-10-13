//! One bounded Spotlight filename query. No shell, private index or content read.
use crate::FileSearchHit;
use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeSet,
    io::Read,
    os::fd::AsRawFd,
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const MAX_BYTES: usize = 256 * 1024;
const MAX_PATH_BYTES: usize = 16 * 1024;
const TIMEOUT: Duration = Duration::from_secs(2);

pub(super) fn search(
    query: &str,
    limit: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<FileSearchHit>> {
    if query.len() > 256
        || query.trim().chars().count() < 2
        || query.chars().any(char::is_control)
        || limit == 0
        || cancelled()
    {
        return Ok(Vec::new());
    }
    search_with_options(
        query,
        limit,
        &crate::FileSearchOptions::default(),
        cancelled,
    )
}

pub(super) fn search_with_options(
    query: &str,
    limit: usize,
    options: &crate::FileSearchOptions,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<FileSearchHit>> {
    use crate::FileSearchEngine;
    match options.engine {
        FileSearchEngine::SystemIndex => index_search(query, limit, options, cancelled, TIMEOUT),
        FileSearchEngine::Ripgrep => {
            crate::file_search::ripgrep(query, limit, options, cancelled, TIMEOUT)
        }
        FileSearchEngine::Combined => {
            let index = index_search(query, limit, options, cancelled, Duration::from_millis(700));
            if cancelled() {
                return Ok(Vec::new());
            }
            let scan = crate::file_search::ripgrep(
                query,
                limit,
                options,
                cancelled,
                Duration::from_millis(1600),
            );
            if cancelled() {
                return Ok(Vec::new());
            }
            crate::file_search::merge(index, scan, limit)
        }
    }
}

fn index_search(
    query: &str,
    limit: usize,
    options: &crate::FileSearchOptions,
    cancelled: &(dyn Fn() -> bool + Sync),
    timeout: Duration,
) -> Result<Vec<FileSearchHit>> {
    if query.len() > 256
        || query.trim().chars().count() < 2
        || query.chars().any(char::is_control)
        || limit == 0
        || cancelled()
    {
        return Ok(Vec::new());
    }
    let roots = crate::file_search::roots(options)?;
    let deadline = Instant::now() + timeout;
    let mut hits = Vec::new();
    let mut seen = BTreeSet::new();
    for root in roots {
        if cancelled() {
            return Ok(Vec::new());
        }
        let root = root
            .canonicalize()
            .context("search directory unavailable")?;
        if !root.is_dir() {
            bail!("search root is not a directory");
        }
        let mut command = Command::new("/usr/bin/mdfind");
        command
            .args(["-0", "-onlyin"])
            .arg(&root)
            .arg("-name")
            .arg(query.trim());
        let remaining = deadline.saturating_duration_since(Instant::now());
        let batch = collect_configured(
            command,
            limit.min(64),
            cancelled,
            remaining,
            options,
            Some(&root),
        )?;
        for hit in batch {
            if seen.insert(hit.path.clone()) {
                hits.push(hit);
            }
            if hits.len() >= limit.min(64) {
                return Ok(hits);
            }
        }
    }
    Ok(hits)
}

struct SearchChild(Child);
impl Drop for SearchChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(test)]
fn visible_path(path: &Path) -> bool {
    crate::file_search::searchable_path(path, false)
}

fn accept(
    bytes: &[u8],
    seen: &mut BTreeSet<PathBuf>,
    options: &crate::FileSearchOptions,
    root: Option<&Path>,
) -> Option<FileSearchHit> {
    let raw = std::str::from_utf8(bytes).ok()?;
    let path = Path::new(raw);
    if !crate::file_search::searchable_path(path, options.include_hidden) {
        return None;
    }
    let canonical = path.canonicalize().ok()?;
    if !crate::file_search::searchable_path(&canonical, options.include_hidden)
        || seen.contains(&canonical)
        || root.is_some_and(|root| !canonical.starts_with(root))
    {
        return None;
    }
    let metadata = canonical.metadata().ok()?;
    if !metadata.is_file() && !metadata.is_dir() {
        return None;
    }
    let directories = if metadata.is_dir() {
        canonical.as_path()
    } else {
        canonical.parent()?
    };
    if directories.components().any(|part| match part {
        Component::Normal(name) => options
            .excluded_directory_names
            .iter()
            .any(|excluded| name == excluded.as_str()),
        _ => false,
    }) {
        return None;
    }
    seen.insert(canonical.clone());
    Some(FileSearchHit {
        path: canonical,
        is_directory: metadata.is_dir(),
    })
}

#[cfg(test)]
fn collect(
    command: Command,
    limit: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
    timeout: Duration,
) -> Result<Vec<FileSearchHit>> {
    collect_configured(
        command,
        limit,
        cancelled,
        timeout,
        &crate::FileSearchOptions::default(),
        None,
    )
}

fn collect_configured(
    mut command: Command,
    limit: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
    timeout: Duration,
    options: &crate::FileSearchOptions,
    root: Option<&Path>,
) -> Result<Vec<FileSearchHit>> {
    if cancelled() {
        return Ok(Vec::new());
    }
    if timeout.is_zero() {
        bail!("file index query timed out");
    }
    let mut child = SearchChild(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("start file index query")?,
    );
    let mut stdout = child.0.stdout.take().context("file index output missing")?;
    let fd = stdout.as_raw_fd();
    // SAFETY: fd is the live, exclusively owned stdout pipe. Only its flags change.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    let deadline = Instant::now() + timeout;
    let mut pending = Vec::new();
    let mut seen = BTreeSet::new();
    let mut hits = Vec::new();
    let mut total = 0;
    let mut buffer = [0u8; 8192];
    loop {
        if cancelled() {
            return Ok(Vec::new());
        }
        if Instant::now() >= deadline {
            bail!("file index query timed out");
        }
        match stdout.read(&mut buffer) {
            Ok(0) => {
                if !pending.is_empty() {
                    bail!("incomplete file index output");
                }
                if let Some(status) = child.0.try_wait()? {
                    if !status.success() {
                        bail!("file index query failed");
                    }
                    return Ok(hits);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(count) => {
                total += count;
                if total > MAX_BYTES {
                    bail!("file index output limit reached");
                }
                for byte in &buffer[..count] {
                    if *byte == 0 {
                        if let Some(hit) = accept(&pending, &mut seen, options, root) {
                            hits.push(hit);
                        }
                        pending.clear();
                        if hits.len() >= limit {
                            return Ok(hits);
                        }
                    } else {
                        pending.push(*byte);
                        if pending.len() > MAX_PATH_BYTES {
                            bail!("file index path limit reached");
                        }
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    #[test]
    fn nul_paths_preserve_newlines_deduplicate_and_filter_noise() {
        // macOS TMPDIR contains a hidden component; use /tmp for visible fixtures.
        let dir = tempfile::Builder::new()
            .prefix("anywhere-search-")
            .tempdir_in("/tmp")
            .unwrap();
        let file = dir.path().join("报告\n季度.txt");
        std::fs::write(&file, "content").unwrap();
        let folder = dir.path().join("reports");
        std::fs::create_dir(&folder).unwrap();
        let mut command = Command::new("/usr/bin/printf");
        command
            .arg("%s\\0")
            .arg(&file)
            .arg(&file)
            .arg(&folder)
            .arg("/not-existing");
        let hits = collect(command, 10, &|| false, TIMEOUT).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].path, file.canonicalize().unwrap());
        assert!(!hits[0].is_directory && hits[1].is_directory);
        for path in [
            "/Users/u/.git/config",
            "/Applications/Test.app/a",
            "/Users/u/Library/x",
            "/Users/u/node_modules/a",
            "relative.txt",
        ] {
            assert!(!visible_path(Path::new(path)));
        }
    }
    #[test]
    fn slow_process_is_reaped_on_cancel_and_timeout() {
        let cancelled = AtomicBool::new(false);
        let mut command = Command::new("/bin/sleep");
        command.arg("5");
        let start = Instant::now();
        let error = collect(command, 8, &|| false, Duration::from_millis(30)).unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(1));
        let mut command = Command::new("/bin/sleep");
        command.arg("5");
        assert!(collect(
            command,
            8,
            &|| cancelled.swap(true, Ordering::AcqRel),
            TIMEOUT
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn incomplete_oversized_and_failed_output_is_not_accepted() {
        let mut command = Command::new("/usr/bin/printf");
        command.args(["%s", "/incomplete"]);
        assert!(collect(command, 8, &|| false, TIMEOUT)
            .unwrap_err()
            .to_string()
            .contains("incomplete"));
        let mut command = Command::new("/usr/bin/printf");
        command.arg("%s").arg("x".repeat(MAX_PATH_BYTES + 1));
        assert!(collect(command, 8, &|| false, TIMEOUT)
            .unwrap_err()
            .to_string()
            .contains("limit"));
        assert!(collect(Command::new("/usr/bin/false"), 8, &|| false, TIMEOUT).is_err());
    }
    #[test]
    fn configured_roots_and_directory_exclusions_filter_canonical_targets() {
        let root = tempfile::Builder::new()
            .prefix("anywhere-config-")
            .tempdir_in("/tmp")
            .unwrap();
        let root = root.path().canonicalize().unwrap();
        let logs = root.join("logs");
        std::fs::create_dir(&logs).unwrap();
        let file = logs.join("report.txt");
        std::fs::write(&file, "data").unwrap();
        let same_name_file = root.join("cache");
        std::fs::write(&same_name_file, "data").unwrap();
        let outside = tempfile::Builder::new()
            .prefix("anywhere-outside-")
            .tempdir_in("/tmp")
            .unwrap();
        let outside_file = outside.path().join("report.txt");
        std::fs::write(&outside_file, "data").unwrap();
        let link = root.join("link.txt");
        std::os::unix::fs::symlink(&outside_file, &link).unwrap();
        let options = crate::FileSearchOptions {
            roots: vec![root.clone()],
            excluded_directory_names: vec!["logs".into(), "cache".into()],
            ..Default::default()
        };
        let mut seen = BTreeSet::new();
        assert!(accept(
            file.to_str().unwrap().as_bytes(),
            &mut seen,
            &options,
            Some(&root)
        )
        .is_none());
        assert!(accept(
            link.to_str().unwrap().as_bytes(),
            &mut seen,
            &options,
            Some(&root)
        )
        .is_none());
        assert!(accept(
            same_name_file.to_str().unwrap().as_bytes(),
            &mut seen,
            &options,
            Some(&root)
        )
        .is_some());
    }
    #[test]
    fn cancellation_and_timeout_reap_the_actual_child_pid() {
        for cancel in [true, false] {
            let temp = tempfile::tempdir().unwrap();
            let pid_file = temp.path().join("pid");
            // Only this deterministic fixture uses a shell; production queries never do.
            let mut command = Command::new("/bin/sh");
            command
                .args([
                    "-c",
                    r#"printf '%s' "$$" > "$1"; exec /bin/sleep 5"#,
                    "fixture",
                ])
                .arg(&pid_file);
            let result = collect(
                command,
                8,
                &|| cancel && pid_file.exists(),
                Duration::from_millis(200),
            );
            if cancel {
                assert!(result.unwrap().is_empty());
            } else {
                assert!(result.is_err());
            }
            let pid: libc::pid_t = std::fs::read_to_string(&pid_file).unwrap().parse().unwrap();
            // SAFETY: kill(pid, 0) only probes the fixture PID; waitpid uses valid storage.
            unsafe {
                assert_eq!(libc::kill(pid, 0), -1, "query child is still alive");
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::ESRCH)
                );
                let mut status = 0;
                assert_eq!(libc::waitpid(pid, &mut status, libc::WNOHANG), -1);
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::ECHILD),
                    "query child was not reaped"
                );
            }
        }
    }
}
