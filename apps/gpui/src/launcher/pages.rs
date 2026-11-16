use crate::host::{
    AppHistoryItem, AppHistoryObjectAction, AppHistorySnapshot, AppHistorySource, ClipboardItem,
    ClipboardSnapshot, ExecutionDetail, ExecutionRow, WebPackage,
};
use anywhere_core::features::application_history::model::Notice as HistoryNotice;
use anywhere_protocol::FileSearchFilter;

#[derive(Clone, Debug, Default)]
pub struct ClipboardPageModel {
    pub query: String,
    pub selected: usize,
    pub browsing: bool,
    pub loading: bool,
    pub status: Option<ClipboardSnapshot>,
    pub query_generation: u64,
    pub operation_generation: u64,
    pub actions_open: bool,
    pub action_target: Option<i64>,
    pub action_selected: usize,
    pub delete_pending: Option<i64>,
    pub delete_confirm_selected: bool,
    pub copied_item: Option<i64>,
    pub space_edit: Option<(String, usize)>,
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
            self.selected = if self.browsing {
                (self.selected + 1).min(self.items().len().saturating_sub(1))
            } else {
                0
            };
            self.browsing = true;
        }
    }

    pub fn select_previous(&mut self) {
        if !self.items().is_empty() {
            self.selected = if !self.browsing || self.selected == 0 {
                0
            } else {
                self.selected - 1
            };
            self.browsing = true;
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
    pub notice: Option<HistoryNotice>,
    pub failed_sources: bool,
    pub loading: bool,
    pub object_actions: Vec<AppHistoryObjectAction>,
    pub object_actions_active: bool,
    pub object_actions_loading: bool,
    pub empty_actions_open: bool,
    pub fallback_actions_open: bool,
    pub fallback_action_selected: usize,
    pub object_action_selected: usize,
    object_actions_item_id: Option<String>,
    object_actions_revision: Option<u32>,
    object_actions_request: u64,
    pub session_generation: u64,
    pub query_generation: u64,
    reset_selection_on_next_snapshot: bool,
}

impl AppHistoryPageModel {
    pub fn begin_open(&mut self) {
        self.query.clear();
        self.reset_selection_on_next_snapshot = true;
        self.items.clear();
        self.selected = 0;
        self.loading = true;
        self.notice = None;
        self.failed_sources = false;
        self.empty_actions_open = false;
        self.fallback_actions_open = false;
        self.fallback_action_selected = 0;
        self.invalidate_object_actions();
    }

    pub fn reset_selection(&mut self) {
        self.reset_selection_on_next_snapshot = true;
    }

    pub fn set_query(&mut self, query: String) {
        if self.query != query {
            self.query = query;
            self.reset_selection_on_next_snapshot = true;
        }
    }

    pub fn apply_sources(&mut self, session_id: Option<u64>, sources: Vec<AppHistorySource>) {
        if let Some(session_id) = session_id {
            self.session_id = Some(session_id);
        }
        self.sources = sources;
        self.invalidate_object_actions();
        let previous_source = self.selected_source.clone();
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
        if self.selected_source != previous_source {
            self.reset_selection_on_next_snapshot = true;
        }
    }

    pub fn apply(&mut self, snapshot: AppHistorySnapshot) {
        self.loading = false;
        let previous_object_key = if self.reset_selection_on_next_snapshot
            || self.selected_source.as_deref() != Some(snapshot.source_id.as_str())
        {
            None
        } else {
            self.selected_item().map(|item| item.object_key.clone())
        };
        self.items = snapshot.items;
        self.selected_source = Some(snapshot.source_id);
        self.selected = previous_object_key
            .and_then(|object_key| {
                self.items
                    .iter()
                    .position(|item| item.object_key == object_key)
            })
            .unwrap_or(0);
        self.reset_selection_on_next_snapshot = false;
        self.failed_sources = !snapshot.failed_sources.is_empty();
        self.notice = snapshot.notice;
        self.empty_actions_open = false;
        self.fallback_actions_open = false;
        self.fallback_action_selected = 0;
        self.invalidate_object_actions();
    }

    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1).min(self.items.len().saturating_sub(1));
            self.invalidate_object_actions();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.items.is_empty() {
            self.selected = self.selected.saturating_sub(1);
            self.invalidate_object_actions();
        }
    }

    pub fn selected_item(&self) -> Option<&AppHistoryItem> {
        self.items.get(self.selected)
    }

    pub fn fail(&mut self) {
        self.loading = false;
        self.items.clear();
        self.selected = 0;
        self.empty_actions_open = false;
        self.fallback_actions_open = false;
        self.fallback_action_selected = 0;
        self.invalidate_object_actions();
    }

    pub fn notice_text(&self, locale: crate::l10n::Locale) -> Option<String> {
        if self.failed_sources {
            return Some(crate::l10n::app_history_read_failed(locale));
        }
        self.notice.map(|notice| match notice {
            HistoryNotice::Disabled => crate::l10n::app_history_disabled(locale),
            HistoryNotice::Unconfigured => crate::l10n::app_history_unconfigured(locale),
            HistoryNotice::ReadFailed => crate::l10n::app_history_read_failed(locale),
            HistoryNotice::Busy => crate::l10n::app_history_busy(locale),
            HistoryNotice::Stale => crate::l10n::app_history_expired(locale),
        })
    }

    pub fn begin_object_actions_request(&mut self, item_id: String, revision: u32) -> u64 {
        self.object_actions_request = self.object_actions_request.wrapping_add(1);
        self.object_actions_item_id = Some(item_id);
        self.object_actions_revision = Some(revision);
        self.object_actions.clear();
        self.object_actions_active = true;
        self.object_actions_loading = true;
        self.object_action_selected = 0;
        self.object_actions_request
    }

    pub fn apply_object_actions(
        &mut self,
        request: u64,
        item_id: &str,
        revision: u32,
        actions: Vec<AppHistoryObjectAction>,
    ) -> bool {
        if request != self.object_actions_request
            || self.object_actions_item_id.as_deref() != Some(item_id)
            || self.object_actions_revision != Some(revision)
            || self.selected_item().map(|item| item.id.as_str()) != Some(item_id)
        {
            return false;
        }
        self.object_actions = actions;
        self.object_actions_loading = false;
        self.object_action_selected = self
            .object_actions
            .iter()
            .position(|action| action.enabled)
            .unwrap_or(0);
        true
    }

    pub fn fail_object_actions(&mut self, request: u64) -> bool {
        if request != self.object_actions_request {
            return false;
        }
        self.invalidate_object_actions();
        true
    }

    pub fn select_object_action_next(&mut self) {
        let count =
            self.object_actions.len() + 1 + usize::from(self.object_actions_fallback_copy());
        for _ in 0..count {
            self.object_action_selected = (self.object_action_selected + 1) % count;
            if self.object_action_selected >= self.object_actions.len()
                || self.object_actions[self.object_action_selected].enabled
            {
                return;
            }
        }
    }

    pub fn select_object_action_previous(&mut self) {
        let count =
            self.object_actions.len() + 1 + usize::from(self.object_actions_fallback_copy());
        for _ in 0..count {
            self.object_action_selected = if self.object_action_selected == 0 {
                count - 1
            } else {
                self.object_action_selected - 1
            };
            if self.object_action_selected >= self.object_actions.len()
                || self.object_actions[self.object_action_selected].enabled
            {
                return;
            }
        }
    }

    pub fn object_actions_fallback_copy(&self) -> bool {
        self.object_actions.is_empty()
            && !self.object_actions_loading
            && self.selected_item().is_some()
    }

    pub fn selected_object_action(&self) -> Option<&AppHistoryObjectAction> {
        self.object_actions.get(self.object_action_selected)
    }

    pub fn close_object_actions(&mut self) {
        self.invalidate_object_actions();
    }

    fn invalidate_object_actions(&mut self) {
        self.object_actions_request = self.object_actions_request.wrapping_add(1);
        self.object_actions_item_id = None;
        self.object_actions_revision = None;
        self.object_actions.clear();
        self.object_actions_active = false;
        self.object_actions_loading = false;
        self.fallback_actions_open = false;
        self.fallback_action_selected = 0;
        self.object_action_selected = 0;
    }
}

