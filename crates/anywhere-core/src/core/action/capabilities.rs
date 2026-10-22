//! Narrow host services shared by declarative v2 and trusted first-party modules.
use anyhow::{ensure, Result};
use anywhere_platform::Platform;
use anywhere_protocol::extension::ExtensionCapability;
use std::{collections::BTreeSet, path::Path, sync::Arc};
#[derive(Debug, Clone, Default)]
pub struct CapabilityGrant(Arc<BTreeSet<ExtensionCapability>>);
impl CapabilityGrant {
    pub fn approve(
        requested: &[ExtensionCapability],
        allowed: &[ExtensionCapability],
    ) -> Result<Self> {
        let allowed: BTreeSet<_> = allowed.iter().copied().collect();
        for capability in requested {
            ensure!(
                allowed.contains(capability),
                "capability not granted: {}",
                capability.as_str()
            );
        }
        Ok(Self(Arc::new(requested.iter().copied().collect())))
    }
    pub fn require(&self, capability: ExtensionCapability) -> Result<()> {
        ensure!(
            self.0.contains(&capability),
            "capability not granted: {}",
            capability.as_str()
        );
        Ok(())
    }
}
pub struct HostServices<'a> {
    platform: &'a dyn Platform,
    grant: CapabilityGrant,
}
impl<'a> HostServices<'a> {
    pub fn new(platform: &'a dyn Platform, grant: &CapabilityGrant) -> Self {
        Self {
            platform,
            grant: grant.clone(),
        }
    }
    pub fn copy_text(&self, value: &str) -> Result<()> {
        self.grant.require(ExtensionCapability::ClipboardWrite)?;
        self.platform.copy_text(value)
    }
    pub fn write_clipboard(
        &self,
        value: &anywhere_platform::clipboard::ClipboardPayload,
    ) -> Result<()> {
        self.grant.require(ExtensionCapability::ClipboardWrite)?;
        self.platform.write_clipboard(value)
    }
    pub fn open_url(&self, value: &str) -> Result<()> {
        self.grant.require(ExtensionCapability::OpenUrl)?;
        self.platform.open_url(value)
    }
    pub fn open_file(&self, path: &Path) -> Result<()> {
        self.grant.require(ExtensionCapability::OpenFile)?;
        self.platform.open_file(path)
    }
    pub fn run_git(
        &self,
        path: &Path,
        operation: anywhere_platform::GitOperation,
    ) -> Result<anywhere_platform::GitOutput> {
        self.grant.require(ExtensionCapability::GitRead)?;
        self.platform.run_git(path, operation)
    }
    pub fn process_request(
        &self,
        request: anywhere_platform::process::ProcessRequest,
    ) -> Result<anywhere_platform::process::ProcessRequest> {
        self.grant.require(ExtensionCapability::ProcessStart)?;
        Ok(request)
    }
}
