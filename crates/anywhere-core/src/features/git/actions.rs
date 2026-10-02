//! Repository actions: status, log and diff.
use crate::core::action::availability::AvailabilityRule;
use crate::core::action::bundle::{ActionBundle, ActionSpec};
use crate::core::action::group::groups;
use crate::core::action::handler::{ActionHandler, ExecutionOutput};
use crate::core::context::ResolvedContext;
use anyhow::{Context, Result};
use anywhere_platform::{GitOperation, Platform};
use anywhere_protocol::*;

pub fn is_git_action(action_id: &str) -> bool {
    matches!(
        action_id,
        "core.git.status" | "core.git.log" | "core.git.diff"
    )
}

#[derive(Clone, Copy)]
struct Git {
    operation: GitOperation,
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
        let output = platform.run_git(repository, self.operation)?;
        Ok(ExecutionOutput::preview(output.text))
    }
}

const STATUS: Git = Git {
    operation: GitOperation::Status,
};
const LOG: Git = Git {
    operation: GitOperation::Log,
};
const DIFF: Git = Git {
    operation: GitOperation::Diff,
};

pub fn bundle() -> ActionBundle {
    ActionBundle {
        group: groups::git(),
        actions: vec![
            ActionSpec {
                id: "core.git.status",
                title: "Git Status",
                accepts: GIT_REPOSITORY,
                handler: &STATUS,
                availability: AvailabilityRule::require(
                    crate::core::session::facts::GIT,
                    crate::core::action::availability::GIT_UNAVAILABLE,
                ),
                base_priority: 50,
                icon: "git",
                aliases: &["status"],
                keywords: &["repository", "branch"],
                home_eligible: true,
                primary_action: false,
            },
            ActionSpec {
                id: "core.git.log",
                title: "Git Log",
                accepts: GIT_REPOSITORY,
                handler: &LOG,
                availability: AvailabilityRule::require(
                    crate::core::session::facts::GIT,
                    crate::core::action::availability::GIT_UNAVAILABLE,
                ),
                base_priority: 25,
                icon: "git",
                aliases: &["log", "history", "commits"],
                keywords: &["repository"],
                home_eligible: true,
                primary_action: false,
            },
            ActionSpec {
                id: "core.git.diff",
                title: "Git Diff",
                accepts: GIT_REPOSITORY,
                handler: &DIFF,
                availability: AvailabilityRule::require(
                    crate::core::session::facts::GIT,
                    crate::core::action::availability::GIT_UNAVAILABLE,
                ),
                base_priority: 15,
                icon: "git",
                aliases: &["diff", "changes"],
                keywords: &["repository", "worktree"],
                home_eligible: true,
                primary_action: false,
            },
        ],
    }
}
