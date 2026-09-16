//! Directory actions.
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
        group: groups::folder(),
        actions: vec![
            ActionSpec {
                id: "core.directory.terminal",
                title: "Open Terminal Here",
                accepts: DIRECTORY,
                executor: BuiltinExecutor::OpenTerminal,
                base_priority: 30,
                icon: "terminal",
                aliases: &["terminal", "shell", "console"],
                keywords: &["folder"],
            },
            ActionSpec {
                id: "core.directory.copy",
                title: "Copy Path",
                accepts: DIRECTORY,
                executor: BuiltinExecutor::CopyPath,
                base_priority: 20,
                icon: "copy",
                aliases: &["copy", "path", "clipboard"],
                keywords: &["folder"],
            },
            ActionSpec {
                id: "core.directory.open",
                title: "Open Directory",
                accepts: DIRECTORY,
                executor: BuiltinExecutor::OpenDirectory,
                base_priority: 10,
                icon: "folder",
                aliases: &["open", "reveal", "finder"],
                keywords: &["folder"],
            },
            ActionSpec {
                id: "core.file.copy",
                title: "Copy Path",
                accepts: FILE,
                executor: BuiltinExecutor::CopyPath,
                base_priority: 10,
                icon: "copy",
                // Both "Copy Path" actions share one group: they do the same thing to a path.
                aliases: &["copy", "path", "clipboard"],
                keywords: &["document"],
            },
        ],
    }
}
