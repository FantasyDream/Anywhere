//! The search pipeline: gather from every candidate source, order, and group into scopes.
//!
//! It knows nothing about what a source contains. Applications, actions and text fallbacks
//! are all `CandidateSource`s, so adding a new kind of result — files, clipboard entries,
//! history — is a new source and a registration, not a branch here.
use super::candidate::{compare_candidates, deduplicate_actions, CandidateSet};
use super::candidate_source::{CandidateSource, SearchContext};
use super::fallback::TextFallbackSource;
use super::query::Query;
use super::scope;
use crate::core::action::descriptor::CommandRegistry;
use crate::core::action::matcher::ActionMatcher;
use crate::core::context::{ContextSource, ResolvedContext};
use crate::core::ranking::score::UsageStats;
use crate::core::session::facts::{context_ownership, Ownership, SessionFacts};
use crate::features::applications::index::IndexedApplication;
use crate::features::applications::source::ApplicationSource;
use std::time::Instant;

pub struct SearchEngine<'a> {
    pub registry: &'a CommandRegistry,
    pub applications: &'a [IndexedApplication],
    /// `(action_id, context_type)` usage, scoped to the current foreground app.
    pub usage: &'a dyn Fn(&str, &str) -> UsageStats,
    /// `(application usage id, application context type)` usage, over every app.
    pub app_usage: &'a dyn Fn(&str, &str) -> UsageStats,
    pub facts: &'a SessionFacts,
    pub now: i64,
}

