//! Closed control protocol. Runtime/document identity is supplied separately by the Host.
use crate::web_plugin::WebResourceKind;
#[cfg(feature = "web-schema")]
use crate::web_plugin::{PluginSnapshot, WebCurrentApp, WebInput, WebInvocation};
use serde::{Deserialize, Serialize};

pub const PROTOCOL: u32 = 2;
pub const MAX_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_CHUNK_BYTES: usize = 4096;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
pub const MAX_TRANSFER_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_REQUESTS: usize = 4096;
pub const MAX_PENDING: usize = 24;
pub const MAX_RUNNING: usize = 8;
pub const REQUEST_TIMEOUT_MS: u64 = 5000;
pub const URL_TIMEOUT_MS: u64 = 30_000;

/// A selected file's safe metadata, with numbers representable in JavaScript.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct WebResourceStat {
    pub handle: String,
    pub name: String,
    pub kind: WebResourceKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified_ms: Option<u64>,
}

/// Trust role assigned by the host for a single invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WebRole {
    Webview,
    Background,
    Resolver,
}

/// Broker policy for a request. The generated call schema describes its parameters.
#[derive(Debug)]
pub struct MethodSpec {
    pub name: &'static str,
    pub roles: &'static [WebRole],
    pub permission: Option<&'static str>,
    pub timeout_ms: u64,
    #[cfg(feature = "web-schema")]
    pub result_schema: fn() -> schemars::Schema,
}

const ALL: &[WebRole] = &[WebRole::Webview, WebRole::Background, WebRole::Resolver];
const EXPLICIT: &[WebRole] = &[WebRole::Webview, WebRole::Background];
const WEBVIEW: &[WebRole] = &[WebRole::Webview];

