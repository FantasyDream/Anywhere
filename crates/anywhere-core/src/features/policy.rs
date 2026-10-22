//! Immutable native feature policy and in-memory request currency.
use std::collections::BTreeSet;

#[derive(Debug)]
pub struct FeaturePolicySnapshot {
    generation: u64,
    disabled: BTreeSet<String>,
    recovery: bool,
}

impl FeaturePolicySnapshot {
    pub(crate) fn new(generation: u64, disabled: BTreeSet<String>, recovery: bool) -> Self {
        Self {
            generation,
            disabled,
            recovery,
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn is_enabled(&self, id: &str) -> bool {
        !self.recovery && !self.disabled.contains(id)
    }
    /// External compatibility contributions have an independent startup grant policy.
    pub(crate) fn allows_contributor(&self, id: &str) -> bool {
        id.starts_with("plugin:") || id.starts_with("web:") || self.is_enabled(id)
    }
    pub fn is_configurable(&self) -> bool {
        !self.recovery
    }
}

/// Published with the action/provider/source catalog under the Application session lock.
/// Pointer identity avoids ABA when another manager starts its generation at one again.
pub(crate) struct FeaturePolicy {
    pub(crate) excluded: std::sync::RwLock<BTreeSet<String>>,
    pub(crate) current: std::sync::RwLock<std::sync::Arc<FeaturePolicySnapshot>>,
}
impl Default for FeaturePolicy {
    fn default() -> Self {
        Self {
            excluded: Default::default(),
            current: std::sync::RwLock::new(std::sync::Arc::new(FeaturePolicySnapshot::new(
                0,
                Default::default(),
                false,
            ))),
        }
    }
}
impl FeaturePolicy {
    pub(crate) fn allows(&self, id: &str) -> bool {
        self.snapshot().allows_contributor(id)
            && self.excluded.read().is_ok_and(|ids| !ids.contains(id))
    }
    pub(crate) fn snapshot(&self) -> std::sync::Arc<FeaturePolicySnapshot> {
        self.current
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub(crate) fn is_current(&self, snapshot: &std::sync::Arc<FeaturePolicySnapshot>) -> bool {
        self.current
            .read()
            .is_ok_and(|current| std::sync::Arc::ptr_eq(&current, snapshot))
    }
    pub(crate) fn request_guard(
        self: &std::sync::Arc<Self>,
    ) -> crate::core::context::engine::ExecutionGuard {
        let policy = self.clone();
        let snapshot = self.snapshot();
        std::sync::Arc::new(move || policy.is_current(&snapshot))
    }
}
