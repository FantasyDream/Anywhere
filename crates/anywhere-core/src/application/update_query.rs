//! Query updates: resolve a new revision and publish it.
//
// A revision becomes current the moment its input arrives, which is what lets the host keep
// typing while the previous revision is still resolving.
use super::launcher_service::{Application, MAX_INPUT_BYTES};
use anyhow::{bail, Result};
use anywhere_protocol::*;
use std::time::Instant;
use tracing::Instrument;

impl Application {
    pub(super) async fn update_query_use_case(
        &self,
        session_id: SessionId,
        input: String,
        revision: Revision,
    ) -> Result<Option<LauncherState>> {
        if input.len() > MAX_INPUT_BYTES {
            bail!("input exceeds 1 MiB");
        }
        let start = Instant::now();
        let execution = {
            let mut session = self.session()?;
            if session.state.session_id != session_id
                || revision <= session.state.revision
                || self
                    .resolver
                    .current
                    .load(std::sync::atomic::Ordering::Acquire)
                    == 0
            {
                return Ok(None);
            }
            // A superseded request never publishes again.
            session.state.revision = revision;
            session.state.input = input.clone();
            session.state.context_complete = false;
            session.committed_revision = None;
            session.snapshot_seq = 0;
            session.published_contexts = 0;
            self.resolver.current.store(
                crate::core::context::engine::stamp(session_id, revision),
                std::sync::atomic::Ordering::Release,
            );
            // Build seeds from the guard we already hold; `seeds_for` would deadlock here.
            crate::core::session::manager::seeds_from(&session, &input)
        };
        tracing::info!("input_revision_changed");
        tracing::info!(
            session_id,
            revision,
            duration_us = start.elapsed().as_micros() as u64,
            contexts = execution.len(),
            "input_context_resolved"
        );
        let execution_context = self.resolver.execution(session_id, revision);
        let run = self.resolver.build_run(&execution_context, execution);
        let state = async {
            let Some(resolution) = self
                .resolver
                .resolve_until_with_grace(session_id, run, self.budget, self.content_grace, start)
                .await
            else {
                return Ok(None);
            };
            let complete_by_budget = resolution.complete;
            let late_run = resolution.run;
            let mut diagnostics = self.startup_diagnostics.clone();
            diagnostics.extend(resolution.diagnostics);
            let committed = self.resolver.commit(
                session_id,
                revision,
                resolution.contexts,
                diagnostics.clone(),
                complete_by_budget,
                start,
            )?;
            if committed.is_some() {
                tracing::info!(
                    session_id,
                    revision,
                    duration_us = start.elapsed().as_micros() as u64,
                    "input_actions_ready"
                );
            }
            if !complete_by_budget {
                self.spawn_completion(session_id, revision, late_run, start, diagnostics);
            }
            Ok(committed)
        }
        .instrument(tracing::info_span!("update_input", session_id, revision))
        .await;
        state
    }
}
