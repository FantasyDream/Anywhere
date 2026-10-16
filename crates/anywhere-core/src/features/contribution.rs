//! Static first-party registration; this is not an external plugin ABI.
use crate::core::{
    action::bundle::{ActionBundle, ContextActionFactory},
    context::provider::ContextProvider,
};
use std::sync::Arc;
pub struct FeatureContribution {
    pub id: &'static str,
    pub(crate) configurable: bool,
    pub bundles: Vec<ActionBundle>,
    pub providers: Vec<Arc<dyn ContextProvider>>,
    pub input_recognizers: Vec<crate::core::context::input_recognizer::InputRecognizer>,
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
            configurable: true,
            bundles,
            providers,
            factories: vec![],
            input_recognizers: vec![],
        }
    }
    pub(crate) fn with_input_recognizer(
        mut self,
        id: &'static str,
        recognize: fn(&str) -> Option<crate::core::context::ResolvedContext>,
    ) -> Self {
        self.input_recognizers
            .push(crate::core::context::input_recognizer::InputRecognizer {
                id,
                recognize,
                priority: self
                    .providers
                    .iter()
                    .find(|provider| provider.descriptor().id == id)
                    .map(|provider| provider.descriptor().priority)
                    .unwrap_or(0),
            });
        self
    }
    fn foundation(
        id: &'static str,
        bundles: Vec<ActionBundle>,
        providers: Vec<Arc<dyn ContextProvider>>,
    ) -> Self {
        Self {
            configurable: false,
            ..Self::new(id, bundles, providers)
        }
    }
}
pub fn foundations() -> Vec<FeatureContribution> {
    use super::*;
    vec![
        FeatureContribution::foundation(
            "filesystem",
            vec![
                filesystem::actions_folder::bundle(),
                filesystem::actions_file::bundle(),
                filesystem::workspace::bundle(),
            ],
            vec![Arc::new(filesystem::provider::PathProvider::default())],
        )
        .with_input_recognizer("core.path", filesystem::provider::recognize_path),
        FeatureContribution::foundation(
            "text",
            vec![],
            vec![Arc::new(text::provider::TextProvider::default())],
        ),
    ]
}
pub fn builtins() -> Vec<FeatureContribution> {
    let mut result = foundations();
    result.extend(crate::features::builtin_contributions().expect("invalid built-in feature"));
    result
}
