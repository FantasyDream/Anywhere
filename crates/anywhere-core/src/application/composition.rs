//! Process-level wiring: the action registry and the provider set a session starts with.
use crate::core::{
    action::descriptor::ActionRegistry, context::provider::ContextProvider,
    context::registry::ContextProviderRegistry,
};
use crate::features::filesystem::provider::{CurrentDirectoryProvider, DirectoryMemo};
use std::sync::Arc;

pub fn builtin_registry() -> ActionRegistry {
    configured_builtins(&Default::default()).0
}

/// Actions and recognizers are assembled from the same enabled contributions.
pub(crate) fn configured_builtins(
    disabled: &std::collections::BTreeSet<String>,
) -> (ActionRegistry, ContextProviderRegistry) {
    let mut registry = ActionRegistry::default();
    let mut providers = ContextProviderRegistry::default();
    for contribution in crate::features::contribution::builtins() {
        if contribution.configurable
            && disabled.contains(&format!("first-party:{}", contribution.id))
        {
            continue;
        }
        append_contribution(&mut registry, &mut providers, contribution)
            .expect("invalid built-in contribution");
    }
    (registry, providers)
}

pub(crate) fn append_contribution(
    registry: &mut ActionRegistry,
    providers: &mut ContextProviderRegistry,
    contribution: crate::features::contribution::FeatureContribution,
) -> anyhow::Result<()> {
    let owner = contribution.configurable.then_some(contribution.id);
    for rule in contribution.input_recognizers {
        providers.register_input_recognizer(rule, owner)?;
    }
    for provider in contribution.providers {
        providers.register_owned(provider, owner)?;
    }
    for bundle in contribution.bundles {
        registry.register_owned_bundle(bundle, owner)?;
    }
    for factory in contribution.factories {
        registry.register_owned_factory(factory, owner);
    }
    Ok(())
}

pub(super) fn builtin_sources(
    platform: Arc<dyn anywhere_platform::Platform>,
    registry: Arc<ActionRegistry>,
) -> (
    Arc<crate::features::filesystem::search::FileSearchSource>,
    Vec<Arc<dyn crate::core::search::candidate_source::AsyncCandidateSource>>,
) {
    let supported = platform.file_search_supported();
    let files = Arc::new(crate::features::filesystem::search::FileSearchSource::new(
        platform, registry,
    ));
    let sources: Vec<Arc<dyn crate::core::search::candidate_source::AsyncCandidateSource>> =
        if supported {
            vec![files.clone()]
        } else {
            vec![]
        };
    (files, sources)
}

pub(super) fn builtin_providers() -> ContextProviderRegistry {
    configured_builtins(&Default::default()).1
}

pub(super) fn session_providers(
    mut registry: ContextProviderRegistry,
    directory: Option<Arc<DirectoryMemo>>,
) -> Vec<Arc<dyn ContextProvider>> {
    if let Some(directory) = directory {
        let _ = registry.register(Arc::new(CurrentDirectoryProvider::new(directory)));
    }
    registry.providers().to_vec()
}
