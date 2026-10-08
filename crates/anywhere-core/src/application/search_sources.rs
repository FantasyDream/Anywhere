//! Debounced async sources enrich only the query that started them.
use super::Application;
use anywhere_protocol::{InputMode, Revision, SessionId};
use std::time::Duration;

impl Application {
    pub(super) fn spawn_search_sources(
        &self,
        session_id: SessionId,
        revision: Revision,
        input: &str,
        scope: Option<&str>,
        mode: InputMode,
    ) {
        if scope.is_some() || mode == InputMode::Content || self.resolver.async_sources.is_empty() {
            return;
        }
        let resolver = self.resolver.clone();
        let sources: Vec<_> = resolver
            .async_sources
            .iter()
            .filter(|source| source.accepts(input))
            .cloned()
            .collect();
        if sources.is_empty() {
            return;
        }
        let query = input.to_owned();
        let ticket = resolver.execution(session_id, revision);
        tokio::spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(150)) => {},
                _ = ticket.superseded() => return,
            }
            if !ticket.is_current() {
                return;
            }
            let mut candidates = Vec::new();
            for source in &sources {
                let result = tokio::select! {
                    result = tokio::time::timeout(Duration::from_secs(3), source.candidates(&query, ticket.clone())) => result.unwrap_or_else(|_| Err(anyhow::anyhow!("search source timed out"))),
                    _ = ticket.superseded() => return,
                };
                if !ticket.is_current() {
                    return;
                }
                match result {
                    Ok(rows) => candidates.extend(rows.into_iter().take(64)),
                    Err(_) => tracing::warn!(source = source.id(), "search_source_unavailable"),
                }
            }
            let committed = {
                let Ok(mut session) = resolver.session.lock() else {
                    return;
                };
                if !ticket.is_current()
                    || session.state.session_id != session_id
                    || session.state.revision != revision
                {
                    return;
                }
                session.resolution.search_candidates = candidates;
                session.note_snapshot_inputs();
                session.resolution.committed_revision == Some(revision)
            };
            if committed {
                resolver.recompute_current(session_id);
            }
        });
    }
}
