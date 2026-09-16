//! Whether an applicable action should be shown, and whether it can run.
//!
//! Separate from `accepts`, which answers "does this action apply to this object at all".
//! `Copy Path` accepts a file or a directory; whether it is worth showing depends on what
//! is already on the clipboard. Conflating the two would make an action's applicability a
//! function of mutable session state.
//!
//! Memory only: no file system, no process, no network. The spec forbids availability from
//! doing IO of its own, so whether a repository still exists is left to execution, which
//! reports it as a normal failure.
use super::descriptor::CommandDescriptor;
use crate::core::context::{ContextValue, ResolvedContext};
use crate::core::session::facts::SessionFacts;
use anywhere_protocol::Availability;

/// Decide what the user sees for one applicable action.
pub fn evaluate(
    command: &CommandDescriptor,
    context: &ResolvedContext,
    facts: &SessionFacts,
) -> Availability {
    let path = context.value.path();
    match command.id.as_str() {
        // The clipboard already holds this exact value, so copying is a no-op.
        "core.directory.copy" | "core.file.copy" => {
            if let Some(path) = path {
                if facts.clipboard_matches(&path.to_string_lossy()) {
                    return Availability::hidden();
                }
            }
        }
        // Availability must use the same source of truth as execution dispatch.
        id if super::descriptor::is_git_action(id) && !facts.git_available => {
            return Availability::disabled("git is not installed");
        }
        _ => {}
    }
    Availability::enabled()
}

/// Contexts whose value is an object the user explicitly provided (a pasted URL, JSON
/// document or path) rather than ambient environment.
pub fn is_user_object(context: &ResolvedContext) -> bool {
    context.source == crate::core::context::ContextSource::UserInput
        && !matches!(context.value, ContextValue::Text(_))
}
