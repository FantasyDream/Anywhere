//! Serial policy/write boundary. Native reads run outside the lock and recheck epoch.
use super::history::*;
use super::policy::{self, HistoryPolicy};
use anyhow::{ensure, Context, Result};
use anywhere_platform::{ContextReadError, Platform};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct HistoryStatus {
    pub recording: bool,
    pub recording_enabled: bool,
    pub policy: HistoryPolicy,
    pub available: bool,
    pub notice: Option<HistoryNotice>,
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
struct State {
    repository: Result<Box<dyn HistoryRepository>, String>,
    feature_enabled: bool,
    recording: bool,
    policy: HistoryPolicy,
    next_cleanup_ms: i64,
    epoch: u64,
    generation: Option<i64>,
    notice: Option<HistoryNotice>,
}
pub struct HistoryService {
    state: Mutex<State>,
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
            state: Mutex::new(State {
                repository,
                feature_enabled: true,
                recording,
                policy,
                next_cleanup_ms: 0,
                epoch: 0,
                generation: None,
                notice: None,
            }),
            preview: Mutex::new(()),
            watcher: Mutex::new(None),
            maintenance: Mutex::new(None),
            clock,
        }
    }
    fn state(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("clipboard history unavailable"))
    }
    pub fn status(&self) -> Result<HistoryStatus> {
        let mut state = self.state()?;
        if state.repository.is_ok() {
            state.expire((self.clock)())?;
        }
        Ok(HistoryStatus {
            recording: state.recording && state.policy.paused_until_ms.is_none(),
            recording_enabled: state.recording,
            policy: state.policy,
            available: state.feature_enabled && state.repository.is_ok(),
            notice: if !state.feature_enabled {
                Some(HistoryNotice::FeatureDisabled)
            } else if state.repository.is_err() {
                Some(HistoryNotice::StorageUnavailable)
            } else {
                state
                    .notice
                    .or_else(|| state.policy.paused_until_ms.map(|_| HistoryNotice::Paused))
            },
        })
    }
    /// Settings-only aggregate; regular history searches need no storage-usage scan.
    pub fn usage(&self) -> Result<(u32, u32)> {
        let mut state = self.state()?;
        if state.repository.is_err() {
            return Ok((0, 0));
        }
        state.expire((self.clock)())?;
        state.repository()?.usage()
    }
    pub fn set_feature_enabled(&self, enabled: bool, platform: &dyn Platform) -> Result<()> {
        let mut state = self.state()?;
        if state.feature_enabled != enabled {
            state.feature_enabled = enabled;
            state.invalidate();
            if enabled && state.recording {
                state.generation = platform.clipboard_generation().ok();
            }
        }
        Ok(())
    }
    pub fn set_recording(&self, enabled: bool) -> Result<()> {
        let mut state = self.state()?;
        state.require_enabled()?;
        state.repository()?.set_recording_enabled(enabled)?;
        state.recording = enabled;
        state.policy.paused_until_ms = None;
        state.invalidate();
        Ok(())
    }
    pub fn request_recording(&self, enabled: bool, platform: &dyn Platform) -> Result<()> {
        if !enabled {
            return self.set_recording(false);
        }
        // Establish the boundary during Enable, not on the worker's next tick:
        // otherwise the first copy made within 500ms would become the baseline.
        let epoch = {
            let mut state = self.state()?;
            state.require_enabled()?;
            let generation = platform.clipboard_generation()?;
            state.repository()?.set_recording_enabled(true)?;
            state.recording = true;
            state.policy.paused_until_ms = None;
            state.invalidate();
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
    pub fn pause(&self, minutes: u32, platform: &dyn Platform) -> Result<()> {
        ensure!(
            matches!(minutes, 0 | 5 | 15 | 60),
            "unsupported pause duration"
        );
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
        state.repository()?.set_policy(policy)?;
        state.policy = policy;
        state.invalidate();
        state.generation = generation;
        drop(state);
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
        state.repository()?.set_policy(policy)?;
        state.policy = policy;
        let generation = state.generation;
        state.invalidate();
        state.generation = generation;
        state.expire((self.clock)())
    }
    /// Uses only metadata while resuming; clipboard contents copied during a pause are discarded.
    fn maintain(&self, platform: &dyn Platform) -> Result<()> {
        let now = (self.clock)();
        let mut state = self.state()?;
        if state.repository.is_err() {
            return Ok(());
        }
        if now >= state.next_cleanup_ms {
            state.expire(now)?;
            state.next_cleanup_ms = now.saturating_add(60_000);
        }
        if state
            .policy
            .paused_until_ms
            .is_some_and(|deadline| deadline <= now)
        {
            let generation = platform.clipboard_generation()?;
            let policy = HistoryPolicy {
                paused_until_ms: None,
                ..state.policy
            };
            state.repository()?.set_policy(policy)?;
            state.policy = policy;
            state.invalidate();
            state.generation = Some(generation);
        }
        Ok(())
    }
    fn wake_maintenance(&self) {
        if let Ok(worker) = self.maintenance.lock() {
            if let Some(worker) = worker.as_ref() {
                let _ = worker.0.send(MaintenanceEvent::Changed);
            }
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
        let mut state = self.state()?;
        state.require_enabled()?;
        state.expire((self.clock)())?;
        state.repository()?.search(query)
    }
    pub fn detail(&self, id: i64) -> Result<Option<HistoryDetail>> {
        self.detail_with(id, super::content::preview)
    }
    fn detail_with(
        &self,
        id: i64,
        render: impl FnOnce(
            HistoryItem,
            anywhere_platform::clipboard::ClipboardPayload,
        ) -> HistoryDetail,
    ) -> Result<Option<HistoryDetail>> {
        // Acquire before loading a body. Decoding never holds the policy/storage
        // lock, so capture, pause, deletion and configuration stay responsive.
        let _preview = self
            .preview
            .lock()
            .map_err(|_| anyhow::anyhow!("clipboard preview unavailable"))?;
        let (epoch, detail) = {
            let mut state = self.state()?;
            state.require_enabled()?;
            state.expire((self.clock)())?;
            let repository = state.repository()?;
            let Some(item) = repository.item(id)? else {
                return Ok(None);
            };
            let detail = repository.payload(id)?.map(|payload| (item, payload));
            (state.epoch, detail)
        };
        let detail = detail.map(|(item, payload)| render(item, payload));
        let mut state = self.state()?;
        state.require_enabled()?;
        state.expire((self.clock)())?;
        // A different window may delete or quota-evict the selected record while
        // its image is decoding. Never publish an orphaned preview.
        Ok(
            if state.epoch == epoch && state.repository()?.item(id)?.is_some() {
                detail
            } else {
                None
            },
        )
    }
    pub fn copy(&self, id: i64, platform: &dyn Platform) -> Result<()> {
        let mut state = self.state()?;
        state.require_enabled()?;
        state.expire((self.clock)())?;
        let payload = state
            .repository()?
            .payload(id)?
            .context("clipboard entry no longer exists")?;
        // Serialize with deletion/disable so a stale view cannot copy a deleted record.
        crate::features::host_services::for_feature(&super::DESCRIPTOR, platform)?
            .write_clipboard(&payload)
    }
    pub fn delete(&self, id: i64) -> Result<()> {
        let mut state = self.state()?;
        state.require_enabled()?;
        state.repository()?.delete(id)
    }
    pub fn clear(&self) -> Result<()> {
        let mut state = self.state()?;
        state.require_enabled()?;
        state.repository()?.clear()?;
        state.invalidate(); // Revoke a capture already in flight.
        Ok(())
    }
    pub fn poll(&self, platform: &dyn Platform, now_ms: i64) -> Result<()> {
        self.maintain(platform)?;
        let epoch = {
            let state = self.state()?;
            if !state.feature_enabled
                || !state.recording
                || state.policy.paused_until_ms.is_some()
                || state.repository.is_err()
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
        let mut state = self.state()?;
        if state.epoch != epoch || state.generation != Some(generation) {
            return Ok(());
        }
        match captured {
            Ok(Some(payload)) => {
                state.repository()?.record_payload(&payload, now_ms)?;
                state.expire((self.clock)())?;
                state.notice = None;
            }
            Ok(None) => {
                state.notice = None;
            }
            Err(error)
                if error.downcast_ref::<ContextReadError>()
                    == Some(&ContextReadError::TooLarge) =>
            {
                state.notice = Some(HistoryNotice::ItemTooLarge);
            }
            Err(error)
                if error.downcast_ref::<ContextReadError>()
                    == Some(&ContextReadError::PermissionDenied) =>
            {
                state.notice = Some(HistoryNotice::AccessRequired);
            }
            Err(_) => {
                state.notice = Some(HistoryNotice::ReadFailed);
            }
        }
        Ok(())
    }
    /// The clipboard-rs watcher retains only a Weak service; Drop requests shutdown.
    pub fn start(self: &Arc<Self>, platform: Arc<dyn Platform>) -> Result<()> {
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
impl State {
    fn expire(&mut self, now_ms: i64) -> Result<()> {
        if let Some(cutoff) = self.policy.cutoff(now_ms) {
            self.repository()?.expire(cutoff)?;
        }
        Ok(())
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
    fn repository(&mut self) -> Result<&mut (dyn HistoryRepository + 'static)> {
        match &mut self.repository {
            Ok(repository) => Ok(repository.as_mut()),
            Err(_) => anyhow::bail!("Clipboard history storage is unavailable"),
        }
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
