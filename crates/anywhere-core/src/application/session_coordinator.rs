//! Session ownership and execution revocation. Acquire access before Web/catalog
//! publication guards; prepare filesystem writes before entering this boundary.
use super::session_model::LauncherSession;
use anyhow::{anyhow, Result};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, MutexGuard,
};

#[derive(Default)]
pub(crate) struct SessionCoordinator {
    state: Mutex<LauncherSession>,
    epoch: Arc<AtomicU64>,
}
impl SessionCoordinator {
    pub(crate) fn access(&self) -> Result<MutexGuard<'_, LauncherSession>> {
        self.state
            .lock()
            .map_err(|_| anyhow!("session lock poisoned"))
    }
    pub(crate) fn execution_epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }
    pub(crate) fn advance_execution(&self) -> u64 {
        self.epoch.fetch_add(1, Ordering::AcqRel)
    }
    pub(crate) fn epoch_guard(&self) -> Arc<AtomicU64> {
        self.epoch.clone()
    }
}
