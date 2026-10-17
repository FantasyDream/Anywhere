//! Typed clipboard adapter. No history body enters launcher queries or usage storage.
use super::*;
use anywhere_core::features::clipboard::{history::HistoryItem, service::HistoryService};
use std::sync::Arc;

fn history() -> Result<Arc<HistoryService>> {
    Ok(settings::extensions()?.clipboard())
}
pub(super) fn status() -> Result<ClipboardHistoryStatusDto> {
    let status = history()?.status()?;
    Ok(ClipboardHistoryStatusDto {
        recording: status.recording,
        available: status.available,
        notice: status.notice,
    })
}
pub(super) fn search(query: String) -> Result<ClipboardHistoryDto> {
    let history = history()?;
    let status = history.status()?;
    let items = if status.available {
        history.search(&query)?
    } else {
        vec![]
    };
    Ok(ClipboardHistoryDto {
        recording: status.recording,
        available: status.available,
        notice: status.notice,
        items: items.into_iter().map(convert).collect(),
    })
}
fn convert(item: HistoryItem) -> ClipboardItemDto {
    ClipboardItemDto {
        id: item.id.to_string(),
        summary: item.summary,
        kind: item.kind,
        copied_at_ms: item.copied_at_ms,
        byte_count: item.byte_count as u32,
    }
}
pub(super) fn detail(id: String) -> Result<Option<ClipboardDetailDto>> {
    Ok(history()?
        .detail(id.parse()?)?
        .map(|detail| ClipboardDetailDto {
            item: convert(detail.item),
            text: detail.text,
            image_png: detail.image_png,
            files: detail.files,
            formats: detail.formats,
        }))
}
pub(super) fn set_recording(enabled: bool) -> Result<()> {
    history()?.request_recording(enabled, application()?.platform())
}
pub(super) fn copy(id: String) -> Result<()> {
    history()?.copy(id.parse()?, application()?.platform())
}
pub(super) fn delete(id: String) -> Result<()> {
    history()?.delete(id.parse()?)
}
pub(super) fn clear() -> Result<()> {
    history()?.clear()
}
