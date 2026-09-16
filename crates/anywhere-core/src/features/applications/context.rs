//! What an application looks like as a context.
use crate::core::context::{ContextSource, ContextValue, ResolvedContext};
use anywhere_platform::ApplicationInfo;
use anywhere_protocol::APPLICATION;

/// Activation root context. Captured by the caller before any host is told to show.
pub fn frontmost_context(application: &ApplicationInfo) -> ResolvedContext {
    ResolvedContext::new(
        APPLICATION,
        ContextValue::Application {
            bundle_id: application.bundle_id.clone(),
            name: application.name.clone(),
        },
        ContextSource::ForegroundApplication,
        "platform.frontmost_application",
    )
}
