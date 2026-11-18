use crate::host::{ExecutionDetail, ExecutionRow};
use anywhere_protocol::ExecutionSessionStatus;

#[derive(Clone, Debug)]
struct CachedTaskOutput {
    text: gpui_kit::SharedString,
    lines: std::sync::Arc<Vec<std::ops::Range<usize>>>,
    longest_line: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct TasksPageModel {
    output_cache: Option<CachedTaskOutput>,
    visible_cache: Option<(String, usize, std::sync::Arc<[usize]>)>,
    pub rows: Vec<ExecutionRow>,
    pub selected: usize,
    pub detail: Option<ExecutionDetail>,
    pub detail_request: u64,
    detail_in_flight: bool,
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
            output_cache: None,
            visible_cache: None,
            rows: Vec::new(),
            selected: 0,
            detail: None,
            detail_request: 0,
            detail_in_flight: false,
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
    pub(crate) fn release_workspace(&mut self) {
        *self = Self {
            detail_request: self.detail_request,
            refresh_generation: self.refresh_generation,
            ..Default::default()
        };
    }

    pub fn apply_rows(&mut self, rows: Vec<ExecutionRow>) {
        let previous_id = self.selected_row().map(|row| row.id.clone());
        let previous_visible_position = self
            .visible_indices()
            .iter()
            .position(|index| *index == self.selected);
        self.rows = rows;
        self.rebuild_visible_cache();
        let visible = self.visible_indices();
        if visible.is_empty() {
            self.selected = 0;
            self.clear_detail();
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
            self.clear_detail();
            self.tool_options_open = false;
            self.tool_option_selected = 0;
            self.task_actions_open = false;
            self.task_action_selected = 0;
        }
    }

    pub fn selected_row(&self) -> Option<&ExecutionRow> {
        self.visible_indices()
            .binary_search(&self.selected)
            .ok()
            .and_then(|_| self.rows.get(self.selected))
    }

    pub fn detail_is_current(&self) -> bool {
        match (&self.detail, self.selected_row()) {
            (Some(detail), Some(row)) => {
                let detail = &detail.row;
                // List and detail are read separately. A detail can already
                // include newer output or the terminal transition.
                detail.id == row.id
                    && detail.output_revision >= row.output_revision
                    && (detail.status == row.status
                        || row.status == ExecutionSessionStatus::Running
                        || (row.status == ExecutionSessionStatus::Stopping
                            && !detail.status.is_active()))
            }
            (None, None) => true,
            (None, Some(_)) | (Some(_), None) => false,
        }
    }

    fn rebuild_visible_cache(&mut self) {
        let terms = task_filter_terms(&self.filter);
        let indices: std::sync::Arc<[usize]> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| task_matches_filter(row, &terms))
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
            .into();
        self.visible_cache = Some((self.filter.clone(), self.rows.len(), indices));
    }

    pub fn visible_indices(&self) -> std::sync::Arc<[usize]> {
        if let Some((cached_filter, cached_len, indices)) = &self.visible_cache {
            if cached_filter == &self.filter && *cached_len == self.rows.len() {
                return indices.clone();
            }
        }
        let terms = task_filter_terms(&self.filter);
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, row)| task_matches_filter(row, &terms))
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
            .into()
    }

    pub fn request_follow_output(&mut self) {
        self.follow_output = true;
        self.follow_output_request = true;
    }

    pub fn output_lines_and_longest(
        &mut self,
        text: gpui_kit::SharedString,
    ) -> (std::sync::Arc<Vec<std::ops::Range<usize>>>, Option<usize>) {
        if let Some(cached) = &self.output_cache {
            if cached.text.as_ptr() == text.as_ptr() && cached.text.len() == text.len() {
                return (cached.lines.clone(), cached.longest_line);
            }
        }
        let mut lines = Vec::new();
        let mut start = 0;
        let mut count = 0;
        let mut max_cols = 0usize;
        let mut longest_index = None;
        let mut current_cols = 0usize;
        for (index, character) in text.char_indices() {
            if character == '\n' || count == 256 {
                let line_idx = lines.len();
                if longest_index.is_none() || current_cols > max_cols {
                    max_cols = current_cols;
                    longest_index = Some(line_idx);
                }
                lines.push(start..index);
                start = if character == '\n' { index + 1 } else { index };
                count = 0;
                current_cols = 0;
            }
            if character != '\n' {
                count += 1;
                current_cols += if character.is_ascii() { 1 } else { 2 };
            }
        }
        let line_idx = lines.len();
        if longest_index.is_none() || current_cols > max_cols {
            longest_index = Some(line_idx);
        }
        lines.push(start..text.len());
        let lines = std::sync::Arc::new(lines);
        self.output_cache = Some(CachedTaskOutput {
            text,
            lines: lines.clone(),
            longest_line: longest_index,
        });
        (lines, longest_index)
    }

    pub fn output_lines(
        &mut self,
        text: gpui_kit::SharedString,
    ) -> std::sync::Arc<Vec<std::ops::Range<usize>>> {
        self.output_lines_and_longest(text).0
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
        self.rebuild_visible_cache();
        let visible = self.visible_indices();
        if !visible.contains(&self.selected) {
            self.selected = visible.first().copied().unwrap_or(0);
            self.clear_detail();
            self.task_actions_open = false;
            self.task_action_selected = 0;
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

    pub fn clear_detail(&mut self) {
        self.detail = None;
        self.output_cache = None;
        self.detail_request = self.detail_request.wrapping_add(1);
        self.detail_in_flight = false;
    }

    pub fn begin_detail_request(&mut self) -> Option<u64> {
        if self.detail_in_flight {
            return None;
        }
        self.detail_request = self.detail_request.wrapping_add(1);
        self.detail_in_flight = true;
        Some(self.detail_request)
    }

    pub fn finish_detail_request(&mut self, request_id: u64) -> bool {
        request_id == self.detail_request && std::mem::take(&mut self.detail_in_flight)
    }

    pub fn apply_detail(&mut self, detail: ExecutionDetail) {
        if self
            .selected_row()
            .is_some_and(|row| row.id == detail.row.id)
        {
            self.detail = Some(detail);
        }
    }
}

fn task_filter_terms(filter: &str) -> Vec<String> {
    filter.split_whitespace().map(str::to_lowercase).collect()
}

fn task_matches_filter(row: &ExecutionRow, terms: &[String]) -> bool {
    if terms.is_empty() {
        return true;
    }
    let title = row.title.to_lowercase();
    let mut directory = None;
    terms.iter().all(|term| {
        title.contains(term)
            || directory
                .get_or_insert_with(|| row.directory.to_lowercase())
                .contains(term)
    })
}
