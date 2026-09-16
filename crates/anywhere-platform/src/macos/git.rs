//! Read-only Git: an allowlisted argv, a bounded drain, a hard deadline.
use crate::{GitOperation, GitOutput};
use anyhow::{bail, Context, Result};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

/// How long a read-only Git operation may run before it is killed.
const GIT_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn git_available() -> bool {
    git_program().is_some()
}

pub(super) fn run_git(repository: &Path, operation: GitOperation) -> Result<GitOutput> {
    if !repository.is_dir() {
        bail!("repository is no longer a directory");
    }
    let program = git_program().context("git is not installed")?;
    // Allowlisted argv only: no shell, no pager, no external helpers.
    let mut child = crate::git::command(&program, repository, operation)
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
        let _ = stdout_tx.send(crate::drain::read_bounded(stdout));
    });
    std::thread::spawn(move || {
        let _ = stderr_tx.send(crate::drain::read_bounded(stderr));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macos::MacOsPlatform;
    use crate::Platform;
    use std::process::Command;

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
}
