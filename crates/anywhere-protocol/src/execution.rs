use super::{
    identifiers::{ExecutionSessionId, Revision, SessionId},
    ui,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionSessionStatus {
    Running,
    Stopping,
    Succeeded,
    Failed,
    Stopped,
}

impl ExecutionSessionStatus {
    pub fn is_active(self) -> bool {
        matches!(self, Self::Running | Self::Stopping)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSessionSummary {
    pub id: ExecutionSessionId,
    pub title: String,
    pub directory: String,
    pub command: Vec<String>,
    pub status: ExecutionSessionStatus,
    pub exit_code: Option<i32>,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    /// Advances when either output stream changes, including a full rolling tail.
    #[serde(default)]
    pub output_revision: i64,
    pub output_truncated: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSessionDetail {
    pub summary: ExecutionSessionSummary,
    /// Each stream is independently bounded. Cross-stream ordering is not promised.
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Succeeded,
    Failed,
}

/// Host-rendered destinations requested by a successfully executed Action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostView {
    FileSearch,
    NativeTool {
        id: String,
        input: String,
    },
    PluginDetail {
        detail: ui::DetailView,
    },
    /// Opaque Host-issued ticket. Not a resource URL or a page-supplied plugin identity.
    WebPlugin {
        invocation_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionResult {
    pub session_id: SessionId,
    pub revision: Revision,
    pub status: ActionStatus,
    /// Present when the action itself failed.
    pub error: Option<String>,
    /// Present when the action succeeded but a follow-up step did not.
    pub warning: Option<String>,
    pub output_text: Option<String>,
    #[serde(default)]
    pub view: Option<HostView>,
    pub close_window: bool,
    #[serde(default)]
    pub execution_session_id: Option<ExecutionSessionId>,
}

impl ActionResult {
    pub fn succeeded(&self) -> bool {
        self.status == ActionStatus::Succeeded
    }

    pub fn notice(&self) -> &str {
        self.error
            .as_deref()
            .or(self.warning.as_deref())
            .unwrap_or("")
    }
}
