//! Read-only Git: an allowlisted argv, a bounded drain, a hard deadline.
use crate::{GitOperation, GitOutput};
use anyhow::{bail, Context, Result};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// How long a read-only Git operation may run before it is killed.
const GIT_TIMEOUT: Duration = Duration::from_secs(5);

pub fn available() -> bool {
    git_program().is_some()
}

pub fn run(repository: &Path, operation: GitOperation) -> Result<GitOutput> {
    if !repository.is_dir() {
        bail!("repository is no longer a directory");
    }
    let program = git_program().context("git is not installed")?;
    let deadline = Instant::now() + GIT_TIMEOUT;
    let command = if operation == GitOperation::RemoteUrl {
        let (names, truncated) = run_command(
            crate::git::remote_names_command(&program, repository),
            operation,
            deadline,
        )?;
        if truncated {
            bail!("remote list is incomplete; clipboard unchanged");
        }
        let names = std::str::from_utf8(&names).context("remote names are not valid UTF-8")?;
        let names: Vec<_> = names.lines().filter(|name| !name.is_empty()).collect();
        let remote = if names.contains(&"origin") {
            "origin"
        } else {
            match names.as_slice() {
                [name] => *name,
                [] => bail!("this repository has no remote URL to copy"),
                _ => bail!("multiple remotes and no origin; cannot choose a remote URL"),
            }
        };
        crate::git::remote_url_command(&program, repository, remote)
    } else {
        crate::git::command(&program, repository, operation)
    };
    let (stdout, truncated) = run_command(command, operation, deadline)?;
    let (text, truncated) = crate::git::format_output(operation, &stdout, truncated)?;
    Ok(GitOutput { text, truncated })
}

fn run_command(
    mut command: Command,
    operation: GitOperation,
    deadline: Instant,
) -> Result<(Vec<u8>, bool)> {
    if Instant::now() >= deadline {
        bail!("git {} timed out", operation.as_str());
    }
    // Allowlisted argv only: no shell, no pager, no external helpers.
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("start read-only git command")?;
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
        if operation == GitOperation::BranchName && exit_status.and_then(|s| s.code()) == Some(1) {
            bail!("HEAD is detached; there is no current branch name to copy");
        }
        if matches!(
            operation,
            GitOperation::CommitHash | GitOperation::ShortCommitHash
        ) {
            bail!(
                "could not resolve HEAD to a commit; the repository may have no commits yet: {}",
                detail.trim()
            );
        }
        bail!("git {} failed: {}", operation.as_str(), detail.trim());
    }
    let truncated = stdout_truncated || stderr_truncated;
    Ok((stdout, truncated))
}

