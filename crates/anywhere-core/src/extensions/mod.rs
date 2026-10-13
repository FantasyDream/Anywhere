//! Build-time first-party modules, not a dynamic Rust ABI or script runtime.
use crate::{
    core::action::capabilities::{CapabilityGrant, HostServices},
    features::contribution::FeatureContribution,
};
use anyhow::{ensure, Result};
use anywhere_platform::Platform;
use anywhere_protocol::extension::{ExtensionCapability as Capability, FirstPartyManifest};
pub mod json;
pub mod url;

pub trait FirstPartyExtension: Send + Sync {
    fn manifest(&self) -> &'static FirstPartyManifest;
    fn contribution(&self) -> FeatureContribution;
}
/// Host-owned approval is independent of metadata authored by a module.
fn approved(id: &str) -> &'static [Capability] {
    match id {
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
    let modules: [&dyn FirstPartyExtension; 2] = [&url::Extension, &json::Extension];
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
