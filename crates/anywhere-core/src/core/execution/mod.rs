//! App-lifetime process sessions. Launcher dismissal and query changes do not own these.
use anyhow::{bail, Context, Result};
use anywhere_platform::process::{OwnedProcess, ProcessRequest};
use anywhere_protocol::*;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;

pub const MAX_RUNNING_SESSIONS: usize = 4;
pub const MAX_RETAINED_SESSIONS: usize = 32;
pub const MAX_STREAM_BYTES: usize = 128 * 1024;

pub struct ExecutionSessionManager {
    state: Mutex<ManagerState>,
    updates: watch::Sender<u64>,
}

impl Default for ExecutionSessionManager {
    fn default() -> Self {
        let (updates, _) = watch::channel(0);
        Self {
            state: Mutex::new(ManagerState::default()),
            updates,
        }
    }
}

#[derive(Default)]
struct ManagerState {
    next_id: u64,
    closed: bool,
    sessions: BTreeMap<ExecutionSessionId, Session>,
}

struct Session {
    record: Arc<Record>,
    worker: JoinHandle<()>,
}

struct Record {
    cancel: AtomicBool,
    state: Mutex<RecordState>,
}

struct RecordState {
    summary: ExecutionSessionSummary,
    stdout: TailBuffer,
    stderr: TailBuffer,
}

#[derive(Default)]
struct TailBuffer {
    bytes: VecDeque<u8>,
    truncated: bool,
}

impl TailBuffer {
    fn append(&mut self, bytes: &[u8]) {
        let overflow = (self.bytes.len() + bytes.len()).saturating_sub(MAX_STREAM_BYTES);
        if overflow > 0 {
            // Drain before extending so VecDeque does not double its capacity at the limit.
            self.bytes.drain(..overflow.min(self.bytes.len()));
            self.truncated = true;
        }
        self.bytes
            .extend(&bytes[bytes.len().saturating_sub(MAX_STREAM_BYTES)..]);
    }

    fn text(&mut self) -> String {
        // A tail may begin mid-codepoint; skip that incomplete prefix only. Invalid
        // process bytes elsewhere are displayed as replacement characters.
        let bytes = self.bytes.make_contiguous();
        let skip = if self.truncated {
            bytes
                .iter()
                .take(3)
                .take_while(|byte| **byte & 0xc0 == 0x80)
                .count()
        } else {
            0
        };
        String::from_utf8_lossy(&bytes[skip..]).into_owned()
    }
}

impl ExecutionSessionManager {
    pub fn updates(&self) -> watch::Receiver<u64> {
        self.updates.subscribe()
    }

    fn publish_update(&self) {
        self.updates.send_modify(|revision| {
            *revision = revision.saturating_add(1);
        });
    }

    pub fn start(&self, request: ProcessRequest) -> Result<ExecutionSessionId> {
        let mut manager = manager_state(&self.state)?;
        if manager.closed {
            bail!("task manager is shutting down");
        }
        let active = manager
            .sessions
            .values()
            .filter(|session| record_state(&session.record).summary.status.is_active())
            .count();
        if active >= MAX_RUNNING_SESSIONS {
            bail!("Four tasks are already running. Stop or wait for one before starting another.");
        }
        // Spawn can fail; do not evict a completed result or publish a running row until it succeeds.
        let process = OwnedProcess::spawn(&request)?;
        manager.next_id = manager
            .next_id
            .checked_add(1)
            .context("execution session IDs exhausted")?;
        let id = format!("execution-{:020}", manager.next_id);
        let record = Arc::new(Record {
            cancel: AtomicBool::new(false),
            state: Mutex::new(RecordState {
                summary: ExecutionSessionSummary {
                    id: id.clone(),
                    title: request.title,
                    directory: request.directory.to_string_lossy().into_owned(),
                    command: std::iter::once(request.program.to_string_lossy().into_owned())
                        .chain(request.arguments)
                        .collect(),
                    status: ExecutionSessionStatus::Running,
                    exit_code: None,
                    started_at_ms: now(),
                    finished_at_ms: None,
                    output_revision: 0,
                    output_truncated: false,
                    error: None,
                },
                stdout: TailBuffer::default(),
                stderr: TailBuffer::default(),
            }),
        });
        let task_record = record.clone();
        let updates = self.updates.clone();
        let worker = thread::Builder::new()
            .name("execution-session".into())
            .spawn(move || run(process, task_record, updates))?;
        let evicted = if manager.sessions.len() >= MAX_RETAINED_SESSIONS {
            let oldest = manager
                .sessions
                .iter()
                .filter_map(|(id, session)| {
                    let state = record_state(&session.record);
                    (!state.summary.status.is_active())
                        .then(|| (state.summary.started_at_ms, id.clone()))
                })
                .min()
                .map(|(_, id)| id)
                .context("completed session required for eviction")?;
            manager.sessions.remove(&oldest)
        } else {
            None
        };
        manager
            .sessions
            .insert(id.clone(), Session { record, worker });
        drop(manager);
        if let Some(session) = evicted {
            let _ = session.worker.join();
        }
        self.publish_update();
        Ok(id)
    }

