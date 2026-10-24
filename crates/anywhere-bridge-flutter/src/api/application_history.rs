//! Metadata-only typed port; source parsing and execution do not run under settings locks.
use super::*;
use anywhere_core::features::application_history::{model::*, service::HistoryService};
use std::sync::Arc;

fn service() -> Result<Arc<HistoryService>> {
    settings::extensions()?.application_history()
}
pub(super) fn sources() -> Result<Vec<AppHistorySourceDto>> {
    Ok(service()?
        .sources()
        .into_iter()
        .map(|(d, p)| AppHistorySourceDto {
            id: d.id,
            title: d.title,
            file_hint: d.file_hint,
            path: p.map_or_else(String::new, |p| p.path.to_string_lossy().into_owned()),
        })
        .collect())
}
pub(super) fn configure(id: String, path: String) -> Result<()> {
    service()?.configure(&id, (!path.trim().is_empty()).then_some(path.as_str()))
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
