//! Product composition. Core algorithms never register concrete features themselves.
use super::session_model::LauncherSession;
use crate::core::{
    action::descriptor::ActionRegistry, context::provider::ContextProvider,
    context::registry::ContextProviderRegistry, context::ResolvedContext,
    ranking::score::UsageStats,
};
use crate::features::applications::index::ApplicationCatalog;
use crate::features::applications::source::ApplicationSource;
use crate::features::filesystem::provider::{CurrentDirectoryProvider, DirectoryMemo};
use std::sync::Arc;

pub fn builtin_registry() -> ActionRegistry {
    let mut registry = ActionRegistry::default();
    for bundle in crate::features::builtin_action_bundles() {
        for action in bundle.descriptors() {
            registry
                .register(action)
                .expect("invalid built-in action registration");
        }
    }
    registry
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

pub(super) fn session_seeds(session: &LauncherSession, input: &str) -> Vec<ResolvedContext> {
    let mut seeds = session.context_cache.clone();
    if let Some(frontmost) = &session.frontmost {
        seeds.push(crate::features::applications::context::frontmost_context(
            frontmost,
        ));
    }
    if let Some(text) = crate::features::text::provider::text_context(input) {
        seeds.push(text);
    }
    seeds
}

pub(super) fn catalog_source<'a>(
    catalog: &'a ApplicationCatalog,
    app_usage: &'a dyn Fn(&str, &str) -> UsageStats,
    now: i64,
) -> ApplicationSource<'a> {
    ApplicationSource {
        applications: catalog.applications(),
        app_usage,
        now,
    }
}
