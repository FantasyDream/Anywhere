//! First-party url module; stable action/provider identities survive migration.
use crate::features::contribution::FeatureContribution;
use crate::features::descriptor::FeatureDescriptor;
use crate::features::BuiltinFeature;
use anywhere_protocol::extension::ExtensionCapability as Capability;
use std::sync::Arc;
pub mod actions;
pub mod provider;
pub struct Feature;
pub const DESCRIPTOR: FeatureDescriptor = FeatureDescriptor::new(
    "url",
    "URL",
    &[Capability::OpenUrl, Capability::ClipboardWrite],
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
            vec![Arc::new(provider::UrlProvider::new())],
        )
        .with_input_recognizer("core.url", provider::recognize)
    }
}
