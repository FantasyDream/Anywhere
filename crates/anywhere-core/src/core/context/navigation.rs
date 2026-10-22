//! Graph identities and their presentation. Navigation references existing context roots.
use super::ResolvedContext;
use anywhere_protocol::APPLICATION;

pub fn key(context: &ResolvedContext) -> String {
    format!(
        "context:{:?}:{}:{:016x}",
        context.source,
        context.type_id,
        context.value.identity_fingerprint()
    )
}

/// Application provenance does not make its directory the application action target.
pub fn root<'a>(
    context: &'a ResolvedContext,
    contexts: &'a [ResolvedContext],
) -> &'a ResolvedContext {
    let mut current = context;
    for _ in 0..contexts.len() {
        let Some(parent) = current
            .source_context_id
            .as_deref()
            .and_then(|id| contexts.iter().find(|item| !id.is_empty() && item.id == id))
        else {
            break;
        };
        if parent.type_id == APPLICATION && current.type_id != APPLICATION {
            break;
        }
        current = parent;
    }
    current
}

pub fn root_key(context: &ResolvedContext, contexts: &[ResolvedContext]) -> String {
    key(root(context, contexts))
}
