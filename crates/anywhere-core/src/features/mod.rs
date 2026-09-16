//! First-party product features.
//!
//! Each owns the contexts it recognizes and the actions it offers. Adding one is a provider
//! and an action bundle under its own directory, plus a registration — never an edit to the
//! engine, the search pipeline or the launcher service.
pub mod applications;
pub mod filesystem;
pub mod git;
pub mod json;
pub mod text;
pub mod url;

use crate::core::context::provider::ContextProvider;
use crate::core::context::ResolvedContext;
use std::sync::Arc;

/// Every action the first-party features contribute, as bundles.
///
/// This is the whole registration surface: adding a feature means adding its bundle here.
pub fn builtin_action_bundles() -> Vec<crate::core::action::bundle::ActionBundle> {
    vec![
        git::actions::bundle(),
        filesystem::actions_folder::bundle(),
        filesystem::actions_file::bundle(),
        url::actions::bundle(),
        json::actions::bundle(),
    ]
}

/// Every provider a session gets from the first-party features.
///
/// The registry decides the order (priority, then id); this only says what exists. A new
/// feature adds its provider here and nothing else has to change.
pub fn builtin_providers() -> Vec<Arc<dyn ContextProvider>> {
    vec![
        Arc::new(url::provider::UrlProvider::new()),
        Arc::new(filesystem::provider::PathProvider::default()),
        Arc::new(json::provider::JsonProvider::new()),
        Arc::new(text::provider::TextProvider::default()),
        Arc::new(git::provider::GitRepositoryProvider::default()),
    ]
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