    pub fn list(&self) -> Result<Vec<ExecutionSessionSummary>> {
        let manager = manager_state(&self.state)?;
        let mut summaries: Vec<_> = manager
            .sessions
            .values()
            .map(|session| record_state(&session.record).summary.clone())
            .collect();
        summaries.sort_by(|left, right| {
            right
                .started_at_ms
                .cmp(&left.started_at_ms)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(summaries)
    }

    pub fn detail(&self, id: &str) -> Result<Option<ExecutionSessionDetail>> {
        let record = {
            let manager = manager_state(&self.state)?;
            let Some(record) = manager
                .sessions
                .get(id)
                .map(|session| session.record.clone())
            else {
                return Ok(None);
            };
            record
        };
        let mut state = record
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("task unavailable"))?;
        Ok(Some(ExecutionSessionDetail {
            summary: state.summary.clone(),
            stdout: state.stdout.text(),
            stderr: state.stderr.text(),
        }))
    }

    pub fn stop(&self, id: &str) -> Result<()> {
        let manager = manager_state(&self.state)?;
        let record = &manager
            .sessions
            .get(id)
            .context("task no longer exists")?
            .record;
        let mut state = record
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("task unavailable"))?;
        if state.summary.status.is_active() {
            state.summary.status = ExecutionSessionStatus::Stopping;
            record.cancel.store(true, Ordering::Release);
            drop(state);
            self.publish_update();
        }
        Ok(())
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        let mut manager = manager_state(&self.state)?;
        if let Some(session) = manager.sessions.get(id) {
            if record_state(&session.record).summary.status.is_active() {
                bail!("stop the task before removing it");
            }
        }
        if let Some(session) = manager.sessions.remove(id) {
            drop(manager);
            let _ = session.worker.join();
            self.publish_update();
        }
        Ok(())
    }

    /// Called at app termination. Closing the task window itself never calls this.
    pub fn shutdown(&self) -> Result<()> {
        let sessions = {
            let mut manager = manager_state(&self.state)?;
            manager.closed = true;
            for session in manager.sessions.values() {
                session.record.cancel.store(true, Ordering::Release);
            }
            std::mem::take(&mut manager.sessions)
        };
        for session in sessions.into_values() {
            let _ = session.worker.join();
        }
        Ok(())
    }
}

impl Drop for ExecutionSessionManager {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            tracing::error!(%error, "execution_session_shutdown_failed");
        }
    }
}

