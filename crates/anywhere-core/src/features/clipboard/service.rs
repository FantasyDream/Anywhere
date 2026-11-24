//! Serial policy/write boundary. Native reads run outside the lock and recheck epoch.
use super::history::*;
use super::policy::{self, HistoryPolicy};
use anyhow::{ensure, Context, Result};
use anywhere_platform::{ClipboardHost, ContextReadError};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct HistoryStatus {
    pub recording: bool,
    pub recording_enabled: bool,
    pub policy: HistoryPolicy,
    pub available: bool,
    pub notice: Option<HistoryNotice>,
}
pub struct HistorySnapshot {
    pub revision: String,
    pub status: HistoryStatus,
    pub items: Vec<HistoryItem>,
}
/// Policy facts only; each presentation chooses its own localized wording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryNotice {
    FeatureDisabled,
    StorageUnavailable,
    AccessRequired,
    ItemTooLarge,
    ReadFailed,
    CaptureUnavailable,
    MonitoringUnavailable,
    Paused,
}
struct PolicyState {
    repository_available: bool,
    feature_enabled: bool,
    recording: bool,
    policy: HistoryPolicy,
    next_cleanup_ms: i64,
    epoch: u64,
    generation: Option<i64>,
    notice: Option<HistoryNotice>,
}
struct RepositoryState {
    inner: Result<Box<dyn HistoryRepository>, String>,
    epoch: u64,
}
impl RepositoryState {
    fn get(&mut self) -> Result<&mut (dyn HistoryRepository + 'static)> {
        match &mut self.inner {
            Ok(repository) => Ok(repository.as_mut()),
            Err(_) => anyhow::bail!("Clipboard history storage is unavailable"),
        }
    }
    fn expire_if_needed(&mut self, cutoff: Option<i64>) -> Result<()> {
        if let Some(cutoff) = cutoff {
            if self.inner.is_ok() {
                self.get()?.expire(cutoff)?;
            }
        }
        Ok(())
    }
}
pub struct HistoryService {
    // Lock order: state -> repository. Never acquire `state` while holding `repository`.
    // Long SQLite reads/writes run under `repository` after dropping `state`, using `epoch`
    // to reject stale writes or reads if policy/clear invalidated the session concurrently.
    state: Mutex<PolicyState>,
    repository: Mutex<RepositoryState>,
    // Shared by Launcher and Shelf: admit one payload/decoder at a time.
    preview: Mutex<()>,
    watcher: Mutex<Option<Box<dyn anywhere_platform::clipboard::ClipboardWatch>>>,
    maintenance: Mutex<Option<Maintenance>>,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}