#[derive(Clone, Debug)]
pub struct TasksPageModel {
    pub rows: Vec<ExecutionRow>,
    pub selected: usize,
    pub detail: Option<ExecutionDetail>,
    pub detail_request: u64,
    pub filter: String,
    pub follow_output: bool,
    pub follow_output_request: bool,
    pub diagnostics: bool,
    pub command_details_open: bool,
    pub tool_options_open: bool,
    pub tool_option_selected: usize,
    pub task_actions_open: bool,
    pub task_action_selected: usize,
    pub refresh_generation: u64,
}

impl Default for TasksPageModel {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            selected: 0,
            detail: None,
            detail_request: 0,
            filter: String::new(),
            follow_output: true,
            follow_output_request: false,
            diagnostics: false,
            command_details_open: false,
            tool_options_open: false,
            tool_option_selected: 0,
            task_actions_open: false,
            task_action_selected: 0,
            refresh_generation: 0,
        }
    }
}

impl TasksPageModel {
    pub fn apply_rows(&mut self, rows: Vec<ExecutionRow>) {
        let previous_id = self.selected_row().map(|row| row.id.clone());
        let previous_visible_position = self
            .visible_indices()
            .iter()
            .position(|index| *index == self.selected);
        self.rows = rows;
        if self.rows.is_empty() {
            self.selected = 0;
            self.detail = None;
            self.tool_options_open = false;
            self.tool_option_selected = 0;
            self.task_actions_open = false;
            self.task_action_selected = 0;
            return;
        }
        let visible = self.visible_indices();
        if visible.is_empty() {
            self.selected = 0;
            self.detail = None;
            self.tool_options_open = false;
            self.tool_option_selected = 0;
            self.task_actions_open = false;
            self.task_action_selected = 0;
            return;
        }
        self.selected = previous_id
            .as_ref()
            .and_then(|id| {
                visible
                    .iter()
                    .copied()
                    .find(|index| self.rows[*index].id.as_str() == id.as_str())
            })
            .unwrap_or_else(|| {
                let position = previous_visible_position
                    .unwrap_or(0)
                    .min(visible.len() - 1);
                visible[position]
            });
        let selection_changed = self.selected_row().map(|row| &row.id) != previous_id.as_ref();
        if selection_changed {
            self.detail = None;
            self.detail_request = self.detail_request.wrapping_add(1);
            self.tool_options_open = false;
            self.tool_option_selected = 0;
            self.task_actions_open = false;
            self.task_action_selected = 0;
        }
    }

