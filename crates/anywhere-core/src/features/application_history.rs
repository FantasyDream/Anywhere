//! Each source contributes an app-owned entry; the reader and page stay shared.
pub mod model;
pub mod service;

use crate::{
    core::{
        action::{
            availability::{self, AvailabilityRule},
            descriptor::{ActionDefinition, ActionExecutor},
            group::groups,
            handler::{ActionHandler, ActionHandlerRef, ExecutionOutput},
        },
        context::ResolvedContext,
        session::facts::SessionFacts,
    },
    features::{
        contribution::FeatureContribution, descriptor::FeatureDescriptor,
        services::FeatureServices, BuiltinFeature, FeatureBindings,
    },
};
use anywhere_protocol::{
    extension::ExtensionCapability as Capability, ContextMatcher, HostView, APPLICATION,
};
use std::sync::Arc;

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
struct OpenSource {
    service: Arc<service::HistoryService>,
    source_id: String,
    availability: AvailabilityRule,
}
impl ActionHandler for OpenSource {
    fn execute(
        &self,
        context: &ResolvedContext,
        _: &dyn anywhere_platform::Platform,
    ) -> anyhow::Result<ExecutionOutput> {
        anyhow::ensure!(
            availability::evaluate(&self.availability, context, &SessionFacts::default())
                .is_executable(),
            "history action does not match this application"
        );
        let input = self.service.page_input(&self.source_id)?;
        Ok(ExecutionOutput::view(HostView::NativeTool {
            id: "application.history".into(),
            input: serde_json::to_string(&input)?,
        }))
    }
}
impl BuiltinFeature for Feature {
    fn descriptor(&self) -> &'static FeatureDescriptor {
        &DESCRIPTOR
    }
    fn contribution(&self) -> FeatureContribution {
        FeatureContribution::new(DESCRIPTOR.id, vec![], vec![])
    }
    fn bind_services(&self, services: &mut FeatureServices) -> anyhow::Result<FeatureBindings> {
        let service = services.application_history()?;
        let actions = service
            .sources()
            .into_iter()
            .map(|(source, _)| {
                let availability = AvailabilityRule::ForApplications {
                    bundle_ids: source.applications.clone(),
                    entry_context: Some(crate::features::tools::ENTRY_CONTEXT.into()),
                };
                ActionDefinition {
                    id: format!("core.application_history.{}.open", source.id),
                    title: source.action.title,
                    localized_titles: source.action.localized_titles,
                    subtitle: None,
                    shortcut: None,
                    accepts: [APPLICATION, crate::features::tools::ENTRY_CONTEXT]
                        .into_iter()
                        .map(|type_id| ContextMatcher {
                            type_id: type_id.into(),
                        })
                        .collect(),
                    executor: ActionExecutor::Native(ActionHandlerRef::owned(OpenSource {
                        service: service.clone(),
                        source_id: source.id,
                        availability: availability.clone(),
                    })),
                    availability,
                    base_priority: 90,
                    icon: "history".into(),
                    group_id: groups::APP.into(),
                    aliases: source.action.aliases,
                    keywords: vec![],
                    owning_app: source.applications.first().cloned(),
                    menu_path: vec![],
                    home_eligible: true,
                    primary_action: false,
                    search_title: None,
                }
            })
            .collect();
        Ok(FeatureBindings {
            actions,
            ..Default::default()
        })
    }
}
