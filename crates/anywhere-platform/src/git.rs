//! Shared Git argv and result formatting.
//!
//! Process execution, bounded draining and deadlines use Rust's shared desktop APIs.

mod runner;
pub use runner::{available, run};

use crate::GitOperation;
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// Read-only, non-interactive argv for one operation. Never a shell, never a pager.
pub fn command(program: &Path, repository: &Path, operation: GitOperation) -> Command {
    let mut command = base_command(program, repository);
    match operation {
        GitOperation::RemoteUrl => {
            command.args(["remote", "get-url", "--", "origin"]);
        }
        _ => append_operation(&mut command, operation),
    }
    command
}

pub(crate) fn remote_names_command(program: &Path, repository: &Path) -> Command {
    let mut command = base_command(program, repository);
    command.arg("remote");
    command
}

pub(crate) fn remote_url_command(program: &Path, repository: &Path, remote: &str) -> Command {
    let mut command = base_command(program, repository);
    command.args(["remote", "get-url", "--"]).arg(remote);
    command
}

fn base_command(program: &Path, repository: &Path) -> Command {
    let mut command = Command::new(program);
    command.args([
        // Paging and external helpers are disabled by configuration as well as by flag:
        // a repository can set `diff.external` or a textconv driver, and git runs those
        // unless they are explicitly cleared.
        "--no-pager",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "pager.status=false",
        "-c",
        "pager.log=false",
        "-c",
        "pager.diff=false",
        "-c",
        "diff.external=",
        "-c",
        "diff.textconv=",
        "-C",
    ]);
    command.arg(repository);
    // Honour the selected repository, not an inherited one.
    command
        .env("LC_ALL", "en_US.UTF-8")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_PAGER", "cat")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .env_remove("GIT_NAMESPACE");
    command
}

fn append_operation(command: &mut Command, operation: GitOperation) {
    match operation {
        GitOperation::Status => {
            command.args(["status", "--porcelain=v1"]);
        }
        GitOperation::Log => {
            command.args(["log", "--oneline", "--no-color", "-n", "100"]);
        }
        GitOperation::Diff => {
            // `--no-ext-diff` covers external diff drivers; a repository can also
            // configure a textconv driver, which needs `--no-textconv`.
            command.args(["diff", "--no-color", "--no-ext-diff", "--no-textconv"]);
        }
        GitOperation::StagedDiff => {
            command.args([
                "diff",
                "--cached",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
            ]);
        }
        GitOperation::Branches => {
            command.args(["branch", "--list", "--no-color"]);
        }
        GitOperation::BranchName => {
            // Keep the full ref: --short may return heads/foo when a tag also
            // has the name foo. Formatting removes exactly refs/heads/.
            command.args(["symbolic-ref", "--quiet", "HEAD"]);
        }
        GitOperation::CommitHash => {
            command.args(["rev-parse", "--verify", "HEAD^{commit}"]);
        }
        GitOperation::ShortCommitHash => {
            command.args(["rev-parse", "--verify", "--short", "HEAD^{commit}"]);
        }
        GitOperation::RemoteUrl => unreachable!("remote URL uses its selected remote"),
    }
}

/// The report shown for a completed, successful operation.
pub fn format_output(
    operation: GitOperation,
    stdout: &[u8],
    truncated: bool,
) -> Result<(String, bool)> {
    if operation.returns_value() {
        if truncated {
            bail!("git {} returned incomplete output", operation.as_str());
        }
        let raw = std::str::from_utf8(stdout)
            .context("git value is not valid UTF-8")?
            .trim_end_matches(['\r', '\n']);
        let value = if operation == GitOperation::BranchName {
            raw.strip_prefix("refs/heads/")
                .context("HEAD does not refer to a local branch")?
        } else {
            raw
        };
        if value.is_empty() || value.contains(['\r', '\n']) {
            bail!("git {} did not return a single value", operation.as_str());
        }
        if matches!(
            operation,
            GitOperation::CommitHash | GitOperation::ShortCommitHash
        ) {
            let valid_length = if operation == GitOperation::CommitHash {
                matches!(value.len(), 40 | 64)
            } else {
                (4..=64).contains(&value.len())
            };
            if !valid_length || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                bail!("git {} did not return a commit hash", operation.as_str());
            }
        }
        return Ok((value.to_owned(), false));
    }
    let stdout = String::from_utf8_lossy(stdout);
    let mut text = stdout.trim_end().to_owned();
    if text.is_empty() {
        text = match operation {
            GitOperation::Status => "Working tree clean.".into(),
            GitOperation::Log => "No commits yet.".into(),
            GitOperation::Diff => "No unstaged changes.".into(),
            GitOperation::StagedDiff => "No staged changes.".into(),
            GitOperation::Branches => "No local branches yet.".into(),
            _ => unreachable!("single-value operations handled above"),
        };
    }
    if truncated {
        text.push_str("\n… output truncated");
    }
    Ok((text, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_values_are_exact_and_never_copy_report_placeholders_or_partial_output() {
        assert_eq!(
            format_output(
                GitOperation::BranchName,
                "refs/heads/feature/中文\n".as_bytes(),
                false
            )
            .unwrap()
            .0,
            "feature/中文"
        );
        assert_eq!(
            format_output(
                GitOperation::CommitHash,
                b"0123456789abcdef0123456789abcdef01234567\r\n",
                false
            )
            .unwrap()
            .0,
            "0123456789abcdef0123456789abcdef01234567"
        );
        for (operation, bytes, truncated) in [
            (
                GitOperation::BranchName,
                b"refs/heads/feature\n".as_slice(),
                true,
            ),
            (
                GitOperation::BranchName,
                b"refs/tags/release\n".as_slice(),
                false,
            ),
            (
                GitOperation::BranchName,
                b"refs/heads/\xff\n".as_slice(),
                false,
            ),
            (
                GitOperation::BranchName,
                b"refs/heads/a\nrefs/heads/b\n".as_slice(),
                false,
            ),
            (GitOperation::CommitHash, b"".as_slice(), false),
            (GitOperation::CommitHash, b"HEAD\n".as_slice(), false),
            (
                GitOperation::ShortCommitHash,
                b"zzzzzzz\n".as_slice(),
                false,
            ),
        ] {
            assert!(format_output(operation, bytes, truncated).is_err());
        }
        assert_eq!(
            format_output(GitOperation::StagedDiff, b"", false)
                .unwrap()
                .0,
            "No staged changes."
        );
        assert_eq!(
            format_output(GitOperation::Branches, b"", false).unwrap().0,
            "No local branches yet."
        );
    }
}
