//! Structure tests.
//!
//! These do not check behaviour; they check that the shape the refactor established is still
//! there. Each one fails if a future change puts a responsibility back where it does not
//! belong, which is the only way a structure survives contact with new features.
use anywhere_core::core::context::ResolvedContext;
use anywhere_core::{
    core::{
        action::{availability, bundle::ActionBundle, descriptor::ActionRegistry, matcher},
        context::{
            engine::{ContextGraph, GraphStep, MAX_DEPTH},
            registry::ContextProviderRegistry,
        },
        search::{
            candidate::{compare_candidates, Candidate},
            candidate_source::{CandidateSource, SearchContext},
        },
    },
    features,
};
use anywhere_protocol::*;
use std::sync::Arc;

/// The action matcher as the pipeline sees it: one source among others.
fn sources_of(contexts: &[ResolvedContext], registry: &ActionRegistry) -> Vec<Candidate> {
    use anywhere_core::core::action::matcher::ActionMatcher;
    use anywhere_core::core::search::candidate_source::{CandidateSource, SearchContext};
    let usage = |_: &str, _: &str| anywhere_core::core::ranking::usage::UsageStats::default();
    let facts = anywhere_core::core::session::facts::SessionFacts::default();
    let matcher = ActionMatcher::new(
        registry,
        &usage,
        &facts,
        1000,
        anywhere_core::features::context_semantic,
    );
    matcher.candidates(&SearchContext {
        scope_id: None,
        query: &anywhere_core::core::search::query::Query::parse("copy"),
        contexts,
    })
}

/// Recognition is a provider chain, and a chain resolves further than its seeds.
///
/// `core.text` → `core.directory` → `git.repository`: the walk has to follow what a provider
/// produced, not just what it was handed.
#[tokio::test]
async fn the_provider_chain_is_recursive_and_deduplicates() {
    let registry = {
        let mut registry = ContextProviderRegistry::default();
        for provider in features::builtin_providers() {
            registry.register(provider).unwrap();
        }
        registry
    };
    // Two different inputs that resolve to the same path must not produce a second directory.
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    std::fs::create_dir(root.join(".git")).unwrap();

    let execution = anywhere_core::core::context::ExecutionContext::detached(1, 0);
    let seeds = features::recognized_contexts(root.to_str().unwrap());
    let mut run = ContextGraph::start(seeds, &execution);
    let started = std::time::Instant::now();
    let mut rounds = 0;
    loop {
        rounds += 1;
        assert!(rounds < 64, "the graph never finished");
        match run.step(registry.providers()) {
            GraphStep::Pending => {
                let Some(outcome) = run.await_pending(None, None, started).await else {
                    continue;
                };
                run.settle(outcome);
            }
            GraphStep::Done { contexts, .. } => {
                assert!(contexts.iter().any(|context| context.type_id == DIRECTORY));
                assert!(contexts
                    .iter()
                    .any(|context| context.type_id == GIT_REPOSITORY));
                assert!(contexts.iter().all(|context| context.depth <= MAX_DEPTH));
                // Nothing is kept twice.
                let mut keys = contexts
                    .iter()
                    .map(|context| {
                        format!(
                            "{}|{}|{:?}",
                            context.type_id,
                            context.value.text(),
                            context.source
                        )
                    })
                    .collect::<Vec<_>>();
                keys.sort();
                let before = keys.len();
                keys.dedup();
                assert_eq!(before, keys.len(), "a context was kept twice: {keys:?}");
                return;
            }
        }
    }
}

/// Every action a context applies to comes out of the matcher, and availability is decided
/// separately from applicability.
#[test]
fn matching_and_availability_are_separate_steps() {
    let registry = anywhere_core::application::builtin_registry();
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let contexts = features::recognized_contexts(root.to_str().unwrap());

    let facts = anywhere_core::core::session::facts::SessionFacts::default().with(
        anywhere_core::core::session::facts::GIT,
        anywhere_core::core::session::facts::FactValue::Flag(true),
    );
    let usage = |_: &str, _: &str| anywhere_core::core::ranking::usage::UsageStats::default();
    let matcher = matcher::ActionMatcher::new(
        &registry,
        &usage,
        &facts,
        1000,
        anywhere_core::features::context_semantic,
    );
    let query = anywhere_core::core::search::query::Query::parse("");
    let matched = matcher.match_actions(&query, &contexts);
    assert!(
        matched
            .iter()
            .any(|candidate| candidate.presentation.title == "Copy Path"),
        "a directory offers its actions"
    );

    // The same applicability with a clipboard that already holds the path is hidden: that is
    // availability, and it is not the matcher's decision to make.
    let facts = anywhere_core::core::session::facts::SessionFacts::default()
        .with(
            anywhere_core::core::session::facts::CLIPBOARD,
            anywhere_core::core::session::facts::FactValue::Text(
                root.to_string_lossy().into_owned(),
            ),
        )
        .with(
            anywhere_core::core::session::facts::GIT,
            anywhere_core::core::session::facts::FactValue::Flag(true),
        );
    let matcher = matcher::ActionMatcher::new(
        &registry,
        &usage,
        &facts,
        1000,
        anywhere_core::features::context_semantic,
    );
    assert!(
        !matcher
            .match_actions(&query, &contexts)
            .iter()
            .any(|candidate| candidate.presentation.title == "Copy Path"),
        "availability removed the row, not applicability"
    );
    assert!(contexts.iter().any(|context| context.type_id == DIRECTORY));
}

