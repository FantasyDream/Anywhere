//! Opening things: URLs, files, directories, a terminal, and Finder reveal.
use anyhow::{bail, Context, Result};
use std::{
    ffi::OsStr,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(super) fn open_url(value: &str) -> Result<()> {
    let url = url::Url::parse(value).context("invalid URL")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        bail!("only absolute http/https URLs are permitted");
    }
    run("/usr/bin/open", &[OsStr::new(url.as_str())], None).map(|_| ())
}

pub(super) fn open_file(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("file no longer exists: {}", path.display());
    }
    let path = path.canonicalize()?;
    run("/usr/bin/open", &[path.as_os_str()], None).map(|_| ())
}

pub(super) fn reveal_file(path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("file no longer exists: {}", path.display());
    }
    let path = path.canonicalize()?;
    run("/usr/bin/open", &[OsStr::new("-R"), path.as_os_str()], None).map(|_| ())
}

pub(super) fn open_directory(path: &Path) -> Result<()> {
    require_directory(path)?;
    let path = path.canonicalize()?;
    run("/usr/bin/open", &[path.as_os_str()], None).map(|_| ())
}

pub(super) fn open_terminal(path: &Path) -> Result<()> {
    require_directory(path)?;
    let path = path.canonicalize()?;
    // Terminal natively accepts directory arguments; no shell interpolation.
    run(
        "/usr/bin/open",
        &[OsStr::new("-a"), OsStr::new("Terminal"), path.as_os_str()],
        None,
    )
    .map(|_| ())
}

pub(super) fn require_directory(path: &Path) -> Result<()> {
    if !path.is_dir() {
        bail!("directory no longer exists: {}", path.display());
    }
    Ok(())
}

pub(super) fn run(program: &str, args: &[&OsStr], stdin: Option<&str>) -> Result<String> {
    let start = Instant::now();
    let result = (|| -> Result<String> {
        let mut child = Command::new(program)
            .args(args)
            // GUI launches often inherit the C locale. pbcopy silently clears
            // non-ASCII text under that locale, despite exiting successfully.
            .env("LC_ALL", "en_US.UTF-8")
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("start {program}"))?;
        if let Some(text) = stdin {
            let mut pipe = child.stdin.take().context("child stdin missing")?;
            if let Err(error) = pipe.write_all(text.as_bytes()) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error).context("write clipboard input");
            }
        }
        loop {
            if child.try_wait()?.is_some() {
                break;
            }
            if start.elapsed() >= Duration::from_secs(5) {
                let _ = child.kill();
                let _ = child.wait();
                bail!("{program} timed out; check macOS Automation permission for Finder");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let output = child.wait_with_output()?;
        if !output.status.success() {
            bail!(
                "{program}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        String::from_utf8(output.stdout).context("platform response is not UTF-8")
    })();
    if let Err(error) = &result {
        tracing::error!(program, error = %error, "platform_api_failure");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macos::MacOsPlatform;
    use crate::Platform;

    #[test]
    fn platform_rejects_unsafe_urls_and_missing_paths_without_side_effects() {
        let platform = MacOsPlatform;
        assert!(platform.open_url("javascript:alert(1)").is_err());
        assert!(platform.open_url("file:///etc/passwd").is_err());
        assert!(platform
            .open_file(Path::new("/nonexistent-launcher-test/file"))
            .is_err());
        assert!(platform
            .open_directory(Path::new("/nonexistent-launcher-test"))
            .is_err());
    }
}
