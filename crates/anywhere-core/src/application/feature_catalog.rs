//! Process-lifetime definitions, contribution selection and publication versions.
//!
//! Application holds Session -> Web before publishing; this service acquires
//! providers -> registry -> sources -> policy, and never acquires Session or Web.
use crate::core::{
    action::descriptor::ActionRegistry,
    context::{
        engine::ExecutionGuard, provider::ContextProvider, registry::ContextProviderRegistry,
    },
    search::candidate_source::{
        AsyncCandidateSource, CandidateSourceRef, SourceCatalog, SourceSet,
    },
};
use crate::features::policy::{FeaturePolicy, FeaturePolicySnapshot};
use anyhow::{anyhow, ensure, Result};
use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, MutexGuard,
    },
};

pub(super) struct FeatureCatalog {
    registry: Mutex<Arc<ActionRegistry>>,
    providers: Mutex<ContextProviderRegistry>,
    sources: Mutex<SourceCatalog>,
    policy: Arc<FeaturePolicy>,
    version: AtomicU64,
    installed: AtomicBool,
}

/// A prepared selection; persistence must succeed before any of it becomes current.
pub(super) struct CatalogSelection {
    pub registry: ActionRegistry,
    pub providers: ContextProviderRegistry,
    pub sources: SourceSet,
    pub policy: Arc<FeaturePolicySnapshot>,
    pub excluded: BTreeSet<String>,
}

impl FeatureCatalog {
    pub fn new(registry: ActionRegistry) -> Self {
        Self {
            registry: Mutex::new(Arc::new(registry)),
            providers: Mutex::new(super::composition::builtin_providers()),
            sources: Default::default(),
            policy: Default::default(),
            version: AtomicU64::new(1),
            installed: AtomicBool::new(false),
        }
    }

