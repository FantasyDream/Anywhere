//! Action matching: which actions apply to the resolved contexts, and what they look like.
//!
//! Produces candidates with Context graph membership. Ordering belongs to ranking,
//! and visibility to the availability evaluator.
use super::availability;
use super::descriptor::{ActionDescriptor, ActionRegistry};
use super::group::{groups, GroupMeta};
use crate::core::context::{ContextSource, ContextValue, ResolvedContext};
use crate::core::ranking::score::{self, UsageStats};
use crate::core::search::candidate::{action_execution_candidate_id, Candidate};
use crate::core::search::candidate_source::{CandidateSource, SearchContext};
use crate::core::search::query::{MatchLevel, Query, SearchFields};
use crate::core::session::facts::{context_ownership, Ownership, SessionFacts};
use anywhere_protocol::*;

pub struct ActionMatcher<'a> {
    registry: &'a ActionRegistry,
    usage: &'a dyn Fn(&str, &str) -> UsageStats,
    facts: &'a SessionFacts,
    now: i64,
    semantic: fn(&str) -> i32,
}

impl<'a> ActionMatcher<'a> {
    pub fn new(
        registry: &'a ActionRegistry,
        usage: &'a dyn Fn(&str, &str) -> UsageStats,
        facts: &'a SessionFacts,
        now: i64,
        semantic: fn(&str) -> i32,
    ) -> Self {
        Self {
            registry,
            usage,
            facts,
            now,
            semantic,
        }
    }

    pub fn registry(&self) -> &'a ActionRegistry {
        self.registry
    }
}

/// Actions reach the search pipeline through the same contract as any other source; what is
/// core about them is only that the launcher always has them.
impl CandidateSource for ActionMatcher<'_> {
    fn id(&self) -> &'static str {
        "actions"
    }

    fn candidates(&self, search: &SearchContext<'_>) -> Vec<Candidate> {
        self.match_scoped_actions(search.query, search.contexts, search.scope_id)
    }
}

