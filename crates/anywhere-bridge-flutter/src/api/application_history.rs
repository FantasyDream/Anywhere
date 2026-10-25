//! Metadata-only typed port; source parsing and execution do not run under settings locks.
use super::*;
use anywhere_core::features::application_history::{model::*, service::HistoryService};
use std::sync::Arc;

fn service() -> Result<Arc<HistoryService>> {
    settings::extensions()?.application_history()
}
pub(super) fn sources() -> Result<Vec<AppHistorySourceDto>> {
    let service = service()?;
    service.refresh()?;
    Ok(service
        .sources()
        .into_iter()
        .map(|s| AppHistorySourceDto {
            id: s.descriptor.id,
            title: s.descriptor.title,
            file_hint: s.descriptor.file_hint,
            action_enabled: s.action_enabled,
            path: s
                .path
                .map_or_else(String::new, |p| p.to_string_lossy().into_owned()),
            mode: match s.mode {
                SourceMode::Automatic => AppHistorySourceModeDto::Automatic,
                SourceMode::Custom => AppHistorySourceModeDto::Custom,
                SourceMode::Disabled => AppHistorySourceModeDto::Disabled,
            },
        })
        .collect())
}
pub(super) fn configure(id: String, path: String, mode: AppHistorySourceModeDto) -> Result<()> {
    let mode = match mode {
        AppHistorySourceModeDto::Automatic => SourceMode::Automatic,
        AppHistorySourceModeDto::Custom => SourceMode::Custom,
        AppHistorySourceModeDto::Disabled => SourceMode::Disabled,
    };
    service()?.configure_mode(&id, mode, Some(path.as_str()))?;
    settings::extensions()?.synchronize_application_history(application()?)
}
pub(super) fn set_action_enabled(id: String, enabled: bool) -> Result<()> {
    service()?.set_action_enabled(&id, enabled)?;
    settings::extensions()?.synchronize_application_history(application()?)
}
pub(super) fn custom_directory(open: bool) -> Result<String> {
    let directory = settings::extensions()?.application_history_directory();
    if open {
        std::fs::create_dir_all(&directory)?;
        application()?.platform().open_directory(&directory)?;
    }
    Ok(directory.to_string_lossy().into_owned())
}
pub(super) fn open() -> Result<u32> {
    Ok(u32::try_from(service()?.open_session()?)?)
}
pub(super) fn cancel(session_id: u32) -> Result<()> {
    service()?.cancel_session(u64::from(session_id));
    Ok(())
}
pub(super) fn search(session_id: u32, query: String, source: String) -> Result<AppHistoryDto> {
    let result = service()?.search(u64::from(session_id), &query, &source)?;
    Ok(AppHistoryDto {
        items: result
            .items
            .into_iter()
            .map(|item| {
                let kind = match item.entry.target {
                    Target::Directory { .. } => "directory",
                    Target::ObsidianVault { .. } => "directory",
                    Target::File { .. } => "file",
                    Target::Workspace { .. } => "workspace",
                    Target::Remote { .. } => "remote",
                };
                AppHistoryItemDto {
                    id: item.id,
                    object_key: format!("{}:{}", item.source_id, item.entry.key),
                    source_id: item.source_id,
                    title: item.entry.title,
                    value: item.entry.target.text(),
                    kind: kind.into(),
                    can_open: kind != "remote",
                }
            })
            .collect(),
        failed_sources: result.failed_sources,
        notice: result.notice.map(|n| match n {
            Notice::Disabled => AppHistoryNoticeDto::Disabled,
            Notice::Unconfigured => AppHistoryNoticeDto::Unconfigured,
            Notice::ReadFailed => AppHistoryNoticeDto::ReadFailed,
            Notice::Busy => AppHistoryNoticeDto::Busy,
            Notice::Stale => AppHistoryNoticeDto::Stale,
        }),
    })
}
pub(super) fn use_item(id: String, copy: bool) -> Result<()> {
    service()?.use_item(&id, copy, application()?.platform())
}
