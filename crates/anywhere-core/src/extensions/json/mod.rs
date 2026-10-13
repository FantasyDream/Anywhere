//! First-party json module; stable action/provider identities survive migration.
use super::FirstPartyExtension;
use crate::features::contribution::FeatureContribution;
use anywhere_protocol::extension::FirstPartyManifest;
use std::sync::{Arc, OnceLock};
pub mod actions;
pub mod provider;
pub struct Extension;
pub fn manifest() -> &'static FirstPartyManifest {
    static METADATA: OnceLock<FirstPartyManifest> = OnceLock::new();
    METADATA.get_or_init(|| {
        anywhere_plugin::extension::parse_first_party_manifest(include_str!(
            "../../../../../extensions/json/manifest.toml"
        ))
        .expect("invalid compiled extension manifest")
    })
}
impl FirstPartyExtension for Extension {
    fn manifest(&self) -> &'static FirstPartyManifest {
        manifest()
    }
    fn contribution(&self) -> FeatureContribution {
        FeatureContribution::new(
            "json",
            vec![actions::bundle()],
            vec![Arc::new(provider::JsonProvider::new())],
        )
    }
}
