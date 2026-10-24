//! Host permissions for compiled features, independent of module-authored metadata.
use crate::{
    core::action::capabilities::{CapabilityGrant, HostServices},
    features::descriptor::FeatureDescriptor,
};
use anyhow::Result;
use anywhere_platform::Platform;
use anywhere_protocol::extension::ExtensionCapability as Capability;
/// Host-owned approval is independent of metadata authored by a module.
fn approved(id: &str) -> &'static [Capability] {
    match id {
        "git" => &[Capability::GitRead, Capability::ClipboardWrite],
        "rust" | "maven" | "gradle" | "node" | "spring_boot" => &[Capability::ProcessStart],
        "saved_directories" => &[Capability::Storage, Capability::OpenFile],
        "clipboard" => &[
            Capability::Storage,
            Capability::ClipboardWrite,
            Capability::RenderUi,
        ],
        "application_history" => &[
            Capability::Storage,
            Capability::RenderUi,
            Capability::OpenFile,
            Capability::ClipboardWrite,
        ],
        "url" => &[Capability::OpenUrl, Capability::ClipboardWrite],
        _ => &[],
    }
}
pub(crate) fn grant(descriptor: &FeatureDescriptor) -> Result<CapabilityGrant> {
    CapabilityGrant::approve(descriptor.capabilities, approved(descriptor.id))
}
pub(crate) fn for_feature<'a>(
    descriptor: &FeatureDescriptor,
    platform: &'a dyn Platform,
) -> Result<HostServices<'a>> {
    let grant = grant(descriptor)?;
    Ok(HostServices::new(platform, &grant))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_metadata_cannot_expand_host_permissions() {
        for module in crate::features::catalog::instantiate().unwrap() {
            assert!(grant(module.descriptor()).is_ok());
        }
        // A typed request still cannot grant itself an operation or a namespace.
        for descriptor in [
            FeatureDescriptor::new("retired_json", "Retired", &[Capability::OpenUrl]),
            FeatureDescriptor::new("url", "URL", &[Capability::ProcessStart]),
            FeatureDescriptor::new("unknown", "Unknown", &[Capability::Storage]),
        ] {
            assert!(grant(&descriptor).is_err());
        }
    }
}
