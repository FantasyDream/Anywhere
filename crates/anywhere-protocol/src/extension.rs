//! Versioned metadata and explicit host operations, independent of Rust Traits.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExtensionCapability {
    #[serde(rename = "clipboard.write")]
    ClipboardWrite,
    #[serde(rename = "ui.render")]
    RenderUi,
    #[serde(rename = "url.open")]
    OpenUrl,
    #[serde(rename = "file.open")]
    OpenFile,
    #[serde(rename = "git.read")]
    GitRead,
    #[serde(rename = "process.start")]
    ProcessStart,
    #[serde(rename = "storage")]
    Storage,
    /// Legacy v1 compatibility only; v2 delegation is not yet exposed.
    #[serde(rename = "builtin.invoke")]
    InvokeBuiltin,
}
impl ExtensionCapability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClipboardWrite => "clipboard.write",
            Self::RenderUi => "ui.render",
            Self::OpenUrl => "url.open",
            Self::OpenFile => "file.open",
            Self::GitRead => "git.read",
            Self::ProcessStart => "process.start",
            Self::Storage => "storage",
            Self::InvokeBuiltin => "builtin.invoke",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirstPartyManifest {
    pub protocol_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub capabilities: Vec<ExtensionCapability>,
}
