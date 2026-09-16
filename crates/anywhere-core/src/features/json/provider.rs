//! JSON recognition. Parses the raw string; the parsed value is what the JSON actions use.
use crate::core::context::provider::{ContextProvider, ProviderDescriptor};
use crate::core::context::{ContextSource, ContextValue, ExecutionContext, ResolvedContext};
use anyhow::Result;
use anywhere_protocol::*;
use async_trait::async_trait;

/// `core.text` to `json.document`.
pub struct JsonProvider {
    descriptor: ProviderDescriptor,
}

impl Default for JsonProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonProvider {
    pub fn new() -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: "core.json".into(),
                accepts: vec![TEXT.into()],
                produces: vec![JSON_DOCUMENT.into()],
                priority: 0,
            },
        }
    }
}

#[async_trait]
impl ContextProvider for JsonProvider {
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
        Ok(recognize(raw).into_iter().collect())
    }
}

pub fn recognize(input: &str) -> Option<ResolvedContext> {
    let parsed = serde_json::from_str(input).ok()?;
    Some(ResolvedContext::new(
        JSON_DOCUMENT,
        ContextValue::Json {
            raw: input.into(),
            parsed,
        },
        ContextSource::UserInput,
        "core.json",
    ))
}
