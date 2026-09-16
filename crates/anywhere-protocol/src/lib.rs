//! Stable, serializable boundary types. No UI or OS dependencies.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type SessionId = u32;
pub type Revision = u32;
pub type ContextId = String;
pub type ContextTypeId = String;
pub type ProviderId = String;
/// Identity of one action, as the plugin manifest and the DTOs spell it.
pub type ActionId = String;
/// Identity of one row in the list: an action *or* an application, with its object.
pub type CandidateId = String;
pub type GroupId = String;
pub type ScopeId = String;

pub const TEXT: &str = "core.text";
pub const URL: &str = "core.url";
/// Canonical JSON context type introduced in V0.2.
pub const JSON_DOCUMENT: &str = "json.document";
/// Legacy V0.1 JSON context type. Normalized to `json.document` at the registry boundary.
pub const JSON_LEGACY: &str = "core.json";
pub const FILE: &str = "core.file";
pub const DIRECTORY: &str = "core.directory";
pub const APPLICATION: &str = "core.application";
pub const GIT_REPOSITORY: &str = "git.repository";

pub fn is_json_context(type_id: &str) -> bool {
    type_id == JSON_DOCUMENT || type_id == JSON_LEGACY
}

/// V0.1 type ids accepted from existing plugins, mapped to their V0.2 canonical name.
pub fn canonical_context_type(type_id: &str) -> &str {
    if type_id == JSON_LEGACY {
        JSON_DOCUMENT
    } else {
        type_id
    }
}

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
pub struct CandidatePresentation {
    pub id: CandidateId,
    pub kind: CandidateKind,
    pub title: String,
    pub subtitle: Option<String>,
    pub icon: IconDescriptor,
    pub score: f32,
    pub group_id: GroupId,
    pub availability: Availability,
    /// Action command id; `None` for an application launch candidate.
    pub command_id: Option<ActionId>,
    /// Bundle identity of an application launch candidate.
    pub bundle_id: Option<String>,
    /// App that owns an action exclusively (Current App scope); `None` for generic actions.
    pub owning_app: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateKind {
    Application,
    Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityState {
    Enabled,
    Disabled,
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Availability {
    pub state: AvailabilityState,
    pub reason: Option<String>,
}

impl Availability {
    pub fn enabled() -> Self {
        Self {
            state: AvailabilityState::Enabled,
            reason: None,
        }
    }
    pub fn hidden() -> Self {
        Self {
            state: AvailabilityState::Hidden,
            reason: None,
        }
    }
    pub fn disabled(reason: impl Into<String>) -> Self {
        Self {
            state: AvailabilityState::Disabled,
            reason: Some(reason.into()),
        }
    }
    pub fn is_visible(&self) -> bool {
        self.state != AvailabilityState::Hidden
    }
    pub fn is_executable(&self) -> bool {
        self.state == AvailabilityState::Enabled
    }
}

/// Scope bar entry. `All` is always first, `App` is always second on the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeEntry {
    pub id: ScopeId,
    pub kind: ScopeKind,
    pub label: String,
    /// Candidates of this scope in ranked order, after availability and filtering.
    pub candidate_ids: Vec<CandidateId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    All,
    App,
    Group,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LauncherState {
    pub session_id: SessionId,
    pub revision: Revision,
    /// Monotonic within a session/revision; a higher value always supersedes a lower one.
    #[serde(default)]
    pub snapshot_seq: u32,
    pub input: String,
    /// True once the context graph for this revision has no pending work.
    #[serde(default)]
    pub context_complete: bool,
    pub context_summary: Vec<ContextPresentation>,
    pub candidates: Vec<CandidatePresentation>,
    pub scopes: Vec<ScopeEntry>,
    pub diagnostics: Vec<String>,
}

impl LauncherState {
    pub fn candidate(&self, id: &str) -> Option<&CandidatePresentation> {
        self.candidates.iter().find(|candidate| candidate.id == id)
    }
    /// Candidate ids of a scope, or every visible candidate when the scope is unknown.
    pub fn scope_candidate_ids(&self, scope_id: &str) -> Vec<&CandidatePresentation> {
        let ids = self
            .scopes
            .iter()
            .find(|scope| scope.id == scope_id)
            .map(|scope| scope.candidate_ids.as_slice());
        match ids {
            Some(ids) => ids
                .iter()
                .filter_map(|id| self.candidate(id))
                .filter(|candidate| candidate.availability.is_visible())
                .collect(),
            None => self
                .candidates
                .iter()
                .filter(|candidate| candidate.availability.is_visible())
                .collect(),
        }
    }
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
    #[serde(default)]
    pub group_id: Option<GroupId>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub action: DeclarativeAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeclarativeAction {
    OpenUrl { url: String },
    OpenFile { path: String },
    Copy { text: String },
    ExecuteBuiltin { command_id: ActionId },
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
        source_context_id: ContextId,
        duration_us: u64,
    },
    ProviderFailed {
        session_id: SessionId,
        revision: Revision,
        provider_id: ProviderId,
        source_context_id: ContextId,
        duration_us: u64,
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

    #[test]
    fn legacy_json_type_normalizes_to_the_canonical_document_type() {
        assert_eq!(canonical_context_type(JSON_LEGACY), JSON_DOCUMENT);
        assert_eq!(canonical_context_type(JSON_DOCUMENT), JSON_DOCUMENT);
        assert_eq!(canonical_context_type(URL), URL);
        assert!(is_json_context(JSON_LEGACY) && is_json_context(JSON_DOCUMENT));
        assert!(!is_json_context(TEXT));
    }

    #[test]
    fn v0_1_plugin_manifest_still_parses_without_v0_2_metadata() {
        // Mirrors plugins/examples/google.toml: no group_id, aliases or keywords.
        let v0_1 = r#"
protocol_version = 1
id = "google"
name = "Google"

[[commands]]
id = "search"
title = "Search Google"
accepts = ["core.text"]
base_priority = 40
action = { type = "open_url", url = "https://www.google.com/search?q={{context.value}}" }
"#;
        let manifest: PluginManifest = toml::from_str(v0_1).unwrap();
        assert_eq!(manifest.commands[0].group_id, None);
        assert!(manifest.commands[0].aliases.is_empty());
        assert!(manifest.commands[0].keywords.is_empty());
    }

    #[test]
    fn scope_lookup_falls_back_to_every_visible_candidate() {
        let state = LauncherState {
            session_id: 1,
            candidates: vec![
                CandidatePresentation {
                    id: "action:core.directory.copy:c1".into(),
                    kind: CandidateKind::Action,
                    title: "Copy Path".into(),
                    subtitle: None,
                    icon: IconDescriptor { key: "copy".into() },
                    score: 1.0,
                    group_id: "folder".into(),
                    availability: Availability::hidden(),
                    command_id: Some("core.directory.copy".into()),
                    bundle_id: None,
                    owning_app: None,
                },
                CandidatePresentation {
                    id: "app:com.google.Chrome".into(),
                    kind: CandidateKind::Application,
                    title: "Google Chrome".into(),
                    subtitle: None,
                    icon: IconDescriptor {
                        key: "application".into(),
                    },
                    score: 2.0,
                    group_id: "app".into(),
                    availability: Availability::enabled(),
                    command_id: None,
                    bundle_id: Some("com.google.Chrome".into()),
                    owning_app: None,
                },
            ],
            ..Default::default()
        };
        let visible = state.scope_candidate_ids("unknown");
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "app:com.google.Chrome");
        assert!(state.candidate("app:com.google.Chrome").is_some());
        assert!(state.candidate("missing").is_none());
    }
}
