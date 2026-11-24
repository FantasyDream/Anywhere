//! Build and publish an already revalidated snapshot while the session is held.
use super::*;
use crate::{application::snapshot::PreparedSnapshot, core::search::candidate::Candidate};

pub(super) struct SnapshotCommit {
    pub session_id: SessionId,
    pub revision: Revision,
    pub contexts: Vec<ResolvedContext>,
    pub facts: crate::core::session::facts::SessionFacts,
    pub selected_scope: Option<String>,
    pub resolve_complete: bool,
    pub action_catalog_version: u64,
    pub started: Instant,
}
impl Resolver {
    pub(super) fn publish_prepared(
        &self,
        session: &mut LauncherSession,
        prepared: PreparedSnapshot,
        commit: SnapshotCommit,
    ) -> Result<LauncherState> {
        let SnapshotCommit {
            session_id,
            revision,
            contexts,
            facts,
            selected_scope,
            resolve_complete,
            action_catalog_version,
            started,
        } = commit;
        crate::features::remember_environment(&mut session.resolution.context_cache, &contexts);
        session.resolution.snapshot_seq = session.resolution.snapshot_seq.saturating_add(1);
        let sequence = session.resolution.snapshot_seq;
        let summary = prepared.summary;
        let late_contexts = summary
            .len()
            .saturating_sub(session.resolution.published_contexts);
        session.resolution.published_contexts = summary.len();
        let mut scopes = prepared.scopes;
        // Current App has a reserved first slot but is never displayed without
        // visible actions. Other late groups only append to the session order.
        if session.resolution.frozen_scope_ids.is_empty() {
            if let Some(app) = &facts.frontmost {
                session
                    .resolution
                    .frozen_scope_ids
                    .push(crate::core::action::group::groups::for_app(&app.bundle_id));
            }
        }
        crate::core::search::scope::freeze(&mut scopes, &mut session.resolution.frozen_scope_ids);
        let mut state = LauncherState {
            session_id,
            revision,
            snapshot_seq: sequence,
            input: session.state.input.clone(),
            context_complete: resolve_complete,
            result_actions: presentations(&prepared.result_actions, &session.state.result_actions),
            search_sources: session.resolution.search_sources.clone(),
            file_search: session.resolution.file_search.clone().map(Box::new),
            context_summary: summary,
            context_candidates: presentations(
                &prepared.context_candidates,
                &session.state.context_candidates,
            ),
            preferred_context_key: prepared.preferred_context_key,
            scopes,
            selected_scope,
            context_captures: session
                .identity
                .captures
                .iter()
                .map(|capture| ContextCapturePresentation {
                    text: if capture.status == ContextCaptureStatus::Captured {
                        capture.contexts.iter().find_map(|ctx| match &ctx.value {
                            crate::core::context::ContextValue::Text(text) => {
                                Some(text.to_string())
                            }
                            _ => None,
                        })
                    } else {
                        None
                    },
                    clipboard_age_ms: if capture.source == ContextSource::Clipboard {
                        session.identity.clipboard_age_ms
                    } else {
                        None
                    },
                    source: capture.source,
                    status: capture.status,
                    captured_at_ms: capture.captured_at_ms,
                })
                .collect(),
            candidates: presentations(&prepared.set.candidates, &session.state.candidates),
            candidate_list_version: session.state.candidate_list_version,
            diagnostics: {
                let mut diagnostics = prepared.diagnostics;
                for item in &session.state.diagnostics {
                    if !diagnostics.contains(item) {
                        diagnostics.push(item.clone());
                    }
                }
                diagnostics
            },
        };
        if !contexts.is_empty() {
            let base = self.base_registry();
            let options = prepared.shortcut_catalog;
            let mut catalog =
                crate::core::sync::lock_cache(&self.shortcut_catalog, "shortcut_catalog");
            *catalog = options
                .into_iter()
                .filter(|a| {
                    base.get(&a.action_id).is_none()
                        && base.supports_persistent_binding(&a.action_id)
                })
                .take(256)
                .collect();
        }
        if !Arc::ptr_eq(&state.candidates, &session.state.candidates) {
            let append = session.input_mode == InputMode::FileSearch
                && state.candidates.starts_with(&session.state.candidates);
            if !append || session.state.candidate_list_version == 0 {
                state.candidate_list_version = session
                    .state
                    .candidate_list_version
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("candidate list version exhausted"))?;
            }
        }
        session.resolution.committed_revision = Some(revision);
        session.resolution.committed_contexts = contexts;
        session.execution.file_candidate_indices.clear();
        if session.input_mode == InputMode::FileSearch {
            session.execution.file_candidates = prepared.set.candidates;
            session.execution.file_candidate_indices = session
                .execution
                .file_candidates
                .iter()
                .enumerate()
                .map(|(index, candidate)| (candidate.presentation.id.clone().into(), index))
                .collect();
            session.execution.candidates = prepared
                .context_candidates
                .into_iter()
                .chain(prepared.result_actions)
                .chain(prepared.host_actions)
                .map(|candidate| (candidate.presentation.id.clone().into(), candidate))
                .collect();
        } else {
            session.execution.file_candidates = Vec::new();
            session.execution.candidates = prepared
                .set
                .candidates
                .into_iter()
                .chain(prepared.context_candidates)
                .chain(prepared.result_actions)
                .chain(prepared.host_actions)
                .map(|candidate| (candidate.presentation.id.clone().into(), candidate))
                .collect();
        }
        session.execution.revision = Some(revision);
        session.execution.action_catalog_version = Some(action_catalog_version);
        // `first_snapshot` marks the snapshot the caller's budget produced; its
        // duration is the spec's `first_snapshot_duration`.
        let is_first_snapshot = sequence == 1;
        tracing::info!(
            session_id,
            revision,
            snapshot_seq = sequence,
            complete = resolve_complete,
            first_snapshot = is_first_snapshot,
            candidates = state.candidates.len(),
            contexts = state.context_summary.len(),
            top_candidate_kinds = ?state
                .candidates
                .iter()
                .take(6)
                .map(|candidate| candidate.kind)
                .collect::<Vec<_>>(),
            // Ranked head, so a ranking question can be answered from the log
            // instead of by reproducing the exact usage history by hand.
            top_candidates = ?state
                .candidates
                .iter()
                .take(6)
                .map(|candidate| format!("{}={:.0}", if candidate.kind == CandidateKind::File { "file" } else { &candidate.title }, candidate.score))
                .collect::<Vec<_>>(),
            late_context_count = late_contexts,
            first_snapshot_duration = if is_first_snapshot {
                started.elapsed().as_micros() as u64
            } else {
                0
            },
            duration_us = started.elapsed().as_micros() as u64,
            "snapshot_committed"
        );
        // The matcher times itself, so this is the candidate work alone and not the
        // session lock or the snapshot bookkeeping around it. The application index is
        // reported separately: matching installed applications is the same kind of work as
        // matching actions, and folding it into `ranking_duration` made that number
        // describe sorting when it was really measuring a name search.
        tracing::info!(
            session_id,
            revision,
            matcher_duration = prepared.set.matcher_us,
            sources_duration = prepared.set.sources_us,
            "matcher_completed"
        );
        tracing::info!(
            session_id,
            revision,
            ranking_duration = prepared.set.ranking_us,
            "ranking_completed"
        );
        session.state = state.clone();
        let shared = Arc::new(state);
        session.published_state = Some(shared.clone());
        self.publish_shared_state(session, shared.clone());

