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

use super::FirstPartyExtension;
use crate::{
    core::{
        action::bundle::{ActionExpansion, ContextActionFactory},
        context::ResolvedContext,
    },
    features::contribution::FeatureContribution,
};
use anywhere_protocol::extension::FirstPartyManifest;
use std::sync::{Arc, OnceLock};
pub struct Extension(pub provider::ProjectKind);
pub fn manifest(kind: provider::ProjectKind) -> &'static FirstPartyManifest {
    macro_rules! load {
        ($id:literal, $slot:ident) => {{
            static $slot: OnceLock<FirstPartyManifest> = OnceLock::new();
            $slot.get_or_init(|| {
                anywhere_plugin::extension::parse_first_party_manifest(include_str!(concat!(
                    "../../../../../extensions/",
                    $id,
                    "/manifest.toml"
                )))
                .expect("invalid compiled developer manifest")
            })
        }};
    }
    match kind {
        provider::ProjectKind::Rust => load!("rust", RUST),
        provider::ProjectKind::Maven => load!("maven", MAVEN),
        provider::ProjectKind::Gradle => load!("gradle", GRADLE),
        provider::ProjectKind::Node => load!("node", NODE),
        provider::ProjectKind::SpringBoot => load!("spring_boot", SPRING),
    }
}
impl FirstPartyExtension for Extension {
    fn manifest(&self) -> &'static FirstPartyManifest {
        manifest(self.0)
    }
    fn contribution(&self) -> FeatureContribution {
        let bundle = match self.0 {
            provider::ProjectKind::Rust => actions::rust_bundle(),
            provider::ProjectKind::Maven => actions::maven_bundle(),
            provider::ProjectKind::Gradle => actions::gradle_bundle(),
            provider::ProjectKind::Node => actions::node_bundle(),
            provider::ProjectKind::SpringBoot => actions::spring_bundle(),
        };
        let id = match self.0 {
            provider::ProjectKind::Rust => "rust",
            provider::ProjectKind::Maven => "maven",
            provider::ProjectKind::Gradle => "gradle",
            provider::ProjectKind::Node => "node",
            provider::ProjectKind::SpringBoot => "spring_boot",
        };
        let mut contribution = FeatureContribution::new(
            id,
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
