//! Bridge files adapter; exported names remain in the api facade.
use super::*;

pub(super) async fn update_file_input(
    session_id: u32,
    input: String,
    revision: u32,
    filter: String,
) -> Result<Option<LauncherStateDto>> {
    let filter = protocol::FileSearchFilter::from_wire(&filter).context("unknown file type")?;
    Ok(application()?
        .update_file_search_input(session_id, input, revision, filter)
        .await?
        .map(Into::into))
}

pub(super) fn file_actions(
    session_id: u32,
    revision: u32,
    candidate_id: String,
) -> Result<Vec<CandidateDto>> {
    Ok(application()?
        .file_actions(session_id, revision, candidate_id)?
        .into_iter()
        .map(candidate_dto)
        .collect())
}

pub(super) async fn load_more_files(
    session_id: u32,
    revision: u32,
) -> Result<Option<LauncherStateDto>> {
    Ok(application()?
        .load_more_files(session_id, revision)
        .await?
        .map(Into::into))
}

pub(super) fn freeze_file_results(session_id: u32, revision: u32) -> Result<()> {
    application()?.freeze_file_results(session_id, revision)
}

pub(super) async fn preview_file(
    session_id: u32,
    revision: u32,
    candidate_id: String,
) -> Result<FilePreviewDto> {
    let p = application()?
        .preview_file(session_id, revision, candidate_id)
        .await?;
    Ok(FilePreviewDto {
        path: p.path,
        is_directory: p.is_directory,
        size_bytes: p.size_bytes,
        modified_ms: p.modified_ms,
        text: p.text,
        truncated_at_bytes: p.truncated_at_bytes,
        encoding: p.encoding,
        encoding_guessed: p.encoding_guessed,
    })
}

pub(super) async fn load_more_files_delta(
    session_id: u32,
    revision: u32,
    version: u32,
    count: u32,
) -> Result<SnapshotWaitDto> {
    Ok(
        match application()?.load_more_files(session_id, revision).await? {
            Some(state) => snapshot_delivery(state, version, count),
            None => SnapshotWaitDto {
                kind: SnapshotWaitKindDto::Superseded,
                state: None,
                candidate_update: CandidateUpdateDto::Replace,
            },
        },
    )
}