        Ok((*shared).clone())
    }
    /// The caller holds the session and policy guards through publication.
    pub(in crate::application) fn publish_state(&self, session: &mut LauncherSession) {
        debug_assert!(self.is_current(session.state.session_id, session.state.revision));
        let shared = Arc::new(session.state.clone());
        session.published_state = Some(shared.clone());
        self.latest_tx.send_replace(Some(shared));
    }

    pub(in crate::application) fn publish_shared_state(
        &self,
        session: &LauncherSession,
        shared: Arc<LauncherState>,
    ) {
        debug_assert!(self.is_current(session.state.session_id, session.state.revision));
        self.latest_tx.send_replace(Some(shared));
    }
}

/// Presentation identity only. Execution targets and guards always come from
/// the newly validated candidates, even when every displayed field is equal.
fn presentations(
    candidates: &[Candidate],
    previous: &Arc<Vec<CandidatePresentation>>,
) -> Arc<Vec<CandidatePresentation>> {
    if candidates.len() == previous.len()
        && candidates
            .iter()
            .map(|candidate| &candidate.presentation)
            .eq(previous.iter())
    {
        return Arc::clone(previous);
    }
    Arc::new(
        candidates
            .iter()
            .map(|candidate| candidate.presentation.clone())
            .collect(),
    )
}
