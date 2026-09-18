//! File actions.
use crate::core::action::availability::AvailabilityRule;
use crate::core::action::bundle::{ActionBundle, ActionSpec};
use crate::core::action::group::groups;
use crate::core::action::handler::{ActionHandler, ExecutionOutput};
use crate::core::context::ResolvedContext;
use anyhow::{Context, Result};
use anywhere_platform::Platform;
use anywhere_protocol::*;

struct OpenFile;
struct RevealFile;

impl ActionHandler for OpenFile {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        platform.open_file(context.value.path().context("file context required")?)?;
        Ok(ExecutionOutput::dismiss())
    }
}

impl ActionHandler for RevealFile {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        platform.reveal_file(context.value.path().context("file context required")?)?;
        Ok(ExecutionOutput::dismiss())
    }
}

pub fn bundle() -> ActionBundle {
    ActionBundle {
        group: groups::file(),
        actions: vec![
            ActionSpec {
                id: "core.file.open",
                title: "Open File",
                accepts: FILE,
                handler: &OpenFile,
                availability: AvailabilityRule::Enabled,
                base_priority: 30,
                icon: "file",
                aliases: &["open", "edit"],
                keywords: &["document"],
            },
            ActionSpec {
                id: "core.file.reveal",
                title: "Reveal in File Manager",
                accepts: FILE,
                handler: &RevealFile,
                availability: AvailabilityRule::Enabled,
                base_priority: 20,
                icon: "folder",
                aliases: &["reveal", "finder", "show"],
                keywords: &["document"],
            },
        ],
    }
}
