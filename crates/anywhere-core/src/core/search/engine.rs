//! The search pipeline: gather from every candidate source, order, and group into scopes.
//!
//! It knows nothing about what a source contains. Applications, actions and text fallbacks
//! are all `CandidateSource`s, so adding a new kind of result — files, clipboard entries,
//! history — is a new source and a registration, not a branch here.
use super::candidate::{compare_candidates, deduplicate_actions, CandidateSet};
use super::candidate_source::{CandidateSource, SearchContext, SourceStage};
use super::fallback::TextFallbackSource;
use super::query::Query;
use super::scope;
use crate::core::action::descriptor::ActionRegistry;
use crate::core::action::matcher::ActionMatcher;
use crate::core::context::{ContextSource, ResolvedContext};
use crate::core::ranking::score::UsageStats;
use crate::core::session::facts::{context_ownership, Ownership, SessionFacts};
use std::time::Instant;

pub struct SearchEngine<'a> {
    /// `(action_id, context_type)` usage, scoped to the current foreground app.
    pub usage: &'a dyn Fn(&str, &str) -> UsageStats,
    pub facts: &'a SessionFacts,
}

/// Action matching plus text fallback, the two sources every search always has.
///
/// Extra sources (application index, later files) are passed into [`search`](Self::search).
/// The first ranked source is timed as `matcher_us`; the rest as `sources_us`.
pub struct ActionSearch<'a> {
    engine: SearchEngine<'a>,
    matcher: ActionMatcher<'a>,
    fallback: TextFallbackSource<'a>,
}

impl<'a> ActionSearch<'a> {
    pub fn new(
        registry: &'a ActionRegistry,
        usage: &'a dyn Fn(&str, &str) -> UsageStats,
        facts: &'a SessionFacts,
        now: i64,
    ) -> Self {
        Self {
            engine: SearchEngine { usage, facts },
            matcher: ActionMatcher {
                registry,
                usage,
                facts,
                now,
            },
            fallback: TextFallbackSource { registry, facts },
        }
    }

    pub fn search(
        &self,
        query: &Query,
        contexts: &[ResolvedContext],
        extra: &[&dyn CandidateSource],
    ) -> CandidateSet {
        let mut sources: Vec<&dyn CandidateSource> = Vec::with_capacity(extra.len() + 2);
        sources.push(&self.matcher);
        sources.extend_from_slice(extra);
        sources.push(&self.fallback);
        self.engine.search(query, contexts, &sources)
    }
}

