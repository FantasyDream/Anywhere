//! Build-time first-party modules, not a dynamic Rust ABI or script runtime.
use crate::{
    core::action::capabilities::{CapabilityGrant, HostServices},
    features::contribution::FeatureContribution,
};
use anyhow::{ensure, Result};
use anywhere_platform::Platform;
use anywhere_protocol::extension::{ExtensionCapability as Capability, FirstPartyManifest};
pub mod developer;
pub mod git;
pub mod json;
pub mod saved_directories;
pub mod url;

pub trait FirstPartyExtension: Send + Sync {
    fn manifest(&self) -> &'static FirstPartyManifest;
    fn contribution(&self) -> FeatureContribution;
    fn runtime(
        &self,
        _storage: Option<std::sync::Arc<dyn crate::core::storage::ExtensionStorage>>,
    ) -> Result<ExtensionRuntime> {
        Ok(ExtensionRuntime::default())
    }
    fn runtime_with_state(
        &self,
        storage: Option<std::sync::Arc<dyn crate::core::storage::ExtensionStorage>>,
        _state: Option<std::sync::Arc<dyn ExtensionState>>,
    ) -> Result<ExtensionRuntime> {
        self.runtime(storage)
    }
}
#[derive(Default, Clone)]
pub struct ExtensionRuntime {
    pub actions: Vec<crate::core::action::descriptor::ActionDefinition>,
    pub sources: crate::core::search::candidate_source::SourceSet,
    pub state: Option<std::sync::Arc<dyn ExtensionState>>,
}
/// Only bounded persistent data is retained across reloads. Processes, UI engines,
/// subscriptions and sources belong to the runtime and release resources on reset.
pub trait ExtensionState: std::any::Any + Send + Sync {
    fn refresh(&self) -> Result<()>;
}
pub fn modules() -> Vec<Box<dyn FirstPartyExtension>> {
    use developer::provider::ProjectKind::*;
    vec![
        Box::new(git::Extension),
        Box::new(url::Extension),
        Box::new(json::Extension),
        Box::new(developer::Extension(Rust)),
        Box::new(developer::Extension(Maven)),
        Box::new(developer::Extension(Gradle)),
        Box::new(developer::Extension(Node)),
        Box::new(developer::Extension(SpringBoot)),
        Box::new(saved_directories::Extension),
    ]
}
/// Host-owned approval is independent of metadata authored by a module.
fn approved(id: &str) -> &'static [Capability] {
    match id {
        "git" => &[Capability::GitRead, Capability::ClipboardWrite],
        "rust" | "maven" | "gradle" | "node" | "spring_boot" => &[Capability::ProcessStart],
        "saved_directories" => &[Capability::Storage, Capability::OpenFile],
        "json" => &[Capability::ClipboardWrite],
        "url" => &[Capability::OpenUrl, Capability::ClipboardWrite],
        _ => &[],
    }
}
pub(crate) fn services<'a>(
    manifest: &FirstPartyManifest,
    platform: &'a dyn Platform,
) -> Result<HostServices<'a>> {
    let grant = CapabilityGrant::approve(&manifest.capabilities, approved(&manifest.id))?;
    Ok(HostServices::new(platform, &grant))
}

pub fn builtins() -> Result<Vec<FeatureContribution>> {
    let modules = modules();
    let mut ids = std::collections::BTreeSet::new();
    let mut contributions = Vec::new();
    for module in modules {
        let metadata = module.manifest();
        ensure!(
            ids.insert(metadata.id.clone()),
            "duplicate first-party extension id"
        );
        CapabilityGrant::approve(&metadata.capabilities, approved(&metadata.id))?;
        let contribution = module.contribution();
        ensure!(
            metadata.id == contribution.id,
            "extension ownership differs from its manifest"
        );
        contributions.push(contribution);
    }
    Ok(contributions)
}

/// The host grants and binds storage; modules never choose a namespace.
pub(crate) fn runtime(
    module: &dyn FirstPartyExtension,
    host: &crate::persistence::module_storage::ModuleStorageHost,
    state: Option<std::sync::Arc<dyn ExtensionState>>,
) -> Result<ExtensionRuntime> {
    let manifest = module.manifest();
    let grant = CapabilityGrant::approve(&manifest.capabilities, approved(&manifest.id))?;
    let storage = if manifest.capabilities.contains(&Capability::Storage) {
        grant.require(Capability::Storage)?;
        Some(host.bind(&manifest.id)?)
    } else {
        None
    };
    module.runtime_with_state(storage, state)
}
