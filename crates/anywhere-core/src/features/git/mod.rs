//! Git repository contexts.
pub mod provider;

pub mod actions;

use crate::features::contribution::FeatureContribution;
use crate::features::descriptor::FeatureDescriptor;
use crate::features::BuiltinFeature;
use anywhere_protocol::extension::ExtensionCapability as Capability;
use std::sync::Arc;
pub struct Feature;
pub const DESCRIPTOR: FeatureDescriptor = FeatureDescriptor::new(
    "git",
    "Git",
    &[Capability::GitRead, Capability::ClipboardWrite],
);
pub fn descriptor() -> &'static FeatureDescriptor {
    &DESCRIPTOR
}
impl BuiltinFeature for Feature {
    fn descriptor(&self) -> &'static FeatureDescriptor {
        descriptor()
    }
    fn contribution(&self) -> FeatureContribution {
        FeatureContribution::new(
            self.descriptor().id,
            vec![actions::bundle()],
            vec![Arc::new(provider::GitRepositoryProvider::default())],
        )
    }
}
