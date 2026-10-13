//! A searchable, executable entry into a Host-rendered file workspace.
use crate::core::{
    action::{
        availability::AvailabilityRule,
        bundle::{ActionBundle, ActionSpec},
        group::ActionGroup,
        handler::{ActionHandler, ExecutionOutput},
    },
    context::{ContextValue, ResolvedContext},
    search::{
        candidate::Candidate,
        candidate_source::{CandidateSource, SearchContext},
        engine::ActionSearch,
        query::Query,
    },
};
use anywhere_protocol::*;

pub const ENTRY_CONTEXT: &str = "core.launcher";
pub const ACTION_ID: &str = "core.files.search";
struct OpenSearch;
impl ActionHandler for OpenSearch {
    fn execute(
        &self,
        _: &ResolvedContext,
        _: &dyn anywhere_platform::Platform,
    ) -> anyhow::Result<ExecutionOutput> {
        Ok(ExecutionOutput::view(HostView::FileSearch))
    }
}
pub fn bundle() -> ActionBundle {
    ActionBundle {
        group: ActionGroup {
            id: crate::core::action::group::groups::TOOLS.into(),
            title: "Tools".into(),
            icon: Some("search".into()),
        },
        actions: vec![ActionSpec {
            id: ACTION_ID,
            title: "File Search",
            accepts: ENTRY_CONTEXT,
            handler: &OpenSearch,
            availability: AvailabilityRule::Enabled,
            base_priority: 30,
            icon: "search",
            aliases: &["files", "文件搜索", "搜索文件"],
            keywords: &[],
            home_eligible: false,
            primary_action: false,
        }],
    }
}

/// The Launcher capability is an execution target, not an ambient Context or Scope.
pub struct EntrySource<'a, 'b> {
    pub search: &'a ActionSearch<'b>,
}
impl EntrySource<'_, '_> {
    /// Shortcut execution must remain available independently of visible search.
    pub fn binding_offers(&self) -> Vec<Candidate> {
        self.offers(&Query::parse(""))
    }

    fn offers(&self, query: &Query) -> Vec<Candidate> {
        let context = ResolvedContext::new(
            ENTRY_CONTEXT,
            ContextValue::Text("".into()),
            ContextSource::ForegroundApplication,
            "core.launcher",
        );
        self.search
            .search(query, &[context], &[])
            .candidates
            .into_iter()
            .filter(|c| c.presentation.command_id.as_deref() == Some(ACTION_ID))
            .collect()
    }
}
impl CandidateSource for EntrySource<'_, '_> {
    fn id(&self) -> &'static str {
        "workspace-actions"
    }
    fn candidates(&self, request: &SearchContext<'_>) -> Vec<Candidate> {
        if request.scope_id.is_some() || request.query.is_empty() {
            return Vec::new();
        }
        self.offers(request.query)
    }
}
