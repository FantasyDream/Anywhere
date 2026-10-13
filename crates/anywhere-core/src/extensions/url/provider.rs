//! URL recognition. Inspects the raw string only: no network, ever.
use crate::core::context::provider::{ContextProvider, ProviderDescriptor};
use crate::core::context::{ContextSource, ContextValue, ExecutionContext, ResolvedContext};
use anyhow::Result;
use anywhere_protocol::*;
use async_trait::async_trait;

/// `core.text` to `core.url`.
pub struct UrlProvider {
    descriptor: ProviderDescriptor,
}

impl Default for UrlProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl UrlProvider {
    pub fn new() -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: "core.url".into(),
                accepts: vec![TEXT.into()],
                produces: vec![URL.into()],
                priority: 10,
            },
        }
    }
}

#[async_trait]
impl ContextProvider for UrlProvider {
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
        Ok(url) if url.host_str().is_some() => Some(ResolvedContext::new(
            URL,
            ContextValue::Url(url),
            ContextSource::UserInput,
            "core.url",
        )),
        _ => None,
    }
}
