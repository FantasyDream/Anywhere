//! Execution: run one committed candidate.
//
// Currency is checked twice, both times before a side effect: once when the request arrives,
// and once when the blocking worker actually starts. It is deliberately not checked
// afterwards — by then refusing cannot prevent anything and would only lose the usage record
// of an action the user watched succeed.
use super::launcher_service::Application;
use anyhow::{bail, Context, Result};
use anywhere_protocol::*;

impl Application {
    pub(super) async fn execute_use_case(
        &self,
        session_id: SessionId,
        revision: Revision,
        candidate_id: CandidateId,
    ) -> Result<ActionResult> {
        let epoch = self.resolver.execution_epoch.clone();
        let execution_epoch = epoch.load(std::sync::atomic::Ordering::Acquire);
        let file_policy = self.file_search_policy.clone();
        let file_generation = file_policy
            .read()
            .map_err(|_| anyhow::anyhow!("search settings unavailable"))?
            .generation;
        let (
            candidate,
            source_app,
            captured_app,
            menu_generation,
            source_bundle,
            catalog_version,
            feature_policy,
        ) = {
            let session = self.session()?;
            if session.suspended
                || session.state.session_id != session_id
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
            let catalog_version = self
                .resolver
                .action_catalog_version
                .load(std::sync::atomic::Ordering::Acquire);
            if session.execution.revision != Some(revision)
                || session.execution.action_catalog_version != Some(catalog_version)
            {
                bail!("action expired; choose a current action");
            }
            let candidate = session
                .execution
                .candidates
                .get(candidate_id.as_str())
                .cloned()
                .context("action expired; choose a current action")?;
            let feature_policy = self.resolver.feature_policy.snapshot();
            if let crate::core::search::candidate::CandidateTarget::Action { action, .. } =
                &candidate.target
            {
                if action
                    .contributor()
                    .is_some_and(|id| !self.resolver.feature_policy.allows(id))
                {
                    bail!("feature is disabled; choose a current action");
                }
            }
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
            (
                candidate,
                session.identity.source_app.clone(),
                session.identity.frontmost.clone(),
                session.menu.generation,
                session
                    .identity
                    .frontmost
                    .as_ref()
                    .map(|app| app.bundle_id.clone()),
                catalog_version,
                feature_policy,
            )
        };
        let platform = self.platform.clone();
        let registry = self.resolver.base_registry();
        let history = self.resolver.history.clone();
        let executions = self.executions.clone();
        let session_for_execution = self.resolver.session.clone();
        let current_catalog_version = self.resolver.action_catalog_version.clone();
        let current_feature_policy = self.resolver.feature_policy.clone();
        let guarded_target = candidate.target.execution_token();
        let records_usage = candidate.target.records_usage();
        let web_plugins = self.web_plugins.clone();
        // Handed to the worker so it can re-check currency when it actually starts running,
        // rather than trusting a check made while the request was still being queued.
        let execution_ticket = self.resolver.execution(session_id, revision);
        let record_id = crate::core::action::identity::BusinessActionId::new(
            candidate
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
                .context("candidate has no executable identity")?,
        );
        let context_type = candidate
            .context()
            .map(|context| context.type_id.clone())
            .unwrap_or_else(|| crate::core::ranking::usage::APPLICATION_CONTEXT_TYPE.to_owned());
        tracing::info!(
            session_id,
            revision,
            candidate_id,
            command_id = record_id.as_str(),
            "action_execution_started"
        );
        let result = tokio::task::spawn_blocking(move || {
            // The opening gate ran when the request arrived; this one runs when the worker
            // actually starts. A blocking worker can wait behind other work, and the user
            // can dismiss the launcher in that gap — acting then would open a window, or
            // overwrite the clipboard, for a session that no longer exists.
            if !execution_ticket.is_current()
                || !current_feature_policy.is_current(&feature_policy)
                || epoch.load(std::sync::atomic::Ordering::Acquire) != execution_epoch
                || current_catalog_version.load(std::sync::atomic::Ordering::Acquire)
                    != catalog_version
            {
                bail!("action expired before it started");
            }
            if let Some(token) = guarded_target {
                let session = session_for_execution
                    .lock()
                    .map_err(|_| anyhow::anyhow!("session lock poisoned"))?;
                if !execution_ticket.is_current()
                    || epoch.load(std::sync::atomic::Ordering::Acquire) != execution_epoch
                    || session.state.session_id != session_id
                    || session.state.revision != revision
                    || session
                        .actions
                        .get(record_id.as_str())
                        .and_then(|action| action.executor.execution_token())
                        != Some(token)
                {
                    bail!("menu discovery expired; choose a current action");
                }
            }
            let mut output = match &candidate.target {
                crate::core::search::candidate::CandidateTarget::Action { action, context } => {
                    if crate::features::filesystem::search::is_search_target(&candidate) {
                        if !file_policy
                            .read()
                            .is_ok_and(|p| p.enabled && p.generation == file_generation)
                        {
                            bail!("file search settings changed; search again");
                        }
                        crate::features::filesystem::search::validate_target(context)?;
                    }
                    crate::core::action::execute::execute(
                        &action.executor,
                        context,
                        platform.as_ref(),
                        &registry,
                    )?
                }
                crate::core::search::candidate::CandidateTarget::LaunchApplication {
                    path, ..
                } => crate::features::applications::launch::execute(path, platform.as_ref())?,
            };
            if let Some(command) = output.web_command.take() {
                // Mint the invocation under the same lock used by hide/activation/catalog
                // publication. A queued worker cannot recreate a ticket after revocation.
                let session = session_for_execution
                    .lock()
                    .map_err(|_| anyhow::anyhow!("session unavailable"))?;
                if session.suspended
                    || !execution_ticket.is_current()
                    || !current_feature_policy.is_current(&feature_policy)
                    || epoch.load(std::sync::atomic::Ordering::Acquire) != execution_epoch
                    || current_catalog_version.load(std::sync::atomic::Ordering::Acquire)
                        != catalog_version
                {
                    bail!("Web action expired before opening");
                }
                let context = candidate
                    .context()
                    .context("Web action has no invocation input")?;
                output.view = Some(
                    web_plugins
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Web service unavailable"))?
                        .start(&command, context, captured_app.as_ref(), &source_app)?,
                );
            }
            // Planning a process is still side-effect free. Revalidate just before spawn;
            // the newly started process then has its own lifetime and identity.
            let execution_session_id = if let Some(request) = output.process.take() {
                if !execution_ticket.is_current()
                    || epoch.load(std::sync::atomic::Ordering::Acquire) != execution_epoch
                    || current_catalog_version.load(std::sync::atomic::Ordering::Acquire)
                        != catalog_version
                {
                    bail!("action expired before the task started");
                }
                Some(executions.start(request)?)
            } else {
                None
            };
            // No currency re-check *after* the side effect. By then refusing cannot prevent
            // anything; it only loses the usage record. Every action that closes the window
            // ends its own session (the host dismisses on focus loss), so checking here
            // would fail exactly the successful executions whose frequency the ranking
            // needs, and report "expired" for an action the user watched succeed. Currency
            // is enforced before the side effect: once when the request arrives, once when
            // the worker starts.
            let history_result = if records_usage {
                history
                    .lock()
                    .map_err(|_| anyhow::anyhow!("history lock poisoned"))
                    .and_then(|mut history| {
                        history.record(
                            record_id.as_str(),
                            &context_type,
                            &source_app,
                            crate::application::session_runtime::now(),
                        )
                    })
            } else {
                Ok(())
            };
            Ok::<_, anyhow::Error>((output, history_result, execution_session_id))
        })
        .await?;
        match result {
            Ok((output, history_result, execution_session_id)) => {
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
