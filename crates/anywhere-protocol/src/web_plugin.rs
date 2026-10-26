//! Web package metadata only. This module grants no host access and describes no UI.
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
pub const API_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebPluginManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub platforms: Vec<WebPlatform>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub entry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<PluginBackground>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub permissions: Vec<WebPermission>,
    #[serde(default)]
    pub commands: Vec<WebCommand>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum WebPlatform {
    Macos,
    Windows,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(tag = "name", deny_unknown_fields)]
pub enum WebPermission {
    #[serde(rename = "context.read")]
    ContextRead { types: Vec<String> },
    #[serde(rename = "context.currentApp")]
    CurrentApp {},
    #[serde(rename = "input.read")]
    InputRead {},
    #[serde(rename = "input.write")]
    InputWrite {},
    #[serde(rename = "app.frontmost")]
    FrontmostApp {},
    #[serde(rename = "clipboard.readText")]
    ClipboardReadText {},
    #[serde(rename = "clipboard.writeText")]
    ClipboardWriteText {},
    #[serde(rename = "storage.local")]
    LocalStorage {},
}

impl WebPermission {
    pub fn name(&self) -> &'static str {
        match self {
            Self::ContextRead { .. } => "context.read",
            Self::CurrentApp {} => "context.currentApp",
            Self::InputRead {} => "input.read",
            Self::InputWrite {} => "input.write",
            Self::FrontmostApp {} => "app.frontmost",
            Self::ClipboardReadText {} => "clipboard.readText",
            Self::ClipboardWriteText {} => "clipboard.writeText",
            Self::LocalStorage {} => "storage.local",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebCommand {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub accepts: Vec<String>,
    /// Host-owned content recognition. No plugin code runs during search or capture.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_matchers: Vec<WebInputMatcher>,
    /// Hide commands that cannot run without recognized content, including keyword search.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub require_input_match: bool,
    pub entrypoint: String,
    #[serde(default, skip_serializing_if = "PluginRuntime::is_webview")]
    pub runtime: PluginRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WebInputMatcher {
    /// Strict JSON objects and arrays; scalars are ordinary search terms.
    Json,
    /// JSON containers encoded as a JSON string (optionally without outer quotes).
    JsonEscaped,
}

/// Minimal input for one explicit invocation; no graph, filesystem paths or AX data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WebInput {
    Empty {
        reason: WebInputReason,
    },
    Text {
        #[cfg_attr(feature = "web-schema", schemars(with = "String"))]
        source: crate::ContextSource,
        text: String,
    },
    Url {
        #[cfg_attr(feature = "web-schema", schemars(with = "String"))]
        source: crate::ContextSource,
        url: String,
    },
    Data {
        type_id: String,
        value: serde_json::Value,
    },
    Resources {
        #[cfg_attr(feature = "web-schema", schemars(with = "String"))]
        source: crate::ContextSource,
        items: Vec<WebResource>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WebInputReason {
    NoObject,
    PermissionDenied,
    UnsupportedType,
    TooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
pub struct WebResource {
    pub handle: String,
    pub name: String,
    pub kind: WebResourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum WebResourceKind {
    File,
    Directory,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
pub struct WebInvocation {
    pub plugin_id: String,
    pub command_id: String,
    pub entrypoint: String,
    pub input: WebInput,
}

/// Frozen before activation; process ids, window titles and AX data are excluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
pub struct WebCurrentApp {
    pub id: String,
    pub name: String,
}

/// Latest approved view of Launcher state. Native graph data never crosses this boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct PluginSnapshot {
    pub version: u64,
    pub context: WebInput,
    pub input: Option<String>,
}

/// Code is compiled to one local ES module when building the package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginBackground {
    pub entry: String,
    #[serde(default)]
    pub resolvers: Vec<PluginResolver>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PluginRuntime {
    #[default]
    Webview,
    Background,
}
impl PluginRuntime {
    pub fn is_webview(&self) -> bool {
        *self == Self::Webview
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginResolver {
    pub id: String,
    pub accepts: Vec<String>,
    pub produces: Vec<String>,
    pub entrypoint: String,
}
