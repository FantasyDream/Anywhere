use super::windows_native::*;
use super::*;
use anyhow::{bail, Context};
use std::{os::windows::ffi::OsStrExt, path::PathBuf, sync::Mutex};

#[derive(Default)]
pub struct WindowsPlatform {
    captured: Mutex<Option<(ApplicationInfo, isize)>>,
}

fn status(operation: &str, result: i32) -> Result<bool> {
    if result < 0 {
        tracing::error!(operation, hresult = result, "platform_api_failure");
        bail!("{operation} failed (HRESULT 0x{:08X})", result as u32);
    }
    Ok(result == 0)
}
fn wide(value: &std::ffi::OsStr) -> Result<Vec<u16>> {
    let mut result: Vec<u16> = value.encode_wide().collect();
    if result.contains(&0) {
        bail!("embedded NUL is not supported");
    }
    result.push(0);
    Ok(result)
}
fn shell_path(path: &Path) -> Result<Vec<u16>> {
    let path = path.canonicalize()?;
    let raw = path.as_os_str().to_string_lossy();
    // Shell applications expect ordinary DOS/UNC paths, not Rust's extended prefix.
    let display = raw
        .strip_prefix(r"\\?\UNC\")
        .map(|s| format!(r"\\{s}"))
        .unwrap_or_else(|| raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_owned());
    wide(std::ffi::OsStr::new(&display))
}
fn open(path: &Path, directory: bool, kind: i32) -> Result<()> {
    if if directory {
        !path.is_dir()
    } else {
        !path.is_file()
    } {
        bail!(
            "{} no longer exists: {}",
            if directory { "directory" } else { "file" },
            path.display()
        );
    }
    let path = shell_path(path)?;
    // SAFETY: null-terminated UTF-16 remains alive for the synchronous native call.
    status("open", unsafe {
        launcher_windows_open(path.as_ptr(), kind)
    })?;
    Ok(())
}
impl Platform for WindowsPlatform {
    fn application_roots(&self) -> Vec<PathBuf> {
        ["APPDATA", "ProgramData"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(|base| {
                PathBuf::from(base)
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs")
            })
            .collect()
    }

    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        let mut snapshot = self
            .captured
            .lock()
            .map_err(|_| anyhow::anyhow!("capture lock poisoned"))?;
        *snapshot = None;
        let mut executable = vec![0; 32768];
        let mut window = 0;
        // SAFETY: output buffers have the supplied capacity; native code retains no pointers.
        if !status("frontmost_application", unsafe {
            launcher_windows_foreground(
                executable.as_mut_ptr(),
                executable.len() as u32,
                &mut window,
            )
        })? {
            return Ok(None);
        }
        let length = executable
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(executable.len());
        let executable = String::from_utf16(&executable[..length])?;
        let app = ApplicationInfo {
            name: Path::new(&executable)
                .file_stem()
                .context("application name missing")?
                .to_string_lossy()
                .into(),
            bundle_id: executable.to_lowercase(),
        };
        *snapshot = Some((app.clone(), window));
        Ok(Some(app))
    }
    fn application_directory(&self, application: &ApplicationInfo) -> Result<Option<PathBuf>> {
        if !application.name.eq_ignore_ascii_case("explorer") {
            return Ok(None);
        }
        let snapshot = self
            .captured
            .lock()
            .map_err(|_| anyhow::anyhow!("capture lock poisoned"))?;
        let Some((_, window)) = snapshot.as_ref().filter(|(app, _)| app == application) else {
            return Ok(None);
        };
        let mut path = vec![0; 32768];
        // SAFETY: native code bounds-checks the output and validates the captured HWND.
        if !status("current_directory", unsafe {
            launcher_windows_directory(*window, path.as_mut_ptr(), path.len() as u32)
        })? {
            return Ok(None);
        }
        let length = path.iter().position(|&c| c == 0).unwrap_or(path.len());
        Ok(Some(PathBuf::from(String::from_utf16(&path[..length])?)))
    }
    fn open_url(&self, value: &str) -> Result<()> {
        let url = url::Url::parse(value).context("invalid URL")?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            bail!("only absolute http/https URLs are permitted");
        }
        let value = wide(std::ffi::OsStr::new(url.as_str()))?;
        // SAFETY: validated null-terminated UTF-16, synchronous call.
        status("open_url", unsafe {
            launcher_windows_open(value.as_ptr(), 0)
        })?;
        Ok(())
    }
    fn open_file(&self, path: &Path) -> Result<()> {
        open(path, false, 0)
    }
    fn reveal_file(&self, path: &Path) -> Result<()> {
        open(path, false, 1)
    }
    fn open_directory(&self, path: &Path) -> Result<()> {
        open(path, true, 0)
    }
    fn open_terminal(&self, path: &Path) -> Result<()> {
        open(path, true, 2)
    }
    fn copy_text(&self, text: &str) -> Result<()> {
        let text = wide(std::ffi::OsStr::new(text))?;
        // SAFETY: length excludes the NUL terminator, but the allocation includes it.
        status("copy_text", unsafe {
            launcher_windows_copy(text.as_ptr(), (text.len() - 1) as u32)
        })?;
        Ok(())
    }

    /// A Start Menu shortcut is the Windows equivalent of an application bundle: it is
    /// what the shell lists and what the user recognizes. Its display name comes from the
    /// shortcut itself, so an entry whose file name is an implementation detail
    /// (`chrome.lnk`) still indexes as "Google Chrome".
    ///
    /// The shortcut stays the launch path. Launching the resolved target instead would drop
    /// the shortcut's arguments and working directory, which applications such as
    /// `Update.exe --processStart …` depend on, and `GetPath` is documented to return the
    /// recorded string, which may still contain unexpanded environment variables.
    fn bundle_application(&self, shortcut: &Path) -> Result<Option<BundleApplication>> {
        if !shortcut.is_file() {
            return Ok(None);
        }
        let path = wide(shortcut.as_os_str())?;
        let mut name = vec![0u16; 512];
        let mut target = vec![0u16; 32768];
        let mut description = vec![0u16; 1024];
        // SAFETY: every buffer is caller-owned with its length passed alongside; native
        // code retains no pointer and bounds every copy.
        if !status("resolve_shortcut", unsafe {
            launcher_windows_resolve_shortcut(
                path.as_ptr(),
                name.as_mut_ptr(),
                name.len() as u32,
                target.as_mut_ptr(),
                target.len() as u32,
                description.as_mut_ptr(),
                description.len() as u32,
            )
        })? {
            return Ok(None);
        }
        let name = utf16_string(&name);
        if name.trim().is_empty() {
            return Ok(None);
        }
        let target = utf16_string(&target);
        let described = utf16_string(&description);
        Ok(Some(BundleApplication {
            // The executable name cannot be the identity: unrelated applications share
            // one (`Update.exe`, `launcher.exe`), and collapsing them would hide every one
            // but the first and share their usage history. The shortcut path is what the
            // shell lists, so it is both unique and stable across scans; the resolved
            // target only feeds the subtitle.
            bundle_id: format!("app.{}", identity_of(shortcut)),
            // A description is not a name: `Google Chrome.lnk` may describe itself as
            // "Access the Internet", so it must not replace the title the user recognizes —
            // but it is worth searching.
            aliases: (!described.trim().is_empty() && !described.eq_ignore_ascii_case(&name))
                .then_some(described)
                .into_iter()
                .collect(),
            path: Some(shortcut.to_string_lossy().into_owned()),
            icon: (!target.is_empty()).then_some(target),
            name,
        }))
    }
    fn application_icon(&self, _path: &str, _size: u32) -> Result<Option<Vec<u8>>> {
        // Icons are presentation only; without an extractor the host falls back to its
        // semantic icon, which is preferable to shipping a wrong one.
        Ok(None)
    }
    fn launch_application(&self, path: &str) -> Result<bool> {
        // ShellExecute resolves a running instance, waits on elevation prompts and handles
        // both an `.exe` and a `.lnk`. There is no "activate instead of launch" signal to
        // report, so a successful start is the answer.
        let path = Path::new(path);
        // The extension decides whether the shell can be handed this entry, not
        // `is_file`: an indexed shortcut exists, but a path recorded with unexpanded
        // variables would fail that check while remaining perfectly launchable.
        let startable = path.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("lnk") || extension.eq_ignore_ascii_case("exe")
        }) || path.is_file();
        if !startable {
            bail!("application no longer exists: {}", path.display());
        }
        open(path, false, 0)?;
        Ok(true)
    }
    fn clipboard_text(&self) -> Result<Option<String>> {
        // Bounded like the macOS snapshot: a huge clipboard must not be pulled into the
        // launcher at activation.
        let mut buffer = vec![0u16; 64 * 1024];
        let mut length = 0u32;
        // SAFETY: the buffer and its capacity are passed together; native code fills at
        // most `capacity - 1` code units and NUL-terminates.
        let result = unsafe {
            launcher_windows_clipboard_text(buffer.as_mut_ptr(), buffer.len() as u32, &mut length)
        };
        // S_FALSE is the shim's "no text on the clipboard", not a failure.
        if !status("clipboard_text", result)? {
            return Ok(None);
        }
        let length = (length as usize).min(buffer.len());
        let text = String::from_utf16_lossy(&buffer[..length]);
        Ok(if text.is_empty() { None } else { Some(text) })
    }
    fn git_available(&self) -> bool {
        git_program().is_some()
    }
    fn run_git(&self, repository: &Path, operation: GitOperation) -> Result<GitOutput> {
        if !repository.is_dir() {
            bail!("repository is no longer a directory");
        }
        let program = git_program().context("git is not installed")?;
        // The argv allowlist and the deadline handling are shared with macOS; only the
        // process API differs, and `std::process` is already correct on both.
        let mut command = crate::git::command(&program, repository, operation);
        configure_no_console(&mut command);
        let mut child = command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .with_context(|| format!("start {}", program.display()))?;
        let stdout = child.stdout.take().context("git stdout missing")?;
        let stderr = child.stderr.take().context("git stderr missing")?;
        // Drain both pipes concurrently: waiting for exit first can deadlock on a large diff.
        let (stdout_tx, stdout_rx) = std::sync::mpsc::channel();
        let (stderr_tx, stderr_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = stdout_tx.send(crate::drain::read_bounded(stdout));
        });
        std::thread::spawn(move || {
            let _ = stderr_tx.send(crate::drain::read_bounded(stderr));
        });
        let deadline = std::time::Instant::now() + GIT_TIMEOUT;
        let mut timed_out = false;
        let mut exit_status = None;
        loop {
            if let Some(status) = child.try_wait()? {
                exit_status = Some(status);
                break;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                timed_out = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        if timed_out {
            // A killed process normally closes both pipes at once. If a grandchild holds
            // one open, reading must not outlive the deadline.
            let _ = crate::drain::join(stdout_rx, crate::drain::DRAIN_TIMEOUT);
            let _ = crate::drain::join(stderr_rx, crate::drain::DRAIN_TIMEOUT);
            bail!(
                "git {} timed out after {}s; the repository may be very large",
                operation.as_str(),
                GIT_TIMEOUT.as_secs()
            );
        }
        let (stdout, stdout_truncated) =
            match crate::drain::join(stdout_rx, crate::drain::DRAIN_TIMEOUT) {
                crate::drain::ReaderOutcome::Drained(bytes, truncated) => (bytes, truncated),
                crate::drain::ReaderOutcome::Stalled => bail!(
                    "git {} output did not close after the process exited",
                    operation.as_str()
                ),
                crate::drain::ReaderOutcome::Failed => {
                    bail!("could not read git {} output", operation.as_str())
                }
            };
        let (stderr, stderr_truncated) =
            match crate::drain::join(stderr_rx, crate::drain::DRAIN_TIMEOUT) {
                crate::drain::ReaderOutcome::Drained(bytes, truncated) => (bytes, truncated),
                crate::drain::ReaderOutcome::Stalled => bail!(
                    "git {} error output did not close after the process exited",
                    operation.as_str()
                ),
                crate::drain::ReaderOutcome::Failed => (Vec::new(), false),
            };
        if exit_status.is_some_and(|status| !status.success()) {
            let detail = String::from_utf8_lossy(&stderr);
            bail!("git {} failed: {}", operation.as_str(), detail.trim());
        }
        let truncated = stdout_truncated || stderr_truncated;
        let (text, truncated) = crate::git::format_output(operation, &stdout, truncated);
        Ok(GitOutput { text, truncated })
    }
}

