//! First-party product features.
//!
//! Adding a capability is a registration, not a branch in core:
//! - an [`ActionBundle`](crate::core::action::bundle::ActionBundle) in [`builtin_action_bundles`]
//! - a [`ContextProvider`] in [`builtin_providers`]
//! - a seed in [`session_seeds`] if it starts the context graph
//! - a [`CandidateSource`](crate::core::search::candidate_source::CandidateSource) passed into search
//!
//! Hide rules name a fact key; capture writes the fact, not a new struct field.
pub mod app_actions;
pub mod applications;
pub mod contribution;
pub mod developer;
pub mod filesystem;
pub mod git;
pub mod groups;
pub mod json;
pub mod text;
pub mod url;

use crate::core::context::provider::ContextProvider;
use crate::core::context::{ContextSource, ResolvedContext};
use anywhere_platform::ApplicationInfo;
use std::sync::Arc;

/// Every action the first-party features contribute, as bundles.
pub fn builtin_action_bundles() -> Vec<crate::core::action::bundle::ActionBundle> {
    contribution::builtins()
        .into_iter()
        .flat_map(|c| c.bundles)
        .collect()
}

pub fn builtin_providers() -> Vec<Arc<dyn ContextProvider>> {
    contribution::builtins()
        .into_iter()
        .flat_map(|c| c.providers)
        .collect()
}

/// Contexts that start a revision: memoized environment, frozen frontmost app, typed text.
pub fn session_seeds(
    frontmost: Option<&ApplicationInfo>,
    cache: impl IntoIterator<Item = ResolvedContext>,
    input: &str,
) -> Vec<ResolvedContext> {
    let mut seeds: Vec<_> = cache.into_iter().collect();
    if let Some(app) = frontmost {
        seeds.push(applications::context::frontmost_context(app));
    }
    if let Some(text) = text::provider::text_context(input) {
        seeds.push(text);
    }
    seeds
}

/// Ranking bias for a context type. Typed text is weaker than a recognized object.
pub fn context_semantic(type_id: &str) -> i32 {
    if type_id == anywhere_protocol::TEXT {
        40
    } else {
        90
    }
}

/// Environment context reused by later revisions of the same session.
///
/// User-typed objects stay out: caching them made the next query inherit the previous path.
pub fn reusable_environment(context: &ResolvedContext) -> bool {
    context.source == ContextSource::ApplicationContext
        && matches!(
            context.type_id.as_str(),
            anywhere_protocol::DIRECTORY | anywhere_protocol::GIT_REPOSITORY
        )
}

pub fn remember_environment(cache: &mut Vec<ResolvedContext>, contexts: &[ResolvedContext]) {
    for context in contexts {
        if !reusable_environment(context) {
            continue;
        }
        let seen = cache.iter().any(|cached| {
            cached.type_id == context.type_id
                && cached.value.shared_text() == context.value.shared_text()
                && cached.source == context.source
        });
        if !seen {
            cache.push(context.clone());
        }
    }
}

/// Every context the built-in recognizers derive from one input.
///
/// The providers are thin adapters over these same functions, so tests and the
/// micro-benchmark exercise the real recognition rules rather than a copy of them. Order is
/// the documented recognition order: the raw text, then a URL, a path, and a JSON document;
/// recognition is additive, never exclusive.
pub fn recognized_contexts(input: &str) -> Vec<ResolvedContext> {
    let mut contexts = Vec::new();
    if let Some(text) = text::provider::text_context(input) {
        contexts.push(text);
    }
    for context in [
        url::provider::recognize(input),
        filesystem::provider::recognize_path(input),
        json::provider::recognize(input),
    ]
    .into_iter()
    .flatten()
    {
        contexts.push(context);
    }
    contexts
}

#[cfg(test)]
mod tests {
    use super::recognized_contexts;
    use anywhere_protocol::*;

    /// The type ids one input produces, text first: the documented recognition order.
    fn types(input: &str) -> Vec<String> {
        recognized_contexts(input)
            .into_iter()
            .map(|context| context.type_id)
            .collect()
    }

    #[test]
    fn recognition_is_additive_and_keeps_json_primitives() {
        assert_eq!(types("https://github.com"), [TEXT, URL]);
        assert_eq!(types("HTTPS://github.com"), [TEXT, URL]);
        assert_eq!(types(r#"{"a":1}"#), [TEXT, JSON_DOCUMENT]);
        for input in ["true", "null", "123", "[]", "\"hello\""] {
            assert_eq!(types(input), [TEXT, JSON_DOCUMENT]);
        }
        assert_eq!(types("random sentence"), [TEXT]);
        for input in [
            "github.com",
            "https://",
            "https:///github.com",
            "https://\\github.com",
            "https://foo bar",
            "ftp://example.com",
        ] {
            assert_eq!(types(input), [TEXT]);
        }
        assert!(types("  ").is_empty());
    }

    /// The recognizers run in the order the pre-provider pipeline used: a string that is
    /// both a URL and a path stays a URL, and one that is both a path and JSON stays a path.
    #[test]
    fn recognizers_run_in_the_documented_order() {
        use crate::core::context::registry::ContextProviderRegistry;
        let mut registry = ContextProviderRegistry::default();
        for provider in super::builtin_providers() {
            registry.register(provider).unwrap();
        }
        let recognizers = registry
            .ids()
            .into_iter()
            .filter(|id| id.starts_with("core.") && *id != "core.text_provider")
            .collect::<Vec<_>>();
        let expected = ["core.url", "core.path", "core.json", "core.git_repository"];
        for id in &expected {
            assert!(
                recognizers.contains(id),
                "{id} missing from {recognizers:?}"
            );
        }
        let position = |id: &str| recognizers.iter().position(|entry| *entry == id).unwrap();
        assert!(position("core.url") < position("core.path"));
        assert!(position("core.path") < position("core.json"));
    }

    #[test]
    fn json_type_is_the_v0_2_canonical_identifier() {
        let json = recognized_contexts("{}")
            .into_iter()
            .find(|context| context.type_id == JSON_DOCUMENT)
            .expect("a JSON document context");
        assert_ne!(json.type_id, JSON_LEGACY);
        assert_eq!(json.value.text(), "{}", "the raw JSON text is preserved");
    }
}