macro_rules! methods {
    ($( $variant:ident { $($(#[$field_meta:meta])* $field:ident: $param:ty),* } =>
        ($name:literal, $roles:ident, $permission:expr, $timeout:expr, $result:ty)
    ),+ $(,)?) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
        #[serde(tag = "method", content = "params", rename_all_fields = "camelCase", deny_unknown_fields)]
        pub enum WebCall {
            $(#[serde(rename = $name)]
            $variant { $($(#[$field_meta])* $field: $param),* },)+
        }

        /// Call metadata from the same declarations that define the wire shape.
        pub const WEB_METHODS: &[MethodSpec] = &[
            $(MethodSpec {
                name: $name,
                roles: $roles,
                permission: $permission,
                timeout_ms: $timeout,
                #[cfg(feature = "web-schema")]
                result_schema: result_schema::<$result>,
            },)+
        ];

        impl WebCall {
            /// Policy for a decoded request.
            pub const fn spec(&self) -> &'static MethodSpec {
                match self {
                    $(Self::$variant { .. } => &MethodSpec {
                        name: $name,
                        roles: $roles,
                        permission: $permission,
                        timeout_ms: $timeout,
                        #[cfg(feature = "web-schema")]
                        result_schema: result_schema::<$result>,
                    },)+
                }
            }
        }
    };
}

#[cfg(feature = "web-schema")]
fn result_schema<T: schemars::JsonSchema>() -> schemars::Schema {
    schemars::generate::SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<T>()
}

#[cfg(feature = "web-schema")]
#[derive(schemars::JsonSchema, Serialize)]
struct UploadTicket {
    ticket: String,
}

#[cfg(feature = "web-schema")]
#[derive(schemars::JsonSchema, Serialize)]
struct TextChunkResult {
    text: String,
    sequence: u32,
    done: bool,
}

#[cfg(feature = "web-schema")]
#[derive(schemars::JsonSchema, Serialize)]
struct CancellationResult {
    cancelled: bool,
}

methods! {
    InvocationGet {} => ("invocation.get", EXPLICIT, None, REQUEST_TIMEOUT_MS, WebInvocation),
    Ready {} => ("lifecycle.ready", WEBVIEW, None, REQUEST_TIMEOUT_MS, ()),
    StateSubscribe {} => ("state.subscribe", ALL, None, REQUEST_TIMEOUT_MS, PluginSnapshot),
    StateGet {} => ("state.get", ALL, None, REQUEST_TIMEOUT_MS, PluginSnapshot),
    ContextGet {} => ("context.get", ALL, None, REQUEST_TIMEOUT_MS, WebInput),
    InputGet {} => ("input.get", ALL, Some("input.read"), REQUEST_TIMEOUT_MS, String),
    InputSet { text: String } => ("input.set", EXPLICIT, Some("input.write"), REQUEST_TIMEOUT_MS, ()),
    SourceApp {} => ("app.getSource", ALL, Some("context.currentApp"), REQUEST_TIMEOUT_MS, Option<WebCurrentApp>),
    FrontmostApp {} => ("app.getFrontmost", ALL, Some("app.frontmost"), REQUEST_TIMEOUT_MS, Option<WebCurrentApp>),
    ClipboardRead {} => ("clipboard.readText", EXPLICIT, Some("clipboard.readText"), REQUEST_TIMEOUT_MS, Option<String>),
    ClipboardWrite { text: String } => ("clipboard.writeText", EXPLICIT, Some("clipboard.writeText"), REQUEST_TIMEOUT_MS, ()),
    StorageGet { key: String } => ("storage.get", EXPLICIT, Some("storage.local"), REQUEST_TIMEOUT_MS, serde_json::Value),
    StorageSet { key: String, value: serde_json::Value } => ("storage.set", EXPLICIT, Some("storage.local"), REQUEST_TIMEOUT_MS, ()),
    StorageRemove { key: String } => ("storage.remove", EXPLICIT, Some("storage.local"), REQUEST_TIMEOUT_MS, ()),
    TextBegin { bytes: usize } => ("text.begin", EXPLICIT, Some("clipboard.writeText"), REQUEST_TIMEOUT_MS, UploadTicket),
    TextChunk { ticket: String, sequence: u32, text: String } => ("text.chunk", EXPLICIT, Some("clipboard.writeText"), REQUEST_TIMEOUT_MS, ()),
    TextCommit { ticket: String } => ("text.commit", EXPLICIT, Some("clipboard.writeText"), REQUEST_TIMEOUT_MS, ()),
    TextRead { ticket: String, sequence: u32 } => ("text.read", EXPLICIT, None, REQUEST_TIMEOUT_MS, TextChunkResult),
    TextCancel { ticket: String } => ("text.cancel", EXPLICIT, None, REQUEST_TIMEOUT_MS, ()),
    Cancel { request_id: String } => ("request.cancel", EXPLICIT, None, REQUEST_TIMEOUT_MS, CancellationResult),
    WindowBack {} => ("window.back", WEBVIEW, None, REQUEST_TIMEOUT_MS, ()),
    WindowClose {} => ("window.close", WEBVIEW, None, REQUEST_TIMEOUT_MS, ()),
    UrlOpen { url: String } => ("url.open", EXPLICIT, Some("url.open"), URL_TIMEOUT_MS, ()),
    ResourceStat { handle: String } => ("resources.stat", EXPLICIT, Some("resources.read"), REQUEST_TIMEOUT_MS, WebResourceStat),
    ResourceReadText { handle: String, #[serde(default, deserialize_with = "optional_limit")] #[cfg_attr(feature = "web-schema", schemars(with = "usize"))] max_bytes: Option<usize> } => ("resources.readText", EXPLICIT, Some("resources.read"), REQUEST_TIMEOUT_MS, String),
    PreviewOpen { handle: String } => ("preview.open", WEBVIEW, Some("preview.open"), REQUEST_TIMEOUT_MS, ()),
}

fn optional_limit<'de, D: serde::Deserializer<'de>>(value: D) -> Result<Option<usize>, D::Error> {
    usize::deserialize(value).map(Some)
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
