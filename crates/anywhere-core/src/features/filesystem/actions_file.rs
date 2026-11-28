//! File actions.
use crate::core::action::availability::AvailabilityRule;
use crate::core::action::bundle::{ActionBundle, ActionSpec};
use crate::core::action::handler::{ActionHandler, ExecutionOutput};
use crate::core::context::{ContextValue, ResolvedContext};
use crate::core::session::facts::CLIPBOARD;
use anyhow::{Context, Result};
use anywhere_platform::{paths::display_path, Platform};
use anywhere_protocol::*;

struct OpenFile;
struct RevealFile;
struct InspectPlugin;
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

impl ActionHandler for InspectPlugin {
    fn execute(&self, context: &ResolvedContext, _: &dyn Platform) -> Result<ExecutionOutput> {
        let ContextValue::File(path) = &context.value else {
            anyhow::bail!("plugin review requires a file context");
        };
        anyhow::ensure!(
            context.type_id == FILE
                && path.is_absolute()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("awplugin")),
            "plugin review requires a local .awplugin file"
        );
        let path = path
            .to_str()
            .context("plugin path is not representable")?
            .to_owned();
        Ok(ExecutionOutput::view(HostView::WebPackageReview { path }))
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
                .map(|path| display_path(path).to_string_lossy())
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
                localized_titles: &[("zh", "将所选路径复制为 JSON")],
                id: "core.file_selection.copy_paths",
                title: "Copy Selected Paths as JSON",
                accepts: FILE_SELECTION,
                handler: &CopySelectionPaths,
                availability: AvailabilityRule::hide_if(CLIPBOARD),
                base_priority: 30,
                icon: "copy-selection",
                aliases: &["copy paths", "selection"],
                keywords: &["files", "finder", "json"],
                primary_action: false,
            },
            ActionSpec {
                localized_titles: &[("zh", "打开文件")],
                id: "core.file.open",
                title: "Open File",
                accepts: FILE,
                handler: &OpenFile,
                availability: AvailabilityRule::Enabled,
                base_priority: 30,
                icon: "file-open",
                aliases: &["open", "edit"],
                keywords: &["document"],
                primary_action: true,
            },
            ActionSpec {
                localized_titles: &[("zh", "审查 Anywhere 插件")],
                id: "core.file.inspect_web_package",
                title: "Review Anywhere Plugin",
                accepts: FILE,
                handler: &InspectPlugin,
                availability: AvailabilityRule::FileExtension("awplugin"),
                base_priority: 25,
                icon: "code",
                aliases: &["install plugin", "import plugin"],
                keywords: &["awplugin"],
                primary_action: false,
            },
            ActionSpec {
                localized_titles: &[("zh", "在文件管理器中显示")],
                id: "core.file.reveal",
                title: "Reveal in File Manager",
                accepts: FILE,
                handler: &RevealFile,
                availability: AvailabilityRule::Enabled,
                base_priority: 20,
                icon: "file-reveal",
                aliases: &["reveal", "finder", "show"],
                keywords: &["document"],
                primary_action: false,
            },
        ],
    }
}
