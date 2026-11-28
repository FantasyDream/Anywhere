//! Web package metadata only. This module grants no host access and describes no UI.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u32 = 1;
pub const API_VERSION: &str = "2";
pub const MAX_PERMISSIONS: usize = 16;
pub const COMMAND_ICONS: &[&str] = &["code", "copy", "file", "globe", "search", "clipboard"];

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
    #[serde(rename = "url.open")]
    UrlOpen { origins: Vec<String> },
    #[serde(rename = "resources.read")]
    ResourcesRead {},
    #[serde(rename = "preview.open")]
    PreviewOpen {},
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
            Self::UrlOpen { .. } => "url.open",
            Self::ResourcesRead {} => "resources.read",
            Self::PreviewOpen {} => "preview.open",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebCommand {
    pub id: String,
    pub title: String,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "unique_map"
    )]
    pub localized_titles: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
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
    /// A typed Context is mandatory even when the command title matches a query.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub require_context: bool,
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
#[serde(
    tag = "kind",
    rename_all_fields = "camelCase",
    rename_all = "snake_case"
)]
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
    pub readable: bool,
    pub previewable: bool,
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
#[serde(rename_all = "camelCase")]
pub struct WebInvocation {
    pub plugin_id: String,
    pub command_id: String,
    pub entrypoint: String,
    pub input: WebInput,
    pub capabilities: WebCapabilities,
}

/// Presentation hints. Each broker operation checks the live grant independently.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct WebCapabilities {
    pub url_open: bool,
    pub resource_read: bool,
    pub preview_open: bool,
    pub window_control: bool,
}

fn unique_map<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    use serde::de::{Error, MapAccess, Visitor};
    struct UniqueMap;
    impl<'de> Visitor<'de> for UniqueMap {
        type Value = BTreeMap<String, String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a map with unique keys")
        }

        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut entries = BTreeMap::new();
            while let Some((key, value)) = map.next_entry()? {
                if entries.insert(key, value).is_some() {
                    return Err(M::Error::custom("duplicate localizedTitles key"));
                }
            }
            Ok(entries)
        }
    }
    deserializer.deserialize_map(UniqueMap)
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

impl WebCommand {
    pub fn needs_context(&self) -> bool {
        self.require_context
            || self.require_input_match
            || (self.runtime == PluginRuntime::Background && !self.accepts.is_empty())
    }
}
