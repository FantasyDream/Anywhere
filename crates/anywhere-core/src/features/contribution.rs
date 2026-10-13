//! Static first-party registration; this is not an external plugin ABI.
use crate::core::{
    action::bundle::{ActionBundle, ActionExpansion, ContextActionFactory},
    context::{provider::ContextProvider, ResolvedContext},
};
use std::sync::Arc;
pub struct FeatureContribution {
    pub id: &'static str,
    pub bundles: Vec<ActionBundle>,
    pub providers: Vec<Arc<dyn ContextProvider>>,
    pub factories: Vec<Arc<dyn ContextActionFactory>>,
    pub source: Option<SourceFactory>,
}
pub type SourceFactory = fn(
    Arc<dyn anywhere_platform::Platform>,
    Arc<crate::core::action::descriptor::ActionRegistry>,
) -> RuntimeSource;
pub struct RuntimeSource {
    pub source: Arc<dyn crate::core::search::candidate_source::AsyncCandidateSource>,
    pub available: bool,
}
impl FeatureContribution {
    fn new(
        id: &'static str,
        bundles: Vec<ActionBundle>,
        providers: Vec<Arc<dyn ContextProvider>>,
    ) -> Self {
        Self {
            id,
            bundles,
            providers,
            factories: vec![],
            source: None,
        }
    }
}
struct ProjectFactory(super::developer::provider::ProjectKind);
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
        id.starts_with(prefix)
            && super::developer::actions::DeveloperActions.supports_persistent_binding(id)
    }
    fn actions(&self, contexts: &[ResolvedContext]) -> ActionExpansion {
        let contexts: Vec<_> = contexts
            .iter()
            .filter(|c| c.type_id == self.0.type_id())
            .cloned()
            .collect();
        super::developer::actions::DeveloperActions.actions(&contexts)
    }
}
pub fn builtins() -> Vec<FeatureContribution> {
    use super::*;
    use developer::provider::{ProjectKind, ProjectProvider};
    let mut filesystem = FeatureContribution::new(
        "filesystem",
        vec![
            filesystem::actions_folder::bundle(),
            filesystem::actions_file::bundle(),
            filesystem::workspace::bundle(),
        ],
        vec![Arc::new(filesystem::provider::PathProvider::default())],
    );
    filesystem.source = Some(|platform, registry| RuntimeSource {
        available: platform.file_search_supported(),
        source: Arc::new(filesystem::search::FileSearchSource::new(
            platform, registry,
        )),
    });
    let mut result = vec![
        FeatureContribution::new(
            "git",
            vec![git::actions::bundle()],
            vec![Arc::new(git::provider::GitRepositoryProvider::default())],
        ),
        filesystem,
        FeatureContribution::new(
            "url",
            vec![url::actions::bundle()],
            vec![Arc::new(url::provider::UrlProvider::new())],
        ),
        FeatureContribution::new(
            "json",
            vec![json::actions::bundle()],
            vec![Arc::new(json::provider::JsonProvider::new())],
        ),
        FeatureContribution::new(
            "text",
            vec![],
            vec![Arc::new(text::provider::TextProvider::default())],
        ),
    ];
    for (id, kind, bundle) in [
        ("rust", ProjectKind::Rust, developer::actions::rust_bundle()),
        (
            "maven",
            ProjectKind::Maven,
            developer::actions::maven_bundle(),
        ),
        (
            "gradle",
            ProjectKind::Gradle,
            developer::actions::gradle_bundle(),
        ),
        ("node", ProjectKind::Node, developer::actions::node_bundle()),
        (
            "spring_boot",
            ProjectKind::SpringBoot,
            developer::actions::spring_bundle(),
        ),
    ] {
        let mut feature =
            FeatureContribution::new(id, vec![bundle], vec![Arc::new(ProjectProvider::new(kind))]);
        if matches!(kind, ProjectKind::Node | ProjectKind::Rust) {
            feature.factories.push(Arc::new(ProjectFactory(kind)));
        }
        result.push(feature);
    }
    result
}
