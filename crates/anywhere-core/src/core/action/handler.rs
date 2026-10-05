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
    pub process: Option<anywhere_platform::process::ProcessRequest>,
}

impl ExecutionOutput {
    pub fn dismiss() -> Self {
        Self {
            text: None,
            close_window: true,
            process: None,
        }
    }

    pub fn preview(text: String) -> Self {
        Self {
            text: Some(text),
            close_window: false,
            process: None,
        }
    }

    pub fn process(request: anywhere_platform::process::ProcessRequest) -> Self {
        Self {
            text: None,
            close_window: true,
            process: Some(request),
        }
    }
}

pub trait ActionHandler: Send + Sync {
    fn binding_policy(&self) -> super::identity::BindingPolicy {
        super::identity::BindingPolicy::Persistent
    }
    /// Unverified dynamic identities must neither consume nor produce persistent usage.
    fn records_usage(&self) -> bool {
        true
    }

    /// A capability for this discovered target, independent of its stable business ID.
    fn execution_token(&self) -> Option<u64> {
        None
    }

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
    pub fn binding_policy(&self) -> super::identity::BindingPolicy {
        self.handler().binding_policy()
    }
    fn handler(&self) -> &dyn ActionHandler {
        match self {
            Self::Static(handler) => *handler,
            Self::Owned(handler) => handler.as_ref(),
        }
    }

    pub fn records_usage(&self) -> bool {
        self.handler().records_usage()
    }

    pub fn execution_token(&self) -> Option<u64> {
        self.handler().execution_token()
    }

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
