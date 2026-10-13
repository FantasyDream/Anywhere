//! Bridge executions adapter; exported names remain in the api facade.
use super::*;

pub(super) fn execution_sessions() -> Result<Vec<ExecutionSessionSummaryDto>> {
    Ok(application()?
        .execution_sessions()
        .into_iter()
        .map(Into::into)
        .collect())
}

pub(super) fn execution_session(id: String) -> Result<Option<ExecutionSessionDetailDto>> {
    Ok(application()?
        .execution_session(&id)
        .map(|detail| ExecutionSessionDetailDto {
            summary: detail.summary.into(),
            stdout: detail.stdout,
            stderr: detail.stderr,
        }))
}

pub(super) fn stop_execution_session(id: String) -> Result<()> {
    application()?.stop_execution_session(&id)
}

pub(super) fn remove_execution_session(id: String) -> Result<()> {
    application()?.remove_execution_session(&id)
}

pub(super) fn shutdown_execution_sessions() -> Result<()> {
    application()?.shutdown_execution_sessions();
    Ok(())
}
