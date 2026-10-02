//! What the user is looking at, and how it is discovered.
//!
//! A [`ResolvedContext`] is an object plus its type and where it came from. Providers turn
//! contexts into further contexts, which is the whole of the resolution model: `core.text`
//! into `core.url`, a directory into a `git.repository`. The engine walks that graph
//! sequentially, and the registry holds the providers available to a session.
pub mod engine;
pub mod navigation;
pub mod provider;
pub mod registry;

pub use engine::{ContextGraphRun, ExecutionContext, GraphStep, MAX_DEPTH};

// The vocabulary a context is expressed in. Re-exported so callers do not have to reach into
// the protocol crate for the types this module is built from.
pub use anywhere_protocol::{ContextSource, ContextTypeId, ProviderId};

use anywhere_protocol::*;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

#[derive(Debug, Clone)]
pub enum ContextValue {
    Text(Arc<str>),
    Url(url::Url),
    Json {
        raw: Arc<str>,
        parsed: Arc<serde_json::Value>,
    },
    File(PathBuf),
    /// One Finder selection, preserving the collection as the user's object.
    FileSelection(Arc<[PathBuf]>),
    Directory(PathBuf),
    /// Git repository root discovered from filesystem metadata only.
    Repository(PathBuf),
    Application {
        bundle_id: String,
        name: String,
    },
    Extension(Arc<str>),
}

impl ContextValue {
    /// Shared identity text. Cloning this does not copy the body.
    pub fn shared_text(&self) -> Arc<str> {
        match self {
            Self::Text(text) | Self::Extension(text) => Arc::clone(text),
            Self::Json { raw, .. } => Arc::clone(raw),
            Self::Url(url) => Arc::from(url.as_str()),
            Self::File(path) | Self::Directory(path) | Self::Repository(path) => {
                Arc::from(path.to_string_lossy().as_ref())
            }
            Self::FileSelection(paths) => Arc::from(
                serde_json::to_string(
                    &paths
                        .iter()
                        .map(|path| path.to_string_lossy())
                        .collect::<Vec<_>>(),
                )
                .expect("path list serializes as JSON"),
            ),
            Self::Application { bundle_id, .. } => Arc::from(bundle_id.as_str()),
        }
    }

    pub fn text(&self) -> String {
        self.shared_text().to_string()
    }

    pub fn identity_fingerprint(&self) -> u64 {
        fingerprint(self.shared_text().as_ref())
    }
    pub fn path(&self) -> Option<&std::path::Path> {
        match self {
            Self::File(p) | Self::Directory(p) | Self::Repository(p) => Some(p),
            _ => None,
        }
    }
}

/// Provenance of a value captured during activation. Derived contexts inherit it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureMetadata {
    pub captured_at_ms: i64,
    pub owning_app: Option<String>,
}

