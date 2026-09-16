//! Repository actions: status, log and diff.
//!
//! Actions are declared as a bundle: the group they appear under plus the actions
//! themselves. Registering the bundle is all it takes for the feature to show up in search
//! and in the scope bar; nothing in the engine, the search pipeline or the launcher service
//! knows this feature exists.
use crate::core::action::bundle::{ActionBundle, ActionSpec};
use crate::core::action::descriptor::BuiltinExecutor;
use crate::core::action::group::groups;
use anywhere_protocol::*;

pub fn bundle() -> ActionBundle {
    ActionBundle {
        group: groups::git(),
        actions: vec![
            ActionSpec {
                id: "core.git.status",
                title: "Git Status",
                accepts: GIT_REPOSITORY,
                executor: BuiltinExecutor::GitStatus,
                base_priority: 50,
                icon: "git",
                aliases: &["status"],
                keywords: &["repository", "branch"],
            },
            ActionSpec {
                id: "core.git.log",
                title: "Git Log",
                accepts: GIT_REPOSITORY,
                executor: BuiltinExecutor::GitLog,
                base_priority: 25,
                icon: "git",
                aliases: &["log", "history", "commits"],
                keywords: &["repository"],
            },
            ActionSpec {
                id: "core.git.diff",
                title: "Git Diff",
                accepts: GIT_REPOSITORY,
                executor: BuiltinExecutor::GitDiff,
                base_priority: 15,
                icon: "git",
                aliases: &["diff", "changes"],
                keywords: &["repository", "worktree"],
            },
        ],
    }
}
