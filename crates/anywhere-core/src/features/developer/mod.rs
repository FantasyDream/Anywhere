//! Developer contexts only inspect metadata; commands start exclusively through Actions.
pub mod actions;
pub mod provider;

mod dynamic;
mod metadata;
pub mod node;
pub mod rust;
mod spring;

struct ProjectFactory(provider::ProjectKind);
impl ContextActionFactory for ProjectFactory {
    fn id(&self) -> &'static str {
        self.0.type_id()
    }
    fn supports_persistent_binding(&self, id: &str) -> bool {
        let prefix = if self.0.type_id() == anywhere_protocol::PROJECT_NODE {
            "developer.node."
        } else {
            "developer.cargo."
        };
        id.starts_with(prefix) && actions::DeveloperActions.supports_persistent_binding(id)
    }
    fn actions(&self, contexts: &[ResolvedContext]) -> ActionExpansion {
        let contexts: Vec<_> = contexts
            .iter()
            .filter(|c| c.type_id == self.0.type_id())
            .cloned()
            .collect();
        actions::DeveloperActions.actions(&contexts)
    }
}

use crate::features::descriptor::FeatureDescriptor;
use crate::features::BuiltinFeature;
use crate::{
    core::{
        action::bundle::{ActionExpansion, ContextActionFactory},
        context::ResolvedContext,
    },
    features::contribution::FeatureContribution,
};
use anywhere_protocol::extension::ExtensionCapability as Capability;
use std::sync::Arc;
pub struct Feature(pub provider::ProjectKind);
const RUST: FeatureDescriptor = FeatureDescriptor::new("rust", "Rust", &[Capability::ProcessStart]);
const MAVEN: FeatureDescriptor =
    FeatureDescriptor::new("maven", "Maven", &[Capability::ProcessStart]);
const GRADLE: FeatureDescriptor =
    FeatureDescriptor::new("gradle", "Gradle", &[Capability::ProcessStart]);
const NODE: FeatureDescriptor = FeatureDescriptor::new("node", "Node", &[Capability::ProcessStart]);
const SPRING_BOOT: FeatureDescriptor =
    FeatureDescriptor::new("spring_boot", "Spring Boot", &[Capability::ProcessStart]);
pub fn descriptor(kind: provider::ProjectKind) -> &'static FeatureDescriptor {
    match kind {
        provider::ProjectKind::Rust => &RUST,
        provider::ProjectKind::Maven => &MAVEN,
        provider::ProjectKind::Gradle => &GRADLE,
        provider::ProjectKind::Node => &NODE,
        provider::ProjectKind::SpringBoot => &SPRING_BOOT,
    }
}
impl BuiltinFeature for Feature {
    fn descriptor(&self) -> &'static FeatureDescriptor {
        descriptor(self.0)
    }
    fn contribution(&self) -> FeatureContribution {
        let bundle = match self.0 {
            provider::ProjectKind::Rust => actions::rust_bundle(),
            provider::ProjectKind::Maven => actions::maven_bundle(),
            provider::ProjectKind::Gradle => actions::gradle_bundle(),
            provider::ProjectKind::Node => actions::node_bundle(),
            provider::ProjectKind::SpringBoot => actions::spring_bundle(),
        };
        let mut contribution = FeatureContribution::new(
            self.descriptor().id,
            vec![bundle],
            vec![Arc::new(provider::ProjectProvider::new(self.0))],
        );
        if matches!(
            self.0,
            provider::ProjectKind::Node | provider::ProjectKind::Rust
        ) {
            contribution
                .factories
                .push(Arc::new(ProjectFactory(self.0)));
        }
        contribution
    }
}
