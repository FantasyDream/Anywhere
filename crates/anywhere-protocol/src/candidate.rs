use super::identifiers::{ActionId, CandidateId, ContextId, ContextKey, GroupId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IconDescriptor {
    /// Semantic icon key; the host maps it to a visual glyph.
    pub key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileObjectKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidatePresentation {
    pub id: CandidateId,
    pub kind: CandidateKind,
    #[serde(default)]
    pub is_menu: bool,
    #[serde(default)]
    pub file_kind: Option<FileObjectKind>,
    pub title: String,
    /// Optional source-supplied labels. The host chooses a language for display only.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub localized_titles: BTreeMap<String, String>,
    pub subtitle: Option<String>,
    #[serde(default)]
    /// Display-only shortcut reported by the source application; never a user binding.
    pub shortcut: Option<String>,
    pub icon: IconDescriptor,
    /// Relevance score. Empty action lists compare usage before this tie-breaker.
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
    /// The default operation when Enter is pressed on its context.
    #[serde(default)]
    pub primary_action: bool,
    /// Core-provided visual section within the context action panel.
    #[serde(default)]
    pub section_label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateKind {
    Application,
    Action,
    File,
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
    #[serde(default)]
    pub action_ids: Vec<CandidateId>,
    #[serde(default)]
    pub menu_ids: Vec<CandidateId>,
}

impl ScopeEntry {
    pub fn contains(&self, candidate: &CandidatePresentation) -> bool {
        candidate.kind == CandidateKind::Action
            && candidate.availability.is_visible()
            && (self.is_current_app || candidate.group_id == self.id)
    }
}
