//! Read-only environment captured once per session.
//!
//! Availability rules inspect a generic map of facts, not dedicated fields. Capture fills
//! the map (clipboard and git); a new hide rule names an existing key
//! or a newly captured value — the struct does not grow. [`SessionFacts::frontmost`] is
//! session identity for ownership and the Current App scope, not a hide rule.
//!
//! Known keys and value kinds are a host-internal contract. Unknown names and kind
//! mismatches are rejected at write and at rule registration, not swallowed as runtime
//! misses. Nothing here does IO: values were snapshotted before the host took focus.
use crate::core::context::{ContextSource, ContextValue, ResolvedContext};
use anyhow::{bail, Result};
use anywhere_platform::ApplicationInfo;
use std::collections::BTreeMap;

pub const CLIPBOARD: &str = "clipboard";
pub const GIT: &str = "git";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactKind {
    Text,
    Flag,
}

pub fn fact_kind(key: &str) -> Option<FactKind> {
    match key {
        CLIPBOARD => Some(FactKind::Text),
        GIT => Some(FactKind::Flag),
        _ => None,
    }
}

pub fn validate_write(key: &str, value: &FactValue) -> Result<()> {
    match (fact_kind(key), value) {
        (Some(FactKind::Text), FactValue::Text(_)) | (Some(FactKind::Flag), FactValue::Flag(_)) => {
            Ok(())
        }
        (None, _) => bail!("unknown fact: {key}"),
        (Some(FactKind::Text), _) => bail!("fact {key} requires text"),
        (Some(FactKind::Flag), _) => bail!("fact {key} requires a flag"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactValue {
    Text(String),
    Flag(bool),
}

impl FactValue {
    fn matches_context(&self, context: &ResolvedContext) -> bool {
        match self {
            Self::Text(text) => match &context.value {
                // Providers parse URL contexts too: compare the same canonical value,
                // while retaining path, query and fragment differences.
                ContextValue::Url(url) => {
                    let text = text.trim();
                    if text == url.as_str() {
                        return true;
                    }
                    let Some((_, authority)) = text.split_once("://") else {
                        return false;
                    };
                    // URL parsing can repair malformed input. Such clipboard text
                    // must not hide a valid context's copy action.
                    if authority.is_empty()
                        || authority.starts_with('/')
                        || text.contains('\\')
                        || text.chars().any(char::is_whitespace)
                    {
                        return false;
                    }
                    url::Url::parse(text).is_ok_and(|parsed| parsed == *url)
                }
                _ => text == &context.value.text(),
            },
            Self::Flag(_) => false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SessionFacts {
    pub frontmost: Option<ApplicationInfo>,
    values: BTreeMap<String, FactValue>,
}

impl SessionFacts {
    pub fn from_map(
        frontmost: Option<ApplicationInfo>,
        values: BTreeMap<String, FactValue>,
    ) -> Result<Self> {
        let mut facts = Self {
            frontmost,
            values: BTreeMap::new(),
        };
        for (key, value) in values {
            facts.insert(key, value)?;
        }
        Ok(facts)
    }

    pub fn insert(&mut self, key: impl Into<String>, value: FactValue) -> Result<()> {
        let key = key.into();
        validate_write(&key, &value)?;
        self.values.insert(key, value);
        Ok(())
    }

    pub fn remove(&mut self, key: &str) {
        self.values.remove(key);
    }

    pub fn with(mut self, key: impl Into<String>, value: FactValue) -> Self {
        self.insert(key, value).expect("valid session fact");
        self
    }

    pub fn get(&self, key: &str) -> Option<&FactValue> {
        self.values.get(key)
    }

    pub fn matches_context(&self, key: &str, context: &ResolvedContext) -> bool {
        self.get(key)
            .is_some_and(|value| value.matches_context(context))
    }

    pub fn flag(&self, key: &str) -> Option<bool> {
        match self.get(key) {
            Some(FactValue::Flag(value)) => Some(*value),
            _ => None,
        }
    }
}

/// Which application, if any, a context belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ownership {
    App(String),
    Ambient,
}

/// Ownership of one context.
///
/// Computed per context and never carried over: an input-derived object must not inherit
/// the ambient application just because an ambient context was visited earlier.
pub fn context_ownership(context: &ResolvedContext, facts: &SessionFacts) -> Ownership {
    match context.source {
        ContextSource::ForegroundApplication
        | ContextSource::ApplicationContext
        | ContextSource::FinderSelection
        | ContextSource::SelectedText
        | ContextSource::BrowserCurrentPage => context
            .capture
            .as_ref()
            .and_then(|capture| capture.owning_app.as_ref())
            .filter(|app| !app.is_empty())
            .cloned()
            .or_else(|| facts.frontmost.as_ref().map(|app| app.bundle_id.clone()))
            .filter(|app| !app.is_empty())
            .map_or(Ownership::Ambient, Ownership::App),
        ContextSource::UserInput | ContextSource::Clipboard => Ownership::Ambient,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_facts_match_canonical_addresses_without_merging_different_targets() {
        let context = ResolvedContext::new(
            anywhere_protocol::URL,
            ContextValue::Url(url::Url::parse("https://example.org/").unwrap()),
            ContextSource::BrowserCurrentPage,
            "test",
        );
        for text in ["https://example.org/", "  https://EXAMPLE.org:443\n"] {
            let facts = SessionFacts::default().with(CLIPBOARD, FactValue::Text(text.into()));
            assert!(facts.matches_context(CLIPBOARD, &context), "{text:?}");
        }
        for text in [
            "http://example.org/",
            "https://other.example.org/",
            "https://example.org:444/",
            "https://example.org/page",
            "https://example.org/?q=1",
            "https://example.org/#section",
            "https:example.org",
            "https:///example.org",
            "https://example.org\\",
            "https://exa\nmple.org/",
            "not a URL",
            "",
        ] {
            let facts = SessionFacts::default().with(CLIPBOARD, FactValue::Text(text.into()));
            assert!(!facts.matches_context(CLIPBOARD, &context), "{text:?}");
        }
        assert!(!SessionFacts::default().matches_context(CLIPBOARD, &context));
    }

    #[test]
    fn text_and_path_facts_keep_exact_matching() {
        for (type_id, value) in [
            (anywhere_protocol::TEXT, ContextValue::Text("/tmp".into())),
            (
                anywhere_protocol::DIRECTORY,
                ContextValue::Directory("/tmp".into()),
            ),
        ] {
            let context = ResolvedContext::new(type_id, value, ContextSource::UserInput, "test");
            let same = SessionFacts::default().with(CLIPBOARD, FactValue::Text("/tmp".into()));
            assert!(same.matches_context(CLIPBOARD, &context));
            let padded = SessionFacts::default().with(CLIPBOARD, FactValue::Text(" /tmp\n".into()));
            assert!(!padded.matches_context(CLIPBOARD, &context));
        }
    }

    #[test]
    fn from_map_rejects_unknown_and_mistyped_values() {
        let mut values = BTreeMap::new();
        values.insert("nope".into(), FactValue::Flag(true));
        assert!(SessionFacts::from_map(None, values)
            .unwrap_err()
            .to_string()
            .contains("unknown fact"));

        let mut values = BTreeMap::new();
        values.insert(CLIPBOARD.into(), FactValue::Flag(true));
        assert!(SessionFacts::from_map(None, values)
            .unwrap_err()
            .to_string()
            .contains("requires text"));

        let mut values = BTreeMap::new();
        values.insert(GIT.into(), FactValue::Text("yes".into()));
        assert!(SessionFacts::from_map(None, values)
            .unwrap_err()
            .to_string()
            .contains("requires a flag"));

        let mut values = BTreeMap::new();
        values.insert(GIT.into(), FactValue::Flag(true));
        values.insert(CLIPBOARD.into(), FactValue::Text("x".into()));
        assert!(SessionFacts::from_map(None, values).is_ok());
    }

    #[test]
    fn flag_preserves_missing_false_and_true() {
        assert_eq!(SessionFacts::default().flag(GIT), None);
        assert_eq!(
            SessionFacts::default()
                .with(GIT, FactValue::Flag(false))
                .flag(GIT),
            Some(false)
        );
        assert_eq!(
            SessionFacts::default()
                .with(GIT, FactValue::Flag(true))
                .flag(GIT),
            Some(true)
        );
    }
}
