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
}

impl ContextProviderRegistry {
    pub fn register(&mut self, provider: Arc<dyn ContextProvider>) -> Result<()> {
        let descriptor = provider.descriptor();
        if self
            .providers
            .iter()
            .any(|existing| existing.descriptor().id == descriptor.id)
        {
            bail!("duplicate provider id: {}", descriptor.id);
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
}