    pub fn selected_row(&self) -> Option<&ExecutionRow> {
        self.rows.get(self.selected)
    }

    pub fn detail_is_current(&self) -> bool {
        match (&self.detail, self.selected_row()) {
            (Some(detail), Some(row)) => detail.row == *row,
            (None, None) => true,
            (None, Some(_)) | (Some(_), None) => false,
        }
    }

    pub fn visible_indices(&self) -> Vec<usize> {
        let terms = self
            .filter
            .split_whitespace()
            .map(str::to_lowercase)
            .collect::<Vec<_>>();
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                terms.is_empty()
                    || terms.iter().all(|term| {
                        row.title.to_lowercase().contains(term)
                            || row.directory.to_lowercase().contains(term)
                    })
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub fn request_follow_output(&mut self) {
        self.follow_output = true;
        self.follow_output_request = true;
    }

    pub fn take_follow_output_request(&mut self) -> bool {
        std::mem::take(&mut self.follow_output_request) && self.follow_output
    }

    pub fn update_follow_output(&mut self, offset: gpui_kit::Pixels, maximum: gpui_kit::Pixels) {
        self.follow_output = offset + maximum <= gpui_kit::px(24.);
        if !self.follow_output {
            self.follow_output_request = false;
        }
    }

    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        let visible = self.visible_indices();
        if !visible.contains(&self.selected) {
            self.selected = visible.first().copied().unwrap_or(0);
            self.detail = None;
            self.detail_request = self.detail_request.wrapping_add(1);
        }
    }

