use super::*;
use anyhow::{bail, Context};
use std::{
    ffi::{CStr, CString, OsStr},
    io::{Read, Write},
    os::raw::c_char,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

extern "C" {
    fn launcher_frontmost_application() -> *mut c_char;
    fn launcher_free_string(value: *mut c_char);
    fn launcher_free_bytes(value: *mut u8);
    fn launcher_bundle_application(path: *const c_char) -> *mut c_char;
    fn launcher_preferred_languages() -> *mut c_char;
    fn launcher_bundle_localized_names(
        path: *const c_char,
        languages: *const c_char,
    ) -> *mut c_char;
    fn launcher_launch_application(path: *const c_char) -> i32;
    fn launcher_application_icon(path: *const c_char, size: u32, out_length: *mut u32) -> *mut u8;
    fn launcher_clipboard_text(output: *mut c_char, capacity: u32, out_length: *mut u32) -> i32;
}

/// Clipboard snapshots are bounded so a huge clipboard cannot stall activation.
pub const MAX_CLIPBOARD_BYTES: usize = 1024 * 1024;
/// Git output cap per stream; larger results are cut and reported as truncated.
const MAX_GIT_BYTES: usize = 1024 * 1024;
const GIT_TIMEOUT: Duration = Duration::from_secs(5);
/// Upper bound on waiting for the pipes to close once the process is gone or killed.
const READER_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct MacOsPlatform;

fn c_path(path: &str) -> Result<CString> {
    CString::new(path).context("path contains a NUL byte")
}

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

    fn bundle_application(&self, bundle: &Path) -> Result<Option<BundleApplication>> {
        let path = c_path(&bundle.to_string_lossy())?;
        // SAFETY: path outlives the call; the native side returns a malloc-owned JSON string.
        let decoded = unsafe {
            let raw = launcher_bundle_application(path.as_ptr());
            if raw.is_null() {
                return Ok(None);
            }
            let decoded: std::result::Result<serde_json::Value, _> =
                serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
            launcher_free_string(raw);
            decoded
        };
        let value = decoded.context("decode bundle metadata")?;
        let string = |key: &str| -> Option<String> {
            value
                .get(key)
                .and_then(|value| value.as_str())
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        };
        let (Some(bundle_id), Some(name)) = (string("bundle_id"), string("name")) else {
            return Ok(None);
        };
        Ok(Some(BundleApplication {
            bundle_id,
            name,
            path: string("path"),
            icon: None,
        }))
    }

    fn preferred_languages(&self) -> Vec<String> {
        // SAFETY: returns a malloc-owned JSON string, freed exactly once here.
        unsafe {
            let raw = launcher_preferred_languages();
            if raw.is_null() {
                return Vec::new();
            }
            let decoded: std::result::Result<Vec<String>, _> =
                serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
            launcher_free_string(raw);
            decoded.unwrap_or_default()
        }
    }

    fn bundle_localized_names(&self, bundle: &Path, languages: &[String]) -> Result<Vec<String>> {
        if languages.is_empty() {
            return Ok(Vec::new());
        }
        let path = c_path(&bundle.to_string_lossy())?;
        let languages = c_path(&languages.join(","))?;
        // SAFETY: both inputs are NUL-terminated and outlive the call; the returned
        // string is malloc-owned and freed exactly once.
        let decoded = unsafe {
            let raw = launcher_bundle_localized_names(path.as_ptr(), languages.as_ptr());
            if raw.is_null() {
                return Ok(Vec::new());
            }
            let decoded: std::result::Result<Vec<String>, _> =
                serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
            launcher_free_string(raw);
            decoded
        };
        Ok(decoded.unwrap_or_default())
    }

    fn launch_application(&self, path: &str) -> Result<bool> {
        if path.is_empty() {
            bail!("application path is empty");
        }
        let path = c_path(path)?;
        // SAFETY: null-terminated path stays alive for the synchronous native call.
        let launched = unsafe { launcher_launch_application(path.as_ptr()) };
        Ok(launched == 1)
    }

    fn application_icon(&self, path: &str, size: u32) -> Result<Option<Vec<u8>>> {
        let path = c_path(path)?;
        let mut length = 0u32;
        // SAFETY: the native side allocates the buffer and reports its exact length.
        let buffer = unsafe { launcher_application_icon(path.as_ptr(), size, &mut length) };
        if buffer.is_null() || length == 0 {
            return Ok(None);
        }
        // SAFETY: `buffer` is a malloc-owned allocation of `length` bytes, freed once.
        let bytes = unsafe { std::slice::from_raw_parts(buffer, length as usize).to_vec() };
        unsafe { launcher_free_bytes(buffer) };
        Ok(Some(bytes))
    }

    fn clipboard_text(&self) -> Result<Option<String>> {
        let mut output = vec![0u8; MAX_CLIPBOARD_BYTES];
        let mut length = 0u32;
        // SAFETY: output has the supplied capacity; native code writes at most that much.
        let status = unsafe {
            launcher_clipboard_text(
                output.as_mut_ptr() as *mut c_char,
                output.len() as u32,
                &mut length,
            )
        };
        match status {
            1 => {
                let length = (length as usize).min(output.len());
                output.truncate(length);
                let text = String::from_utf8(output).context("clipboard is not valid UTF-8")?;
                if text.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(text))
                }
            }
            // No text, or the value is larger than the bounded snapshot.
            2 | 3 => Ok(None),
            other => bail!("clipboard read failed (status {other})"),
        }
    }

    fn git_available(&self) -> bool {
        git_program().is_some()
    }

    fn run_git(&self, repository: &Path, operation: GitOperation) -> Result<GitOutput> {
        if !repository.is_dir() {
            bail!("repository is no longer a directory");
        }
        let program = git_program().context("git is not installed")?;
        // Allowlisted argv only: no shell, no pager, no external helpers.
        let mut args = vec![
            OsStr::new("--no-pager"),
            OsStr::new("-c"),
            OsStr::new("core.fsmonitor=false"),
            OsStr::new("-c"),
            OsStr::new("pager.status=false"),
            OsStr::new("-c"),
            OsStr::new("pager.log=false"),
            OsStr::new("-c"),
            OsStr::new("pager.diff=false"),
            OsStr::new("-c"),
            OsStr::new("diff.external="),
            OsStr::new("-c"),
            OsStr::new("diff.textconv="),
            OsStr::new("-C"),
            repository.as_os_str(),
        ];
        match operation {
            GitOperation::Status => {
                args.extend([OsStr::new("status"), OsStr::new("--porcelain=v1")]);
            }
            GitOperation::Log => {
                args.extend([
                    OsStr::new("log"),
                    OsStr::new("--oneline"),
                    OsStr::new("--no-color"),
                    OsStr::new("-n"),
                    OsStr::new("100"),
                ]);
            }
            GitOperation::Diff => {
                args.extend([
                    OsStr::new("diff"),
                    OsStr::new("--no-color"),
                    // Both helpers must be off. `--no-ext-diff` only covers external
                    // diff drivers; a repository can also configure a textconv driver,
                    // which git runs unless it is disabled explicitly.
                    OsStr::new("--no-ext-diff"),
                    OsStr::new("--no-textconv"),
                ]);
            }
        }
        let mut child = Command::new(&program)
            .args(&args)
            .env("LC_ALL", "en_US.UTF-8")
            // Honour the selected repository, not an inherited one.
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_PAGER", "cat")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("start {}", program.display()))?;
        let stdout = child.stdout.take().context("git stdout missing")?;
        let stderr = child.stderr.take().context("git stderr missing")?;
        // Drain both pipes concurrently: waiting for exit first can deadlock on a large diff.
        // The sender side of each channel doubles as the completion signal, so a reader that
        // outlives the deadline cannot hold the launcher open; see `BoundedReader::join`.
        let (stdout_tx, stdout_rx) = std::sync::mpsc::channel();
        let (stderr_tx, stderr_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = stdout_tx.send(read_bounded(stdout));
        });
        std::thread::spawn(move || {
            let _ = stderr_tx.send(read_bounded(stderr));
        });
        let deadline = Instant::now() + GIT_TIMEOUT;
        let mut timed_out = false;
        let mut exit_status = None;
        loop {
            if let Some(status) = child.try_wait()? {
                exit_status = Some(status);
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                timed_out = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if timed_out {
            // A killed child normally closes both pipes at once. If it does not — a
            // grandchild that inherited a pipe keeps it open — reading must not outlive the
            // deadline, so the wait is bounded and reported as the timeout this already is.
            let _ = join_reader(stdout_rx, READER_DRAIN_TIMEOUT);
            let _ = join_reader(stderr_rx, READER_DRAIN_TIMEOUT);
            bail!(
                "git {} timed out after {}s; the repository may be very large",
                operation.as_str(),
                GIT_TIMEOUT.as_secs()
            );
        }
        let (stdout, stdout_truncated) = match join_reader(stdout_rx, READER_DRAIN_TIMEOUT) {
            ReaderOutcome::Drained(bytes, truncated) => (bytes, truncated),
            ReaderOutcome::Stalled => bail!(
                "git {} output did not close after the process exited",
                operation.as_str()
            ),
            ReaderOutcome::Failed => bail!("could not read git {} output", operation.as_str()),
        };
        let (stderr, stderr_truncated) = match join_reader(stderr_rx, READER_DRAIN_TIMEOUT) {
            ReaderOutcome::Drained(bytes, truncated) => (bytes, truncated),
            ReaderOutcome::Stalled => bail!(
                "git {} error output did not close after the process exited",
                operation.as_str()
            ),
            ReaderOutcome::Failed => (Vec::new(), false),
        };
        if exit_status.is_some_and(|status| !status.success()) {
            let detail = String::from_utf8_lossy(&stderr);
            bail!("git {} failed: {}", operation.as_str(), detail.trim());
        }
        let stdout = String::from_utf8_lossy(&stdout).into_owned();
        let mut text = stdout.trim_end().to_owned();
        if text.is_empty() {
            text = match operation {
                GitOperation::Status => "Working tree clean.".into(),
                GitOperation::Log => "No commits yet.".into(),
                GitOperation::Diff => "No unstaged changes.".into(),
            };
        }
        let truncated = stdout_truncated || stderr_truncated;
        if truncated {
            text.push_str("\n… output truncated");
        }
        Ok(GitOutput { text, truncated })
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

/// Git is looked up once per call from the usual locations; never shelled out.
fn git_program() -> Option<PathBuf> {
    for candidate in [
        "/usr/bin/git",
        "/usr/local/bin/git",
        "/opt/homebrew/bin/git",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// Read at most `MAX_GIT_BYTES`, then keep draining so the child never blocks.
fn read_bounded(mut reader: impl Read) -> std::io::Result<(Vec<u8>, bool)> {
    let mut collected = Vec::new();
    let mut buffer = [0u8; 16 * 1024];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok((collected, truncated));
        }
        if collected.len() < MAX_GIT_BYTES {
            let room = MAX_GIT_BYTES - collected.len();
            collected.extend_from_slice(&buffer[..read.min(room)]);
            if read > room {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }
}

/// Outcome of waiting for one pipe reader.
enum ReaderOutcome {
    /// The pipe reached EOF and the output is complete.
    Drained(Vec<u8>, bool),
    /// The reader was still blocked when the wait expired; the thread keeps draining in
    /// the background, which is the only option for a blocking pipe read.
    Stalled,
    /// The read failed; the error is already logged at debug level.
    Failed,
}

/// Wait for one pipe reader, giving up after `timeout` so a stuck pipe cannot hold the
/// launcher open any longer than the Git deadline intends.
fn join_reader(
    rx: std::sync::mpsc::Receiver<std::io::Result<(Vec<u8>, bool)>>,
    timeout: Duration,
) -> ReaderOutcome {
    match rx.recv_timeout(timeout) {
        Ok(Ok((bytes, truncated))) => ReaderOutcome::Drained(bytes, truncated),
        Ok(Err(error)) => {
            tracing::debug!(error = %error, "git_output_read_failed");
            ReaderOutcome::Failed
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => ReaderOutcome::Stalled,
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => ReaderOutcome::Failed,
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

    /// A reader that produces a fixed number of bytes without ever blocking.
    struct Fixed(usize);
    impl Read for Fixed {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let produce = self.0.min(buffer.len());
            for byte in buffer.iter_mut().take(produce) {
                *byte = b'x';
            }
            self.0 -= produce;
            Ok(produce)
        }
    }

    #[test]
    fn bounded_reader_caps_output_but_keeps_draining_the_pipe() {
        let (collected, truncated) = read_bounded(Fixed(MAX_GIT_BYTES + 4096)).unwrap();
        assert_eq!(collected.len(), MAX_GIT_BYTES);
        assert!(truncated, "over-cap output must be reported as truncated");
        let (small, truncated) = read_bounded(Fixed(32)).unwrap();
        assert_eq!(small.len(), 32);
        assert!(!truncated);
        // The reader is drained to EOF, so the child process can never block on a full pipe.
        let mut exhausted = Fixed(0);
        let mut buffer = [0u8; 8];
        assert_eq!(exhausted.read(&mut buffer).unwrap(), 0);
    }

    #[test]
    fn git_execution_rejects_missing_repositories_before_starting_a_process() {
        let platform = MacOsPlatform;
        let missing = Path::new("/nonexistent-launcher-test/repository");
        assert!(platform.run_git(missing, GitOperation::Status).is_err());
        assert!(platform.run_git(missing, GitOperation::Log).is_err());
        assert!(platform.run_git(missing, GitOperation::Diff).is_err());
    }

    #[test]
    fn git_availability_matches_the_allowlisted_program_lookup() {
        let platform = MacOsPlatform;
        if let Some(program) = git_program() {
            assert!(program.is_absolute());
            assert!(platform.git_available());
        }
    }

    /// Real Git invocation against a throwaway repository, when Git is installed.
    #[test]
    fn git_operations_report_real_repository_state() {
        let Some(program) = git_program() else {
            return;
        };
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let git = |args: &[&str]| {
            let status = Command::new(&program)
                .args(args)
                .current_dir(&root)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("run git in test repository");
            assert!(status.success(), "git {args:?} failed");
        };
        git(&["init", "--quiet"]);
        git(&["config", "user.email", "test@example.com"]);
        git(&["config", "user.name", "Test"]);
        std::fs::write(root.join("file.txt"), "hello\n").unwrap();
        git(&["add", "file.txt"]);
        git(&["commit", "--quiet", "-m", "initial commit"]);
        std::fs::write(root.join("file.txt"), "hello world\n").unwrap();

        let platform = MacOsPlatform;
        let status = platform.run_git(&root, GitOperation::Status).unwrap();
        assert!(
            status.text.contains("file.txt"),
            "status output: {}",
            status.text
        );
        assert!(!status.truncated);

        let log = platform.run_git(&root, GitOperation::Log).unwrap();
        assert!(
            log.text.contains("initial commit"),
            "log output: {}",
            log.text
        );

        let diff = platform.run_git(&root, GitOperation::Diff).unwrap();
        assert!(
            diff.text.contains("hello world"),
            "diff output: {}",
            diff.text
        );
    }

    #[test]
    fn git_operations_fail_cleanly_outside_a_repository() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        assert!(MacOsPlatform.run_git(&root, GitOperation::Status).is_err());
    }

    #[test]
    fn bundle_metadata_is_reported_for_a_real_system_application() {
        let platform = MacOsPlatform;
        let finder = Path::new("/System/Library/CoreServices/Finder.app");
        if finder.is_dir() {
            let bundle = platform.bundle_application(finder).unwrap().unwrap();
            assert_eq!(bundle.bundle_id, "com.apple.finder");
            assert!(!bundle.name.is_empty());
        }
        // A plain directory is not an application.
        let plain = tempfile::tempdir().unwrap();
        assert!(platform.bundle_application(plain.path()).unwrap().is_none());
    }
}
