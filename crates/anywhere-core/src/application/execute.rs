//! Execution: run one committed candidate.
//
// Currency is checked twice, both times before a side effect: once when the request arrives,
// and once when the blocking worker actually starts. It is deliberately not checked
// afterwards — by then refusing cannot prevent anything and would only lose the usage record
// of an action the user watched succeed.
use super::launcher_service::Application;
use crate::core::action::executor as execution;
use anyhow::{bail, Context, Result};
use anywhere_protocol::*;

impl Application {
    pub(super) async fn execute_use_case(
        &self,
        session_id: SessionId,
        revision: Revision,
        candidate_id: ActionId,
    ) -> Result<ActionResult> {
        let (candidate, source_app) = {
            let session = self.session()?;
            if session.state.session_id != session_id
                || self
                    .resolver
                    .current
                    .load(std::sync::atomic::Ordering::Acquire)
                    != crate::core::context::engine::stamp(session_id, session.state.revision)
                || session.state.revision != revision
            {
                bail!("stale session");
            }
            // The revision is current, but its candidates may not be published yet:
            // `update_input` makes the new revision current before it resolves, so the
            // map still holds the previous revision's candidates. They carry the previous
            // context and execution target, so serving them here would execute an action
            // that belongs to an input the user has already replaced.
            if session.candidates_revision != Some(revision) {
                bail!("action expired; choose a current action");
            }
            let candidate = session
                .state_candidates
                .get(&candidate_id)
                .cloned()
                .context("action expired; choose a current action")?;
            if !candidate.presentation.availability.is_executable() {
                bail!(
                    "{}",
                    candidate
                        .presentation
                        .availability
                        .reason
                        .clone()
                        .unwrap_or_else(|| "action is not available".into())
                );
            }
            (candidate, session.source_app.clone())
        };
        let platform = self.platform.clone();
        let registry = self.resolver.registry.clone();
        let history = self.resolver.history.clone();
        // Handed to the worker so it can re-check currency when it actually starts running,
        // rather than trusting a check made while the request was still being queued.
        let execution_ticket = self.resolver.execution(session_id, revision);
        let record_id = candidate
            .presentation
            .command_id
            .clone()
            .or_else(|| {
                candidate
                    .presentation
                    .bundle_id
                    .as_ref()
                    .map(|bundle| crate::core::ranking::usage::application_usage_id(bundle))
            })
            .context("candidate has no executable identity")?;
        let context_type = candidate
            .context
            .as_ref()
            .map(|context| context.type_id.clone())
            .unwrap_or_else(|| crate::core::ranking::usage::APPLICATION_CONTEXT_TYPE.to_owned());
        tracing::info!(
            session_id,
            revision,
            candidate_id,
            command_id = record_id,
            "action_execution_started"
        );
        let result = tokio::task::spawn_blocking(move || {
            // The opening gate ran when the request arrived; this one runs when the worker
            // actually starts. A blocking worker can wait behind other work, and the user
            // can dismiss the launcher in that gap — acting then would open a window, or
            // overwrite the clipboard, for a session that no longer exists.
            if !execution_ticket.is_current() {
                bail!("action expired before it started");
            }
            let output = match (
                &candidate.action,
                &candidate.context,
                &candidate.application,
            ) {
                (Some(action), Some(context), _) => {
                    execution::execute(&action.executor, context, platform.as_ref(), &registry)?
                }
                (None, None, Some(application)) => {
                    if !platform.launch_application(&application.path)? {
                        bail!("could not launch {}", application.path);
                    }
                    execution::ExecutionOutput {
                        text: None,
                        close_window: true,
                    }
                }
                _ => bail!("candidate cannot be executed"),
            };
            // No currency re-check *after* the side effect. By then refusing cannot prevent
            // anything; it only loses the usage record. Every action that closes the window
            // ends its own session (the host dismisses on focus loss), so checking here
            // would fail exactly the successful executions whose frequency the ranking
            // needs, and report "expired" for an action the user watched succeed. Currency
            // is enforced before the side effect: once when the request arrives, once when
            // the worker starts.
            let history_result = history
                .lock()
                .map_err(|_| anyhow::anyhow!("history lock poisoned"))
                .and_then(|mut history| {
                    history.record(
                        &record_id,
                        &context_type,
                        &source_app,
                        crate::core::session::manager::now(),
                    )
                });
            Ok::<_, anyhow::Error>((output, history_result))
        })
        .await?;
        match result {
            Ok((output, history_result)) => {
                let mut message = "Done".to_owned();
                let mut close_window = output.close_window;
                if let Err(error) = history_result {
                    message = format!("Action completed, but usage could not be saved: {error:#}");
                    close_window = false;
                    tracing::error!(session_id, revision, error = %error, "usage_record_failed");
                }
                tracing::info!(session_id, revision, "action_execution_completed");
                Ok(ActionResult {
                    session_id,
                    revision,
                    success: true,
                    message,
                    output_text: output.text,
                    close_window,
                })
            }
            Err(error) => {
                tracing::error!(session_id, revision, error = %error, "action_execution_failed");
                Ok(ActionResult {
                    session_id,
                    revision,
                    success: false,
                    message: format!("{error:#}"),
                    output_text: None,
                    close_window: false,
                })
            }
        }
    }
}
