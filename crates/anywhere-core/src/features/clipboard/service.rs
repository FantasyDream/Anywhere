//! Serial policy/write boundary. Native reads run outside the lock and recheck epoch.
use super::history::*;
use anyhow::{ensure, Context, Result};
use anywhere_platform::{ContextReadError, Platform};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct HistoryStatus {
    pub recording: bool,
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
}
struct State {
    repository: Result<Box<dyn HistoryRepository>, String>,
    feature_enabled: bool,
    recording: bool,
    epoch: u64,
    generation: Option<i64>,
    notice: Option<HistoryNotice>,
}
pub struct HistoryService {
    state: Mutex<State>,
    // Shared by Launcher and Shelf: admit one payload/decoder at a time.
    preview: Mutex<()>,
    watcher: Mutex<Option<Box<dyn anywhere_platform::clipboard::ClipboardWatch>>>,
}
impl HistoryService {
    pub fn new(mut repository: Result<Box<dyn HistoryRepository>, String>) -> Self {
        let recording = match repository.as_ref().map(|r| r.recording_enabled()) {
            Ok(Ok(recording)) => recording,
            Ok(Err(_)) => {
                repository = Err("clipboard preferences unavailable".into());
                false
            }
            Err(_) => false,
        };
        Self {
            state: Mutex::new(State {
                repository,
                feature_enabled: true,
                recording,
                epoch: 0,
                generation: None,
                notice: None,
            }),
            preview: Mutex::new(()),
            watcher: Mutex::new(None),
        }
    }
    fn state(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("clipboard history unavailable"))
    }
    pub fn status(&self) -> Result<HistoryStatus> {
        let state = self.state()?;
        Ok(HistoryStatus {
            recording: state.recording,
            available: state.feature_enabled && state.repository.is_ok(),
            notice: if !state.feature_enabled {
                Some(HistoryNotice::FeatureDisabled)
            } else if state.repository.is_err() {
                Some(HistoryNotice::StorageUnavailable)
            } else {
                state.notice
            },
        })
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
    pub fn search(&self, query: &str) -> Result<Vec<HistoryItem>> {
        let mut state = self.state()?;
        state.require_enabled()?;
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
        let epoch = {
            let state = self.state()?;
            if !state.feature_enabled || !state.recording || state.repository.is_err() {
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
        {
            let mut state = self.state()?;
            if state.feature_enabled && state.recording {
                state.generation = platform.clipboard_generation().ok();
            }
        }
        let service = Arc::downgrade(self);
        let capture_platform = platform.clone();
        let watcher = platform.watch_clipboard(Box::new(move || {
            let Some(service) = service.upgrade() else {
                return;
            };
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .min(i64::MAX as u128) as i64;
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