    pub fn select_next_visible(&mut self) {
        self.select_visible_relative(1);
    }

    pub fn select_previous_visible(&mut self) {
        self.select_visible_relative(-1);
    }

    fn select_visible_relative(&mut self, direction: isize) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            return;
        }
        let position = visible
            .iter()
            .position(|index| *index == self.selected)
            .map(|index| index as isize)
            .unwrap_or(-1);
        let target = (position + direction).clamp(0, visible.len() as isize - 1) as usize;
        self.selected = visible[target];
    }

    pub fn begin_detail_request(&mut self) -> u64 {
        self.detail_request = self.detail_request.wrapping_add(1);
        self.detail_request
    }

    pub fn apply_detail(&mut self, request_id: u64, detail: ExecutionDetail) {
        if request_id == self.detail_request
            && self
                .selected_row()
                .is_some_and(|row| row.id == detail.row.id)
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

    pub fn select_next(&mut self) {
        if !self.packages.is_empty() {
            self.selected = (self.selected + 1) % self.packages.len();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.packages.is_empty() {
            self.selected = if self.selected == 0 {
                self.packages.len() - 1
            } else {
                self.selected - 1
            };
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FileSearchPageModel {
    pub filter: FileSearchFilter,
    pub browsing: bool,
    pub page_generation: u64,
    pub page_in_flight: bool,
    pub select_after_load: bool,
    pub space_edit: Option<(String, usize)>,
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

    pub fn previous_filter(&mut self) {
        self.filter = match self.filter {
            FileSearchFilter::All => FileSearchFilter::Code,
            FileSearchFilter::Files => FileSearchFilter::All,
            FileSearchFilter::Folders => FileSearchFilter::Files,
            FileSearchFilter::Documents => FileSearchFilter::Folders,
            FileSearchFilter::Pdf => FileSearchFilter::Documents,
            FileSearchFilter::Images => FileSearchFilter::Pdf,
            FileSearchFilter::Audio => FileSearchFilter::Images,
            FileSearchFilter::Video => FileSearchFilter::Audio,
            FileSearchFilter::Archives => FileSearchFilter::Video,
            FileSearchFilter::Code => FileSearchFilter::Archives,
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
        model.select_next();
        model.select_next();
        assert_eq!(model.selected_item().map(|item| item.id), Some(30));
        model.select_next();
        assert_eq!(
            model.selected_item().map(|item| item.id),
            Some(30),
            "clipboard navigation clamps at the last row"
        );
        model.select_previous();
        assert_eq!(model.selected_item().map(|item| item.id), Some(20));
        model.reset_selection();
        model.select_previous();
        assert_eq!(model.selected_item().map(|item| item.id), Some(10));

        model.apply(clipboard_snapshot(&[10, 30]));
        assert_eq!(model.selected_item().map(|item| item.id), Some(10));
        model.apply(clipboard_snapshot(&[10]));
        assert_eq!(model.selected_item().map(|item| item.id), Some(10));
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

    #[test]
    fn web_package_keyboard_selection_wraps_in_both_directions() {
        let package = |id: &str| WebPackage {
            id: id.to_owned(),
            name: id.to_owned(),
            version: "1.0.0".to_owned(),
            digest: String::new(),
            entry: "index.html".to_owned(),
            enabled: false,
            available: true,
            rollback: false,
            permission_count: 0,
        };
        let mut model = WebPageModel::default();
        model.apply(vec![package("one"), package("two")]);
        assert_eq!(model.selected, 0);
        model.select_next();
        assert_eq!(model.selected, 1);
        model.select_next();
        assert_eq!(model.selected, 0);
        model.select_previous();
        assert_eq!(model.selected, 1);
    }

    #[test]
    fn task_details_accept_only_the_latest_matching_request() {
        let row = |id: &str| ExecutionRow {
            id: id.to_owned(),
            title: id.to_owned(),
            directory: "/tmp".to_owned(),
            command: "true".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Succeeded,
            output_revision: 0,
            exit_code: Some(0),
            started_at_ms: 0,
            finished_at_ms: Some(1),
            output_truncated: false,
            error: None,
        };
        let detail = |id: &str, output: &str| ExecutionDetail {
            row: row(id),
            stdout: output.to_owned(),
            stderr: String::new(),
        };
        let mut model = TasksPageModel::default();
        model.apply_rows(vec![row("a"), row("b")]);
        let older = model.begin_detail_request();
        model.selected = 1;
        let latest = model.begin_detail_request();
        model.apply_detail(older, detail("a", "old"));
        assert!(model.detail.is_none());
        model.apply_detail(latest, detail("b", "latest"));
        assert_eq!(
            model.detail.as_ref().map(|detail| detail.stdout.as_str()),
            Some("latest")
        );
        model.apply_rows(vec![row("a")]);
        assert!(model.detail.is_none());
    }

    #[test]
    fn task_details_refresh_only_when_the_selected_summary_changes() {
        let row = ExecutionRow {
            id: "task".to_owned(),
            title: "task".to_owned(),
            directory: "/tmp".to_owned(),
            command: "true".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Running,
            output_revision: 3,
            exit_code: None,
            started_at_ms: 0,
            finished_at_ms: None,
            output_truncated: false,
            error: None,
        };
        let mut model = TasksPageModel::default();
        model.apply_rows(vec![row.clone()]);
        assert!(!model.detail_is_current());
        let request = model.begin_detail_request();
        model.apply_detail(
            request,
            ExecutionDetail {
                row: row.clone(),
                stdout: "output".into(),
                stderr: String::new(),
            },
        );
        assert!(model.detail_is_current());
        model.apply_rows(vec![row.clone()]);
        assert!(model.detail_is_current());
        let mut changed = row;
        changed.output_revision += 1;
        model.apply_rows(vec![changed]);
        assert!(!model.detail_is_current());
    }

    #[test]
    fn task_filter_navigates_only_visible_rows() {
        let row = |id: &str, directory: &str| ExecutionRow {
            id: id.to_owned(),
            title: id.to_owned(),
            directory: directory.to_owned(),
            command: "true".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Succeeded,
            output_revision: 0,
            exit_code: Some(0),
            started_at_ms: 0,
            finished_at_ms: Some(1),
            output_truncated: false,
            error: None,
        };
        let mut model = TasksPageModel::default();
        model.apply_rows(vec![
            row("cargo-check", "/work/site"),
            row("npm-test", "/work/web"),
            row("cargo-test", "/work/site"),
        ]);
        model.set_filter("cargo site".to_owned());
        assert_eq!(
            model.visible_indices(),
            vec![0, 2],
            "every filter term must match the task or its directory"
        );
        model.set_filter("cargo".to_owned());
        assert_eq!(model.visible_indices(), vec![0, 2]);
        assert_eq!(
            model.selected_row().map(|row| row.id.as_str()),
            Some("cargo-check")
        );
        model.select_next_visible();
        assert_eq!(
            model.selected_row().map(|row| row.id.as_str()),
            Some("cargo-test")
        );
        model.select_next_visible();
        assert_eq!(
            model.selected_row().map(|row| row.id.as_str()),
            Some("cargo-test"),
            "task navigation clamps at the end of the visible filter"
        );
        model.select_previous_visible();
        assert_eq!(
            model.selected_row().map(|row| row.id.as_str()),
            Some("cargo-check")
        );
        model.select_previous_visible();
        assert_eq!(
            model.selected_row().map(|row| row.id.as_str()),
            Some("cargo-check"),
            "task navigation clamps at the start of the visible filter"
        );
    }

    #[test]
    fn task_refresh_keeps_selection_on_a_visible_filtered_row() {
        let row = |id: &str, directory: &str| ExecutionRow {
            id: id.to_owned(),
            title: id.to_owned(),
            directory: directory.to_owned(),
            command: "true".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Succeeded,
            output_revision: 0,
            exit_code: Some(0),
            started_at_ms: 0,
            finished_at_ms: Some(1),
            output_truncated: false,
            error: None,
        };
        let mut model = TasksPageModel::default();
        model.apply_rows(vec![
            row("alpha-a", "/work/alpha"),
            row("beta-a", "/work/beta"),
            row("alpha-selected", "/work/alpha"),
            row("beta-b", "/work/beta"),
        ]);
        model.set_filter("alpha".to_owned());
        model.selected = 2;
        model.apply_rows(vec![
            row("beta-new-a", "/work/beta"),
            row("alpha-new", "/work/alpha"),
            row("beta-new-b", "/work/beta"),
            row("beta-new-c", "/work/beta"),
        ]);
        assert_eq!(
            model.selected_row().map(|row| row.id.as_str()),
            Some("alpha-new")
        );
        assert!(model.visible_indices().contains(&model.selected));
    }

    #[test]
    fn task_rows_keep_the_nearest_position_when_the_selected_task_disappears() {
        let row = |id: &str| ExecutionRow {
            id: id.to_owned(),
            title: id.to_owned(),
            directory: "/tmp".to_owned(),
            command: "true".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Succeeded,
            output_revision: 0,
            exit_code: Some(0),
            started_at_ms: 0,
            finished_at_ms: Some(1),
            output_truncated: false,
            error: None,
        };
        let mut model = TasksPageModel::default();
        model.apply_rows(vec![row("a"), row("b"), row("c"), row("d")]);
        model.selected = 2;
        model.apply_rows(vec![row("a"), row("b"), row("d")]);
        assert_eq!(
            model.selected_row().map(|row| row.id.as_str()),
            Some("d"),
            "a removed selected task keeps the same position when possible"
        );
        model.apply_rows(vec![row("a")]);
        assert_eq!(model.selected_row().map(|row| row.id.as_str()), Some("a"));
    }

    #[test]
    fn task_output_follow_uses_distance_from_the_bottom() {
        let mut model = TasksPageModel::default();
        model.update_follow_output(gpui_kit::px(-100.), gpui_kit::px(100.));
        assert!(model.follow_output);
        model.update_follow_output(gpui_kit::px(0.), gpui_kit::px(100.));
        assert!(!model.follow_output);
        model.request_follow_output();
        assert!(model.follow_output);
        assert!(model.take_follow_output_request());
        assert!(!model.take_follow_output_request());
    }

    #[test]
    fn app_history_actions_reject_stale_owners_and_localize_notices() {
        let item = |id: &str| AppHistoryItem {
            id: id.to_owned(),
            object_key: id.to_owned(),
            source_id: "code".to_owned(),
            title: id.to_owned(),
            value: format!("/tmp/{id}"),
            kind: "file",
            can_open: true,
        };
        let mut model = AppHistoryPageModel::default();
        model.apply(AppHistorySnapshot {
            source_id: "code".to_owned(),
            items: vec![item("a"), item("b")],
            failed_sources: vec![],
            notice: Some(HistoryNotice::Busy),
        });
        assert_eq!(
            model.notice_text(crate::l10n::Locale::En),
            Some(crate::l10n::app_history_busy(crate::l10n::Locale::En))
        );

        let older = model.begin_object_actions_request("a".to_owned(), 7);
        model.select_next();
        let latest = model.begin_object_actions_request("b".to_owned(), 7);
        assert!(!model.apply_object_actions(
            older,
            "a",
            7,
            vec![AppHistoryObjectAction {
                id: "old".to_owned(),
                title: "Old".to_owned(),
                localized_titles: Default::default(),
                enabled: true,
            }],
        ));
        assert!(model.apply_object_actions(
            latest,
            "b",
            7,
            vec![AppHistoryObjectAction {
                id: "new".to_owned(),
                title: "New".to_owned(),
                localized_titles: Default::default(),
                enabled: true,
            }],
        ));
        assert_eq!(
            model
                .selected_object_action()
                .map(|action| action.id.as_str()),
            Some("new")
        );
        model.select_object_action_next();
        assert_eq!(model.object_action_selected, model.object_actions.len());
        model.select_object_action_next();
        assert_eq!(model.object_action_selected, 0);
        model.select_object_action_previous();
        assert_eq!(model.object_action_selected, model.object_actions.len());
        let failed = model.begin_object_actions_request("a".to_owned(), 7);
        assert!(!model.fail_object_actions(older));
        assert!(model.object_actions_active);
        assert!(model.fail_object_actions(failed));
        assert!(!model.object_actions_active);
        assert!(model.object_actions.is_empty());

        model.failed_sources = true;
        assert_eq!(
            model.notice_text(crate::l10n::Locale::Zh),
            Some(crate::l10n::app_history_read_failed(
                crate::l10n::Locale::Zh
            ))
        );
    }

    #[test]
    fn app_history_refresh_preserves_identity_and_a_new_query_resets_selection() {
        let item = |id: &str| AppHistoryItem {
            id: format!("row-{id}"),
            object_key: format!("object-{id}"),
            source_id: "code".to_owned(),
            title: id.to_owned(),
            value: format!("/tmp/{id}"),
            kind: "file",
            can_open: true,
        };
        let snapshot = |items| AppHistorySnapshot {
            source_id: "code".to_owned(),
            items,
            failed_sources: vec![],
            notice: None,
        };
        let mut model = AppHistoryPageModel::default();
        model.apply(snapshot(vec![item("a"), item("b"), item("c")]));
        model.select_next();
        model.select_next();
        assert_eq!(
            model.selected_item().map(|item| item.object_key.as_str()),
            Some("object-c")
        );

        model.apply(snapshot(vec![
            item("new-a"),
            item("new-b"),
            item("a"),
            item("b"),
            item("c"),
        ]));
        assert_eq!(
            model.selected_item().map(|item| item.object_key.as_str()),
            Some("object-c"),
            "a refresh keeps the selected object identity"
        );
        assert_eq!(model.selected, 4);
        model.select_next();
        assert_eq!(model.selected, 4, "history navigation clamps at the end");
        model.select_previous();
        assert_eq!(model.selected, 3);
        model.selected = 0;
        model.select_previous();
        assert_eq!(model.selected, 0, "history navigation clamps at the start");

        model.set_query("new".to_owned());
        model.apply(snapshot(vec![item("new-a"), item("new-b")]));
        assert_eq!(model.selected, 0);
        assert_eq!(
            model.selected_item().map(|item| item.object_key.as_str()),
            Some("object-new-a")
        );

        model.begin_open();
        assert!(model.query.is_empty());
        assert!(model.items.is_empty());
        assert!(model.loading);
        assert_eq!(model.selected, 0);
        model.apply(snapshot(vec![item("fresh")]));
        model.fail();
        assert!(model.items.is_empty());
        assert!(!model.loading);
    }
}
