//! Read-only repository reports and clipboard actions.
use crate::core::action::availability::AvailabilityRule;
use crate::core::action::bundle::{ActionBundle, ActionSpec};
use crate::core::action::handler::{ActionHandler, ExecutionOutput};
use crate::core::context::ResolvedContext;
use anyhow::{bail, Context, Result};
use anywhere_platform::{GitOperation, Platform};
use anywhere_protocol::*;

pub fn is_git_action(action_id: &str) -> bool {
    matches!(
        action_id,
        "core.git.status"
            | "core.git.log"
            | "core.git.diff"
            | "core.git.staged_diff"
            | "core.git.branches"
            | "core.git.copy_branch_name"
            | "core.git.copy_commit_hash"
            | "core.git.copy_short_commit_hash"
            | "core.git.copy_repository_path"
            | "core.git.copy_remote_url"
    )
}

#[derive(Clone, Copy)]
enum Destination {
    Preview,
    Clipboard,
}

struct Git {
    operation: GitOperation,
    destination: Destination,
}

impl Git {
    const fn preview(operation: GitOperation) -> Self {
        Self {
            operation,
            destination: Destination::Preview,
        }
    }
    const fn copy(operation: GitOperation) -> Self {
        Self {
            operation,
            destination: Destination::Clipboard,
        }
    }
}

impl ActionHandler for Git {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        let repository = context
            .value
            .path()
            .context("repository context required")?;
        let host = crate::features::host_services::for_feature(super::descriptor(), platform)?;
        let output = host.run_git(repository, self.operation)?;
        match self.destination {
            Destination::Preview => Ok(ExecutionOutput::preview(output.text)),
            Destination::Clipboard => {
                let text = output.text.trim_end_matches(['\r', '\n']);
                if output.truncated || text.is_empty() || text.contains(['\r', '\n']) {
                    bail!("git did not return a complete single value; clipboard unchanged");
                }
                host.copy_text(text)?;
                Ok(ExecutionOutput::dismiss())
            }
        }
    }
}

const STATUS: Git = Git::preview(GitOperation::Status);
const LOG: Git = Git::preview(GitOperation::Log);
const DIFF: Git = Git::preview(GitOperation::Diff);
const STAGED_DIFF: Git = Git::preview(GitOperation::StagedDiff);
const BRANCHES: Git = Git::preview(GitOperation::Branches);
const COPY_BRANCH: Git = Git::copy(GitOperation::BranchName);
const COPY_COMMIT: Git = Git::copy(GitOperation::CommitHash);
const COPY_SHORT_COMMIT: Git = Git::copy(GitOperation::ShortCommitHash);
const COPY_REMOTE: Git = Git::copy(GitOperation::RemoteUrl);

struct CopyRepositoryPath;
impl ActionHandler for CopyRepositoryPath {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        let path = context
            .value
            .path()
            .context("repository context required")?;
        crate::features::host_services::for_feature(super::descriptor(), platform)?.copy_text(
            path.to_str()
                .context("repository path is not valid UTF-8")?,
        )?;
        Ok(ExecutionOutput::dismiss())
    }
}

