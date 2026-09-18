//! Directory actions.
use crate::core::action::availability::AvailabilityRule;
use crate::core::action::bundle::{ActionBundle, ActionSpec};
use crate::core::action::group::groups;
use crate::core::action::handler::{ActionHandler, ExecutionOutput};
use crate::core::context::ResolvedContext;
use anyhow::{Context, Result};
use anywhere_platform::Platform;
use anywhere_protocol::*;

struct OpenTerminal;
struct CopyPath;
struct OpenDirectory;

impl ActionHandler for OpenTerminal {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        platform.open_terminal(context.value.path().context("directory context required")?)?;
        Ok(ExecutionOutput::dismiss())
    }
}

impl ActionHandler for CopyPath {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        let path = context.value.path().context("path context required")?;
        platform.copy_text(&path.to_string_lossy())?;
        Ok(ExecutionOutput::dismiss())
    }
}

impl ActionHandler for OpenDirectory {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        platform.open_directory(context.value.path().context("directory context required")?)?;
        Ok(ExecutionOutput::dismiss())
    }
}

pub fn bundle() -> ActionBundle {
    ActionBundle {
        group: groups::folder(),
        actions: vec![
            ActionSpec {
                id: "core.directory.terminal",
                title: "Open Terminal Here",
                accepts: DIRECTORY,
                handler: &OpenTerminal,
                availability: AvailabilityRule::Enabled,
                base_priority: 30,
                icon: "terminal",
                aliases: &["terminal", "shell", "console"],
                keywords: &["folder"],
            },
            ActionSpec {
                id: "core.directory.copy",
                title: "Copy Path",
                accepts: DIRECTORY,
                handler: &CopyPath,
                availability: AvailabilityRule::hide_if(crate::core::session::facts::CLIPBOARD),
                base_priority: 20,
                icon: "copy",
                aliases: &["copy", "path", "clipboard"],
                keywords: &["folder"],
            },
            ActionSpec {
                id: "core.directory.open",
                title: "Open Directory",
                accepts: DIRECTORY,
                handler: &OpenDirectory,
                availability: AvailabilityRule::Enabled,
                base_priority: 10,
                icon: "folder",
                aliases: &["open", "reveal", "finder"],
                keywords: &["folder"],
            },
            ActionSpec {
                id: "core.file.copy",
                title: "Copy Path",
                accepts: FILE,
                handler: &CopyPath,
                availability: AvailabilityRule::hide_if(crate::core::session::facts::CLIPBOARD),
                base_priority: 10,
                icon: "copy",
                aliases: &["copy", "path", "clipboard"],
                keywords: &["document"],
            },
        ],
    }
}
