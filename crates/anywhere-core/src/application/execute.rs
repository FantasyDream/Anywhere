//! Execution: run one committed candidate.
//
// Currency is checked twice, both times before a side effect: once when the request arrives,
// and once when the blocking worker actually starts. It is deliberately not checked
// afterwards — by then refusing cannot prevent anything and would only lose the usage record
// of an action the user watched succeed.
use super::{
    execution_request::{ExecutionOutcome, ExecutionRequest},
    launcher_service::Application,
};
use anyhow::Result;
use anywhere_protocol::*;

impl Application {
    pub(super) async fn execute_use_case(
        &self,
        session_id: SessionId,
        revision: Revision,
        candidate_id: CandidateId,
    ) -> Result<ActionResult> {
        let request = ExecutionRequest::prepare(self, session_id, revision, candidate_id)?;
        let source_bundle = request.source_bundle.clone();
        let menu_generation = request.menu_generation;
        let result = request.spawn().await?;
        let result = async {
            let mut completed = result?;
            if self
                .run_headless_web_action(completed.output.view.as_ref(), session_id, revision)
                .await?
            {
                completed.output.view = None;
                completed.output.close_window = true;
            }
            Ok::<_, anyhow::Error>(completed)
        }
        .await;
        match result {
            Ok(ExecutionOutcome {
                output,
                history_result,
                execution_session_id,
            }) => {
                let (warning, close_window) = match history_result {
                    Ok(()) => (None, output.close_window),
                    Err(error) => {
                        tracing::error!(session_id, revision, error = %error, "usage_record_failed");
                        (
                            Some(format!(
                                "Action completed, but usage could not be saved: {error:#}"
                            )),
                            false,
                        )
                    }
                };
                tracing::info!(session_id, revision, "action_execution_completed");
                Ok(ActionResult {
                    session_id,
                    revision,
                    status: ActionStatus::Succeeded,
                    error: None,
                    warning,
                    output_text: output.text,
                    view: output.view,
                    close_window,
                    execution_session_id,
                })
            }
            Err(error) => {
                if error.is::<crate::features::app_actions::handler::MenuExecutionFailure>() {
                    if let Some(bundle) = source_bundle.as_deref() {
                        self.app_actions.invalidate_through(bundle, menu_generation);
                    }
                    self.refresh_app_actions(session_id, Some(menu_generation));
                }
                tracing::error!(session_id, revision, error = %error, "action_execution_failed");
                Ok(ActionResult {
                    session_id,
                    revision,
                    status: ActionStatus::Failed,
                    error: Some(format!("{error:#}")),
                    warning: None,
                    output_text: None,
                    view: None,
                    close_window: false,
                    execution_session_id: None,
                })
            }
        }
    }
}
