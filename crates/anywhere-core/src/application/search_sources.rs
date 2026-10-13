//! Independent generic source orchestration. Domain workspaces adapt their own queries.
use super::{session_runtime::Resolver, Application};
use crate::core::search::candidate_source::{AsyncCandidateBatch, AsyncSearchRequest, BatchUpdate};
use anywhere_protocol::*;
use std::{collections::HashSet, sync::Arc};
impl Application {
    pub(super) fn spawn_search_sources(
        &self,
        session_id: SessionId,
        revision: Revision,
        input: &str,
        scope: Option<&str>,
        mode: InputMode,
    ) {
        if scope.is_some() || mode == InputMode::Content {
            return;
        }
        let sources: Vec<_> = self
            .resolver
            .sources
            .lock()
            .expect("sources lock poisoned")
            .asynchronous()
            .into_iter()
            .filter(|s| s.accepts_mode(mode) && s.accepts(input))
            .collect();
        // Product workspace dispatch lives at the Application boundary, never in
        // the generic runner, request or publication protocol.
        if mode == InputMode::FileSearch {
            self.spawn_file_search_sources(session_id, revision, input, sources);
            return;
        }
        mark_sources_loading(
            &self.resolver,
            session_id,
            revision,
            &sources
                .iter()
                .map(|s| SearchSourcePresentation {
                    id: s.id().into(),
                    state: SearchSourceState::Loading,
                })
                .collect::<Vec<_>>(),
        );
        for source in sources {
            let id = source.id();
            let resolver = self.resolver.clone();
            super::source_runtime::spawn_source(
                source,
                AsyncSearchRequest {
                    text: input.into(),
                    options: None,
                },
                self.resolver.execution(session_id, revision),
                self.resolver.source_workers.clone(),
                Arc::new(|| true),
                Arc::new(move |batch| {
                    commit_plain_batch(&resolver, session_id, revision, id, batch)
                }),
            );
        }
    }
}
pub(super) fn mark_sources_loading(
    resolver: &Resolver,
    session_id: SessionId,
    revision: Revision,
    states: &[SearchSourcePresentation],
) {
    let Ok(mut session) = resolver.session.lock() else {
        return;
    };
    if !resolver.is_current(session_id, revision) {
        return;
    }
    session.resolution.search_sources = states.to_vec();
    session.note_snapshot_inputs();
}
fn commit_plain_batch(
    resolver: &Resolver,
    session_id: SessionId,
    revision: Revision,
    id: &str,
    batch: AsyncCandidateBatch,
) {
    let committed = {
        let Ok(mut session) = resolver.session.lock() else {
            return;
        };
        if !resolver.is_current(session_id, revision)
            || session.state.session_id != session_id
            || session.state.revision != revision
        {
            return;
        }
        let rows = session
            .resolution
            .source_candidates
            .entry(id.into())
            .or_default();
        match batch.update {
            BatchUpdate::Replace => {
                *rows = batch
                    .candidates
                    .into_iter()
                    .take(policy::MAX_SEARCH_SOURCE_ROWS as usize)
                    .collect()
            }
            BatchUpdate::Append => {
                let mut ids: HashSet<_> = rows.iter().map(|c| c.presentation.id.clone()).collect();
                for c in batch.candidates {
                    if rows.len() >= policy::MAX_SEARCH_SOURCE_ROWS as usize {
                        break;
                    }
                    if ids.insert(c.presentation.id.clone()) {
                        rows.push(c);
                    }
                }
            }
        }
        session.resolution.search_candidates = session
            .resolution
            .source_candidates
            .values()
            .flatten()
            .cloned()
            .collect();
        if let Some(s) = session
            .resolution
            .search_sources
            .iter_mut()
            .find(|s| s.id == id)
        {
            s.state = batch.state;
        }
        session.note_snapshot_inputs();
        session.resolution.committed_revision == Some(revision)
    };
    if committed {
        resolver.recompute_current(session_id);
    }
}
