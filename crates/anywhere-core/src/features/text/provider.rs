//! The activation seed: a non-empty input is always preserved as `core.text`.
//!
//! It accepts nothing, because it is not derived from another context; the caller inserts it
//! as a seed. Every other recognizer reads this context rather than the raw string.
use crate::core::context::provider::{ContextProvider, ProviderDescriptor};
use crate::core::context::{ContextSource, ContextValue, ExecutionContext, ResolvedContext};
use anyhow::Result;
use anywhere_protocol::TEXT;
use async_trait::async_trait;

/// Emits the preserved raw text for any non-empty user input.
pub struct TextProvider {
    descriptor: ProviderDescriptor,
}
impl Default for TextProvider {
    fn default() -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: "core.text_provider".into(),
                accepts: vec![],
                produces: vec![TEXT.into()],
                priority: 0,
            },
        }
    }
}
#[async_trait]
impl ContextProvider for TextProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }
    async fn resolve(
        &self,
        input: &ResolvedContext,
        _: &ExecutionContext,
    ) -> Result<Vec<ResolvedContext>> {
        let ContextValue::Text(raw) = &input.value else {
            return Ok(vec![]);
        };
        Ok(text_context(raw).into_iter().collect())
    }
}

/// Convenience for callers that need the text seed without the full engine.
pub fn text_provider_contexts(raw: &str) -> Vec<ResolvedContext> {
    text_context(raw).into_iter().collect()
}

/// Always emit the raw text for a non-empty input.
pub fn text_context(input: &str) -> Option<ResolvedContext> {
    if input.trim().is_empty() {
        return None;
    }
    Some(ResolvedContext::new(
        TEXT,
        ContextValue::Text(input.into()),
        ContextSource::UserInput,
        "core.text_provider",
    ))
}