fn git_program() -> Option<PathBuf> {
    crate::process::find_executable("git")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn git_execution_rejects_missing_repositories_before_starting_a_process() {
        let missing = Path::new("/nonexistent-launcher-test/repository");
        assert!(run(missing, GitOperation::Status).is_err());
        assert!(run(missing, GitOperation::Log).is_err());
        assert!(run(missing, GitOperation::Diff).is_err());
    }
    #[test]
    fn git_availability_matches_the_allowlisted_program_lookup() {
        if let Some(program) = git_program() {
            assert!(program.is_absolute());
            assert!(available());
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

        let status = run(&root, GitOperation::Status).unwrap();
        assert!(
            status.text.contains("file.txt"),
            "status output: {}",
            status.text
        );
        assert!(!status.truncated);

        let log = run(&root, GitOperation::Log).unwrap();
        assert!(
            log.text.contains("initial commit"),
            "log output: {}",
            log.text
        );

        let diff = run(&root, GitOperation::Diff).unwrap();
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
        assert!(run(&root, GitOperation::Status).is_err());
    }
    struct Repository {
        _directory: tempfile::TempDir,
        root: PathBuf,
        program: PathBuf,
    }
    impl Repository {
        fn new() -> Option<Self> {
            let program = git_program()?;
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().canonicalize().unwrap();
            let repo = Self {
                _directory: directory,
                root,
                program,
            };
            repo.git(&["init", "--quiet", "--template="]);
            repo.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
            repo.git(&["config", "user.name", "Test"]);
            repo.git(&["config", "user.email", "test@example.com"]);
            repo.git(&[
                "config",
                "core.hooksPath",
                repo.root.join(".git/disabled-hooks").to_str().unwrap(),
            ]);
            Some(repo)
        }
        fn git(&self, args: &[&str]) -> String {
            let output = Command::new(&self.program)
                .args(["-c", "commit.gpgsign=false"])
                .args(args)
                .current_dir(&self.root)
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_INDEX_FILE")
                .env_remove("GIT_COMMON_DIR")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout)
                .unwrap()
                .trim_end()
                .to_owned()
        }
        fn commit(&self) {
            std::fs::write(self.root.join("file.txt"), "initial\n").unwrap();
            self.git(&["add", "file.txt"]);
            self.git(&["commit", "--quiet", "-m", "initial"]);
        }
    }

    #[test]
    fn remote_url_prefers_origin_respects_rewrites_and_rejects_ambiguity() {
        let Some(repo) = Repository::new() else {
            return;
        };
        assert!(run(&repo.root, GitOperation::RemoteUrl).is_err());
        repo.git(&["remote", "add", "upstream", "git@example.com:team/repo.git"]);
        assert_eq!(
            run(&repo.root, GitOperation::RemoteUrl).unwrap().text,
            "git@example.com:team/repo.git"
        );
        repo.git(&["remote", "add", "backup", "https://example.com/backup.git"]);
        assert!(run(&repo.root, GitOperation::RemoteUrl).is_err());
        repo.git(&["remote", "add", "origin", "https://example.com/main.git"]);
        assert_eq!(
            run(&repo.root, GitOperation::RemoteUrl).unwrap().text,
            "https://example.com/main.git"
        );
        repo.git(&[
            "remote",
            "set-url",
            "--push",
            "origin",
            "ssh://example.com/push-only.git",
        ]);
        assert_eq!(
            run(&repo.root, GitOperation::RemoteUrl).unwrap().text,
            "https://example.com/main.git"
        );
        repo.git(&["remote", "set-url", "origin", "team:main.git"]);
        repo.git(&["config", "url.ssh://git@example.com/.insteadOf", "team:"]);
        assert_eq!(
            run(&repo.root, GitOperation::RemoteUrl).unwrap().text,
            "ssh://git@example.com/main.git"
        );
        // The value is read again each execution, without contacting a remote.
        repo.git(&[
            "remote",
            "set-url",
            "origin",
            "ssh://git@example.com/new.git",
        ]);
        assert_eq!(
            run(&repo.root, GitOperation::RemoteUrl).unwrap().text,
            "ssh://git@example.com/new.git"
        );
    }

    #[test]
    fn branch_and_commit_values_handle_unborn_detached_and_ambiguous_refs() {
        let Some(repo) = Repository::new() else {
            return;
        };
        // A named branch exists before its first commit, but a commit hash does not.
        assert_eq!(
            run(&repo.root, GitOperation::BranchName).unwrap().text,
            "main"
        );
        assert!(run(&repo.root, GitOperation::CommitHash).is_err());
        assert!(run(&repo.root, GitOperation::ShortCommitHash).is_err());
        assert_eq!(
            run(&repo.root, GitOperation::Branches).unwrap().text,
            "No local branches yet."
        );
        repo.commit();
        repo.git(&["config", "core.abbrev", "9"]);
        repo.git(&["checkout", "--quiet", "-b", "feature/中文"]);
        repo.git(&["tag", "feature/中文"]);
        assert_eq!(
            run(&repo.root, GitOperation::BranchName).unwrap().text,
            "feature/中文"
        );
        let hash = repo.git(&["rev-parse", "HEAD"]);
        assert_eq!(
            run(&repo.root, GitOperation::CommitHash).unwrap().text,
            hash
        );
        let short = run(&repo.root, GitOperation::ShortCommitHash).unwrap().text;
        assert_eq!(short.len(), 9);
        assert!(hash.starts_with(&short));
        assert!(run(&repo.root, GitOperation::Branches)
            .unwrap()
            .text
            .contains("feature/中文"));
        repo.git(&["checkout", "--quiet", "--detach"]);
        let error = run(&repo.root, GitOperation::BranchName).unwrap_err();
        assert!(error.to_string().contains("detached"));
        assert_eq!(
            run(&repo.root, GitOperation::CommitHash).unwrap().text,
            hash
        );
    }

    #[test]
    fn staged_diff_reads_only_the_index_and_works_before_the_first_commit() {
        let Some(repo) = Repository::new() else {
            return;
        };
        assert_eq!(
            run(&repo.root, GitOperation::StagedDiff).unwrap().text,
            "No staged changes."
        );
        std::fs::write(repo.root.join("file.txt"), "staged content\n").unwrap();
        repo.git(&["add", "file.txt"]);
        std::fs::write(repo.root.join("file.txt"), "unstaged content\n").unwrap();
        let staged = run(&repo.root, GitOperation::StagedDiff).unwrap().text;
        assert!(staged.contains("+staged content"));
        assert!(!staged.contains("unstaged content"));
        assert!(run(&repo.root, GitOperation::Diff)
            .unwrap()
            .text
            .contains("+unstaged content"));
    }

    #[test]
    fn linked_worktrees_read_their_own_head() {
        let Some(repo) = Repository::new() else {
            return;
        };
        repo.commit();
        let linked = repo.root.join("linked worktree");
        repo.git(&[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "feature/linked",
            linked.to_str().unwrap(),
        ]);
        assert_eq!(
            run(&linked, GitOperation::BranchName).unwrap().text,
            "feature/linked"
        );
        assert_eq!(
            run(&repo.root, GitOperation::BranchName).unwrap().text,
            "main"
        );
        assert_eq!(
            run(&linked, GitOperation::CommitHash).unwrap().text,
            run(&repo.root, GitOperation::CommitHash).unwrap().text
        );
    }
}
