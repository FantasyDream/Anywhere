use crate::host::{
    AppHistoryItem, AppHistorySnapshot, AppHistorySource, ClipboardItem, ClipboardSnapshot,
    ExecutionDetail, ExecutionRow, WebPackage,
};
use anywhere_protocol::FileSearchFilter;

#[derive(Clone, Debug, Default)]
pub struct ClipboardPageModel {
    pub query: String,
    pub selected: usize,
    pub browsing: bool,
    pub loading: bool,
    pub status: Option<ClipboardSnapshot>,
}

impl ClipboardPageModel {
    pub fn apply(&mut self, snapshot: ClipboardSnapshot) {
        self.loading = false;
        let previous = self.selected_id();
        let selected = previous.and_then(|id| snapshot.items.iter().position(|item| item.id == id));
        self.selected = selected.unwrap_or(0);
        if self.browsing && selected.is_none() {
            self.browsing = false;
        }
        if self.selected >= snapshot.items.len() {
            self.selected = snapshot.items.len().saturating_sub(1);
        }
        self.status = Some(snapshot);
    }

    pub fn loading(&mut self) {
        self.loading = true;
    }

    pub fn items(&self) -> &[ClipboardItem] {
        self.status
            .as_ref()
            .map(|snapshot| snapshot.items.as_slice())
            .unwrap_or_default()
    }

    pub fn selected_id(&self) -> Option<i64> {
        if !self.browsing {
            return None;
        }
        self.items().get(self.selected).map(|item| item.id)
    }

    pub fn selected_item(&self) -> Option<&ClipboardItem> {
        if !self.browsing {
            return None;
        }
        self.items().get(self.selected)
    }

    pub fn select_next(&mut self) {
        if !self.items().is_empty() {
            if self.browsing {
                self.selected = (self.selected + 1) % self.items().len();
            } else {
                self.browsing = true;
                self.selected = 0;
            }
        }
    }

    pub fn select_previous(&mut self) {
        if !self.items().is_empty() {
            if self.browsing {
                self.selected = if self.selected == 0 {
                    self.items().len() - 1
                } else {
                    self.selected - 1
                };
            } else {
                self.browsing = true;
                self.selected = self.items().len() - 1;
            }
        }
    }

