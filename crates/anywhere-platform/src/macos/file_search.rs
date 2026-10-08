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
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("home directory unavailable")?;
    let mut command = Command::new("/usr/bin/mdfind");
    command
        .args(["-0", "-onlyin"])
        .arg(home)
        .arg("-name")
        .arg(query.trim());
    collect(command, limit.min(64), cancelled, TIMEOUT)
}

struct SearchChild(Child);
impl Drop for SearchChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn visible_path(path: &Path) -> bool {
    path.is_absolute()
        && !path.components().any(|component| {
            let Component::Normal(part) = component else {
                return false;
            };
            let part = part.to_string_lossy();
            part.starts_with('.')
                || matches!(
                    part.as_ref(),
                    "Library" | "node_modules" | "target" | "build"
                )
                || part.ends_with(".app")
                || part.ends_with(".framework")
        })
}

fn accept(bytes: &[u8], seen: &mut BTreeSet<PathBuf>) -> Option<FileSearchHit> {
    let raw = std::str::from_utf8(bytes).ok()?;
    let path = Path::new(raw);
    if !visible_path(path) {
        return None;
    }
    let canonical = path.canonicalize().ok()?;
    if !visible_path(&canonical) || seen.contains(&canonical) {
        return None;
    }
    let metadata = canonical.metadata().ok()?;
    if !metadata.is_file() && !metadata.is_dir() {
        return None;
    }
    seen.insert(canonical.clone());
    Some(FileSearchHit {
        path: canonical,
        is_directory: metadata.is_dir(),
    })
}

fn collect(
    mut command: Command,
    limit: usize,
    cancelled: &(dyn Fn() -> bool + Sync),
    timeout: Duration,
) -> Result<Vec<FileSearchHit>> {
    if cancelled() {
        return Ok(Vec::new());
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
                        if let Some(hit) = accept(&pending, &mut seen) {
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
}
