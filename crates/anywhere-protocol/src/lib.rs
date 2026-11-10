//! Stable, serializable boundary types. No UI or OS dependencies.
mod candidate;
mod context;
mod declarative;
mod execution;
pub mod extension;
mod identifiers;
mod launcher;
pub mod policy;
pub mod ui;
pub mod web_bridge;
pub mod web_plugin;

pub use candidate::{
    Availability, AvailabilityState, CandidateKind, CandidatePresentation, FileObjectKind,
    IconDescriptor, Scope, ScopeEntry,
};
pub use context::{
    canonical_context_type, is_json_context, ContextCapturePresentation, ContextCaptureStatus,
    ContextDescriptor, ContextMatcher, ContextPresentation, ContextSource, InputMode, APPLICATION,
    DIRECTORY, FILE, FILE_SELECTION, GIT_REPOSITORY, JSON_DOCUMENT, JSON_LEGACY,
    LAUNCHER_ENTRY_CONTEXT, TEXT, URL,
};
pub use declarative::{DeclarativeAction, PluginCommand, PluginManifest};
pub use execution::{
    ActionResult, ActionStatus, ExecutionSessionDetail, ExecutionSessionStatus,
    ExecutionSessionSummary, HostView,
};
pub use identifiers::{
    ActionId, CandidateId, ContextId, ContextKey, ContextTypeId, ExecutionSessionId, GroupId,
    ProviderId, Revision, SessionId, JAVA_SPRING_BOOT, PROJECT_GRADLE, PROJECT_MAVEN, PROJECT_NODE,
    PROJECT_RUST,
};
pub use launcher::{
    FileSearchProgress, LauncherEvent, LauncherState, SearchSourcePresentation, SearchSourceState,
};

mod file_filter;
pub use file_filter::FileSearchFilter;

#[cfg(test)]
use std::collections::BTreeMap;
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
                text: None,
                clipboard_age_ms: None,
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
                    localized_titles: Default::default(),
                    id: "action:core.directory.copy:c1".into(),
                    kind: CandidateKind::Action,
                    is_menu: false,
                    file_kind: None,
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
                    primary_action: false,
                    section_label: None,
                },
                CandidatePresentation {
                    localized_titles: Default::default(),
                    id: "app:com.google.Chrome".into(),
                    kind: CandidateKind::Application,
                    is_menu: false,
                    file_kind: None,
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
                    primary_action: true,
                    section_label: None,
                },
            ]
            .into(),
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
            localized_titles: Default::default(),
            id: "hidden".into(),
            kind: CandidateKind::Action,
            is_menu: false,
            file_kind: None,
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
                    action_ids: Vec::new(),
                    menu_ids: Vec::new(),
                },
                ScopeEntry {
                    id: "folder".into(),
                    label: "Folder".into(),
                    is_current_app: false,
                    action_ids: Vec::new(),
                    menu_ids: Vec::new(),
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

    #[test]
    fn representative_json_keeps_tags_defaults_and_labels() {
        assert_eq!(
            serde_json::to_value(DeclarativeAction::OpenUrl {
                url: "https://example.com/search?q=anywhere".into(),
            })
            .unwrap(),
            serde_json::json!({
                "type": "open_url",
                "url": "https://example.com/search?q=anywhere",
            })
        );
        assert_eq!(
            serde_json::to_value(LauncherEvent::ProviderCompleted {
                session_id: 7,
                revision: 9,
                provider_id: "example.provider".into(),
                source_context_id: "context-1".into(),
                duration_us: 123,
            })
            .unwrap(),
            serde_json::json!({
                "type": "provider_completed",
                "session_id": 7,
                "revision": 9,
                "provider_id": "example.provider",
                "source_context_id": "context-1",
                "duration_us": 123,
            })
        );

        let candidate = CandidatePresentation {
            id: "candidate-1".into(),
            kind: CandidateKind::Action,
            is_menu: false,
            file_kind: None,
            title: "Open".into(),
            localized_titles: BTreeMap::from([("ja".into(), "開く".into())]),
            subtitle: None,
            shortcut: Some("⌘K".into()),
            icon: IconDescriptor { key: "open".into() },
            score: 1.5,
            group_id: "open".into(),
            availability: Availability::enabled(),
            command_id: Some("core.open".into()),
            bundle_id: None,
            owning_app: None,
            menu_path: Vec::new(),
            context_keys: Vec::new(),
            execution_context_id: None,
            execution_target_label: None,
            primary_action: false,
            section_label: Some("Actions".into()),
        };
        let encoded = serde_json::to_value(candidate).unwrap();
        assert_eq!(encoded["localized_titles"]["ja"], "開く");
        assert_eq!(encoded["shortcut"], "⌘K");
        assert_eq!(encoded["section_label"], "Actions");

        let old_state = serde_json::json!({
            "session_id": 7,
            "revision": 2,
            "input": "example",
            "context_summary": [],
            "preferred_context_key": null,
            "selected_scope": null,
            "candidates": [],
            "diagnostics": [],
        });
        let state: LauncherState = serde_json::from_value(old_state.clone()).unwrap();
        assert_eq!(state.snapshot_seq, 0);
        assert!(!state.context_complete);
        assert!(state.search_sources.is_empty());
        assert!(state.file_search.is_none());
        assert!(state.result_actions.is_empty());
        assert!(state.context_candidates.is_empty());
        assert!(state.scopes.is_empty());
        assert!(state.context_captures.is_empty());
        assert_eq!(state.candidate_list_version, 0);

        let mut future_state = old_state;
        future_state["future_optional_field"] = serde_json::json!(true);
        assert!(serde_json::from_value::<LauncherState>(future_state).is_ok());

        let manifest_with_unknown_field = r#"
protocol_version = 1
id = "legacy"
name = "Legacy"
future_required_field = true
commands = []
"#;
        assert!(toml::from_str::<PluginManifest>(manifest_with_unknown_field).is_err());
    }
}