impl SearchEngine<'_> {
    /// Gather, rank and group. Callers decide which sources run; this does not construct any.
    pub fn search(
        &self,
        query: &Query,
        contexts: &[ResolvedContext],
        sources: &[&dyn CandidateSource],
    ) -> CandidateSet {
        let search = SearchContext { query, contexts };
        let mut ranked = Vec::new();
        let mut fallback = Vec::new();
        for source in sources {
            match source.stage() {
                SourceStage::Ranked => ranked.push(*source),
                SourceStage::Fallback => fallback.push(*source),
            }
        }

        // Matching. The first ranked source (actions, by convention) and the rest are timed
        // apart: folding application matching into `ranking_us` once hid a 1.2s regression
        // behind a metric that was supposed to describe sorting.
        let mut matcher_us = 0;
        let mut sources_us = 0;
        let mut candidates = Vec::new();
        for (index, source) in ranked.iter().enumerate() {
            let started = Instant::now();
            let batch = source.contribute(&search, &[]);
            let elapsed = started.elapsed().as_micros() as u64;
            if index == 0 {
                matcher_us = elapsed;
            } else {
                sources_us += elapsed;
            }
            candidates.extend(batch);
        }

        let ranking_started = Instant::now();
        candidates.sort_by(compare_candidates);
        deduplicate_actions(&mut candidates);
        for source in fallback {
            candidates.extend(source.contribute(&search, &candidates));
        }

        // Scope membership is built from every visible candidate, before any view limit, so
        // a group never looks empty just because its rows fall outside the top seven.
        let scope_use = scope::usage_by_group(&candidates, self.usage);
        let current_app = self.ambient_owner(contexts);
        let scopes = scope::build(
            &candidates,
            &current_app,
            &self.app_group_label(),
            &scope_use,
        );
        let ranking_us = ranking_started.elapsed().as_micros() as u64;

        CandidateSet {
            candidates,
            scopes,
            scope_use,
            matcher_us,
            sources_us,
            ranking_us,
        }
    }

    /// Which application the Current App scope stands for: the ambient owner, taken from
    /// the ambient contexts only.
    fn ambient_owner(&self, contexts: &[ResolvedContext]) -> Option<String> {
        contexts
            .iter()
            .filter(|context| context.source != ContextSource::UserInput)
            .find_map(|context| match context_ownership(context, self.facts) {
                Ownership::App(app) => Some(app),
                Ownership::Ambient => None,
            })
    }

    fn app_group_label(&self) -> String {
        self.facts
            .frontmost
            .as_ref()
            .map(|app| app.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Current App".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::action::availability::AvailabilityRule;
    use crate::core::action::bundle::ActionSpec;
    use crate::core::action::descriptor::ActionRegistry;
    use crate::core::action::handler::{ActionHandler, ExecutionOutput};
    use crate::core::context::{ContextValue, ResolvedContext};
    use crate::core::ranking::score::{UsageStats, FALLBACK_CEILING};
    use crate::core::search::candidate::action_candidate_id;
    use crate::core::session::facts::{FactValue, SessionFacts, CLIPBOARD, GIT};
    use crate::features::applications::index::IndexedApplication;
    use crate::features::applications::source::application_candidate_id;
    use crate::features::applications::source::ApplicationSource;
    use crate::features::recognized_contexts;
    use anywhere_platform::ApplicationInfo;
    use anywhere_protocol::*;

    fn contexts(input: &str) -> Vec<ResolvedContext> {
        recognized_contexts(input)
    }

    fn facts() -> SessionFacts {
        SessionFacts::default().with(GIT, FactValue::Flag(true))
    }

    fn no_usage(_: &str, _: &str) -> UsageStats {
        UsageStats::default()
    }

    struct Noop;
    impl ActionHandler for Noop {
        fn execute(
            &self,
            _: &ResolvedContext,
            _: &dyn anywhere_platform::Platform,
        ) -> anyhow::Result<ExecutionOutput> {
            Ok(ExecutionOutput::dismiss())
        }
    }

    fn text_action(
        id: &'static str,
        title: &'static str,
        availability: AvailabilityRule,
    ) -> ActionSpec {
        ActionSpec {
            id,
            title,
            accepts: TEXT,
            handler: &Noop,
            availability,
            base_priority: 0,
            icon: "copy",
            aliases: &[],
            keywords: &[],
        }
    }

    fn registry_with_text_actions(actions: Vec<ActionSpec>) -> ActionRegistry {
        let mut registry = crate::application::composition::builtin_registry();
        for spec in actions {
            registry.register(spec.descriptor("test")).unwrap();
        }
        registry
            .register_plugin(
                anywhere_plugin::parse_manifest(anywhere_plugin::EXAMPLE_PLUGIN).unwrap(),
            )
            .unwrap();
        registry
    }

    fn availability_of<'a>(
        set: &'a CandidateSet,
        command: &str,
    ) -> Option<&'a anywhere_protocol::Availability> {
        set.candidates
            .iter()
            .find(|candidate| candidate.presentation.command_id.as_deref() == Some(command))
            .map(|candidate| &candidate.presentation.availability)
    }

    fn builtin_engine<'a>(
        registry: &'a ActionRegistry,
        usage: &'a dyn Fn(&str, &str) -> UsageStats,
        facts: &'a SessionFacts,
    ) -> ActionSearch<'a> {
        ActionSearch::new(registry, usage, facts, 1000)
    }

    /// The application index as a candidate source, for the tests that search it.
    fn application_sources<'a>(
        applications: &'a [IndexedApplication],
        usage: &'a dyn Fn(&str, &str) -> UsageStats,
    ) -> Vec<&'a dyn crate::core::search::candidate_source::CandidateSource> {
        // Leaked deliberately: a test-local source only has to outlive the call.
        vec![Box::leak(Box::new(ApplicationSource {
            applications,
            app_usage: usage,
            now: 1000,
        }))]
    }

    #[test]
    fn directory_context_orders_the_documented_examples() {
        let registry = crate::application::composition::builtin_registry();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap();
        std::fs::create_dir(path.join(".git")).unwrap();
        let mut contexts = vec![ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory(path.clone()),
            ContextSource::ApplicationContext,
            "finder",
        )];
        contexts.push(ResolvedContext::new(
            GIT_REPOSITORY,
            ContextValue::Repository(path.clone()),
            ContextSource::ApplicationContext,
            "git",
        ));
        let usage = no_usage;
        let facts = SessionFacts::default().with(GIT, FactValue::Flag(true));
        let engine = builtin_engine(&registry, &usage, &facts);
        let set = engine.search(&Query::parse(""), &contexts, &[]);
        let titles = set
            .candidates
            .iter()
            .map(|c| c.presentation.title.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            &titles[..3],
            ["Git Status", "Open Terminal Here", "Git Log"],
            "git directory example"
        );

        contexts.retain(|context| context.type_id != GIT_REPOSITORY);
        let set = engine.search(&Query::parse(""), &contexts, &[]);
        let titles = set
            .candidates
            .iter()
            .map(|c| c.presentation.title.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            &titles[..3],
            ["Open Terminal Here", "Copy Path", "Open Directory"]
        );
    }

    /// Home is a view limit, not a reason to drop candidates.
    ///
    /// Truncating the set left scope id lists referring to entries that no longer
    /// existed, so a populated group could render empty.
    #[test]
    fn every_scope_id_resolves_to_a_candidate() {
        let registry = crate::application::composition::builtin_registry();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap();
        let mut contexts = vec![
            ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory(path.clone()),
                ContextSource::ApplicationContext,
                "finder",
            ),
            ResolvedContext::new(
                GIT_REPOSITORY,
                ContextValue::Repository(path),
                ContextSource::ApplicationContext,
                "git",
            ),
        ];
        // Several distinct directories, so the set comfortably exceeds the Home limit.
        let extra = tempfile::tempdir().unwrap();
        for index in 0..4 {
            let nested = extra.path().join(format!("d{index}"));
            std::fs::create_dir_all(&nested).unwrap();
            contexts.push(ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory(nested.canonicalize().unwrap()),
                ContextSource::ApplicationContext,
                "finder",
            ));
        }
        let usage = no_usage;
        let facts = facts();
        let engine = builtin_engine(&registry, &usage, &facts);
        let set = engine.search(&Query::parse(""), &contexts, &[]);
        assert!(
            set.candidates.len() > crate::core::search::scope::HOME_LIMIT,
            "this fixture must exceed the Home limit to be meaningful: {} candidates",
            set.candidates.len()
        );
        for scope in &set.scopes {
            for id in &scope.candidate_ids {
                assert!(
                    set.candidates
                        .iter()
                        .any(|candidate| &candidate.presentation.id == id),
                    "scope {} refers to a dropped candidate {id}",
                    scope.id
                );
            }
        }
    }

    #[test]
    fn home_is_bounded_while_search_stays_scrollable() {
        let registry = crate::application::composition::builtin_registry();
        let mut contexts = contexts(&"a".repeat(60));
        contexts.push(ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory("/tmp".into()),
            ContextSource::ApplicationContext,
            "finder",
        ));
        let usage = no_usage;
        let facts = facts();
        let engine = builtin_engine(&registry, &usage, &facts);
        // Home keeps every candidate; the host limits what it displays.
        let home = engine.search(&Query::parse(""), &contexts, &[]);
        assert!(!home.candidates.is_empty());
        let search = engine.search(&Query::parse("copy"), &contexts, &[]);
        // Search results stay scrollable; only Home is capped at seven rows.
        assert!(!search.candidates.is_empty(), "{:?}", search.candidates);
        assert!(search.candidates.iter().all(|c| c
            .presentation
            .title
            .to_lowercase()
            .contains("copy")
            || c.presentation.title == "Web Search"));
    }

    #[test]
    fn groups_are_visible_before_scope_filtering_and_home_truncation() {
        let registry = crate::application::composition::builtin_registry();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap();
        let contexts = vec![
            ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory(path.clone()),
                ContextSource::ApplicationContext,
                "finder",
            ),
            ResolvedContext::new(
                GIT_REPOSITORY,
                ContextValue::Repository(path),
                ContextSource::ApplicationContext,
                "git",
            ),
        ];
        let usage = no_usage;
        let facts = facts();
        let engine = builtin_engine(&registry, &usage, &facts);
        let set = engine.search(&Query::parse(""), &contexts, &[]);
        let scope_ids = set.scopes.iter().map(|s| s.id.as_str()).collect::<Vec<_>>();
        assert!(!scope_ids.contains(&"all"));
        assert!(!scope_ids.contains(&"app"));
        assert!(scope_ids.contains(&"git"));
        assert!(scope_ids.contains(&"folder"));
    }

    /// The scope bar leads with what the user actually uses, not a fixed order.
    #[test]
    fn scopes_are_ordered_by_usage_with_a_deterministic_tie_break() {
        let registry = crate::application::composition::builtin_registry();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap();
        let contexts = vec![
            ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory(path.clone()),
                ContextSource::ApplicationContext,
                "finder",
            ),
            ResolvedContext::new(
                GIT_REPOSITORY,
                ContextValue::Repository(path),
                ContextSource::ApplicationContext,
                "git",
            ),
        ];
        let facts = facts();

        // Heavy use of the folder actions; the Git group has never been used.
        let folder_heavy = |action: &str, kind: &str| {
            if kind == DIRECTORY {
                UsageStats {
                    execute_count: 8,
                    last_executed_at: 1000,
                }
            } else {
                let _ = action;
                UsageStats::default()
            }
        };
        let engine = builtin_engine(&registry, &folder_heavy, &facts);
        let set = engine.search(&Query::parse(""), &contexts, &[]);
        let ids = set
            .scopes
            .iter()
            .map(|scope| scope.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["folder", "git"], "folder is used, git is not");
        assert_eq!(
            set.scope_use.get("folder"),
            Some(&24),
            "8 uses of 3 folder actions"
        );

        // With the same usage everywhere, the documented order decides.
        let none_used = |_: &str, _: &str| UsageStats::default();
        let engine = builtin_engine(&registry, &none_used, &facts);
        let set = engine.search(&Query::parse(""), &contexts, &[]);
        let ids = set
            .scopes
            .iter()
            .map(|scope| scope.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            ["git", "folder"],
            "fresh install keeps the built-in group order"
        );

        // Using only the Git actions flips the two.
        let git_heavy = |_: &str, kind: &str| {
            if kind == GIT_REPOSITORY {
                UsageStats {
                    execute_count: 9,
                    last_executed_at: 1000,
                }
            } else {
                UsageStats::default()
            }
        };
        let engine = builtin_engine(&registry, &git_heavy, &facts);
        let set = engine.search(&Query::parse(""), &contexts, &[]);
        let ids = set
            .scopes
            .iter()
            .map(|scope| scope.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["git", "folder"], "git use puts git first");
        assert!(set.scope_use.get("git").copied().unwrap_or_default() > 0);
    }

    #[test]
    fn copy_path_is_hidden_when_the_clipboard_already_holds_it() {
        let registry = crate::application::composition::builtin_registry();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap();
        let contexts = vec![ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory(path.clone()),
            ContextSource::ApplicationContext,
            "finder",
        )];
        let usage = no_usage;
        let mut session = facts();
        let engine = builtin_engine(&registry, &usage, &session);
        assert!(engine
            .search(&Query::parse(""), &contexts, &[])
            .candidates
            .iter()
            .any(|c| c.presentation.title == "Copy Path"));

        session
            .insert(
                CLIPBOARD,
                FactValue::Text(path.to_string_lossy().into_owned()),
            )
            .unwrap();
        let engine = builtin_engine(&registry, &usage, &session);
        assert!(!engine
            .search(&Query::parse(""), &contexts, &[])
            .candidates
            .iter()
            .any(|c| c.presentation.title == "Copy Path"));
    }

    #[test]
    fn git_actions_are_disabled_rather_than_hidden_without_git() {
        let registry = crate::application::composition::builtin_registry();
        let contexts = vec![ResolvedContext::new(
            GIT_REPOSITORY,
            ContextValue::Repository("/tmp".into()),
            ContextSource::ApplicationContext,
            "git",
        )];
        let usage = no_usage;
        let facts = SessionFacts::default();
        let engine = builtin_engine(&registry, &usage, &facts);
        let set = engine.search(&Query::parse(""), &contexts, &[]);
        let status = set
            .candidates
            .iter()
            .find(|c| c.presentation.title == "Git Status")
            .expect("git status stays visible so the group is discoverable");
        assert_eq!(
            status.presentation.availability.state,
            AvailabilityState::Disabled
        );
        assert_eq!(
            status.presentation.availability.reason.as_deref(),
            Some("无法确认 Git 是否可用")
        );

        let facts = SessionFacts::default().with(GIT, FactValue::Flag(false));
        let engine = builtin_engine(&registry, &usage, &facts);
        let set = engine.search(&Query::parse(""), &contexts, &[]);
        let status = set
            .candidates
            .iter()
            .find(|c| c.presentation.title == "Git Status")
            .unwrap();
        assert_eq!(
            status.presentation.availability.reason.as_deref(),
            Some("Git 不可用")
        );
    }

    #[test]
    fn application_search_matches_names_aliases_and_keywords() {
        let registry = crate::application::composition::builtin_registry();
        let applications = vec![IndexedApplication {
            bundle_id: "com.google.Chrome".into(),
            name: "Google Chrome".into(),
            path: "/Applications/Google Chrome.app".into(),
            aliases: vec!["chrome".into()],
            keywords: vec!["browser".into()],
        }];
        let chrome_contexts = contexts("chrome");
        let usage = no_usage;
        let facts = facts();
        let engine = builtin_engine(&registry, &usage, &facts);
        let sources = application_sources(&applications, &usage);
        let set = engine.search(&Query::parse("chrome"), &chrome_contexts, &sources);
        assert_eq!(set.candidates[0].presentation.title, "Google Chrome");
        assert_eq!(
            set.candidates[0].presentation.kind,
            CandidateKind::Application
        );
        let by_keyword = engine.search(
            &Query::parse("browser"),
            &self::contexts("browser"),
            &sources,
        );
        assert_eq!(by_keyword.candidates[0].presentation.title, "Google Chrome");
        // Applications never appear in Context Home.
        let home = engine.search(&Query::parse(""), &chrome_contexts, &sources);
        assert!(home
            .candidates
            .iter()
            .all(|c| c.presentation.kind != CandidateKind::Application));
    }

    #[test]
    fn action_search_finds_git_status_by_partial_words() {
        let registry = crate::application::composition::builtin_registry();
        let contexts = vec![
            ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory("/tmp".into()),
                ContextSource::ApplicationContext,
                "finder",
            ),
            ResolvedContext::new(
                GIT_REPOSITORY,
                ContextValue::Repository("/tmp".into()),
                ContextSource::ApplicationContext,
                "git",
            ),
        ];
        let usage = no_usage;
        let facts = facts();
        let engine = builtin_engine(&registry, &usage, &facts);
        let set = engine.search(&Query::parse("git sta"), &contexts, &[]);
        assert_eq!(set.candidates[0].presentation.title, "Git Status");
    }

    #[test]
    fn typed_object_actions_survive_a_query_without_a_title_match() {
        let registry = crate::application::composition::builtin_registry();
        let url_contexts = contexts("https://github.com");
        let usage = no_usage;
        let facts = facts();
        let engine = builtin_engine(&registry, &usage, &facts);
        let set = engine.search(&Query::parse("https://github.com"), &url_contexts, &[]);
        let titles = set
            .candidates
            .iter()
            .map(|c| c.presentation.title.as_str())
            .collect::<Vec<_>>();
        assert!(titles.contains(&"Open in Default Browser"), "{titles:?}");
        assert!(titles.contains(&"Copy URL"), "{titles:?}");
        // No text fallback ships with the host: a built-in web search could only
        // hardcode one provider, so it is a plugin decision instead.
        assert!(
            !titles.iter().any(|title| title.contains("Web Search")),
            "the host must not ship a hardcoded web search: {titles:?}"
        );

        // An unrelated sentence must not surface URL actions for a URL context.
        let unrelated = contexts("hello world");
        let set = engine.search(&Query::parse("hello"), &unrelated, &[]);
        assert!(set
            .candidates
            .iter()
            .all(|c| c.presentation.title != "Open in Default Browser"));
    }

    #[test]
    fn candidate_ids_are_stable_business_identity() {
        let context = ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory("/tmp".into()),
            ContextSource::ApplicationContext,
            "finder",
        );
        let id = action_candidate_id("core.directory.copy", &context);
        assert_eq!(id, "action:core.directory.copy:core.directory:/tmp");
        assert_eq!(
            application_candidate_id("com.google.Chrome", "/Applications/Chrome.app"),
            "app:com.google.Chrome:/Applications/Chrome.app"
        );
    }

    #[test]
    fn current_app_scope_uses_ambient_provenance_and_excludes_launch_candidates() {
        let registry = crate::application::composition::builtin_registry();
        let contexts = vec![ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory("/tmp".into()),
            ContextSource::ApplicationContext,
            "finder",
        )];
        let usage = no_usage;
        let mut facts = facts();
        facts.frontmost = Some(ApplicationInfo {
            bundle_id: "com.apple.finder".into(),
            name: "Finder".into(),
        });
        let engine = builtin_engine(&registry, &usage, &facts);
        let set = engine.search(&Query::parse(""), &contexts, &[]);
        let app_scope = set.scopes.iter().find(|s| s.id == "app").unwrap();
        assert_eq!(set.scopes[0].id, "app");
        assert_eq!(app_scope.label, "Finder");
        assert!(!app_scope.candidate_ids.is_empty());
        assert!(app_scope
            .candidate_ids
            .iter()
            .all(|id| !id.starts_with("app:")));
    }

    #[test]
    fn text_action_availability_matches_on_name_hit_and_fallback() {
        let registry = registry_with_text_actions(vec![
            text_action(
                "test.text.hide",
                "Hidden Echo",
                AvailabilityRule::hide_if(CLIPBOARD),
            ),
            text_action(
                "test.text.require",
                "Need Git Text",
                AvailabilityRule::require(GIT, "git is not installed"),
            ),
        ]);
        let usage = no_usage;
        let query_hit = "hidden echo";
        let query_miss = "zzzznotaname";

        let enabled = SessionFacts::default()
            .with(GIT, FactValue::Flag(true))
            .with(CLIPBOARD, FactValue::Text("unrelated".into()));
        let engine = builtin_engine(&registry, &usage, &enabled);
        for query in [query_hit, query_miss] {
            let set = engine.search(&Query::parse(query), &contexts(query), &[]);
            let hide = availability_of(&set, "test.text.hide").expect(query);
            let require = availability_of(&set, "test.text.require").expect(query);
            assert_eq!(hide.state, AvailabilityState::Enabled, "{query}");
            assert_eq!(require.state, AvailabilityState::Enabled, "{query}");
        }

        let hidden = SessionFacts::default()
            .with(GIT, FactValue::Flag(true))
            .with(CLIPBOARD, FactValue::Text(query_hit.into()));
        let engine = builtin_engine(&registry, &usage, &hidden);
        let matched = engine.search(&Query::parse(query_hit), &contexts(query_hit), &[]);
        assert!(availability_of(&matched, "test.text.hide").is_none());
        let fallback_hidden = SessionFacts::default()
            .with(GIT, FactValue::Flag(true))
            .with(CLIPBOARD, FactValue::Text(query_miss.into()));
        let engine = builtin_engine(&registry, &usage, &fallback_hidden);
        let fallback = engine.search(&Query::parse(query_miss), &contexts(query_miss), &[]);
        assert!(availability_of(&fallback, "test.text.hide").is_none());

        let disabled = SessionFacts::default().with(GIT, FactValue::Flag(false));
        let engine = builtin_engine(&registry, &usage, &disabled);
        for query in ["need git text", query_miss] {
            let set = engine.search(&Query::parse(query), &contexts(query), &[]);
            let require = availability_of(&set, "test.text.require").expect(query);
            assert_eq!(require.state, AvailabilityState::Disabled, "{query}");
            assert_eq!(require.reason.as_deref(), Some("git is not installed"));
        }
    }

    #[test]
    fn enabled_text_fallback_follows_ranked_matches_without_duplicates() {
        let registry = registry_with_text_actions(Vec::new());
        let usage = no_usage;
        let facts = facts();
        let engine = builtin_engine(&registry, &usage, &facts);
        let url = "https://github.com";
        let set = engine.search(&Query::parse(url), &contexts(url), &[]);
        let ids = set
            .candidates
            .iter()
            .filter_map(|candidate| candidate.presentation.command_id.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(
            ids.iter().filter(|id| **id == "example.search").count(),
            1,
            "{ids:?}"
        );
        let fallback_at = ids.iter().position(|id| *id == "example.search").unwrap();
        assert!(
            fallback_at > 0,
            "fallback must follow ranked matches: {ids:?}"
        );
        assert!(
            set.candidates[fallback_at].presentation.score <= FALLBACK_CEILING,
            "{}",
            set.candidates[fallback_at].presentation.score
        );

        let set = engine.search(&Query::parse("search"), &contexts("search"), &[]);
        let matches = set
            .candidates
            .iter()
            .filter(|candidate| {
                candidate.presentation.command_id.as_deref() == Some("example.search")
            })
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        assert!(matches[0].presentation.score > FALLBACK_CEILING);
    }
}
