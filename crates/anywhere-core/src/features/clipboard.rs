//! App-owned clipboard history. Capturing is opt-in and independent of activation input.
pub(crate) mod content;
pub mod history;
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

pub const ACTION_ID: &str = "core.clipboard.history";
pub const PAGE_ID: &str = "clipboard.history";
pub const SHELF_ACTION_ID: &str = "core.clipboard.shelf";
pub const SHELF_PAGE_ID: &str = "clipboard.shelf";
pub struct Feature;
const DESCRIPTOR: FeatureDescriptor = FeatureDescriptor::new(
    "clipboard",
    "Clipboard History",
    &[
        Capability::Storage,
        Capability::ClipboardWrite,
        Capability::RenderUi,
    ],
);
struct OpenHistory(&'static str);
impl ActionHandler for OpenHistory {
    fn execute(
        &self,
        _: &ResolvedContext,
        _: &dyn anywhere_platform::Platform,
    ) -> anyhow::Result<ExecutionOutput> {
        Ok(ExecutionOutput::view(HostView::NativeTool {
            id: self.0.into(),
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
                    icon: Some("clipboard".into()),
                },
                actions: vec![
                    ActionSpec {
                        localized_titles: &[("zh", "剪贴板历史")],
                        id: ACTION_ID,
                        title: "Clipboard History",
                        accepts: crate::features::tools::ENTRY_CONTEXT,
                        handler: &OpenHistory(PAGE_ID),
                        availability: AvailabilityRule::Enabled,
                        base_priority: 30,
                        icon: "clipboard-history",
                        aliases: &["clipboard", "history", "剪贴板", "剪切板", "复制历史"],
                        keywords: &[],
                        home_eligible: false,
                        primary_action: false,
                    },
                    ActionSpec {
                        localized_titles: &[("zh", "剪贴板悬浮窗")],
                        id: SHELF_ACTION_ID,
                        title: "Clipboard Shelf",
                        accepts: crate::features::tools::ENTRY_CONTEXT,
                        handler: &OpenHistory(SHELF_PAGE_ID),
                        availability: AvailabilityRule::Enabled,
                        base_priority: 30,
                        icon: "clipboard-shelf",
                        aliases: &[
                            "clipboard shelf",
                            "floating clipboard",
                            "剪贴板悬浮窗",
                            "剪切板悬浮窗",
                            "常驻剪贴板",
                        ],
                        keywords: &[],
                        home_eligible: false,
                        primary_action: false,
                    },
                ],
            }],
            vec![],
        )
    }
}
