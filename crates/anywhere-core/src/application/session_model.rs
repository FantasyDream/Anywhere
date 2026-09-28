//! Application-owned state for one launcher activation.
use crate::core::context::ResolvedContext;
use crate::core::ranking::score::UsageStats;
use crate::core::search::candidate::Candidate;
use crate::core::session::facts::{FactValue, SessionFacts};
use anyhow::Result;
use anywhere_platform::ApplicationInfo;
use anywhere_protocol::*;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

#[derive(Default)]
pub struct SessionIdentity {
    pub frontmost: Option<ApplicationInfo>,
    pub source_app: String,
    /// One-shot activation reads. They survive query revisions but never a new session.
    pub captures: Vec<ActivationCapture>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureStatus {
    Captured,
    Empty,
    PermissionDenied,
    Unavailable,
    Failed,
}

impl From<CaptureStatus> for ContextCaptureStatus {
    fn from(status: CaptureStatus) -> Self {
        match status {
            CaptureStatus::Captured => Self::Captured,
            CaptureStatus::Empty => Self::Empty,
            CaptureStatus::PermissionDenied => Self::PermissionDenied,
            CaptureStatus::Unavailable => Self::Unavailable,
            CaptureStatus::Failed => Self::Failed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActivationCapture {
    pub source: ContextSource,
    pub captured_at_ms: i64,
    pub status: CaptureStatus,
    pub contexts: Vec<ResolvedContext>,
}

#[derive(Default)]
pub struct SessionResolution {
    pub context_cache: Vec<ResolvedContext>,
    pub committed_revision: Option<Revision>,
    pub committed_contexts: Vec<ResolvedContext>,
    pub published_contexts: usize,
    pub snapshot_seq: u32,
    /// Facts and usage copied for a snapshot. A commit retries when this changes.
    pub inputs_epoch: u64,
    pub frozen_scope_ids: Vec<ScopeId>,
}

#[derive(Default)]
pub struct SessionExecution {
    pub candidates: HashMap<ActionId, Candidate>,
    pub revision: Option<Revision>,
}

#[derive(Default)]
pub struct SessionMenu {
    pub generation: u64,
    pub trusted: bool,
}

#[derive(Default)]
pub struct LauncherSession {
    pub state: LauncherState,
    pub activation_started: bool,
    pub identity: SessionIdentity,
    pub environment: BTreeMap<String, FactValue>,
    pub resolution: SessionResolution,
    pub execution: SessionExecution,
    pub menu: SessionMenu,
    pub usage: Arc<HashMap<(String, String), UsageStats>>,
    pub app_usage: Arc<HashMap<(String, String), UsageStats>>,
    pub actions: Arc<crate::core::action::descriptor::ActionRegistry>,
    /// Clipboard, git availability and history are applied after the first snapshot.
    pub supplemental_pending: bool,
}

impl LauncherSession {
    pub fn reserved(
        session_id: SessionId,
        actions: Arc<crate::core::action::descriptor::ActionRegistry>,
    ) -> Self {
        Self {
            state: LauncherState {
                session_id,
                ..Default::default()
            },
            actions,
            ..Default::default()
        }
    }

    pub fn begin_revision(&mut self, revision: Revision, input: String) {
        self.state.revision = revision;
        self.state.input = input;
        self.state.context_complete = false;
        self.resolution.committed_revision = None;
        self.resolution.snapshot_seq = 0;
        self.resolution.published_contexts = 0;
    }

    pub fn invalidate_execution(&mut self) {
        self.execution.candidates.clear();
        self.resolution.context_cache.clear();
    }

    pub fn record_fact(&mut self, key: impl Into<String>, value: FactValue) -> Result<()> {
        let key = key.into();
        crate::core::session::facts::validate_write(&key, &value)?;
        self.environment.insert(key, value);
        self.note_snapshot_inputs();
        Ok(())
    }

    pub fn record_capture(&mut self, capture: ActivationCapture) {
        self.identity
            .captures
            .retain(|held| held.source != capture.source);
        self.identity.captures.push(capture);
        self.resolution.inputs_epoch = self.resolution.inputs_epoch.saturating_add(1);
    }

    pub fn replace_usage(
        &mut self,
        usage: Arc<HashMap<(String, String), UsageStats>>,
        app_usage: Arc<HashMap<(String, String), UsageStats>>,
    ) {
        self.usage = usage;
        self.app_usage = app_usage;
        self.note_snapshot_inputs();
    }

    fn note_snapshot_inputs(&mut self) {
        self.resolution.inputs_epoch = self.resolution.inputs_epoch.saturating_add(1);
    }

    pub fn facts(&self) -> SessionFacts {
        SessionFacts::from_map(self.identity.frontmost.clone(), self.environment.clone())
            .expect("session facts are validated at capture")
    }
}