    pub fn registry(&self) -> Arc<ActionRegistry> {
        self.registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Caller holds Session; publication consumers revalidate this version.
    pub fn replace_registry(&self, mut registry: ActionRegistry) {
        registry.retain_contributors(|id| self.allows(id));
        if let Ok(mut slot) = self.registry.lock() {
            *slot = Arc::new(registry);
        }
        self.version.fetch_add(1, Ordering::AcqRel);
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    pub fn policy(&self) -> Arc<FeaturePolicySnapshot> {
        self.policy.snapshot()
    }

    pub fn is_policy_current(&self, policy: &Arc<FeaturePolicySnapshot>) -> bool {
        self.policy.is_current(policy)
    }

    pub fn allows(&self, contributor: &str) -> bool {
        self.policy.allows(contributor)
    }

    pub fn request_guard(&self) -> ExecutionGuard {
        self.policy.request_guard()
    }

    /// Web publication and input recognition hold this guard only while preparing
    /// definitions. OS reads and provider execution always use a cloned registry.
    pub fn providers(&self) -> Result<MutexGuard<'_, ContextProviderRegistry>> {
        self.providers
            .lock()
            .map_err(|_| anyhow!("providers lock poisoned"))
    }

    pub fn register_provider(&self, provider: Arc<dyn ContextProvider>) -> Result<()> {
        self.providers()?.register(provider)
    }

    pub fn synchronous_sources(&self) -> Result<Vec<CandidateSourceRef>> {
        Ok(self
            .sources
            .lock()
            .map_err(|_| anyhow!("sources lock poisoned"))?
            .synchronous())
    }

    pub fn asynchronous_sources(&self) -> Result<Vec<Arc<dyn AsyncCandidateSource>>> {
        Ok(self
            .sources
            .lock()
            .map_err(|_| anyhow!("sources lock poisoned"))?
            .asynchronous())
    }

    pub fn register_source(&self, source: CandidateSourceRef) {
        self.sources
            .lock()
            .expect("sources lock poisoned")
            .base
            .synchronous
            .push(source);
    }

    pub fn register_async_source(&self, source: Arc<dyn AsyncCandidateSource>) -> Result<()> {
        let mut catalog = self
            .sources
            .lock()
            .map_err(|_| anyhow!("sources lock poisoned"))?;
        ensure!(
            !source.id().is_empty()
                && catalog.asynchronous().iter().all(|s| s.id() != source.id())
                && catalog.synchronous().iter().all(|s| s.id() != source.id()),
            "duplicate or empty search source id: {}",
            source.id()
        );
        catalog.base.asynchronous.push(source);
        Ok(())
    }

    /// Commit under Session -> Web. Revoke the caller's retained execution state
    /// before releasing policy/definition locks; reset retired sources afterwards.
    pub fn publish(
        &self,
        selection: CatalogSelection,
        persist: impl FnOnce() -> Result<()>,
        revoke_execution: impl FnOnce(Arc<ActionRegistry>),
    ) -> Result<SourceSet> {
        let CatalogSelection {
            mut registry,
            mut providers,
            mut sources,
            policy,
            excluded,
        } = selection;
        let mut provider_slot = self.providers()?;
        let mut registry_slot = self
            .registry
            .lock()
            .map_err(|_| anyhow!("registry lock poisoned"))?;
        let mut source_slot = self
            .sources
            .lock()
            .map_err(|_| anyhow!("sources lock poisoned"))?;
        let mut source_ids: BTreeSet<_> = source_slot.base.ids().collect();
        for id in sources.ids() {
            ensure!(
                !id.is_empty() && source_ids.insert(id),
                "duplicate or empty source id: {id}"
            );
        }
        let mut policy_slot = self
            .policy
            .current
            .write()
            .map_err(|_| anyhow!("feature policy lock poisoned"))?;
        let mut excluded_slot = self
            .policy
            .excluded
            .write()
            .map_err(|_| anyhow!("contribution policy lock poisoned"))?;
        let enabled = |id: &str| policy.allows_contributor(id) && !excluded.contains(id);
        registry.retain_contributors(enabled);
        providers.retain_contributors(enabled);
        sources.retain_contributors(enabled);
        persist()?;
        *policy_slot = policy;
        *excluded_slot = excluded;
        let retired = std::mem::replace(&mut source_slot.extensions, sources);
        *provider_slot = providers;
        *registry_slot = Arc::new(registry);
        self.version.fetch_add(1, Ordering::AcqRel);
        revoke_execution(registry_slot.clone());
        Ok(retired)
    }

    /// A failed startup assembly may retry; a successful binding is process-lifetime.
    pub fn begin_install(&self) -> Result<CatalogInstallation<'_>> {
        ensure!(
            self.installed
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok(),
            "feature catalogue already installed; restart instead of rebinding native code"
        );
        Ok(CatalogInstallation {
            installed: &self.installed,
            committed: false,
        })
    }
}

pub(super) struct CatalogInstallation<'a> {
    installed: &'a AtomicBool,
    committed: bool,
}
impl CatalogInstallation<'_> {
    pub fn commit(mut self) {
        self.committed = true;
    }
}
impl Drop for CatalogInstallation<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.installed.store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_blocks_catalog_readers_until_execution_revocation_finishes() {
        let catalog = FeatureCatalog::new(ActionRegistry::default());
        let policy = Arc::new(FeaturePolicySnapshot::new(1, Default::default(), false));
        let revoked = AtomicBool::new(false);
        catalog
            .publish(
                CatalogSelection {
                    registry: ActionRegistry::default(),
                    providers: ContextProviderRegistry::default(),
                    sources: SourceSet::default(),
                    policy: policy.clone(),
                    excluded: Default::default(),
                },
                || Ok(()),
                |_| {
                    // New policy/definitions must not become readable while the
                    // caller still has retained execution offers from the old set.
                    assert!(catalog.providers.try_lock().is_err());
                    assert!(catalog.registry.try_lock().is_err());
                    assert!(catalog.sources.try_lock().is_err());
                    assert!(catalog.policy.current.try_read().is_err());
                    assert!(catalog.policy.excluded.try_read().is_err());
                    revoked.store(true, Ordering::Release);
                },
            )
            .unwrap();
        assert!(revoked.load(Ordering::Acquire));
        assert!(catalog.is_policy_current(&policy));
        assert_eq!(catalog.version(), 2);
    }
}
