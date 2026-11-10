//! Blocking execution and handoff to independent Web/process lifetimes.
use super::{ExecutionOutcome, ExecutionRequest};
use anyhow::{bail, Context, Result};
use std::sync::atomic::Ordering;

impl ExecutionRequest {
    pub fn spawn(self) -> tokio::task::JoinHandle<Result<ExecutionOutcome>> {
        tokio::task::spawn_blocking(move || self.run())
    }

    fn run(self) -> Result<ExecutionOutcome> {
        let candidate = &self.candidate;
        let resolver = &self.resolver;
        let execution_ticket = &self.execution_ticket;
        let session_id = execution_ticket.session_id;
        let revision = execution_ticket.revision;
        let epoch = &resolver.execution_epoch;
        let catalog = &resolver.catalog;
        let history = &resolver.history;
        let session_for_execution = &resolver.session;
        let guarded_target = candidate.target.execution_token();
        let records_usage = candidate.target.records_usage();
        // The opening gate ran when the request arrived; this one runs when the worker
        // actually starts. A blocking worker can wait behind other work, and the user
        // can dismiss the launcher in that gap — acting then would open a window, or
        // overwrite the clipboard, for a session that no longer exists.
        if !self.is_current() {
            bail!("action expired before it started");
        }
        if let Some(token) = guarded_target {
            let session = session_for_execution
                .lock()
                .map_err(|_| anyhow::anyhow!("session lock poisoned"))?;
            if !execution_ticket.is_current()
                || epoch.load(Ordering::Acquire) != self.execution_epoch
                || session.state.session_id != session_id
                || session.state.revision != revision
                || session
                    .actions
                    .get(self.record_id.as_str())
                    .and_then(|action| action.executor.execution_token())
                    != Some(token)
            {
                bail!("menu discovery expired; choose a current action");
            }
        }
        candidate.validate()?;
        let mut output = match &candidate.target {
            crate::core::search::candidate::CandidateTarget::Action { action, context } => {
                if crate::features::filesystem::search::is_search_target(candidate) {
                    if !self
                        .file_policy
                        .read()
                        .is_ok_and(|p| p.enabled && p.generation == self.file_generation)
                    {
                        bail!("file search settings changed; search again");
                    }
                    crate::features::filesystem::search::validate_target(context)?;
                }
                crate::core::action::execute::execute(
                    &action.executor,
                    context,
                    self.platform.as_ref(),
                    &self.registry,
                )?
            }
            crate::core::search::candidate::CandidateTarget::LaunchApplication { path, .. } => {
                crate::features::applications::launch::execute(path, self.platform.as_ref())?
            }
        };
        if let Some(command) = output.web_command.take() {
            candidate.validate()?;
            // Mint the invocation under the same lock used by hide/activation/catalog
            // publication. A queued worker cannot recreate a ticket after revocation.
            let session = session_for_execution
                .lock()
                .map_err(|_| anyhow::anyhow!("session unavailable"))?;
            if session.suspended || !self.is_current() {
                bail!("Web action expired before opening");
            }
            let context = candidate
                .context()
                .context("Web action has no invocation input")?;
            output.view = Some(
                self.web_plugins
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Web service unavailable"))?
                    .start(
                        &command,
                        context,
                        self.captured_app.as_ref(),
                        &self.source_app,
                    )?,
            );
        }
        // Planning a process is still side-effect free. Revalidate just before spawn;
        // the newly started process then has its own lifetime and identity.
        let execution_session_id = if let Some(request) = output.process.take() {
            if !execution_ticket.is_current()
                || epoch.load(Ordering::Acquire) != self.execution_epoch
                || catalog.version() != self.catalog_version
            {
                bail!("action expired before the task started");
            }
            candidate.validate()?;
            Some(self.executions.start(request)?)
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
                        self.record_id.as_str(),
                        &self.context_type,
                        &self.source_app,
                        crate::application::session_runtime::now(),
                    )
                })
        } else {
            Ok(())
        };
        Ok(ExecutionOutcome {
            output,
            history_result,
            execution_session_id,
        })
    }
}
