//! File actions.
use crate::core::action::availability::AvailabilityRule;
use crate::core::action::bundle::{ActionBundle, ActionSpec};
use crate::core::action::handler::{ActionHandler, ExecutionOutput};
use crate::core::context::{ContextValue, ResolvedContext};
use crate::core::session::facts::CLIPBOARD;
use anyhow::{Context, Result};
use anywhere_platform::Platform;
use anywhere_protocol::*;

struct OpenFile;
struct RevealFile;
struct CopySelectionPaths;

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

impl ActionHandler for CopySelectionPaths {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        let ContextValue::FileSelection(paths) = &context.value else {
            anyhow::bail!("Finder selection context required");
        };
        let json = serde_json::to_string(
            &paths
                .iter()
                .map(|path| path.to_string_lossy())
                .collect::<Vec<_>>(),
        )?;
        platform.copy_text(&json)?;
        Ok(ExecutionOutput::dismiss())
    }
}

pub fn bundle() -> ActionBundle {
    ActionBundle {
        group: crate::features::groups::file(),
        actions: vec![
            ActionSpec {
                id: "core.file_selection.copy_paths",
                title: "Copy Selected Paths as JSON",
                accepts: FILE_SELECTION,
                handler: &CopySelectionPaths,
                availability: AvailabilityRule::hide_if(CLIPBOARD),
                base_priority: 30,
                icon: "copy",
                aliases: &["copy paths", "selection"],
                keywords: &["files", "finder", "json"],
                home_eligible: true,
                primary_action: false,
            },
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
                home_eligible: true,
                primary_action: true,
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
                home_eligible: true,
                primary_action: false,
            },
        ],
    }
}
