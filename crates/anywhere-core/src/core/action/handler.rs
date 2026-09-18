//! What an action does when the user runs it.
//!
//! Visibility is a closed [`super::availability::AvailabilityRule`] on the spec, not code
//! on the handler. Custom actions will reuse that rule set; this trait is only the
//! first-party side effect.
use crate::core::context::ResolvedContext;
use anyhow::Result;
use anywhere_platform::Platform;

pub struct ExecutionOutput {
    pub text: Option<String>,
    pub close_window: bool,
}

impl ExecutionOutput {
    pub fn dismiss() -> Self {
        Self {
            text: None,
            close_window: true,
        }
    }

    pub fn preview(text: String) -> Self {
        Self {
            text: Some(text),
            close_window: false,
        }
    }
}

pub trait ActionHandler: Send + Sync {
    fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput>;
}

#[derive(Clone)]
pub enum ActionHandlerRef {
    Static(&'static dyn ActionHandler),
    Owned(std::sync::Arc<dyn ActionHandler>),
}

impl ActionHandlerRef {
    pub const fn new(handler: &'static dyn ActionHandler) -> Self {
        Self::Static(handler)
    }

    pub fn owned(handler: impl ActionHandler + 'static) -> Self {
        Self::Owned(std::sync::Arc::new(handler))
    }

    pub fn execute(
        &self,
        context: &ResolvedContext,
        platform: &dyn Platform,
    ) -> Result<ExecutionOutput> {
        match self {
            Self::Static(handler) => handler.execute(context, platform),
            Self::Owned(handler) => handler.execute(context, platform),
        }
    }
}

impl std::fmt::Debug for ActionHandlerRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ActionHandler")
    }
}