fn fingerprint(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[derive(Debug, Clone)]
pub struct ResolvedContext {
    pub id: ContextId,
    pub type_id: ContextTypeId,
    pub value: ContextValue,
    pub source: ContextSource,
    pub provider_id: ProviderId,
    pub confidence: f32,
    pub depth: u8,
    pub source_context_id: Option<ContextId>,
    pub capture: Option<CaptureMetadata>,
    pub session_id: SessionId,
    pub revision: Revision,
}

impl ResolvedContext {
    pub fn new(type_id: &str, value: ContextValue, source: ContextSource, provider: &str) -> Self {
        Self {
            id: String::new(),
            type_id: type_id.into(),
            value,
            source,
            provider_id: provider.into(),
            confidence: 1.0,
            depth: 0,
            source_context_id: None,
            capture: None,
            session_id: 0,
            revision: 0,
        }
    }
    pub fn display_detail(&self) -> String {
        const MAX: usize = 120;
        // Clipboard values remain available to actions and providers, but neither
        // the context summary nor candidate subtitles should echo their contents.
        let text = if self.source == ContextSource::Clipboard {
            match &self.value {
                ContextValue::Text(text) => {
                    Arc::from(format!("{} characters", text.chars().count()))
                }
                ContextValue::Url(_) => Arc::from("Copied URL"),
                ContextValue::Json { .. } => Arc::from("Copied JSON"),
                _ => Arc::from("Copied context"),
            }
        } else {
            match &self.value {
                ContextValue::Application { name, .. } => Arc::from(name.as_str()),
                ContextValue::FileSelection(paths) => {
                    Arc::from(format!("{} selected items", paths.len()))
                }
                _ => self.value.shared_text(),
            }
        };
        let mut shortened = String::new();
        let mut chars = text.chars();
        for _ in 0..MAX {
            match chars.next() {
                Some(ch) => shortened.push(ch),
                None => return shortened,
            }
        }
        if chars.next().is_some() {
            shortened.push('…');
        }
        shortened
    }

    pub fn descriptor(&self) -> ContextDescriptor {
        let mut properties = BTreeMap::new();
        if let ContextValue::Application { name, .. } = &self.value {
            properties.insert("name".into(), name.clone());
        }
        if let Some(capture) = &self.capture {
            properties.insert("captured_at_ms".into(), capture.captured_at_ms.to_string());
            if let Some(owning_app) = &capture.owning_app {
                properties.insert("owning_app".into(), owning_app.clone());
            }
        }
        ContextDescriptor {
            id: self.id.clone(),
            type_id: self.type_id.clone(),
            value: self.value.text(),
            properties,
            source: self.source,
            provider_id: self.provider_id.clone(),
            confidence: self.confidence,
            depth: self.depth,
            source_context_id: self.source_context_id.clone(),
            session_id: self.session_id,
            revision: self.revision,
        }
    }
    pub fn presentation(&self) -> ContextPresentation {
        let label = match &self.value {
            ContextValue::Text(_) => "Text",
            ContextValue::Url(_) => "URL",
            ContextValue::Json { .. } => "JSON",
            ContextValue::File(_) => "File",
            ContextValue::FileSelection(_) => "Finder Selection",
            ContextValue::Directory(_) if self.type_id == PROJECT_RUST => "Rust Project",
            ContextValue::Directory(_) => "Directory",
            ContextValue::Repository(_) => "Git Repository",
            ContextValue::Application { name, .. } => name,
            ContextValue::Extension(_) => &self.type_id,
        };
        ContextPresentation {
            id: self.id.clone(),
            type_id: self.type_id.clone(),
            label: label.into(),
            detail: self.display_detail(),
            source: self.source,
            key: navigation::key(self),
            root_key: navigation::key(self),
            icon_key: String::new(),
            capabilities: Vec::new(),
            candidate_ids: Vec::new(),
            primary_candidate_id: None,
        }
    }
}

/// The contexts one resolution pass has produced.
///
/// Owns the two rules that were previously spread across the caller: a context is kept once
/// — identity is `(type, value, source)`, and source is significant because explicit user
/// input must not be swallowed by ambient context — and each context gets a stable row id in
/// the order it was found, which is what `source_context_id` refers to.
#[derive(Default)]
pub struct ContextSet {
    contexts: Vec<ResolvedContext>,
    seen: std::collections::HashSet<(ContextTypeId, Arc<str>, ContextSource)>,
}

impl ContextSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Keep `context` unless an equivalent one is already present.
    ///
    /// Returns whether it was kept. The row id and the session/revision stamps are assigned
    /// here, so a caller cannot forget them.
    pub fn insert(
        &mut self,
        mut context: ResolvedContext,
        session_id: SessionId,
        revision: Revision,
    ) -> bool {
        if !self.seen.insert((
            context.type_id.clone(),
            context.value.shared_text(),
            context.source,
        )) {
            return false;
        }
        context.session_id = session_id;
        context.revision = revision;
        context.id = format!("c{}", self.contexts.len());
        self.contexts.push(context);
        true
    }

    pub fn len(&self) -> usize {
        self.contexts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.contexts.is_empty()
    }

    pub fn as_slice(&self) -> &[ResolvedContext] {
        &self.contexts
    }

    pub fn iter(&self) -> std::slice::Iter<'_, ResolvedContext> {
        self.contexts.iter()
    }

    pub fn get(&self, index: usize) -> Option<&ResolvedContext> {
        self.contexts.get(index)
    }

    /// The row at `index`; the run only ever indexes inside its own bounds.
    pub fn row(&self, index: usize) -> &ResolvedContext {
        &self.contexts[index]
    }

    /// Row position of a context, by the id it was given here.
    pub fn position(&self, id: &str) -> Option<usize> {
        self.contexts.iter().position(|context| context.id == id)
    }

    /// Whether any context has this type: the lookup provenance rules need.
    pub fn has_type(&self, type_id: &str) -> bool {
        self.contexts
            .iter()
            .any(|context| context.type_id == type_id)
    }

    /// Every context of one type, in resolution order.
    pub fn of_type<'a>(
        &'a self,
        type_id: &'a str,
    ) -> impl Iterator<Item = &'a ResolvedContext> + 'a {
        self.contexts
            .iter()
            .filter(move |context| context.type_id == type_id)
    }

    /// Hand the contexts over, leaving the set empty.
    pub fn take(&mut self) -> Vec<ResolvedContext> {
        self.seen.clear();
        std::mem::take(&mut self.contexts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(type_id: &str, value: &str, source: ContextSource) -> ResolvedContext {
        ResolvedContext::new(type_id, ContextValue::Text(value.into()), source, "test")
    }

    #[test]
    fn equivalent_contexts_are_kept_once_and_row_ids_are_stable() {
        let mut set = ContextSet::new();
        assert!(set.insert(context(TEXT, "a", ContextSource::UserInput), 1, 0));
        assert!(!set.insert(context(TEXT, "a", ContextSource::UserInput), 1, 0));
        // Source is part of identity: the same value from the environment is a different row.
        assert!(set.insert(context(TEXT, "a", ContextSource::ApplicationContext), 1, 0));
        assert!(set.insert(context(URL, "a", ContextSource::UserInput), 1, 0));
        assert_eq!(set.len(), 3);
        assert_eq!(set.as_slice()[0].id, "c0");
        assert_eq!(set.as_slice()[2].id, "c2");
        assert_eq!(set.as_slice()[2].session_id, 1);
        assert_eq!(set.position("c1"), Some(1));
        assert!(set.has_type(URL));
        assert_eq!(set.of_type(TEXT).count(), 2);
        assert_eq!(set.take().len(), 3);
        assert!(set.is_empty());
    }

    #[test]
    fn clipboard_derived_values_never_expose_their_body_in_display_details() {
        let secret = "https://example.org/?token=private";
        let url = ResolvedContext::new(
            URL,
            ContextValue::Url(url::Url::parse(secret).unwrap()),
            ContextSource::Clipboard,
            "test",
        );
        let json = ResolvedContext::new(
            JSON_DOCUMENT,
            ContextValue::Json {
                raw: r#"{"token":"private"}"#.into(),
                parsed: Arc::new(serde_json::json!({"token":"private"})),
            },
            ContextSource::Clipboard,
            "test",
        );
        assert_eq!(url.presentation().detail, "Copied URL");
        assert_eq!(json.presentation().detail, "Copied JSON");
        assert_eq!(
            context(TEXT, "private", ContextSource::Clipboard).display_detail(),
            "7 characters"
        );
        assert_eq!(
            ResolvedContext::new(
                URL,
                ContextValue::Url(url::Url::parse(secret).unwrap()),
                ContextSource::UserInput,
                "test"
            )
            .display_detail(),
            secret
        );
    }
}
