//! Process-level wiring: the action registry and the provider set a session starts with.
use crate::core::{
    action::descriptor::ActionRegistry, context::provider::ContextProvider,
    context::registry::ContextProviderRegistry,
};
use crate::features::filesystem::provider::{CurrentDirectoryProvider, DirectoryMemo};
use std::sync::Arc;

pub fn builtin_registry() -> ActionRegistry {
    let mut registry = ActionRegistry::default();
    for bundle in crate::features::builtin_action_bundles() {
        registry
            .register_bundle(bundle)
            .expect("invalid built-in action registration");
    }
    registry.register_factory(crate::features::developer::actions::DeveloperActions);
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
