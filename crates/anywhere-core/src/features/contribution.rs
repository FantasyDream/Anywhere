//! Static first-party registration; this is not an external plugin ABI.
use crate::core::{
    action::bundle::{ActionBundle, ContextActionFactory},
    context::provider::ContextProvider,
};
use std::sync::Arc;
pub struct FeatureContribution {
    pub id: &'static str,
    pub bundles: Vec<ActionBundle>,
    pub providers: Vec<Arc<dyn ContextProvider>>,
    pub factories: Vec<Arc<dyn ContextActionFactory>>,
}
impl FeatureContribution {
    pub(crate) fn new(
        id: &'static str,
        bundles: Vec<ActionBundle>,
        providers: Vec<Arc<dyn ContextProvider>>,
    ) -> Self {
        Self {
            id,
            bundles,
            providers,
            factories: vec![],
        }
    }
}
pub fn builtins() -> Vec<FeatureContribution> {
    use super::*;
    let filesystem = FeatureContribution::new(
        "filesystem",
        vec![
            filesystem::actions_folder::bundle(),
            filesystem::actions_file::bundle(),
            filesystem::workspace::bundle(),
        ],
        vec![Arc::new(filesystem::provider::PathProvider::default())],
    );
    let mut result = vec![filesystem];
    result.extend(crate::extensions::builtins().expect("invalid first-party extension"));
    result.push(FeatureContribution::new(
        "text",
        vec![],
        vec![Arc::new(text::provider::TextProvider::default())],
    ));
    result
}
