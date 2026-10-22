//! Built-in product features and their registration contracts.
//!
//! Domain modules implement [`BuiltinFeature`] and declare module-owned
//! `registration.rs` entries. Cargo generates the catalogue; Application binds
//! services once at startup and preferences select the captured contributions.
//! Providers, Actions and Sources use shared Core contracts. The collection
//! helpers below support isolated consumers; they do not reload the live app.
pub mod app_actions;
pub mod applications;
pub mod catalog;
pub mod clipboard;
pub mod contribution;
pub mod descriptor;
pub mod filesystem;
pub mod groups;
pub mod policy;
pub mod services;
pub mod text;
pub mod tools;

pub mod developer;
pub mod git;
pub(crate) mod host_services;
pub mod module;
pub mod saved_directories;
pub mod url;
pub use module::{builtin_contributions, BuiltinFeature, FeatureBindings};

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

/// Static lightweight recognition catalogue, also useful for tools and tests.
pub fn input_recognizers() -> Vec<crate::core::context::input_recognizer::InputRecognizer> {
    let mut registry = crate::core::context::registry::ContextProviderRegistry::default();
    for contribution in contribution::builtins() {
        for rule in contribution.input_recognizers {
            registry
                .register_input_recognizer(rule, None)
                .expect("invalid input recognizer registration");
        }
    }
    registry.input_recognizers()
}

/// Every context the built-in recognizers derive from one input.
///
/// The providers are thin adapters over these same functions, so tests and the
/// micro-benchmark exercise the real recognition rules rather than a copy of them. Order is
/// the documented recognition order: the raw text, then a URL and a path;
/// recognition is additive, never exclusive.
pub fn recognized_contexts(input: &str) -> Vec<ResolvedContext> {
    let mut contexts = Vec::new();
    if let Some(text) = text::provider::text_context(input) {
        contexts.push(text);
    }
    let rules = input_recognizers();
    for rule in rules {
        if let Some(context) = (rule.recognize)(input) {
            contexts.push(context);
        }
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
    fn recognition_is_additive_without_deferred_tool_contexts() {
        assert_eq!(types("https://github.com"), [TEXT, URL]);
        assert_eq!(types("HTTPS://github.com"), [TEXT, URL]);
        assert_eq!(types(r#"{"a":1}"#), [TEXT]);
        for input in ["true", "null", "123", "[]", "\"hello\""] {
            assert_eq!(types(input), [TEXT]);
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
    /// both a URL and a path stays a URL.
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
        let expected = ["core.url", "core.path", "core.git_repository"];
        for id in &expected {
            assert!(
                recognizers.contains(id),
                "{id} missing from {recognizers:?}"
            );
        }
        let position = |id: &str| recognizers.iter().position(|entry| *entry == id).unwrap();
        assert!(position("core.url") < position("core.path"));
    }
}
