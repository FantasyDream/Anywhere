//! Shared Git argv and result formatting.
//!
//! Only the argv allowlist and the result formatting live here; spawning, pipes and
//! deadlines stay per platform because they use different process APIs.

use crate::GitOperation;
use std::path::Path;
use std::process::Command;

/// Read-only, non-interactive argv for one operation. Never a shell, never a pager.
pub fn command(program: &Path, repository: &Path, operation: GitOperation) -> Command {
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
    }
    // Honour the selected repository, not an inherited one.
    command
        .env("LC_ALL", "en_US.UTF-8")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_PAGER", "cat")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    command
}

/// The report shown for a completed, successful operation.
pub fn format_output(operation: GitOperation, stdout: &[u8], truncated: bool) -> (String, bool) {
    let stdout = String::from_utf8_lossy(stdout);
    let mut text = stdout.trim_end().to_owned();
    if text.is_empty() {
        text = match operation {
            GitOperation::Status => "Working tree clean.".into(),
            GitOperation::Log => "No commits yet.".into(),
            GitOperation::Diff => "No unstaged changes.".into(),
        };
    }
    if truncated {
        text.push_str("\n… output truncated");
    }
    (text, truncated)
}
