//! Bridge queries adapter; exported names remain in the api facade.
use super::*;

pub(super) fn ping() -> String {
    anywhere_core::ping().to_owned()
}

pub(super) fn begin_activation() -> Result<u32> {
    application()?.begin_activation()
}

pub(super) async fn activate(
    session_id: u32,
    prompt_accessibility: bool,
    captured_environment: bool,
) -> Result<Option<LauncherStateDto>> {
    Ok(application()?
        .activate_session_in_mode(
            session_id,
            prompt_accessibility,
            if captured_environment {
                protocol::InputMode::CapturedEnvironment
            } else {
                protocol::InputMode::Environment
            },
        )
        .await?
        .map(Into::into))
}

pub(super) async fn update_input(
    session_id: u32,
    input: String,
    revision: u32,
    scope_id: Option<String>,
    input_mode: InputModeDto,
) -> Result<Option<LauncherStateDto>> {
    Ok(application()?
        .update_context_input(
            session_id,
            input,
            revision,
            scope_id,
            match input_mode {
                InputModeDto::Environment => protocol::InputMode::Environment,
                InputModeDto::Content => protocol::InputMode::Content,
                InputModeDto::Search => protocol::InputMode::Search,
                InputModeDto::FileSearch => protocol::InputMode::FileSearch,
            },
        )
        .await?
        .map(Into::into))
}

pub(super) fn latest_state(session_id: u32) -> Option<LauncherStateDto> {
    application().ok()?.latest_state(session_id).map(Into::into)
}

pub(super) async fn wait_for_snapshot(session_id: u32, after: u64) -> Result<SnapshotWaitDto> {
    Ok(
        match application()?.wait_for_snapshot(session_id, after).await {
            anywhere_core::application::SnapshotWait::Updated(state) => SnapshotWaitDto {
                kind: SnapshotWaitKindDto::Updated,
                candidate_update: CandidateUpdateDto::Replace,
                state: Some((*state).into()),
            },
            anywhere_core::application::SnapshotWait::TimedOut => SnapshotWaitDto {
                kind: SnapshotWaitKindDto::TimedOut,
                candidate_update: CandidateUpdateDto::Replace,
                state: None,
            },
            anywhere_core::application::SnapshotWait::Closed => SnapshotWaitDto {
                kind: SnapshotWaitKindDto::Closed,
                candidate_update: CandidateUpdateDto::Replace,
                state: None,
            },
            anywhere_core::application::SnapshotWait::Superseded => SnapshotWaitDto {
                kind: SnapshotWaitKindDto::Superseded,
                candidate_update: CandidateUpdateDto::Replace,
                state: None,
            },
        },
    )
}

pub(super) async fn execute(
    session_id: u32,
    revision: u32,
    candidate_id: String,
) -> Result<ActionResultDto> {
    Ok(application()?
        .execute(session_id, revision, candidate_id)
        .await?
        .into())
}

pub(super) fn application_icon(candidate_id: String) -> Result<Option<Vec<u8>>> {
    let result = application()?.application_icon(&candidate_id);
    match &result {
        Ok(Some(bytes)) => tracing::debug!(candidate_id, bytes = bytes.len(), "app_icon_served"),
        Ok(None) => tracing::info!(candidate_id, "app_icon_absent"),
        Err(error) => tracing::error!(candidate_id, error = %error, "app_icon_failed"),
    }
    result
}

pub(super) async fn suspend_launcher(session_id: u32) -> Result<()> {
    application()?.suspend_session(session_id).await
}

pub(super) fn resume_launcher(session_id: u32) -> Result<Option<LauncherStateDto>> {
    Ok(application()?.resume_session(session_id)?.map(Into::into))
}

pub(super) fn deactivate(session_id: u32) -> Result<()> {
    application()?.deactivate(session_id)
}

pub(super) fn prompt_accessibility() -> Result<bool> {
    application()?.prompt_accessibility()
}

pub(super) fn choose_activation_input(
    captures: Vec<ContextCaptureDto>,
    now_ms: i64,
    selected_enabled: bool,
    clipboard_enabled: bool,
    allow_pending_selection: bool,
) -> Option<ActivationInputDto> {
    let captures: Vec<_> = captures
        .into_iter()
        .map(|c| {
            let source = match c.source {
                ContextSourceDto::UserInput => protocol::ContextSource::UserInput,
                ContextSourceDto::ForegroundApplication => {
                    protocol::ContextSource::ForegroundApplication
                }
                ContextSourceDto::ApplicationContext => protocol::ContextSource::ApplicationContext,
                ContextSourceDto::FinderSelection => protocol::ContextSource::FinderSelection,
                ContextSourceDto::SelectedText => protocol::ContextSource::SelectedText,
                ContextSourceDto::BrowserCurrentPage => protocol::ContextSource::BrowserCurrentPage,
                ContextSourceDto::Clipboard => protocol::ContextSource::Clipboard,
            };
            protocol::ContextCapturePresentation {
                source,
                text: c.text,
                clipboard_age_ms: c.clipboard_age_ms,
                captured_at_ms: c.captured_at_ms,
                status: if c.status == ContextCaptureStatusDto::Captured {
                    protocol::ContextCaptureStatus::Captured
                } else {
                    protocol::ContextCaptureStatus::Empty
                },
            }
        })
        .collect();
    anywhere_core::application::input_policy::choose_activation_input(
        &captures,
        now_ms,
        selected_enabled,
        clipboard_enabled,
        allow_pending_selection,
    )
    .map(|v| ActivationInputDto {
        source: context_source_dto(v.source),
        text: v.text,
    })
}

pub(super) async fn wait_for_snapshot_delta(
    session_id: u32,
    after: u64,
    candidate_list_version: u32,
    loaded_count: u32,
) -> Result<SnapshotWaitDto> {
    let notice = application()?.wait_for_snapshot(session_id, after).await;
    Ok(match notice {
        anywhere_core::application::SnapshotWait::Updated(state) => {
            snapshot_delivery(*state, candidate_list_version, loaded_count)
        }
        anywhere_core::application::SnapshotWait::TimedOut => SnapshotWaitDto {
            kind: SnapshotWaitKindDto::TimedOut,
            state: None,
            candidate_update: CandidateUpdateDto::Replace,
        },
        anywhere_core::application::SnapshotWait::Closed => SnapshotWaitDto {
            kind: SnapshotWaitKindDto::Closed,
            state: None,
            candidate_update: CandidateUpdateDto::Replace,
        },
        anywhere_core::application::SnapshotWait::Superseded => SnapshotWaitDto {
            kind: SnapshotWaitKindDto::Superseded,
            state: None,
            candidate_update: CandidateUpdateDto::Replace,
        },
    })
}
