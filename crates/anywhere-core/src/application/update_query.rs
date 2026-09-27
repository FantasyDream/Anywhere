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
                || self.resolver.is_abandoned()
            {
                return Ok(None);
            }
            session.begin_revision(revision, input.clone());
            self.resolver.mark_current(session_id, revision);
            crate::application::session_runtime::seeds_from(&session, &input)
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
        async {
            let committed = self
                .publish_resolution(
                    session_id,
                    revision,
                    run,
                    start,
                    self.startup_diagnostics.clone(),
                )
                .await?;
            if committed.is_some() {
                tracing::info!(
                    session_id,
                    revision,
                    duration_us = start.elapsed().as_micros() as u64,
                    "input_actions_ready"
                );
            }
            Ok(committed)
        }
        .instrument(tracing::info_span!("update_input", session_id, revision))
        .await
    }
}