    pub fn reset_selection(&mut self) {
        self.selected = 0;
        self.browsing = false;
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppHistoryPageModel {
    pub query: String,
    pub session_id: Option<u64>,
    pub sources: Vec<AppHistorySource>,
    pub selected_source: Option<String>,
    pub items: Vec<AppHistoryItem>,
    pub selected: usize,
    pub notice: Option<String>,
    pub loading: bool,
}

impl AppHistoryPageModel {
    pub fn apply_sources(&mut self, session_id: Option<u64>, sources: Vec<AppHistorySource>) {
        if let Some(session_id) = session_id {
            self.session_id = Some(session_id);
        }
        self.sources = sources;
        let current = self.selected_source.as_deref();
        if !self
            .sources
            .iter()
            .any(|source| Some(source.id.as_str()) == current && source.action_enabled)
        {
            self.selected_source = self
                .sources
                .iter()
                .find(|source| source.action_enabled)
                .map(|source| source.id.clone());
        }
    }

    pub fn apply(&mut self, snapshot: AppHistorySnapshot) {
        self.loading = false;
        self.selected = 0;
        self.items = snapshot.items;
        self.selected_source = Some(snapshot.source_id);
        self.notice = if snapshot.failed_sources.is_empty() && snapshot.notice.is_none() {
            None
        } else {
            Some(format!(
                "failed={:?}; notice={:?}",
                snapshot.failed_sources, snapshot.notice
            ))
        };
    }

    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.items.is_empty() {
            self.selected = if self.selected == 0 {
                self.items.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn selected_item(&self) -> Option<&AppHistoryItem> {
        self.items.get(self.selected)
    }
}

#[derive(Clone, Debug, Default)]
pub struct TasksPageModel {
    pub rows: Vec<ExecutionRow>,
    pub selected: usize,
    pub detail: Option<ExecutionDetail>,
}

impl TasksPageModel {
    pub fn apply_rows(&mut self, rows: Vec<ExecutionRow>) {
        let selected_id = self.selected_row().map(|row| row.id.clone());
        self.rows = rows;
        self.selected = selected_id
            .and_then(|id| self.rows.iter().position(|row| row.id == id))
            .unwrap_or(0);
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len().saturating_sub(1);
            self.detail = None;
        }
    }

    pub fn selected_row(&self) -> Option<&ExecutionRow> {
        self.rows.get(self.selected)
    }

    pub fn apply_detail(&mut self, detail: ExecutionDetail) {
        if self
            .rows
            .iter()
            .any(|candidate| candidate.id == detail.row.id)
        {
            self.detail = Some(detail);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct WebPageModel {
    pub packages: Vec<WebPackage>,
    pub selected: usize,
}

impl WebPageModel {
    pub fn apply(&mut self, packages: Vec<WebPackage>) {
        let selected = self
            .packages
            .get(self.selected)
            .map(|package| package.id.clone());
        self.packages = packages;
        self.selected = selected
            .and_then(|id| self.packages.iter().position(|package| package.id == id))
            .unwrap_or(0);
        if self.selected >= self.packages.len() {
            self.selected = self.packages.len().saturating_sub(1);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FileSearchPageModel {
    pub filter: FileSearchFilter,
}

impl FileSearchPageModel {
    pub fn next_filter(&mut self) {
        self.filter = match self.filter {
            FileSearchFilter::All => FileSearchFilter::Files,
            FileSearchFilter::Files => FileSearchFilter::Folders,
            FileSearchFilter::Folders => FileSearchFilter::Documents,
            FileSearchFilter::Documents => FileSearchFilter::Pdf,
            FileSearchFilter::Pdf => FileSearchFilter::Images,
            FileSearchFilter::Images => FileSearchFilter::Audio,
            FileSearchFilter::Audio => FileSearchFilter::Video,
            FileSearchFilter::Video => FileSearchFilter::Archives,
            FileSearchFilter::Archives => FileSearchFilter::Code,
            FileSearchFilter::Code => FileSearchFilter::All,
        };
    }

    pub fn label(&self, locale: crate::l10n::Locale) -> String {
        use crate::l10n;
        match self.filter {
            FileSearchFilter::All => l10n::all_types(locale),
            FileSearchFilter::Files => l10n::files(locale),
            FileSearchFilter::Folders => l10n::folders(locale),
            FileSearchFilter::Documents => l10n::documents(locale),
            FileSearchFilter::Pdf => l10n::pdf(locale),
            FileSearchFilter::Images => l10n::images(locale),
            FileSearchFilter::Audio => l10n::audio(locale),
            FileSearchFilter::Video => l10n::video(locale),
            FileSearchFilter::Archives => l10n::archives(locale),
            FileSearchFilter::Code => l10n::code(locale),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clipboard_snapshot(ids: &[i64]) -> ClipboardSnapshot {
        ClipboardSnapshot {
            recording: true,
            recording_enabled: true,
            available: true,
            paused_until_ms: None,
            retention_days: 30,
            stored_entries: ids.len() as u32,
            stored_bytes: 0,
            notice: None,
            items: ids
                .iter()
                .map(|id| ClipboardItem {
                    id: *id,
                    summary: format!("entry {id}"),
                    kind: "text".to_owned(),
                    copied_at_ms: 0,
                    byte_count: 1,
                })
                .collect(),
        }
    }

    #[test]
    fn clipboard_selection_requires_navigation_and_starts_at_boundaries() {
        let mut model = ClipboardPageModel::default();
        model.apply(clipboard_snapshot(&[10, 20, 30]));
        assert_eq!(model.selected_item(), None);

        model.select_next();
        assert_eq!(model.selected_item().map(|item| item.id), Some(10));
        model.reset_selection();
        model.select_previous();
        assert_eq!(model.selected_item().map(|item| item.id), Some(30));

        model.apply(clipboard_snapshot(&[10, 30]));
        assert_eq!(model.selected_item().map(|item| item.id), Some(30));
        model.apply(clipboard_snapshot(&[10]));
        assert_eq!(model.selected_item(), None);
    }

    #[test]
    fn file_filter_cycle_covers_every_product_type() {
        let mut model = FileSearchPageModel::default();
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..10 {
            seen.insert(format!("{:?}", model.filter));
            model.next_filter();
        }
        assert_eq!(model.filter, FileSearchFilter::All);
        assert_eq!(seen.len(), 10);
    }
}
