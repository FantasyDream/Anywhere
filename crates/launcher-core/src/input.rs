use crate::context::{ContextValue, ResolvedContext};
use launcher_protocol::{ContextSource::UserInput, *};
use std::path::PathBuf;
use std::time::Instant;

pub trait InputResolver: Send + Sync {
    fn id(&self) -> &'static str;
    fn resolve(&self, input: &str) -> Vec<ResolvedContext>;
}

pub struct TextResolver;
impl InputResolver for TextResolver {
    fn id(&self) -> &'static str {
        "core.text_resolver"
    }
    fn resolve(&self, input: &str) -> Vec<ResolvedContext> {
        if input.trim().is_empty() {
            return vec![];
        }
        vec![ResolvedContext::new(
            TEXT,
            ContextValue::Text(input.into()),
            UserInput,
            self.id(),
        )]
    }
}

pub struct UrlResolver;
impl InputResolver for UrlResolver {
    fn id(&self) -> &'static str {
        "core.url_resolver"
    }
    fn resolve(&self, input: &str) -> Vec<ResolvedContext> {
        let trimmed = input.trim();
        let Some((scheme, authority)) = trimmed.split_once("://") else {
            return vec![];
        };
        if !(scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http"))
            || authority.is_empty()
            || authority.starts_with('/')
            || trimmed.contains('\\')
            || trimmed.chars().any(char::is_whitespace)
        {
            return vec![];
        }
        match url::Url::parse(trimmed) {
            Ok(url) if url.host_str().is_some() => vec![ResolvedContext::new(
                URL,
                ContextValue::Url(url),
                UserInput,
                self.id(),
            )],
            _ => vec![],
        }
    }
}

pub struct JsonResolver;
impl InputResolver for JsonResolver {
    fn id(&self) -> &'static str {
        "core.json_resolver"
    }
    fn resolve(&self, input: &str) -> Vec<ResolvedContext> {
        match serde_json::from_str(input) {
            Ok(parsed) => vec![ResolvedContext::new(
                JSON,
                ContextValue::Json {
                    raw: input.into(),
                    parsed,
                },
                UserInput,
                self.id(),
            )],
            Err(_) => vec![],
        }
    }
}

pub struct PathResolver;
impl InputResolver for PathResolver {
    fn id(&self) -> &'static str {
        "core.path_resolver"
    }
    fn resolve(&self, input: &str) -> Vec<ResolvedContext> {
        if input.trim().is_empty() {
            return vec![];
        }
        let Ok(path) = PathBuf::from(input)
            .canonicalize()
            .or_else(|_| PathBuf::from(input.trim()).canonicalize())
        else {
            return vec![];
        };
        let (kind, value) = if path.is_dir() {
            (DIRECTORY, ContextValue::Directory(path))
        } else if path.is_file() {
            (FILE, ContextValue::File(path))
        } else {
            return vec![];
        };
        vec![ResolvedContext::new(kind, value, UserInput, self.id())]
    }
}

#[derive(Default)]
pub struct InputEngine;
impl InputEngine {
    pub fn resolve(&self, input: &str) -> Vec<ResolvedContext> {
        let started = Instant::now();
        let resolvers: [&dyn InputResolver; 4] =
            [&TextResolver, &UrlResolver, &PathResolver, &JsonResolver];
        let mut contexts = Vec::new();
        for resolver in resolvers {
            let results = resolver.resolve(input);
            tracing::info!(
                provider_id = resolver.id(),
                count = results.len(),
                "resolver_completed"
            );
            contexts.extend(results);
        }
        tracing::info!(
            duration_us = started.elapsed().as_micros() as u64,
            "input_resolved"
        );
        contexts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn types(input: &str) -> Vec<String> {
        InputEngine
            .resolve(input)
            .into_iter()
            .map(|c| c.type_id)
            .collect()
    }
    #[test]
    fn resolvers_are_additive_and_keep_json_primitives() {
        assert_eq!(types("https://github.com"), [TEXT, URL]);
        assert_eq!(types("HTTPS://github.com"), [TEXT, URL]);
        assert_eq!(types(r#"{"a":1}"#), [TEXT, JSON]);
        for input in ["true", "null", "123", "[]", "\"hello\""] {
            assert_eq!(types(input), [TEXT, JSON]);
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
        assert_eq!(JsonResolver.resolve(input)[0].value.text(), input);
    }
    #[test]
    fn only_existing_regular_files_and_directories_are_paths() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("a file.txt");
        std::fs::write(&file, "test").unwrap();
        assert_eq!(types(file.to_str().unwrap()), [TEXT, FILE]);
        let spaced_file = directory.path().join("trailing space ");
        std::fs::write(&spaced_file, "test").unwrap();
        let resolved = PathResolver.resolve(spaced_file.to_str().unwrap());
        assert_eq!(
            resolved[0].value.text(),
            spaced_file.canonicalize().unwrap().to_str().unwrap()
        );
        assert_eq!(types(directory.path().to_str().unwrap()), [TEXT, DIRECTORY]);
        assert_eq!(
            types(directory.path().join("missing").to_str().unwrap()),
            [TEXT]
        );
    }
}