enum MaintenanceEvent {
    Changed,
    Stop,
}
struct Maintenance(std::sync::mpsc::Sender<MaintenanceEvent>);
impl Drop for Maintenance {
    fn drop(&mut self) {
        let _ = self.0.send(MaintenanceEvent::Stop);
    }
}
impl HistoryService {
    pub fn new(repository: Result<Box<dyn HistoryRepository>, String>) -> Self {
        Self::with_clock(repository, Arc::new(policy::now_ms))
    }
    fn with_clock(
        mut repository: Result<Box<dyn HistoryRepository>, String>,
        clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    ) -> Self {
        let recording = match repository.as_ref().map(|r| r.recording_enabled()) {
            Ok(Ok(recording)) => recording,
            Ok(Err(_)) => {
                repository = Err("clipboard preferences unavailable".into());
                false
            }
            Err(_) => false,
        };
        let policy = match repository.as_ref().map(|r| r.policy()) {
            Ok(Ok(policy)) => policy,
            _ => {
                repository = Err("clipboard policy unavailable".into());
                HistoryPolicy::default()
            }
        };
        Self {
            state: Mutex::new(PolicyState {
                repository_available: repository.is_ok(),
                feature_enabled: true,
                recording,
                policy,
                next_cleanup_ms: 0,
                epoch: 0,
                generation: None,
                notice: None,
            }),
            repository: Mutex::new(RepositoryState {
                inner: repository,
                epoch: 0,
            }),
            preview: Mutex::new(()),
            watcher: Mutex::new(None),
            maintenance: Mutex::new(None),
            clock,
        }
    }
    fn state(&self) -> Result<std::sync::MutexGuard<'_, PolicyState>> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("clipboard history unavailable"))
    }
    fn repository(&self) -> Result<std::sync::MutexGuard<'_, RepositoryState>> {
        self.repository
            .lock()
            .map_err(|_| anyhow::anyhow!("clipboard history storage unavailable"))
    }
    pub fn status(&self) -> Result<HistoryStatus> {
        Ok(self.state()?.status())
    }
    /// One atomic view of status and metadata. Unchanged polls stop before SQL
    /// row loading, DTO conversion and bridge transport; expiry still runs.
    pub fn snapshot(&self, query: &str, after: Option<&str>) -> Result<Option<HistorySnapshot>> {
        ensure!(query.len() <= MAX_QUERY_BYTES, "clipboard query too large");
        let query_digest = hex::encode(Sha256::digest(query.as_bytes()));
        loop {
            let (epoch, available, status, cutoff) = {
                let state = self.state()?;
                (
                    state.epoch,
                    state.repository_available,
                    state.status(),
                    state.policy.cutoff((self.clock)()),
                )
            };
            let mut repository = self.repository()?;
            if repository.epoch != epoch {
                drop(repository);
                continue;
            }
            if available {
                repository.expire_if_needed(cutoff)?;
            }
            let changes = repository.inner.as_ref().map_or(0, |repo| repo.revision());
            let revision = format!("{epoch}:{changes}:{:?}:{query_digest}", status.notice);
            if after == Some(revision.as_str()) {
                return Ok(None);
            }
            let items = if status.available {
                repository.get()?.search(query)?
            } else {
                vec![]
            };
            return Ok(Some(HistorySnapshot {
                revision,
                status,
                items,
            }));
        }
    }
    /// Settings-only aggregate; regular history searches need no storage-usage scan.
    pub fn usage(&self) -> Result<(u32, u32)> {
        loop {
            let (epoch, policy) = {
                let state = self.state()?;
                if !state.repository_available {
                    return Ok((0, 0));
                }
                (state.epoch, state.policy)
            };
            let cutoff = policy.cutoff((self.clock)());
            let mut repository = self.repository()?;
            if repository.epoch != epoch {
                continue;
            }
            repository.expire_if_needed(cutoff)?;
            return repository.get()?.usage();
        }
    }
    pub fn set_feature_enabled(&self, enabled: bool, platform: &dyn ClipboardHost) -> Result<()> {
        let mut state = self.state()?;
        if state.feature_enabled != enabled {
            state.feature_enabled = enabled;
            state.invalidate();
            if enabled && state.recording {
                state.generation = platform.clipboard_generation().ok();
            }
            self.repository()?.epoch = state.epoch;
        }
        Ok(())
    }
    pub fn set_recording(&self, enabled: bool) -> Result<()> {
        let mut state = self.state()?;
        state.require_enabled()?;
        let mut repository = self.repository()?;
        repository.get()?.set_recording_enabled(enabled)?;
        state.recording = enabled;
        state.policy.paused_until_ms = None;
        state.invalidate();
        repository.epoch = state.epoch;
        Ok(())
    }
    pub fn request_recording(&self, enabled: bool, platform: &dyn ClipboardHost) -> Result<()> {
        if !enabled {
            return self.set_recording(false);
        }
        // Establish the boundary during Enable, not on the worker's next tick:
        // otherwise the first copy made within 500ms would become the baseline.
        let epoch = {
            let mut state = self.state()?;
            state.require_enabled()?;
            let generation = platform.clipboard_generation()?;
            let mut repository = self.repository()?;
            repository.get()?.set_recording_enabled(true)?;
            state.recording = true;
            state.policy.paused_until_ms = None;
            state.invalidate();
            repository.epoch = state.epoch;
            state.generation = Some(generation);
            state.epoch
        };
        // Only the user's Enable action can ask for access. Never save the value
        // returned while requesting permission, and never do this on app restart.
        if platform.request_clipboard_history_access().is_err() {
            let mut state = self.state()?;
            if state.epoch == epoch {
                state.notice = Some(HistoryNotice::AccessRequired);
            }
        }
        Ok(())
    }
    /// A user-requested timed pause. Clearing it establishes a fresh metadata baseline.
    pub fn pause(&self, minutes: u32, platform: &dyn ClipboardHost) -> Result<()> {
        ensure!(
            matches!(minutes, 0 | 5 | 15 | 60),
            "unsupported pause duration"
        );
        {
            let mut state = self.state()?;
            state.require_enabled()?;
            ensure!(state.recording, "clipboard recording is not enabled");
            let generation = if minutes == 0 {
                Some(platform.clipboard_generation()?)
            } else {
                None
            };
            let policy = HistoryPolicy {
                paused_until_ms: (minutes != 0)
                    .then(|| (self.clock)().saturating_add(i64::from(minutes) * 60_000)),
                ..state.policy
            };
            let mut repository = self.repository()?;
            repository.get()?.set_policy(policy)?;
            state.policy = policy;
            state.invalidate();
            repository.epoch = state.epoch;
            state.generation = generation;
        }
        self.wake_maintenance();
        Ok(())
    }
    pub fn set_retention_days(&self, days: u32) -> Result<()> {
        let mut state = self.state()?;
        state.require_enabled()?;
        let policy = HistoryPolicy {
            retention_days: days,
            ..state.policy
        };
        policy.validate()?;
        let mut repository = self.repository()?;
        repository.get()?.set_policy(policy)?;
        state.policy = policy;
        let generation = state.generation;
        state.invalidate();
        repository.epoch = state.epoch;
        state.generation = generation;
        repository.expire_if_needed(policy.cutoff((self.clock)()))
    }
    /// Uses only metadata while resuming; clipboard contents copied during a pause are discarded.
    fn maintain(&self, platform: &dyn ClipboardHost) -> Result<()> {
        let now = (self.clock)();
        let (available, needs_cleanup, cutoff, paused_expired, epoch) = {
            let state = self.state()?;
            (
                state.repository_available,
                now >= state.next_cleanup_ms,
                state.policy.cutoff(now),
                state
                    .policy
                    .paused_until_ms
                    .is_some_and(|deadline| deadline <= now),
                state.epoch,
            )
        };
        if !available {
            return Ok(());
        }
        if needs_cleanup {
            {
                let mut repository = self.repository()?;
                if repository.epoch == epoch {
                    repository.expire_if_needed(cutoff)?;
                }
            }
            let mut state = self.state()?;
            if state.epoch == epoch {
                state.next_cleanup_ms = now.saturating_add(60_000);
            }
        }
        if paused_expired {
            let generation = platform.clipboard_generation()?;
            let mut state = self.state()?;
            if state
                .policy
                .paused_until_ms
                .is_some_and(|deadline| deadline <= now)
            {
                let policy = HistoryPolicy {
                    paused_until_ms: None,
                    ..state.policy
                };
                let mut repository = self.repository()?;
                repository.get()?.set_policy(policy)?;
                state.policy = policy;
                state.invalidate();
                repository.epoch = state.epoch;
                state.generation = Some(generation);
            }
        }
        Ok(())
    }
    fn wake_maintenance(&self) {
        let worker = crate::core::sync::lock_for_revoke(&self.maintenance, "clipboard_maintenance");
        if let Some(worker) = worker.as_ref() {
            let _ = worker.0.send(MaintenanceEvent::Changed);
        }
    }
    fn maintenance_wait(&self) -> std::time::Duration {
        let now = (self.clock)();
        let next = self
            .state()
            .map(|state| {
                state
                    .policy
                    .paused_until_ms
                    .unwrap_or(i64::MAX)
                    .min(state.next_cleanup_ms)
            })
            .unwrap_or(now);
        std::time::Duration::from_millis(next.saturating_sub(now).clamp(1_000, 60_000) as u64)
    }
    pub fn search(&self, query: &str) -> Result<Vec<HistoryItem>> {
        ensure!(query.len() <= MAX_QUERY_BYTES, "clipboard query too large");
        loop {
            let (epoch, policy) = {
                let state = self.state()?;
                state.require_enabled()?;
                (state.epoch, state.policy)
            };
            let cutoff = policy.cutoff((self.clock)());
            let mut repository = self.repository()?;
            if repository.epoch != epoch {
                continue;
            }
            repository.expire_if_needed(cutoff)?;
            return repository.get()?.search(query);
        }
    }
    /// Explicit native preview only; preserve payload bytes and recheck deletion/policy after preparation.
    pub fn inspect_payload<T>(
        &self,
        id: i64,
        render: impl FnOnce(HistoryItem, anywhere_platform::clipboard::ClipboardPayload) -> Result<T>,
    ) -> Result<Option<T>> {
        let _preview = self
            .preview
            .lock()
            .map_err(|_| anyhow::anyhow!("clipboard preview unavailable"))?;
        let (epoch, cutoff) = {
            let state = self.state()?;
            state.require_enabled()?;
            (state.epoch, state.policy.cutoff((self.clock)()))
        };
        let value = {
            let mut repository = self.repository()?;
            if repository.epoch != epoch {
                return Ok(None);
            }
            repository.expire_if_needed(cutoff)?;
            let repo = repository.get()?;
            let Some(item) = repo.item(id)? else {
                return Ok(None);
            };
            repo.payload(id)?.map(|payload| (item, payload))
        };
        let value = value
            .map(|(item, payload)| render(item, payload))
            .transpose()?;
        let (epoch2, cutoff2) = {
            let state = self.state()?;
            state.require_enabled()?;
            (state.epoch, state.policy.cutoff((self.clock)()))
        };
        if epoch2 != epoch {
            return Ok(None);
        }
        let mut repository = self.repository()?;
        Ok(if repository.epoch == epoch {
            repository.expire_if_needed(cutoff2)?;
            if repository.get()?.item(id)?.is_some() {
                value
            } else {
                None
            }
        } else {
            None
        })
    }
    pub fn copy(&self, id: i64, platform: &dyn ClipboardHost) -> Result<()> {
        let state = self.state()?;
        state.require_enabled()?;
        let cutoff = state.policy.cutoff((self.clock)());
        let mut repository = self.repository()?;
        drop(state);
        repository.expire_if_needed(cutoff)?;
        let payload = repository
            .get()?
            .payload(id)?
            .context("clipboard entry no longer exists")?;
        // Serialize with deletion/disable so a stale view cannot copy a deleted record.
        crate::features::host_services::for_feature(&super::DESCRIPTOR, platform)?
            .write_clipboard(&payload)
    }
    pub fn delete(&self, id: i64) -> Result<()> {
        let state = self.state()?;
        state.require_enabled()?;
        self.repository()?.get()?.delete(id)
    }
    pub fn clear(&self) -> Result<()> {
        let mut state = self.state()?;
        state.require_enabled()?;
        let mut repository = self.repository()?;
        repository.get()?.clear()?;
        state.invalidate(); // Revoke a capture already in flight.
        repository.epoch = state.epoch;
        Ok(())
    }
    pub fn poll(&self, platform: &dyn ClipboardHost, now_ms: i64) -> Result<()> {
        self.maintain(platform)?;
        let epoch = {
            let state = self.state()?;
            if !state.feature_enabled
                || !state.recording
                || state.policy.paused_until_ms.is_some()
                || !state.repository_available
            {
                return Ok(());
            }
            state.epoch
        };
        let generation = platform.clipboard_generation()?;
        {
            let mut state = self.state()?;
            if state.epoch != epoch {
                return Ok(());
            }
            let previous = state.generation.replace(generation);
            // Baseline after enable, resume or clear: never import the old clipboard.
            if previous.is_none() || previous == Some(generation) {
                return Ok(());
            }
        }
        let captured = platform.clipboard_history_payload(generation);
        let cutoff = {
            let state = self.state()?;
            if state.epoch != epoch || state.generation != Some(generation) {
                return Ok(());
            }
            state.policy.cutoff((self.clock)())
        };
        match captured {
            Ok(Some(payload)) => {
                {
                    let mut repository = self.repository()?;
                    if repository.epoch != epoch {
                        return Ok(());
                    }
                    repository.get()?.record_payload(&payload, now_ms)?;
                    repository.expire_if_needed(cutoff)?;
                }
                let mut state = self.state()?;
                if state.epoch == epoch && state.generation == Some(generation) {
                    state.notice = None;
                }
            }
            Ok(None) => {
                let mut state = self.state()?;
                if state.epoch == epoch && state.generation == Some(generation) {
                    state.notice = None;
                }
            }
            Err(error)
                if error.downcast_ref::<ContextReadError>()
                    == Some(&ContextReadError::TooLarge) =>
            {
                let mut state = self.state()?;
                if state.epoch == epoch && state.generation == Some(generation) {
                    state.notice = Some(HistoryNotice::ItemTooLarge);
                }
            }
            Err(error)
                if error.downcast_ref::<ContextReadError>()
                    == Some(&ContextReadError::PermissionDenied) =>
            {
                let mut state = self.state()?;
                if state.epoch == epoch && state.generation == Some(generation) {
                    state.notice = Some(HistoryNotice::AccessRequired);
                }
            }
            Err(_) => {
                let mut state = self.state()?;
                if state.epoch == epoch && state.generation == Some(generation) {
                    state.notice = Some(HistoryNotice::ReadFailed);
                }
            }
        }
        Ok(())
    }
    /// The clipboard-rs watcher retains only a Weak service; Drop requests shutdown.
    pub fn start(self: &Arc<Self>, platform: Arc<dyn ClipboardHost>) -> Result<()> {
        // An optional history policy or pasteboard failure must not stop Launcher startup.
        if self.maintain(platform.as_ref()).is_err() {
            self.state()?.notice = Some(HistoryNotice::CaptureUnavailable);
        }
        {
            let mut state = self.state()?;
            if state.feature_enabled && state.recording {
                state.generation = platform.clipboard_generation().ok();
            }
        }
        let service = Arc::downgrade(self);
        let maintenance_platform = platform.clone();
        let (stop, receiver) = std::sync::mpsc::channel();
        let maintenance = std::thread::Builder::new()
            .name("clipboard-policy".into())
            .spawn(move || {
                loop {
                    let wait = match service.upgrade() {
                        Some(service) => service.maintenance_wait(),
                        None => break,
                    };
                    match receiver.recv_timeout(wait) {
                        Ok(MaintenanceEvent::Stop)
                        | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                        Ok(MaintenanceEvent::Changed)
                        | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    let Some(service) = service.upgrade() else {
                        break;
                    };
                    // Errors remain local and fail closed; the next tick retries without reading a body.
                    let _ = service.maintain(maintenance_platform.as_ref());
                }
            });
        if maintenance.is_ok() {
            *self
                .maintenance
                .lock()
                .map_err(|_| anyhow::anyhow!("clipboard policy worker unavailable"))? =
                Some(Maintenance(stop));
        } else {
            self.state()?.notice = Some(HistoryNotice::CaptureUnavailable);
        }
        let service = Arc::downgrade(self);
        let capture_platform = platform.clone();
        let watcher = platform.watch_clipboard(Box::new(move || {
            let Some(service) = service.upgrade() else {
                return;
            };
            let now = (service.clock)();
            if service.poll(capture_platform.as_ref(), now).is_err() {
                if let Ok(mut state) = service.state() {
                    // Never log errors containing bodies, paths or SQL parameters.
                    if state.recording && state.feature_enabled {
                        state.notice = Some(HistoryNotice::CaptureUnavailable);
                    }
                }
            }
        }));
        let watcher = match watcher {
            Ok(watcher) => watcher,
            Err(_) => {
                let mut state = self.state()?;
                state.notice = Some(HistoryNotice::MonitoringUnavailable);
                return Ok(());
            }
        };
        *self
            .watcher
            .lock()
            .map_err(|_| anyhow::anyhow!("clipboard watcher unavailable"))? = Some(watcher);
        Ok(())
    }
}
impl PolicyState {
    fn status(&self) -> HistoryStatus {
        let available = self.feature_enabled && self.repository_available;
        HistoryStatus {
            recording: available && self.recording && self.policy.paused_until_ms.is_none(),
            recording_enabled: self.recording,
            policy: self.policy,
            available,
            notice: if !self.feature_enabled {
                Some(HistoryNotice::FeatureDisabled)
            } else if !self.repository_available {
                Some(HistoryNotice::StorageUnavailable)
            } else {
                self.notice
                    .or_else(|| self.policy.paused_until_ms.map(|_| HistoryNotice::Paused))
            },
        }
    }
    fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.generation = None;
        self.notice = None;
    }
    fn require_enabled(&self) -> Result<()> {
        ensure!(self.feature_enabled, "Clipboard History is disabled");
        Ok(())
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
