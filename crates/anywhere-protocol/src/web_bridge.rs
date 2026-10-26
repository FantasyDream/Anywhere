//! Closed control protocol. Runtime/document identity is supplied separately by the Host.
use serde::{Deserialize, Serialize};

pub const PROTOCOL: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_CHUNK_BYTES: usize = 4096;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
pub const MAX_TRANSFER_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_REQUESTS: usize = 4096;
pub const MAX_PENDING: usize = 24;
pub const MAX_RUNNING: usize = 8;
pub const REQUEST_TIMEOUT_MS: u64 = 5000;
pub const CONFIRM_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(tag = "method", content = "params", deny_unknown_fields)]
pub enum WebCall {
    #[serde(rename = "invocation.get")]
    InvocationGet {},
    #[serde(rename = "lifecycle.ready")]
    Ready {},
    #[serde(rename = "context.getCurrentApp")]
    CurrentApp {},
    #[serde(rename = "state.subscribe")]
    StateSubscribe {},
    #[serde(rename = "state.get")]
    StateGet {},
    #[serde(rename = "context.get")]
    ContextGet {},
    #[serde(rename = "input.get")]
    InputGet {},
    #[serde(rename = "input.set")]
    InputSet { text: String },
    #[serde(rename = "app.getSource")]
    SourceApp {},
    #[serde(rename = "app.getFrontmost")]
    FrontmostApp {},
    #[serde(rename = "clipboard.readText")]
    ClipboardRead {},
    #[serde(rename = "clipboard.writeText")]
    ClipboardWrite { text: String },
    #[serde(rename = "storage.get")]
    StorageGet { key: String },
    #[serde(rename = "storage.set")]
    StorageSet {
        key: String,
        value: serde_json::Value,
    },
    #[serde(rename = "storage.remove")]
    StorageRemove { key: String },
    #[serde(rename = "text.begin")]
    TextBegin { bytes: usize },
    #[serde(rename = "text.chunk")]
    TextChunk {
        ticket: String,
        sequence: u32,
        text: String,
    },
    #[serde(rename = "text.commit")]
    TextCommit { ticket: String },
    #[serde(rename = "text.read")]
    TextRead { ticket: String, sequence: u32 },
    #[serde(rename = "text.cancel")]
    TextCancel { ticket: String },
    #[serde(rename = "request.cancel")]
    Cancel { request_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WebErrorCode {
    InvalidRequest,
    UnsupportedMethod,
    IncompatibleProtocol,
    UntrustedDocument,
    Inactive,
    PermissionDenied,
    DuplicateRequest,
    QuotaExceeded,
    Busy,
    TooLarge,
    Expired,
    Cancelled,
    UserCancelled,
    Timeout,
    StorageUnavailable,
    HostFailure,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebError {
    pub code: WebErrorCode,
    pub message: &'static str,
    pub retryable: bool,
}
impl WebError {
    pub fn new(code: WebErrorCode) -> Self {
        Self {
            code,
            message: match code {
                WebErrorCode::PermissionDenied => "This operation is not permitted.",
                WebErrorCode::UserCancelled => "The operation was declined.",
                WebErrorCode::Busy => "The plugin has too many pending requests.",
                WebErrorCode::TooLarge => "The data exceeds the bridge limit.",
                WebErrorCode::StorageUnavailable => "Plugin storage is unavailable.",
                _ => "The plugin request could not be completed.",
            },
            retryable: false,
        }
    }
}
pub type WebResult<T> = Result<T, WebError>;

#[derive(Serialize)]
pub struct WebResponse {
    #[serde(rename = "requestId")]
    pub request_id: Option<String>,
    #[serde(flatten)]
    pub outcome: WebOutcome,
}
#[derive(Serialize)]
#[serde(untagged)]
pub enum WebOutcome {
    Success { result: serde_json::Value },
    Failure { error: WebError },
}
