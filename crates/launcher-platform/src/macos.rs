use super::*;
use anyhow::{bail, Context};
use std::{
    ffi::{CStr, OsStr},
    io::Write,
    os::raw::c_char,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

extern "C" {
    fn launcher_frontmost_application() -> *mut c_char;
    fn launcher_free_string(value: *mut c_char);
}

#[derive(Default)]
pub struct MacOsPlatform;

impl Platform for MacOsPlatform {
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        // SAFETY: Native side returns a malloc-owned NUL-terminated string, freed exactly once.
        let result = unsafe {
            let raw = launcher_frontmost_application();
            if raw.is_null() {
                return Ok(None);
            }
            let result = serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
            launcher_free_string(raw);
            result
        };
        result.map(Some).context("decode frontmost application")
    }

    fn application_directory(&self, application: &ApplicationInfo) -> Result<Option<PathBuf>> {
        if application.bundle_id != "com.apple.finder" {
            return Ok(None);
        }
        let script = include_str!("../native/finder.applescript");
        let output = run(
            "/usr/bin/osascript",
            &[OsStr::new("-e"), OsStr::new(script)],
            None,
        )?;
        // osascript appends one newline; do not trim valid spaces from a filesystem name.
        let path = output.strip_suffix('\n').unwrap_or(&output);
        if path.is_empty() {
            return Ok(None);
        }
        let path = PathBuf::from(path);
        require_directory(&path)?;
        Ok(Some(path.canonicalize()?))
    }

    fn open_url(&self, value: &str) -> Result<()> {
        let url = url::Url::parse(value).context("invalid URL")?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            bail!("only absolute http/https URLs are permitted");
        }
        run("/usr/bin/open", &[OsStr::new(url.as_str())], None).map(|_| ())
    }
    fn open_file(&self, path: &Path) -> Result<()> {
        if !path.is_file() {
            bail!("file no longer exists: {}", path.display());
        }
        let path = path.canonicalize()?;
        run("/usr/bin/open", &[path.as_os_str()], None).map(|_| ())
    }
    fn reveal_file(&self, path: &Path) -> Result<()> {
        if !path.is_file() {
            bail!("file no longer exists: {}", path.display());
        }
        let path = path.canonicalize()?;
        run("/usr/bin/open", &[OsStr::new("-R"), path.as_os_str()], None).map(|_| ())
    }
    fn open_directory(&self, path: &Path) -> Result<()> {
        require_directory(path)?;
        let path = path.canonicalize()?;
        run("/usr/bin/open", &[path.as_os_str()], None).map(|_| ())
    }
    fn open_terminal(&self, path: &Path) -> Result<()> {
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
    fn copy_text(&self, text: &str) -> Result<()> {
        run("/usr/bin/pbcopy", &[], Some(text)).map(|_| ())
    }
}

fn require_directory(path: &Path) -> Result<()> {
    if !path.is_dir() {
        bail!("directory no longer exists: {}", path.display());
    }
    Ok(())
}

fn run(program: &str, args: &[&OsStr], stdin: Option<&str>) -> Result<String> {
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