impl ActionMatcher<'_> {
    /// Every action the contexts make applicable, scored but not yet ordered.
    pub fn match_actions(&self, query: &Query, contexts: &[ResolvedContext]) -> Vec<Candidate> {
        self.match_scoped_actions(query, contexts, None)
    }

    fn match_scoped_actions(
        &self,
        query: &Query,
        contexts: &[ResolvedContext],
        scope_id: Option<&str>,
    ) -> Vec<Candidate> {
        let searching = !query.is_empty();
        let group_filter =
            crate::core::search::scope::group_filter(scope_id, self.facts.frontmost.as_ref());
        let grouped_finder_selection = contexts.iter().any(|context| {
            context.source == ContextSource::FinderSelection
                && matches!(&context.value, ContextValue::FileSelection(paths) if paths.len() > 1)
        });
        let mut candidates = Vec::new();
        for context in contexts {
            let text = match &context.value {
                ContextValue::Text(text) => text.as_ref(),
                _ => "",
            };
            let mut input_matches =
                crate::core::context::input_patterns::InputPatternMatches::new(text);
            // The collection is the user's object. Expanding selected children for
            // providers must not create an Open/Reveal row for every selected file.
            if grouped_finder_selection
                && context.source == ContextSource::FinderSelection
                && matches!(context.type_id.as_str(), FILE | DIRECTORY)
            {
                continue;
            }
            // Ownership is per context, never carried over: an input-derived object
            // must not inherit the ambient application just because an ambient context
            // was visited earlier in this loop.
            let owned_by = match context_ownership(context, self.facts) {
                Ownership::App(app) => Some(app),
                Ownership::Ambient => None,
            };
            for action in self.registry.matching(context) {
                let group_id = match &context.value {
                    ContextValue::Application { bundle_id, .. }
                        if action.group_id == groups::APP =>
                    {
                        groups::for_app(bundle_id)
                    }
                    _ => action.group_id.clone(),
                };
                // The boundary is applied before query matching, availability and scoring.
                if group_filter.is_some_and(|scope| scope != group_id) {
                    continue;
                }
                // Query text becomes a Web input only through an explicit bounded
                // matcher. Generic text consumers must not receive search keywords.
                let content_match = context.type_id == TEXT
                    && match &action.executor {
                        super::descriptor::ActionExecutor::WebCommand(command) => {
                            command.matches_input(&mut input_matches)
                        }
                        _ => false,
                    };
                if let super::descriptor::ActionExecutor::WebCommand(command) = &action.executor {
                    if !content_match
                        && (command.require_input_match
                            || (context.source == ContextSource::UserInput
                                && context.type_id == TEXT))
                    {
                        continue;
                    }
                }
                let action_query = if searching && scope_id.is_none() && action.owning_app.is_some()
                {
                    // A command such as Finder's "Find" must keep its direct title match
                    // even when the same token is also a prefix of the application name.
                    if SearchFields::new(&action.title, &[], &[])
                        .matched(query)
                        .is_some_and(|level| level >= MatchLevel::WordPrefix)
                    {
                        query.clone()
                    } else {
                        match qualified_app_query(query, &context.value) {
                            Some(query) => query,
                            None => continue,
                        }
                    }
                } else {
                    query.clone()
                };
                let match_level = action.search().matched(&action_query);
                if searching
                    && match_level.is_none()
                    && !availability::is_user_object(context)
                    && !(content_match && context.source == ContextSource::UserInput)
                {
                    continue;
                }
                // Optional content matchers narrow automatic suggestions. An explicit
                // keyword still opens the tool for editing invalid/incomplete input.
                if let super::descriptor::ActionExecutor::WebCommand(command) = &action.executor {
                    if context.type_id == TEXT
                        && !command.input_matchers.is_empty()
                        && !content_match
                        && match_level.is_none()
                    {
                        continue;
                    }
                }
                let availability =
                    availability::evaluate(&action.availability, context, self.facts);
                if !availability.is_visible() {
                    continue;
                }
                let stats = if action.executor.uses_usage() {
                    (self.usage)(&action.id, &context.type_id)
                } else {
                    Default::default()
                };
                let score = score::score(
                    match_level,
                    searching,
                    context.source,
                    (self.semantic)(&context.type_id),
                    action.base_priority,
                    stats,
                    self.now,
                );
                candidates.push(Candidate {
                    guard: None,
                    usage: stats,
                    presentation: CandidatePresentation {
                        localized_titles: action.localized_titles.clone(),
                        id: action_execution_candidate_id(action, context),
                        kind: CandidateKind::Action,
                        is_menu: action.executor.is_menu(),
                        file_kind: None,
                        title: action.title.clone(),
                        subtitle: Some(subtitle(action, context)),
                        shortcut: action.shortcut.clone(),
                        icon: IconDescriptor {
                            key: action.icon.clone(),
                        },
                        score,
                        group_id,
                        availability,
                        command_id: Some(action.id.clone()),
                        bundle_id: None,
                        owning_app: if context.type_id == LAUNCHER_ENTRY_CONTEXT {
                            action.owning_app.clone().or_else(|| owned_by.clone())
                        } else {
                            owned_by.clone().or_else(|| action.owning_app.clone())
                        },
                        menu_path: action.menu_path.clone(),
                        context_keys: vec![crate::core::context::navigation::root_key(
                            context, contexts,
                        )],
                        execution_context_id: Some(context.id.clone()),
                        execution_target_label: Some(context.display_detail()),
                        primary_action: action.primary_action,
                        section_label: Some(match &context.value {
                            ContextValue::Application { name, .. }
                                if action.owning_app.is_some() =>
                            {
                                name.clone()
                            }
                            _ => self.registry.label(&action.group_id),
                        }),
                    },
                    target: crate::core::search::candidate::CandidateTarget::from_action(
                        action.clone(),
                        context.clone(),
                    ),
                });
            }
        }
        candidates
    }
}

/// Home-only weighting: the ambient object leads, so its actions come first.
///
/// There is deliberately no per-group term here: Git actions only ever accept a
/// `git.repository` context, so a rule keyed on a directory could never apply.
fn subtitle(action: &ActionDescriptor, context: &ResolvedContext) -> String {
    if !action.menu_path.is_empty() {
        return action.menu_path.join(" › ");
    }
    let detail = context.display_detail();
    match &action.subtitle {
        Some(subtitle) => format!("{subtitle} · {detail}"),
        None => detail,
    }
}

/// Application qualifiers narrow a command query, but app-name-only searches
/// must not surface every menu of that application.
fn qualified_app_query(query: &Query, value: &ContextValue) -> Option<Query> {
    let ContextValue::Application {
        name, bundle_id, ..
    } = value
    else {
        return Some(query.clone());
    };
    let fields = SearchFields::new(
        name,
        &[],
        &[
            bundle_id.clone(),
            bundle_id.rsplit('.').next().unwrap_or(bundle_id).into(),
        ],
    );
    let mut qualified = false;
    let remaining: Vec<_> = query
        .tokens
        .iter()
        .filter(|token| {
            if matches!(
                fields.matched(&Query::parse(token)),
                Some(MatchLevel::Exact | MatchLevel::Prefix | MatchLevel::WordPrefix)
            ) {
                qualified = true;
                false
            } else {
                true
            }
        })
        .cloned()
        .collect();
    if !qualified {
        return Some(query.clone());
    }
    if remaining.is_empty() {
        None
    } else {
        Some(Query::parse(&remaining.join(" ")))
    }
}
