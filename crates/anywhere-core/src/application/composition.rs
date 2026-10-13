//! Process-level wiring: the action registry and the provider set a session starts with.
use crate::core::{
    action::descriptor::ActionRegistry, context::provider::ContextProvider,
    context::registry::ContextProviderRegistry,
};
use crate::features::filesystem::provider::{CurrentDirectoryProvider, DirectoryMemo};
use std::sync::Arc;

pub fn builtin_registry() -> ActionRegistry {
    let mut registry = ActionRegistry::default();
    for contribution in crate::features::contribution::builtins() {
        for bundle in contribution.bundles {
            registry
                .register_bundle(bundle)
                .expect("invalid built-in action registration");
        }
        for factory in contribution.factories {
            registry.register_factory_ref(factory);
        }
    }
    registry
}

pub(super) fn builtin_sources(
    platform: Arc<dyn anywhere_platform::Platform>,
    registry: Arc<ActionRegistry>,
) -> (
    Arc<crate::features::filesystem::search::FileSearchSource>,
    Vec<Arc<dyn crate::core::search::candidate_source::AsyncCandidateSource>>,
) {
    let mut sources = Vec::new();
    let mut files = None;
    for feature in crate::features::contribution::builtins() {
        if let Some(factory) = feature.source {
            let contribution = factory(platform.clone(), registry.clone());
            let source = contribution.source;
            let service: Arc<dyn std::any::Any + Send + Sync> = source.clone();
            if let Ok(file_service) =
                service.downcast::<crate::features::filesystem::search::FileSearchSource>()
            {
                files = Some(file_service);
            }
            if contribution.available {
                sources.push(source);
            }
        }
    }
    (files.expect("filesystem contribution missing"), sources)
}

pub(super) fn builtin_providers() -> ContextProviderRegistry {
    let mut registry = ContextProviderRegistry::default();
    for provider in crate::features::builtin_providers() {
        registry
            .register(provider)
            .expect("invalid built-in provider registration");
    }
    registry
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
