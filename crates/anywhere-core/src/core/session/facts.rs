//! Read-only environment an availability rule or an ownership rule may inspect.
//!
//! Captured once per session, before the host takes focus. Nothing here does IO: the
//! clipboard was already snapshotted, Git was already probed, and the foreground
//! application was already frozen.
use crate::core::context::{ContextSource, ResolvedContext};
use anywhere_platform::ApplicationInfo;

#[derive(Debug, Clone, Default)]
pub struct SessionFacts {
    pub clipboard_text: Option<String>,
    pub git_available: bool,
    pub frontmost: Option<ApplicationInfo>,
}

impl SessionFacts {
    pub fn clipboard_matches(&self, value: &str) -> bool {
        self.clipboard_text.as_deref() == Some(value)
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
        ContextSource::ForegroundApplication | ContextSource::ApplicationContext => match facts
            .frontmost
            .as_ref()
            .filter(|app| !app.bundle_id.is_empty())
        {
            Some(application) => Ownership::App(application.bundle_id.clone()),
            None => Ownership::Ambient,
        },
        ContextSource::UserInput => Ownership::Ambient,
    }
}