/// Stable identity for one Start Menu shortcut.
///
/// The whole path, lowercased, with separators folded so the same shortcut yields the same
/// id regardless of how the enumerator spelled the drive or the trailing separator.
fn identity_of(shortcut: &Path) -> String {
    shortcut
        .to_string_lossy()
        .to_lowercase()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_owned()
}

/// Run the child without a console window.
///
/// A GUI process started from Explorer has no console of its own, so a console subsystem
/// child such as `git.exe` would get a new one — which can take focus from the launcher and
/// dismiss it, throwing away the output this call exists to produce. Both pipes are already
/// captured, so nothing needs a console.
#[cfg(target_os = "windows")]
fn configure_no_console(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    /// `CREATE_NO_WINDOW`.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

/// How long the read-only Git operations may run before they are killed.
const GIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Locate `git.exe` on `PATH` without running a shell.
///
/// Checked once per call rather than cached: the launcher is long-lived, and a user who
/// installs Git while it is running should not have to restart it.
fn git_program() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        for name in ["git.exe", "git.cmd"] {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Decode a NUL-terminated UTF-16 buffer filled by the native shim.
fn utf16_string(buffer: &[u16]) -> String {
    let length = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..length])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_urls_and_missing_paths_before_windows_side_effects() {
        let platform = WindowsPlatform::default();
        for url in ["javascript:alert(1)", "file:///C:/Windows", "https://"] {
            assert!(platform.open_url(url).is_err());
        }
        let missing = std::env::temp_dir()
            .join(format!("launcher-missing-{}", std::process::id()))
            .join("file.txt");
        assert!(platform.open_file(&missing).is_err());
        assert!(platform.reveal_file(&missing).is_err());
        assert!(platform.open_directory(&missing).is_err());
        assert!(platform.open_terminal(&missing).is_err());
        assert!(platform.copy_text("invalid\0text").is_err());
    }
}
