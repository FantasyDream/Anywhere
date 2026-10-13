//! Git repository contexts.
pub mod provider;

pub mod actions;

use super::FirstPartyExtension;
use crate::features::contribution::FeatureContribution;
use anywhere_protocol::extension::FirstPartyManifest;
use std::sync::{Arc, OnceLock};
pub struct Extension;
pub fn manifest() -> &'static FirstPartyManifest {
    static M: OnceLock<FirstPartyManifest> = OnceLock::new();
    M.get_or_init(|| {
        anywhere_plugin::extension::parse_first_party_manifest(include_str!(
            "../../../../../extensions/git/manifest.toml"
        ))
        .expect("invalid compiled Git manifest")
    })
}
impl FirstPartyExtension for Extension {
    fn manifest(&self) -> &'static FirstPartyManifest {
        manifest()
    }
    fn contribution(&self) -> FeatureContribution {
        FeatureContribution::new(
            "git",
            vec![actions::bundle()],
            vec![Arc::new(provider::GitRepositoryProvider::default())],
        )
    }
}
