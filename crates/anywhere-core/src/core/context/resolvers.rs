//! Pure `core.text` to typed-context recognition.
//!
//! Resolvers only inspect the raw string. They never touch the filesystem beyond
//! the explicit path check, never run a process and never reach the network
//! (`PathResolver` does call `canonicalize`, which is the existing V0.1 behaviour).
use crate::core::context::{ContextValue, ResolvedContext};
use anywhere_protocol::ContextSource::UserInput;
use anywhere_protocol::*;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct ResolvedInput {
    pub type_id: ContextTypeId,
    pub value: ContextValue,
}

pub trait InputResolver: Send + Sync {
    fn id(&self) -> &'static str;
    fn resolve(&self, input: &str) -> Option<ResolvedInput>;
}

pub struct UrlResolver;
impl InputResolver for UrlResolver {
    fn id(&self) -> &'static str {
        "core.input.url"
    }
    fn resolve(&self, input: &str) -> Option<ResolvedInput> {
        let trimmed = input.trim();
        let (scheme, authority) = trimmed.split_once("://")?;
        if !(scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http"))
            || authority.is_empty()
            || authority.starts_with('/')
            || trimmed.contains('\\')
            || trimmed.chars().any(char::is_whitespace)
        {
            return None;
        }
        match url::Url::parse(trimmed) {
            Ok(url) if url.host_str().is_some() => Some(ResolvedInput {
                type_id: URL.into(),
                value: ContextValue::Url(url),
            }),
            _ => None,
        }
    }
}

pub struct JsonResolver;
impl InputResolver for JsonResolver {
    fn id(&self) -> &'static str {
        "core.input.json"
    }
    fn resolve(&self, input: &str) -> Option<ResolvedInput> {
        let parsed = serde_json::from_str(input).ok()?;
        Some(ResolvedInput {
            type_id: JSON_DOCUMENT.into(),
            value: ContextValue::Json {
                raw: input.into(),
                parsed,
            },
        })
    }
}

pub struct PathResolver;
impl InputResolver for PathResolver {
    fn id(&self) -> &'static str {
        "core.input.path"
    }
    fn resolve(&self, input: &str) -> Option<ResolvedInput> {
        if input.trim().is_empty() {
            return None;
        }
        let path = PathBuf::from(input)
            .canonicalize()
            .or_else(|_| PathBuf::from(input.trim()).canonicalize())
            .ok()?;
        let (type_id, value) = if path.is_dir() {
            (DIRECTORY, ContextValue::Directory(path))
        } else if path.is_file() {
            (FILE, ContextValue::File(path))
        } else {
            return None;
        };
        Some(ResolvedInput {
            type_id: type_id.into(),
            value,
        })
    }
}

/// Recognition order defines tie-breaking only; results are additive, never exclusive.
#[derive(Default)]
pub struct InputEngine;

impl InputEngine {
    pub fn resolvers(&self) -> [&'static dyn InputResolver; 3] {
        [&UrlResolver, &PathResolver, &JsonResolver]
    }

    /// Resolve a raw input into additional typed contexts. `core.text` is emitted
    /// separately by the text provider so the raw text is always preserved.
    pub fn resolve(&self, input: &str) -> Vec<ResolvedContext> {
        let started = Instant::now();
        let mut contexts = Vec::new();
        for resolver in self.resolvers() {
            let resolved = resolver.resolve(input);
            tracing::info!(
                provider_id = resolver.id(),
                matched = resolved.is_some() as u8,
                "resolver_completed"
            );
            if let Some(resolved) = resolved {
                contexts.push(ResolvedContext::new(
                    &resolved.type_id,
                    resolved.value,
                    UserInput,
                    resolver.id(),
                ));
            }
        }
        tracing::info!(
            duration_us = started.elapsed().as_micros() as u64,
            count = contexts.len(),
            "input_recognized"
        );
        contexts
    }
}

/// Always emit the raw text for a non-empty input.
pub fn text_context(input: &str) -> Option<ResolvedContext> {
    if input.trim().is_empty() {
        return None;
    }
    Some(ResolvedContext::new(
        TEXT,
        ContextValue::Text(input.into()),
        UserInput,
        "core.input.text",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn types(input: &str) -> Vec<String> {
        let mut types = Vec::new();
        if text_context(input).is_some() {
            types.push(TEXT.to_owned());
        }
        types.extend(InputEngine.resolve(input).into_iter().map(|c| c.type_id));
        types
    }
    #[test]
    fn resolvers_are_additive_and_keep_json_primitives() {
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
        let input = "  {\"a\":1} \n";
        assert_eq!(
            JsonResolver.resolve(input).unwrap().value.text(),
            input,
            "raw JSON text is preserved"
        );
    }
    #[test]
    fn only_existing_regular_files_and_directories_are_paths() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("a file.txt");
        std::fs::write(&file, "test").unwrap();
        assert_eq!(types(file.to_str().unwrap()), [TEXT, FILE]);
        let spaced_file = directory.path().join("trailing space ");
        std::fs::write(&spaced_file, "test").unwrap();
        let resolved = PathResolver.resolve(spaced_file.to_str().unwrap()).unwrap();
        assert_eq!(
            resolved.value.text(),
            spaced_file.canonicalize().unwrap().to_str().unwrap()
        );
        assert_eq!(types(directory.path().to_str().unwrap()), [TEXT, DIRECTORY]);
        assert_eq!(
            types(directory.path().join("missing").to_str().unwrap()),
            [TEXT]
        );
    }
    #[test]
    fn json_type_is_the_v0_2_canonical_identifier() {
        let json = JsonResolver.resolve("{}").unwrap();
        assert_eq!(json.type_id, JSON_DOCUMENT);
        assert_ne!(json.type_id, JSON_LEGACY);
    }
}