/// Applications and actions reach the list through the same contract.
#[test]
fn every_candidate_kind_has_a_source_behind_it() {
    let registry = anywhere_core::application::builtin_registry();
    let usage = |_: &str, _: &str| anywhere_core::core::ranking::usage::UsageStats::default();
    let facts = anywhere_core::core::session::facts::SessionFacts::default();
    let search = anywhere_core::core::search::engine::ActionSearch::new(
        &registry,
        &usage,
        &facts,
        1000,
        anywhere_core::features::context_semantic,
    );
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let contexts = features::recognized_contexts(root.to_str().unwrap());
    let set = search.search(
        &anywhere_core::core::search::query::Query::parse("copy"),
        &contexts,
        &[],
    );
    assert!(
        set.candidates
            .iter()
            .all(|candidate| candidate.presentation.kind == CandidateKind::Action),
        "with no application index there are no application rows"
    );
    // Ordering is a property of candidates, and every source's rows go through it.
    let mut sorted = set.candidates.clone();
    sorted.sort_by(compare_candidates);
    assert!(sorted
        .windows(2)
        .all(|pair| pair[0].presentation.score >= pair[1].presentation.score));

    // Both kinds arrive through the same contract: the core's own actions and whatever a
    // feature registers. That is what makes a new kind of result additive.
    let matched = sources_of(&contexts, &registry);
    assert!(!matched.is_empty(), "actions are a source too");

    struct Marker;
    impl CandidateSource for Marker {
        fn candidates(&self, _: &SearchContext<'_>) -> Vec<Candidate> {
            vec![Candidate {
                presentation: CandidatePresentation {
                    shortcut: None,
                    id: "marker:test".into(),
                    kind: CandidateKind::Application,
                    is_menu: false,
                    file_kind: None,
                    title: "Marker".into(),
                    subtitle: None,
                    icon: IconDescriptor {
                        key: "application".into(),
                    },
                    score: 1.0,
                    group_id: "app".into(),
                    availability: Availability::enabled(),
                    command_id: None,
                    bundle_id: Some("marker".into()),
                    owning_app: None,
                    menu_path: Vec::new(),
                    context_keys: Vec::new(),
                    execution_context_id: None,
                    execution_target_label: None,
                    home_eligible: true,
                    primary_action: false,
                    section_label: None,
                },
                target:
                    anywhere_core::core::search::candidate::CandidateTarget::LaunchApplication {
                        path: "/marker".into(),
                        icon_source: "/marker".into(),
                    },
            }]
        }
    }
    let with_source = search.search(
        &anywhere_core::core::search::query::Query::parse("copy"),
        &contexts,
        &[&Marker],
    );
    assert!(
        with_source
            .candidates
            .iter()
            .any(|candidate| candidate.presentation.id == "marker:test"),
        "a new source reaches the engine without changing it"
    );
}

/// A feature registers a bundle; nothing else has to know it exists.
#[test]
fn features_register_through_bundles() {
    let bundles: Vec<ActionBundle> = features::builtin_action_bundles();
    assert!(
        bundles.len() >= 4,
        "each feature contributes its own bundle"
    );
    let registry = anywhere_core::application::builtin_registry();
    for bundle in &bundles {
        assert!(
            bundle.actions.iter().all(|action| !action.id.is_empty()),
            "a bundle declares its actions"
        );
        for action in &bundle.actions {
            assert!(
                registry.get(action.id).is_some(),
                "{} was not registered",
                action.id
            );
        }
    }
    // Providers are registered the same way.
    assert!(features::builtin_providers().len() >= 4);
    let ids = features::builtin_providers()
        .iter()
        .map(|provider| provider.descriptor().id.clone())
        .collect::<Vec<_>>();
    let unique = ids.iter().collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), unique.len(), "provider ids are unique");

    // Availability is reachable without going through matching or execution.
    let _ = availability::is_user_object;
    let _ = Arc::new(());
}

#[test]
fn feature_contributions_have_unique_ownership_and_share_registration_contract() {
    let contributions = features::contribution::builtins();
    let mut ids = std::collections::HashSet::new();
    for feature in &contributions {
        assert!(ids.insert(feature.id));
    }
    let git = contributions.iter().find(|c| c.id == "git").unwrap();
    let node = contributions.iter().find(|c| c.id == "node").unwrap();
    assert!(!git.providers.is_empty() && !git.bundles.is_empty());
    assert!(!node.providers.is_empty() && !node.bundles.is_empty() && !node.factories.is_empty());
    assert!(contributions
        .iter()
        .find(|c| c.id == "filesystem")
        .unwrap()
        .source
        .is_some());
}

#[test]
fn menu_semantics_are_explicit_and_plugin_ids_are_opaque() {
    let raw = anywhere_plugin::EXAMPLE_PLUGIN.replace("id = \"example\"", "id = \"app\"");
    let mut registry = anywhere_core::application::builtin_registry();
    registry
        .register_plugin(anywhere_plugin::parse_manifest(&raw).unwrap())
        .unwrap();
    assert!(!registry.get("app.search").unwrap().executor.is_menu());
    let app = anywhere_platform::ApplicationInfo {
        bundle_id: "test.editor".into(),
        name: "Editor".into(),
        pid: 1,
        instance_id: Some("one".into()),
    };
    let mut menus = anywhere_core::features::app_actions::convert::descriptors(
        &app,
        vec![anywhere_platform::DiscoveredMenuItem {
            title: "Root Leaf".into(),
            menu_path: vec!["Root Leaf".into()],
            identifiers: vec![None],
            shortcut: None,
            enabled: true,
        }],
    );
    let leaf = menus.pop().unwrap();
    assert!(leaf.menu_path.is_empty());
    assert!(leaf.executor.is_menu());
}
