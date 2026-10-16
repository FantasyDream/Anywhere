//! A searchable, executable entry into a Host-rendered file workspace.
use crate::core::{
    action::{
        availability::AvailabilityRule,
        bundle::{ActionBundle, ActionSpec},
        group::ActionGroup,
        handler::{ActionHandler, ExecutionOutput},
    },
    context::ResolvedContext,
};
use anywhere_protocol::*;

use crate::features::tools::ENTRY_CONTEXT;
pub const ACTION_ID: &str = "core.files.search";
struct OpenSearch;
impl ActionHandler for OpenSearch {
    fn execute(
        &self,
        _: &ResolvedContext,
        _: &dyn anywhere_platform::Platform,
    ) -> anyhow::Result<ExecutionOutput> {
        Ok(ExecutionOutput::view(HostView::FileSearch))
    }
}
pub fn bundle() -> ActionBundle {
    ActionBundle {
        group: ActionGroup {
            id: crate::core::action::group::groups::TOOLS.into(),
            title: "Tools".into(),
            icon: Some("search".into()),
        },
        actions: vec![ActionSpec {
            id: ACTION_ID,
            title: "File Search",
            accepts: ENTRY_CONTEXT,
            handler: &OpenSearch,
            availability: AvailabilityRule::Enabled,
            base_priority: 30,
            icon: "search",
            aliases: &["files", "文件搜索", "搜索文件"],
            keywords: &[],
            home_eligible: false,
            primary_action: false,
        }],
    }
}
