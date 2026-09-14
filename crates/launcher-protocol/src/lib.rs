//! Stable, serializable boundary types. No UI or OS dependencies.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type SessionId = u32;
pub type Revision = u32;
pub type ContextId = String;
pub type ContextTypeId = String;
pub type ProviderId = String;
pub type CommandId = String;
pub type ActionId = String;

pub const TEXT: &str = "core.text";
pub const URL: &str = "core.url";
pub const JSON: &str = "core.json";
pub const FILE: &str = "core.file";
pub const DIRECTORY: &str = "core.directory";
pub const APPLICATION: &str = "core.application";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    UserInput,
    ForegroundApplication,
    ApplicationContext,
}

/// Open type identifier + transport value. New types don't change this protocol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextDescriptor {
    pub id: ContextId,
    pub type_id: ContextTypeId,
    pub value: String,
    #[serde(default)]
    pub properties: BTreeMap<String, String>,
    pub source: ContextSource,
    pub provider_id: ProviderId,
    pub source_context_id: Option<ContextId>,
    pub confidence: f32,
    pub depth: u8,
    pub session_id: SessionId,
    pub revision: Revision,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextPresentation {
    pub id: ContextId,
    pub type_id: ContextTypeId,
    pub label: String,
    pub detail: String,
    pub source: ContextSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconDescriptor {
    /// Semantic icon key; the host maps it to a visual glyph.
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionPresentation {
    pub id: ActionId,
    pub command_id: CommandId,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: IconDescriptor,
    pub score: f32,
}
pub type ActionDescriptor = ActionPresentation;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LauncherState {
    pub session_id: SessionId,
    pub revision: Revision,
    pub input: String,
    pub context_summary: Vec<ContextPresentation>,
    pub actions: Vec<ActionPresentation>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionResult {
    pub session_id: SessionId,
    pub revision: Revision,
    pub success: bool,
    pub message: String,
    pub output_text: Option<String>,
    pub close_window: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextMatcher {
    pub type_id: ContextTypeId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub protocol_version: u32,
    pub id: String,
    pub name: String,
    pub commands: Vec<PluginCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginCommand {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub accepts: Vec<ContextTypeId>,
    #[serde(default)]
    pub base_priority: i32,
    pub action: DeclarativeAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeclarativeAction {
    OpenUrl { url: String },
    OpenFile { path: String },
    Copy { text: String },
    ExecuteBuiltin { command_id: CommandId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LauncherEvent {
    ContextAdded {
        context: ContextDescriptor,
    },
    ActionsUpdated {
        state: LauncherState,
    },
    ProviderCompleted {
        session_id: SessionId,
        revision: Revision,
        provider_id: ProviderId,
    },
    ProviderFailed {
        session_id: SessionId,
        revision: Revision,
        provider_id: ProviderId,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identifiers_and_custom_context_survive_serialization() {
        assert_eq!(
            serde_json::to_string(&ContextSource::UserInput).unwrap(),
            "\"user_input\""
        );
        let descriptor = ContextDescriptor {
            id: "1:2:3".into(),
            type_id: "github.issue".into(),
            value: "42".into(),
            properties: BTreeMap::new(),
            source: ContextSource::ApplicationContext,
            provider_id: "example.issues".into(),
            source_context_id: Some("1:2:1".into()),
            confidence: 0.9,
            depth: 2,
            session_id: 1,
            revision: 2,
        };
        let json = serde_json::to_string(&descriptor).unwrap();
        assert_eq!(descriptor, serde_json::from_str(&json).unwrap());
        let state = LauncherState {
            session_id: 1,
            ..Default::default()
        };
        assert_eq!(
            state,
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap()
        );
    }
}