impl SearchEngine<'_> {
    pub fn search(&self, query: &Query, contexts: &[ResolvedContext]) -> CandidateSet {
        let search = SearchContext { query, contexts };

        // Matching. Action matching and application matching are timed apart: they are both
        // matching, and folding the second into `ranking_us` once hid a 1.2s regression
        // behind a metric that was supposed to describe sorting.
        let actions_started = Instant::now();
        let matcher = ActionMatcher {
            registry: self.registry,
            usage: self.usage,
            facts: self.facts,
            now: self.now,
        };
        let mut action_candidates = matcher.match_actions(query, contexts);
        let matcher_us = actions_started.elapsed().as_micros() as u64;

        let apps_started = Instant::now();
        let application_source = ApplicationSource {
            applications: self.applications,
            app_usage: self.app_usage,
            now: self.now,
        };
        let mut applications = application_source.candidates(&search);
        let apps_us = apps_started.elapsed().as_micros() as u64;

        // Ordering. Everything that matched goes through the same sort and the same
        // deduplication, whatever source produced it.
        let ranking_started = Instant::now();
        action_candidates.sort_by(compare_candidates);
        deduplicate_actions(&mut action_candidates);
        let mut candidates = action_candidates;
        if !applications.is_empty() {
            applications.sort_by(compare_candidates);
            candidates.extend(applications);
            candidates.sort_by(compare_candidates);
        }
        // Text fallbacks come last, after sorting, so they never displace a real match.
        if super::candidate::is_searching(query) {
            let fallbacks = TextFallbackSource {
                registry: self.registry,
            };
            candidates.extend(fallbacks.candidates(query, &candidates));
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
            apps_us,
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
    use crate::core::action::descriptor::CommandRegistry;
    use crate::core::context::resolvers::{text_context, InputEngine};
    use crate::core::context::{ContextValue, ResolvedContext};
    use crate::core::ranking::usage::UsageStats;
    use crate::core::search::candidate::action_candidate_id;
    use crate::core::session::facts::SessionFacts;
    use crate::features::applications::index::IndexedApplication;
    use crate::features::applications::source::application_candidate_id;
    use anywhere_platform::ApplicationInfo;
    use anywhere_protocol::*;

    fn contexts(input: &str) -> Vec<ResolvedContext> {
        let mut contexts = InputEngine.resolve(input);
        if let Some(text) = text_context(input) {
            contexts.insert(0, text);
        }
        contexts
    }

    fn facts() -> SessionFacts {
        SessionFacts {
            git_available: true,
            ..Default::default()
        }
    }

    fn no_usage(_: &str, _: &str) -> UsageStats {
        UsageStats::default()
    }

    fn builtin_engine<'a>(
        registry: &'a CommandRegistry,
        applications: &'a [IndexedApplication],
        usage: &'a dyn Fn(&str, &str) -> UsageStats,
        facts: &'a SessionFacts,
    ) -> SearchEngine<'a> {
        SearchEngine {
            registry,
            applications,
            usage,
            app_usage: usage,
            facts,
            now: 1000,
        }
    }

    #[test]
    fn directory_context_orders_the_documented_examples() {
        let registry = CommandRegistry::builtin();
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
        let applications = Vec::new();
        let usage = no_usage;
        let facts = facts();
        let engine = builtin_engine(&registry, &applications, &usage, &facts);
        let set = engine.search(&Query::parse(""), &contexts);
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
        let set = engine.search(&Query::parse(""), &contexts);
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
        let registry = CommandRegistry::builtin();
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
        let applications = Vec::new();
        let usage = no_usage;
        let facts = facts();
        let engine = builtin_engine(&registry, &applications, &usage, &facts);
        let set = engine.search(&Query::parse(""), &contexts);
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
        let registry = CommandRegistry::builtin();
        let mut contexts = contexts(&"a".repeat(60));
        contexts.push(ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory("/tmp".into()),
            ContextSource::ApplicationContext,
            "finder",
        ));
        let applications = Vec::new();
        let usage = no_usage;
        let facts = facts();
        let engine = builtin_engine(&registry, &applications, &usage, &facts);
        // Home keeps every candidate; the host limits what it displays.
        let home = engine.search(&Query::parse(""), &contexts);
        assert!(!home.candidates.is_empty());
        let search = engine.search(&Query::parse("copy"), &contexts);
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
        let registry = CommandRegistry::builtin();
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
        let applications = Vec::new();
        let usage = no_usage;
        let facts = facts();
        let engine = builtin_engine(&registry, &applications, &usage, &facts);
        let set = engine.search(&Query::parse(""), &contexts);
        let scope_ids = set.scopes.iter().map(|s| s.id.as_str()).collect::<Vec<_>>();
        assert_eq!(scope_ids[0], "all");
        assert_eq!(scope_ids[1], "app");
        assert!(scope_ids.contains(&"git"));
        assert!(scope_ids.contains(&"folder"));
    }

    /// The scope bar leads with what the user actually uses, not a fixed order.
    #[test]
    fn scopes_are_ordered_by_usage_with_a_deterministic_tie_break() {
        let registry = CommandRegistry::builtin();
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
        let applications = Vec::new();
        let facts = facts();

        // Heavy use of the folder actions; the Git group has never been used.
        let folder_heavy = |command: &str, kind: &str| {
            if kind == DIRECTORY {
                UsageStats {
                    execute_count: 8,
                    last_executed_at: 1000,
                }
            } else {
                let _ = command;
                UsageStats::default()
            }
        };
        let engine = builtin_engine(&registry, &applications, &folder_heavy, &facts);
        let set = engine.search(&Query::parse(""), &contexts);
        let ids = set
            .scopes
            .iter()
            .map(|scope| scope.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            ["all", "app", "folder", "git"],
            "folder is used, git is not"
        );
        assert_eq!(
            set.scope_use.get("folder"),
            Some(&24),
            "8 uses of 3 folder actions"
        );

        // With the same usage everywhere, the documented order decides.
        let none_used = |_: &str, _: &str| UsageStats::default();
        let engine = builtin_engine(&registry, &applications, &none_used, &facts);
        let set = engine.search(&Query::parse(""), &contexts);
        let ids = set
            .scopes
            .iter()
            .map(|scope| scope.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            ["all", "app", "git", "folder"],
            "fresh install keeps the docs order"
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
        let engine = builtin_engine(&registry, &applications, &git_heavy, &facts);
        let set = engine.search(&Query::parse(""), &contexts);
        let ids = set
            .scopes
            .iter()
            .map(|scope| scope.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            ["all", "app", "git", "folder"],
            "git use puts git first"
        );
        assert!(set.scope_use.get("git").copied().unwrap_or_default() > 0);
    }

    #[test]
    fn copy_path_is_hidden_when_the_clipboard_already_holds_it() {
        let registry = CommandRegistry::builtin();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap();
        let contexts = vec![ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory(path.clone()),
            ContextSource::ApplicationContext,
            "finder",
        )];
        let applications = Vec::new();
        let usage = no_usage;
        let mut session = facts();
        let engine = builtin_engine(&registry, &applications, &usage, &session);
        assert!(engine
            .search(&Query::parse(""), &contexts)
            .candidates
            .iter()
            .any(|c| c.presentation.title == "Copy Path"));

        session.clipboard_text = Some(path.to_string_lossy().into_owned());
        let engine = builtin_engine(&registry, &applications, &usage, &session);
        assert!(!engine
            .search(&Query::parse(""), &contexts)
            .candidates
            .iter()
            .any(|c| c.presentation.title == "Copy Path"));
    }

    #[test]
    fn git_actions_are_disabled_rather_than_hidden_without_git() {
        let registry = CommandRegistry::builtin();
        let contexts = vec![ResolvedContext::new(
            GIT_REPOSITORY,
            ContextValue::Repository("/tmp".into()),
            ContextSource::ApplicationContext,
            "git",
        )];
        let applications = Vec::new();
        let usage = no_usage;
        let facts = SessionFacts::default();
        let engine = builtin_engine(&registry, &applications, &usage, &facts);
        let set = engine.search(&Query::parse(""), &contexts);
        let status = set
            .candidates
            .iter()
            .find(|c| c.presentation.title == "Git Status")
            .expect("git status stays visible so the group is discoverable");
        assert_eq!(
            status.presentation.availability.state,
            AvailabilityState::Disabled
        );
        assert!(status.presentation.availability.reason.is_some());
    }

    #[test]
    fn application_search_matches_names_aliases_and_keywords() {
        let registry = CommandRegistry::builtin();
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
        let engine = builtin_engine(&registry, &applications, &usage, &facts);
        let set = engine.search(&Query::parse("chrome"), &chrome_contexts);
        assert_eq!(set.candidates[0].presentation.title, "Google Chrome");
        assert_eq!(
            set.candidates[0].presentation.kind,
            CandidateKind::Application
        );
        let by_keyword = engine.search(&Query::parse("browser"), &self::contexts("browser"));
        assert_eq!(by_keyword.candidates[0].presentation.title, "Google Chrome");
        // Applications never appear in Context Home.
        let home = engine.search(&Query::parse(""), &chrome_contexts);
        assert!(home
            .candidates
            .iter()
            .all(|c| c.presentation.kind != CandidateKind::Application));
    }

    #[test]
    fn action_search_finds_git_status_by_partial_words() {
        let registry = CommandRegistry::builtin();
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
        let applications = Vec::new();
        let usage = no_usage;
        let facts = facts();
        let engine = builtin_engine(&registry, &applications, &usage, &facts);
        let set = engine.search(&Query::parse("git sta"), &contexts);
        assert_eq!(set.candidates[0].presentation.title, "Git Status");
    }

    #[test]
    fn typed_object_actions_survive_a_query_without_a_title_match() {
        let registry = CommandRegistry::builtin();
        let url_contexts = contexts("https://github.com");
        let applications = Vec::new();
        let usage = no_usage;
        let facts = facts();
        let engine = builtin_engine(&registry, &applications, &usage, &facts);
        let set = engine.search(&Query::parse("https://github.com"), &url_contexts);
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
        let set = engine.search(&Query::parse("hello"), &unrelated);
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
        let registry = CommandRegistry::builtin();
        let contexts = vec![ResolvedContext::new(
            DIRECTORY,
            ContextValue::Directory("/tmp".into()),
            ContextSource::ApplicationContext,
            "finder",
        )];
        let applications = vec![IndexedApplication {
            bundle_id: "com.apple.finder".into(),
            name: "Finder".into(),
            path: "/System/Library/CoreServices/Finder.app".into(),
            ..Default::default()
        }];
        let usage = no_usage;
        let facts = SessionFacts {
            git_available: true,
            frontmost: Some(ApplicationInfo {
                bundle_id: "com.apple.finder".into(),
                name: "Finder".into(),
            }),
            ..Default::default()
        };
        let engine = builtin_engine(&registry, &applications, &usage, &facts);
        let set = engine.search(&Query::parse(""), &contexts);
        let app_scope = set.scopes.iter().find(|s| s.id == "app").unwrap();
        assert_eq!(app_scope.label, "Finder");
        assert!(!app_scope.candidate_ids.is_empty());
        assert!(app_scope
            .candidate_ids
            .iter()
            .all(|id| !id.starts_with("app:")));
    }
}
