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
/// Stable semantic identity of a context, independent of snapshot row numbering.
pub type ContextKey = String;
/// A process lifetime, independent of Launcher SessionId and query Revision.
pub type ExecutionSessionId = String;
pub const PROJECT_RUST: &str = "project.rust";
pub const PROJECT_MAVEN: &str = "project.maven";
pub const PROJECT_GRADLE: &str = "project.gradle";
pub const JAVA_SPRING_BOOT: &str = "java.spring_boot";
pub const PROJECT_NODE: &str = "project.node";

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

pub const TEXT: &str = "core.text";
pub const URL: &str = "core.url";
/// Canonical JSON context type introduced in V0.2.
pub const JSON_DOCUMENT: &str = "json.document";
/// Legacy V0.1 JSON context type. Normalized to `json.document` at the registry boundary.
pub const JSON_LEGACY: &str = "core.json";
pub const FILE: &str = "core.file";
pub const FILE_SELECTION: &str = "core.file_selection";
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
    FinderSelection,
    SelectedText,
    BrowserCurrentPage,
    Clipboard,
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
    pub key: ContextKey,
    pub root_key: ContextKey,
    pub icon_key: String,
    pub capabilities: Vec<String>,
    pub candidate_ids: Vec<CandidateId>,
    pub primary_candidate_id: Option<CandidateId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCaptureStatus {
    Captured,
    Empty,
    PermissionDenied,
    Unsupported,
    ApplicationChanged,
    TimedOut,
    TooLarge,
    TooManyItems,
    Unavailable,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCapturePresentation {
    pub source: ContextSource,
    pub status: ContextCaptureStatus,
    pub captured_at_ms: i64,
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
    #[serde(default)]
    /// Display-only shortcut reported by the source application; never a user binding.
    pub shortcut: Option<String>,
    pub icon: IconDescriptor,
    pub score: f32,
    pub group_id: GroupId,
    pub availability: Availability,
    /// Action command id; `None` for an application launch candidate.
    pub command_id: Option<ActionId>,
    /// Bundle identity of an application launch candidate.
    pub bundle_id: Option<String>,
    /// App provenance for an action; generic actions can also appear in Current App.
    pub owning_app: Option<String>,
    /// Menu ancestors. Empty when the title is the whole label.
    #[serde(default)]
    pub menu_path: Vec<String>,
    /// Context roots that expose this offer. Canonical duplicate targets union membership.
    #[serde(default)]
    pub context_keys: Vec<ContextKey>,
    /// Actual graph node used for execution; may be a capability derived from the root.
    pub execution_context_id: Option<ContextId>,
    pub execution_target_label: Option<String>,
    /// Whether this candidate may be recommended directly on an empty-query home.
    #[serde(default = "default_true")]
    pub home_eligible: bool,
    /// The default operation when Enter is pressed on its context.
    #[serde(default)]
    pub primary_action: bool,
    /// Core-provided visual section within the context action panel.
    #[serde(default)]
    pub section_label: Option<String>,
}

fn default_true() -> bool {
    true
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

/// Query boundary. The current application's entry includes all actions;
/// other entries filter a domain group independently of Context types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    #[default]
    Universal,
    Group(GroupId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeEntry {
    pub id: GroupId,
    pub label: String,
    pub is_current_app: bool,
}

impl ScopeEntry {
    pub fn contains(&self, candidate: &CandidatePresentation) -> bool {
        candidate.kind == CandidateKind::Action
            && candidate.availability.is_visible()
            && (self.is_current_app || candidate.group_id == self.id)
    }
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
    #[serde(default)]
    pub context_candidates: Vec<CandidatePresentation>,
    pub preferred_context_key: Option<ContextKey>,
    #[serde(default)]
    pub scopes: Vec<ScopeEntry>,
    pub selected_scope: Option<GroupId>,
    #[serde(default)]
    pub context_captures: Vec<ContextCapturePresentation>,
    pub candidates: Vec<CandidatePresentation>,
    pub diagnostics: Vec<String>,
}

impl LauncherState {
    pub fn candidate(&self, id: &str) -> Option<&CandidatePresentation> {
        self.candidates
            .iter()
            .chain(&self.context_candidates)
            .find(|candidate| candidate.id == id)
    }
    /// Missing groups remain empty; they never widen to Universal.
    pub fn scoped_candidates(&self, scope: &Scope) -> Vec<&CandidatePresentation> {
        match scope {
            Scope::Universal => self
                .candidates
                .iter()
                .filter(|candidate| candidate.availability.is_visible())
                .collect(),
            Scope::Group(key) => match self.scopes.iter().find(|entry| entry.id == *key) {
                Some(entry) => self
                    .context_candidates
                    .iter()
                    .filter(|candidate| entry.contains(candidate))
                    .collect(),
                None => Vec::new(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Succeeded,
    Failed,
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
    fn capture_status_round_trips_and_old_states_default_to_empty() {
        for status in [
            ContextCaptureStatus::Unsupported,
            ContextCaptureStatus::ApplicationChanged,
            ContextCaptureStatus::TimedOut,
            ContextCaptureStatus::TooLarge,
            ContextCaptureStatus::TooManyItems,
        ] {
            let encoded = serde_json::to_value(status).unwrap();
            assert_eq!(
                serde_json::from_value::<ContextCaptureStatus>(encoded).unwrap(),
                status
            );
        }
        let state = LauncherState {
            context_captures: vec![ContextCapturePresentation {
                source: ContextSource::SelectedText,
                status: ContextCaptureStatus::PermissionDenied,
                captured_at_ms: 42,
            }],
            ..Default::default()
        };
        let encoded = serde_json::to_value(&state).unwrap();
        assert_eq!(
            encoded["context_captures"][0]["status"],
            "permission_denied"
        );
        assert_eq!(
            serde_json::from_value::<LauncherState>(encoded.clone()).unwrap(),
            state
        );
        let mut previous = encoded;
        previous.as_object_mut().unwrap().remove("context_captures");
        assert!(serde_json::from_value::<LauncherState>(previous)
            .unwrap()
            .context_captures
            .is_empty());
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
    fn scope_lookup_hides_unavailable_and_missing_context_does_not_widen() {
        let state = LauncherState {
            session_id: 1,
            candidates: vec![
                CandidatePresentation {
                    id: "action:core.directory.copy:c1".into(),
                    kind: CandidateKind::Action,
                    title: "Copy Path".into(),
                    subtitle: None,
                    shortcut: None,
                    icon: IconDescriptor { key: "copy".into() },
                    score: 1.0,
                    group_id: "folder".into(),
                    availability: Availability::hidden(),
                    command_id: Some("core.directory.copy".into()),
                    bundle_id: None,
                    owning_app: None,
                    menu_path: Vec::new(),
                    context_keys: Vec::new(),
                    execution_context_id: None,
                    execution_target_label: None,
                    home_eligible: true,
                    primary_action: false,
                    section_label: None,
                },
                CandidatePresentation {
                    id: "app:com.google.Chrome".into(),
                    kind: CandidateKind::Application,
                    title: "Google Chrome".into(),
                    subtitle: None,
                    shortcut: None,
                    icon: IconDescriptor {
                        key: "application".into(),
                    },
                    score: 2.0,
                    group_id: "app".into(),
                    availability: Availability::enabled(),
                    command_id: None,
                    bundle_id: Some("com.google.Chrome".into()),
                    owning_app: None,
                    menu_path: Vec::new(),
                    context_keys: Vec::new(),
                    execution_context_id: None,
                    execution_target_label: None,
                    home_eligible: true,
                    primary_action: true,
                    section_label: None,
                },
            ],
            ..Default::default()
        };
        let visible = state.scoped_candidates(&Scope::Universal);
        assert_eq!(visible.len(), 1);
        assert!(state
            .scoped_candidates(&Scope::Group("missing".into()))
            .is_empty());
        assert_eq!(visible[0].id, "app:com.google.Chrome");
        assert!(state.candidate("app:com.google.Chrome").is_some());
        assert!(state.candidate("missing").is_none());
    }

    #[test]
    fn current_app_membership_includes_domains_and_disabled_actions_only() {
        let hidden = CandidatePresentation {
            id: "hidden".into(),
            kind: CandidateKind::Action,
            title: "Copy Path".into(),
            subtitle: None,
            shortcut: None,
            icon: IconDescriptor { key: "copy".into() },
            score: 1.0,
            group_id: "folder".into(),
            availability: Availability::hidden(),
            command_id: Some("core.directory.copy".into()),
            bundle_id: None,
            owning_app: None,
            menu_path: Vec::new(),
            context_keys: Vec::new(),
            execution_context_id: None,
            execution_target_label: None,
            home_eligible: true,
            primary_action: false,
            section_label: None,
        };
        let mut folder = hidden.clone();
        folder.id = "folder-action".into();
        folder.availability = Availability::enabled();
        let mut git = folder.clone();
        git.id = "git-action".into();
        git.group_id = "git".into();
        git.availability = Availability::disabled("Git unavailable");
        let mut application = folder.clone();
        application.id = "application".into();
        application.kind = CandidateKind::Application;
        let state = LauncherState {
            context_candidates: vec![hidden, folder, git, application],
            scopes: vec![
                ScopeEntry {
                    id: "app.finder".into(),
                    label: "Finder".into(),
                    is_current_app: true,
                },
                ScopeEntry {
                    id: "folder".into(),
                    label: "Folder".into(),
                    is_current_app: false,
                },
            ],
            ..Default::default()
        };
        let ids = |scope: &str| {
            state
                .scoped_candidates(&Scope::Group(scope.into()))
                .iter()
                .map(|action| action.id.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids("app.finder"), ["folder-action", "git-action"]);
        assert_eq!(ids("folder"), ["folder-action"]);
        assert!(ids("app.other").is_empty());
    }
}