fn run(mut process: OwnedProcess, record: Arc<Record>, updates: watch::Sender<u64>) {
    let result = (|| -> Result<std::process::ExitStatus> {
        loop {
            let received_output = drain(&mut process, &record, &updates, 4)?;
            if record.cancel.load(Ordering::Acquire) {
                break process.stop();
            }
            if let Some(status) = process.poll()? {
                break Ok(status);
            }
            // Idle tasks need fewer wakeups; active streams are drained promptly.
            thread::sleep(Duration::from_millis(if received_output { 10 } else { 50 }));
        }
    })();
    // Finite final drain: a descendant cannot hold the session open forever.
    let drain_error = drain(&mut process, &record, &updates, 32).err();
    drop(process);
    let mut state = record_state(&record);
    state.summary.finished_at_ms = Some(now());
    match result {
        Ok(status) => {
            state.summary.exit_code = status.code();
            state.summary.status = if record.cancel.load(Ordering::Acquire) {
                ExecutionSessionStatus::Stopped
            } else if status.success() {
                ExecutionSessionStatus::Succeeded
            } else {
                ExecutionSessionStatus::Failed
            };
        }
        Err(_) => {
            state.summary.status = ExecutionSessionStatus::Failed;
            state.summary.error =
                Some("The task could not be monitored; its processes were stopped.".into());
        }
    }
    if drain_error.is_some() {
        state.summary.error = Some("Some task output could not be read.".into());
    }
    drop(state);
    updates.send_modify(|revision| {
        *revision = revision.saturating_add(1);
    });
}

fn drain(
    process: &mut OwnedProcess,
    record: &Record,
    updates: &watch::Sender<u64>,
    rounds: usize,
) -> Result<bool> {
    let mut buffer = [0; 8192];
    let mut received_output = false;
    for _ in 0..rounds {
        let stdout = process.read_stdout(&mut buffer)?;
        // Publish stdout even if reading stderr fails afterwards.
        if stdout > 0 {
            let mut state = record_state(record);
            state.append_output(&buffer[..stdout], &[]);
            received_output = true;
        }
        let stderr = process.read_stderr(&mut buffer)?;
        if stderr > 0 {
            let mut state = record_state(record);
            state.append_output(&[], &buffer[..stderr]);
            received_output = true;
        }
        if stdout > 0 || stderr > 0 {
            updates.send_modify(|revision| {
                *revision = revision.saturating_add(1);
            });
        }
        if stdout == 0 && stderr == 0 {
            break;
        }
    }
    Ok(received_output)
}

impl RecordState {
    fn append_output(&mut self, stdout: &[u8], stderr: &[u8]) {
        if stdout.is_empty() && stderr.is_empty() {
            return;
        }
        self.stdout.append(stdout);
        self.stderr.append(stderr);
        self.summary.output_revision = self.summary.output_revision.saturating_add(1);
        self.summary.output_truncated = self.stdout.truncated || self.stderr.truncated;
    }
}

fn record_state(record: &Record) -> std::sync::MutexGuard<'_, RecordState> {
    record.state.lock().unwrap_or_else(|poisoned| {
        tracing::error!("execution_record_lock_poisoned");
        poisoned.into_inner()
    })
}

fn manager_state(state: &Mutex<ManagerState>) -> Result<std::sync::MutexGuard<'_, ManagerState>> {
    state
        .lock()
        .map_err(|_| anyhow::anyhow!("session manager unavailable"))
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_output_tail_does_not_double_allocation_and_preserves_unicode() {
        let mut tail = TailBuffer::default();
        tail.append(&vec![b'a'; MAX_STREAM_BYTES]);
        tail.append("🙂done".as_bytes());
        assert_eq!(tail.bytes.len(), MAX_STREAM_BYTES);
        assert!(tail.bytes.capacity() <= MAX_STREAM_BYTES);
        assert!(tail.text().ends_with("🙂done"));
        let mut unicode = TailBuffer::default();
        unicode.append("🙂".repeat(MAX_STREAM_BYTES / 4).as_bytes());
        unicode.append(b"x");
        assert!(unicode.truncated);
        assert!(!unicode.text().contains('�'));
        assert!(unicode.text().ends_with('x'));
    }

    #[test]
    fn poisoned_manager_reports_unavailable_instead_of_an_empty_success() {
        let manager = ExecutionSessionManager::default();
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _state = manager.state.lock().unwrap();
            panic!("poison the synthetic manager");
        }));
        assert!(poisoned.is_err());

        let error = manager.list().unwrap_err();
        assert!(error.to_string().contains("session manager unavailable"));
        assert!(manager.shutdown().is_err());
    }
}
