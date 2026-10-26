//! The set of providers available to a session.
//!
//! Registration owns identity and order: ids are unique (a duplicate is a programming error,
//! not a silent replacement) and execution order is by priority, then id, so the order never
//! depends on registration sequence.
use crate::core::context::provider::ContextProvider;
use anyhow::{bail, Result};
use std::sync::Arc;

#[derive(Default, Clone)]
pub struct ContextProviderRegistry {
    providers: Vec<Arc<dyn ContextProvider>>,
    contributors: std::collections::BTreeMap<String, Arc<str>>,
    input_recognizers: Vec<(super::input_recognizer::InputRecognizer, Option<Arc<str>>)>,
}

impl ContextProviderRegistry {
    pub fn register(&mut self, provider: Arc<dyn ContextProvider>) -> Result<()> {
        self.register_owned(provider, None)
    }

    pub fn register_owned(
        &mut self,
        provider: Arc<dyn ContextProvider>,
        contributor: Option<&str>,
    ) -> Result<()> {
        let descriptor = provider.descriptor();
        if self
            .providers
            .iter()
            .any(|existing| existing.descriptor().id == descriptor.id)
        {
            bail!("duplicate provider id: {}", descriptor.id);
        }
        if let Some(contributor) = contributor {
            self.contributors
                .insert(descriptor.id.clone(), contributor.into());
        }
        self.providers.push(provider);
        self.providers.sort_by(|a, b| {
            b.descriptor()
                .priority
                .cmp(&a.descriptor().priority)
                .then_with(|| a.descriptor().id.cmp(&b.descriptor().id))
        });
        Ok(())
    }

    pub fn register_input_recognizer(
        &mut self,
        recognizer: super::input_recognizer::InputRecognizer,
        contributor: Option<&str>,
    ) -> Result<()> {
        if recognizer.id.is_empty()
            || self
                .input_recognizers
                .iter()
                .any(|(existing, _)| existing.id == recognizer.id)
        {
            bail!("duplicate or empty input recognizer id: {}", recognizer.id);
        }
        self.input_recognizers
            .push((recognizer, contributor.map(Into::into)));
        self.input_recognizers
            .sort_by(|(a, _), (b, _)| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        Ok(())
    }
    pub fn input_recognizers(&self) -> Vec<super::input_recognizer::InputRecognizer> {
        self.input_recognizers
            .iter()
            .map(|(rule, _)| rule.clone())
            .collect()
    }

    pub fn retain_contributors(&mut self, enabled: impl Fn(&str) -> bool) {
        self.providers.retain(|provider| {
            self.contributors
                .get(&provider.descriptor().id)
                .is_none_or(|id| enabled(id))
        });
        self.contributors.retain(|_, id| enabled(id));
        self.input_recognizers
            .retain(|(_, owner)| owner.as_deref().is_none_or(&enabled));
    }

    pub fn providers(&self) -> &[Arc<dyn ContextProvider>] {
        &self.providers
    }

    /// Provider ids in execution order, for diagnostics and tests.
    pub fn ids(&self) -> Vec<&str> {
        self.providers
            .iter()
            .map(|provider| provider.descriptor().id.as_str())
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::provider::ProviderDescriptor;
    use anyhow::Result;
    use async_trait::async_trait;

    struct Probe {
        descriptor: ProviderDescriptor,
    }

    #[async_trait]
    impl ContextProvider for Probe {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }
        async fn resolve(
            &self,
            _: &crate::core::context::ResolvedContext,
            _: &crate::core::context::ExecutionContext,
        ) -> Result<Vec<crate::core::context::ResolvedContext>> {
            Ok(vec![])
        }
    }

    #[test]
    fn registration_sorts_by_priority_then_id_and_rejects_duplicates() {
        let mut registry = ContextProviderRegistry::default();
        for (id, priority) in [("b.later", 0), ("a.first", 10), ("c.last", 0)] {
            registry
                .register(Arc::new(Probe {
                    descriptor: ProviderDescriptor {
                        id: id.into(),
                        accepts: vec![],
                        produces: vec![],
                        priority,
                    },
                }))
                .unwrap();
        }
        // Higher priority first, then id: the order never depends on registration sequence.
        assert_eq!(registry.ids(), ["a.first", "b.later", "c.last"]);
        assert!(registry
            .register(Arc::new(Probe {
                descriptor: ProviderDescriptor {
                    id: "a.first".into(),
                    accepts: vec![],
                    produces: vec![],
                    priority: 99,
                },
            }))
            .is_err());
    }
    #[test]
    fn input_recognizer_identity_is_unique_and_owner_filter_preserves_foundations() {
        use super::super::input_recognizer::InputRecognizer;
        let rule = |id, priority| InputRecognizer {
            id: String::from(id),
            priority,
            recognize: Arc::new(|_| None),
        };
        let mut registry = ContextProviderRegistry::default();
        registry
            .register_input_recognizer(rule("core.path", 1), None)
            .unwrap();
        registry
            .register_input_recognizer(rule("core.json", 2), Some("json"))
            .unwrap();
        assert!(registry
            .register_input_recognizer(rule("core.json", 3), None)
            .is_err());
        assert!(registry
            .register_input_recognizer(rule("", 3), None)
            .is_err());
        assert_eq!(
            registry
                .input_recognizers()
                .iter()
                .map(|r| r.id.clone())
                .collect::<Vec<_>>(),
            ["core.json", "core.path"]
        );
        registry.retain_contributors(|_| false);
        assert_eq!(
            registry
                .input_recognizers()
                .iter()
                .map(|r| r.id.clone())
                .collect::<Vec<_>>(),
            ["core.path"]
        );
    }
}
