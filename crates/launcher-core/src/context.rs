use launcher_protocol::*;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone)]
pub enum ContextValue {
    Text(String),
    Url(url::Url),
    Json {
        raw: String,
        parsed: serde_json::Value,
    },
    File(PathBuf),
    Directory(PathBuf),
    Application {
        bundle_id: String,
        name: String,
    },
    Extension(String),
}

impl ContextValue {
    pub fn text(&self) -> String {
        match self {
            Self::Text(s) | Self::Extension(s) => s.clone(),
            Self::Url(url) => url.to_string(),
            Self::Json { raw, .. } => raw.clone(),
            Self::File(p) | Self::Directory(p) => p.to_string_lossy().into_owned(),
            Self::Application { bundle_id, .. } => bundle_id.clone(),
        }
    }
    pub fn path(&self) -> Option<&std::path::Path> {
        match self {
            Self::File(p) | Self::Directory(p) => Some(p),
            _ => None,
        }
    }
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
            session_id: 0,
            revision: 0,
        }
    }
    pub fn descriptor(&self) -> ContextDescriptor {
        let mut properties = BTreeMap::new();
        if let ContextValue::Application { name, .. } = &self.value {
            properties.insert("name".into(), name.clone());
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
            ContextValue::Directory(_) => "Directory",
            ContextValue::Application { name, .. } => name,
            ContextValue::Extension(_) => &self.type_id,
        };
        ContextPresentation {
            id: self.id.clone(),
            type_id: self.type_id.clone(),
            label: label.into(),
            detail: self.value.text(),
            source: self.source,
        }
    }
}