pub fn bundle() -> ActionBundle {
    ActionBundle {
        group: crate::features::groups::git(),
        actions: vec![
            ActionSpec {
                localized_titles: &[("zh", "复制远程仓库地址")],
                id: "core.git.copy_remote_url",
                title: "Copy Remote URL",
                accepts: GIT_REPOSITORY,
                handler: &COPY_REMOTE,
                availability: AvailabilityRule::require(
                    crate::core::session::facts::GIT,
                    crate::core::action::availability::GIT_UNAVAILABLE,
                ),
                base_priority: 5,
                icon: "copy-url",
                aliases: &["remote url", "origin", "clone url", "复制远程地址"],
                keywords: &["git", "repository", "clipboard", "remote"],
                primary_action: false,
            },
            ActionSpec {
                localized_titles: &[("zh", "Git 状态")],
                id: "core.git.status",
                title: "Git Status",
                accepts: GIT_REPOSITORY,
                handler: &STATUS,
                availability: AvailabilityRule::require(
                    crate::core::session::facts::GIT,
                    crate::core::action::availability::GIT_UNAVAILABLE,
                ),
                base_priority: 50,
                icon: "git-status",
                aliases: &["status"],
                keywords: &["repository", "branch"],
                primary_action: false,
            },
            ActionSpec {
                localized_titles: &[("zh", "Git 日志")],
                id: "core.git.log",
                title: "Git Log",
                accepts: GIT_REPOSITORY,
                handler: &LOG,
                availability: AvailabilityRule::require(
                    crate::core::session::facts::GIT,
                    crate::core::action::availability::GIT_UNAVAILABLE,
                ),
                base_priority: 25,
                icon: "git-log",
                aliases: &["log", "history", "commits"],
                keywords: &["repository"],
                primary_action: false,
            },
            ActionSpec {
                localized_titles: &[("zh", "Git 差异")],
                id: "core.git.diff",
                title: "Git Diff",
                accepts: GIT_REPOSITORY,
                handler: &DIFF,
                availability: AvailabilityRule::require(
                    crate::core::session::facts::GIT,
                    crate::core::action::availability::GIT_UNAVAILABLE,
                ),
                base_priority: 15,
                icon: "git-diff",
                aliases: &["diff", "changes"],
                keywords: &["repository", "worktree"],
                primary_action: false,
            },
            ActionSpec {
                localized_titles: &[("zh", "Git 暂存区差异")],
                id: "core.git.staged_diff",
                title: "Git Staged Diff",
                accepts: GIT_REPOSITORY,
                handler: &STAGED_DIFF,
                availability: AvailabilityRule::require(
                    crate::core::session::facts::GIT,
                    crate::core::action::availability::GIT_UNAVAILABLE,
                ),
                base_priority: 5,
                icon: "git-staged",
                aliases: &["staged diff", "cached diff", "index changes", "暂存区差异"],
                keywords: &["repository", "staged"],
                primary_action: false,
            },
            ActionSpec {
                localized_titles: &[("zh", "Git 分支列表")],
                id: "core.git.branches",
                title: "Git Branches",
                accepts: GIT_REPOSITORY,
                handler: &BRANCHES,
                availability: AvailabilityRule::require(
                    crate::core::session::facts::GIT,
                    crate::core::action::availability::GIT_UNAVAILABLE,
                ),
                base_priority: 5,
                icon: "git-branches",
                aliases: &["branches", "local branches", "分支列表"],
                keywords: &["repository", "branch"],
                primary_action: false,
            },
            ActionSpec {
                localized_titles: &[("zh", "复制分支名")],
                id: "core.git.copy_branch_name",
                title: "Copy Branch Name",
                accepts: GIT_REPOSITORY,
                handler: &COPY_BRANCH,
                availability: AvailabilityRule::require(
                    crate::core::session::facts::GIT,
                    crate::core::action::availability::GIT_UNAVAILABLE,
                ),
                base_priority: 5,
                icon: "copy-branch",
                aliases: &["current branch", "branch name", "复制分支名"],
                keywords: &["git", "repository", "clipboard"],
                primary_action: false,
            },
            ActionSpec {
                localized_titles: &[("zh", "复制提交哈希")],
                id: "core.git.copy_commit_hash",
                title: "Copy Commit Hash",
                accepts: GIT_REPOSITORY,
                handler: &COPY_COMMIT,
                availability: AvailabilityRule::require(
                    crate::core::session::facts::GIT,
                    crate::core::action::availability::GIT_UNAVAILABLE,
                ),
                base_priority: 0,
                icon: "copy-commit",
                aliases: &["commit hash", "sha", "复制提交哈希"],
                keywords: &["git", "repository", "clipboard", "head"],
                primary_action: false,
            },
            ActionSpec {
                localized_titles: &[("zh", "复制短提交哈希")],
                id: "core.git.copy_short_commit_hash",
                title: "Copy Short Commit Hash",
                accepts: GIT_REPOSITORY,
                handler: &COPY_SHORT_COMMIT,
                availability: AvailabilityRule::require(
                    crate::core::session::facts::GIT,
                    crate::core::action::availability::GIT_UNAVAILABLE,
                ),
                base_priority: 0,
                icon: "copy-short-commit",
                aliases: &["short sha", "short hash", "复制短哈希"],
                keywords: &["git", "repository", "clipboard", "head"],
                primary_action: false,
            },
            ActionSpec {
                localized_titles: &[("zh", "复制仓库路径")],
                id: "core.git.copy_repository_path",
                title: "Copy Repository Path",
                accepts: GIT_REPOSITORY,
                handler: &CopyRepositoryPath,
                availability: AvailabilityRule::hide_if(crate::core::session::facts::CLIPBOARD),
                base_priority: 0,
                icon: "copy-path",
                aliases: &["repo path", "repository root", "复制仓库路径"],
                keywords: &["git", "clipboard", "root"],
                primary_action: false,
            },
        ],
    }
}
