//! App-owned history presentation; source formats and scripts stay behind a port.
pub mod model;
pub mod service;

use crate::{
    core::{
        action::{
            availability::AvailabilityRule,
            bundle::{ActionBundle, ActionSpec},
            group::{groups, ActionGroup},
            handler::{ActionHandler, ExecutionOutput},
        },
        context::ResolvedContext,
    },
    features::{contribution::FeatureContribution, descriptor::FeatureDescriptor, BuiltinFeature},
};
use anywhere_protocol::{extension::ExtensionCapability as Capability, HostView};

pub struct Feature;
const DESCRIPTOR: FeatureDescriptor = FeatureDescriptor::new(
    "application_history",
    "App History",
    &[
        Capability::Storage,
        Capability::RenderUi,
        Capability::OpenFile,
        Capability::ClipboardWrite,
    ],
);
struct Open;
impl ActionHandler for Open {
    fn execute(
        &self,
        _: &ResolvedContext,
        _: &dyn anywhere_platform::Platform,
    ) -> anyhow::Result<ExecutionOutput> {
        Ok(ExecutionOutput::view(HostView::NativeTool {
            id: "application.history".into(),
            input: String::new(),
        }))
    }
}
impl BuiltinFeature for Feature {
    fn descriptor(&self) -> &'static FeatureDescriptor {
        &DESCRIPTOR
    }
    fn contribution(&self) -> FeatureContribution {
        FeatureContribution::new(
            DESCRIPTOR.id,
            vec![ActionBundle {
                group: ActionGroup {
                    id: groups::TOOLS.into(),
                    title: "Tools".into(),
                    icon: Some("history".into()),
                },
                actions: vec![ActionSpec {
                    id: "core.application_history.open",
                    title: "App History",
                    localized_titles: &[("zh", "应用历史")],
                    accepts: crate::features::tools::ENTRY_CONTEXT,
                    handler: &Open,
                    availability: AvailabilityRule::Enabled,
                    base_priority: 30,
                    icon: "history",
                    aliases: &[
                        "recent projects",
                        "recent files",
                        "app history",
                        "最近项目",
                        "最近文件",
                        "应用历史",
                        "历史记录",
                    ],
                    keywords: &[],
                    home_eligible: false,
                    primary_action: false,
                }],
            }],
            vec![],
        )
    }
}
