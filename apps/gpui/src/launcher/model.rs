use crate::l10n::Locale;
use anywhere_protocol::{
    AvailabilityState, CandidateKind, CandidatePresentation, ContextCapturePresentation,
    ContextSource, FileObjectKind, FileSearchProgress, LauncherState, Scope,
    SearchSourcePresentation,
};
use std::{collections::HashSet, sync::Arc};

const MAX_HOME_ROWS: usize = 5;
const MAX_QUERY_ROWS: usize = 7;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LauncherScope {
    pub id: String,
    pub label: String,
    pub is_current_app: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LauncherRow {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub shortcut: Option<String>,
    pub icon: String,
    pub kind: CandidateKind,
    pub enabled: bool,
    pub is_menu: bool,
    pub menu_path: Vec<String>,
    pub command_id: Option<String>,
    pub file_kind: Option<FileObjectKind>,
    pub source_label: Option<String>,
    pub icon_identity: Option<String>,
    pub execution_context_id: Option<String>,
    pub primary_action: bool,
}

impl LauncherRow {
    fn from_protocol(
        candidate: &CandidatePresentation,
        state: Option<&LauncherState>,
        locale: Locale,
    ) -> Self {
        let localized = match locale {
            Locale::En => candidate.localized_titles.get("en"),
            Locale::Zh => candidate.localized_titles.get("zh"),
        };
        let menu = candidate.menu_path.join(" › ");
        let subtitle = if candidate.availability.is_executable() {
            (!menu.is_empty())
                .then_some(menu)
                .or_else(|| candidate.subtitle.clone())
                .filter(|value| {
                    !value.trim().is_empty()
                        && value != &candidate.title
                        && !looks_like_bundle_id(value)
                })
        } else {
            candidate
                .availability
                .reason
                .clone()
                .or_else(|| Some(crate::l10n::not_available_right_now(locale)))
        };
        let source_label = candidate
            .availability
            .is_executable()
            .then(|| source_label(candidate, state, locale))
            .flatten();
        let icon_identity = candidate
            .bundle_id
            .clone()
            .or_else(|| candidate.owning_app.clone())
            .filter(|identity| !identity.trim().is_empty());
        Self {
            id: candidate.id.clone(),
            title: localized.unwrap_or(&candidate.title).clone(),
            subtitle,
            shortcut: candidate.shortcut.clone(),
            icon: candidate.icon.key.clone(),
            kind: candidate.kind,
            enabled: candidate.availability.state == AvailabilityState::Enabled,
            is_menu: candidate.is_menu,
            menu_path: candidate.menu_path.clone(),
            command_id: candidate.command_id.clone(),
            file_kind: candidate.file_kind,
            source_label,
            icon_identity,
            execution_context_id: candidate.execution_context_id.clone(),
            primary_action: candidate.primary_action,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LauncherWorkspace {
    query: String,
    selected_scope: Option<String>,
    selection_id: Option<String>,
    object_actions_active: bool,
    menu_path: Option<Vec<String>>,
    object_action_parent: Option<String>,
}

fn looks_like_bundle_id(value: &str) -> bool {
    let mut components = value.split('.');
    let Some(first) = components.next() else {
        return false;
    };
    let rest: Vec<_> = components.collect();
    first
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic())
        && first
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
        && rest.len() >= 2
        && rest.iter().all(|component| {
            !component.is_empty()
                && component
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
        })
}

fn source_label(
    candidate: &CandidatePresentation,
    state: Option<&LauncherState>,
    locale: Locale,
) -> Option<String> {
    if candidate.kind == CandidateKind::File {
        return Some(crate::l10n::files(locale));
    }
    if candidate.kind != CandidateKind::Action {
        return None;
    }
    let state = state?;
    if candidate.is_menu {
        return state
            .selected_scope
            .is_none()
            .then(|| candidate.section_label.clone())
            .flatten();
    }
    let target = state.context_summary.iter().find(|context| {
        candidate
            .execution_context_id
            .as_ref()
            .is_some_and(|id| context.id == *id)
            || candidate.context_keys.contains(&context.key)
    });
    match target.map(|context| context.source) {
        Some(ContextSource::BrowserCurrentPage) => Some(crate::l10n::current_page(locale)),
        Some(ContextSource::Clipboard) => Some(crate::l10n::clipboard(locale)),
        Some(ContextSource::SelectedText) => Some(crate::l10n::selected_text(locale)),
        Some(ContextSource::FinderSelection) => Some(crate::l10n::selected_files(locale)),
        Some(ContextSource::UserInput) => Some(crate::l10n::input(locale)),
        _ => None,
    }
}

/// Frontend-only presentation state. Matching, ranking and eligibility remain
/// in Rust Core; this model only applies Scope and selection to an authoritative
/// snapshot.
#[derive(Clone, Debug)]
pub struct LauncherModel {
    pub query: String,
    pub selected_scope: Option<String>,
    pub scopes: Vec<LauncherScope>,
    pub rows: Vec<LauncherRow>,
    pub selected: usize,
    pub loading: bool,
    pub diagnostics: Vec<String>,
    pub revision: u32,
    pub object_actions_active: bool,
    menu_path: Option<Vec<String>>,
    locale: Locale,
    object_action_parent: Option<String>,
    source: Option<Arc<LauncherState>>,
    preview_rows: Vec<LauncherRow>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LauncherApply {
    pub input_changed: bool,
    pub rows_changed: bool,
    pub scopes_changed: bool,
}

impl LauncherModel {
    pub fn preview(locale: Locale) -> Self {
        let mut model = Self {
            query: String::new(),
            selected_scope: None,
            scopes: Vec::new(),
            rows: Vec::new(),
            selected: 0,
            loading: false,
            diagnostics: Vec::new(),
            revision: 0,
            object_actions_active: false,
            menu_path: None,
            locale,
            object_action_parent: None,
            source: None,
            preview_rows: preview_rows(locale),
        };
        model.rebuild_rows();
        model
    }

    pub fn context_captures(&self) -> &[ContextCapturePresentation] {
        self.source
            .as_ref()
            .map(|state| state.context_captures.as_slice())
            .unwrap_or_default()
    }

    pub fn loading(locale: Locale) -> Self {
        Self {
            loading: true,
            ..Self::preview(locale)
        }
    }

    pub fn selection_id(&self) -> Option<&str> {
        self.rows.get(self.selected).map(|row| row.id.as_str())
    }

    pub fn selected_row(&self) -> Option<&LauncherRow> {
        self.rows.get(self.selected)
    }

    pub fn can_show_object_actions(&self, row: &LauncherRow, file_search: bool) -> bool {
        let Some(execution_context_id) = row.execution_context_id.as_deref() else {
            return false;
        };
        row.primary_action
            && (file_search
                || self.source.as_ref().is_some_and(|state| {
                    state.result_actions.iter().any(|candidate| {
                        candidate.execution_context_id.as_deref() == Some(execution_context_id)
                    })
                }))
    }

    pub fn file_search_progress(&self) -> Option<&FileSearchProgress> {
        self.source
            .as_ref()
            .and_then(|state| state.file_search.as_deref())
    }

    pub fn search_sources(&self) -> &[SearchSourcePresentation] {
        self.source
            .as_ref()
            .map(|state| state.search_sources.as_slice())
            .unwrap_or_default()
    }

    pub fn context_summary(&self) -> &[anywhere_protocol::ContextPresentation] {
        self.source
            .as_ref()
            .map(|state| state.context_summary.as_slice())
            .unwrap_or_default()
    }

    pub fn context_complete(&self) -> bool {
        self.source
            .as_ref()
            .is_some_and(|state| state.context_complete)
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
        if self.source.is_none() {
            self.refresh_preview();
        }
    }

    pub fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
        self.rebuild_rows();
    }

    pub fn select_scope(&mut self, scope: Option<String>) {
        self.selected_scope = scope;
        self.menu_path = None;
        self.selected = 0;
        self.rebuild_rows();
    }

    pub fn cycle_scope(&mut self) {
        self.cycle_scope_with_direction(1);
    }

    pub fn cycle_scope_previous(&mut self) {
        self.cycle_scope_with_direction(-1);
    }

    fn cycle_scope_with_direction(&mut self, direction: isize) {
        if self.scopes.is_empty() {
            return;
        }
        let next = match self
            .selected_scope
            .as_ref()
            .and_then(|id| self.scopes.iter().position(|scope| &scope.id == id))
        {
            None if direction < 0 => self.scopes.len() - 1,
            None => 0,
            Some(index) => {
                (index as isize + direction).rem_euclid(self.scopes.len() as isize) as usize
            }
        };
        self.select_scope(Some(self.scopes[next].id.clone()));
    }

    pub fn select_previous(&mut self) {
        if !self.rows.is_empty() {
            self.selected = if self.selected == 0 {
                self.rows.len() - 1
            } else {
                self.selected - 1
            };
        }
        self.skip_disabled(-1);
    }

    pub fn select_first(&mut self) {
        if !self.rows.is_empty() {
            self.selected = 0;
        }
        self.skip_disabled(1);
    }

    pub fn select_last(&mut self) {
        if !self.rows.is_empty() {
            self.selected = self.rows.len() - 1;
        }
        self.skip_disabled(-1);
    }

    pub fn select_next(&mut self) {
        if !self.rows.is_empty() {
            self.selected = (self.selected + 1) % self.rows.len();
        }
        self.skip_disabled(1);
    }

    pub fn select_row(&mut self, index: usize) {
        if index < self.rows.len() {
            self.selected = index;
        }
    }

    pub fn open_selected_menu(&mut self) -> bool {
        let Some(path) = self
            .selected_row()
            .filter(|row| row.is_menu)
            .map(|row| row.menu_path.clone())
        else {
            return false;
        };
        let has_children = self.source.as_ref().is_none_or(|source| {
            source.candidates.iter().any(|candidate| {
                candidate.is_menu
                    && candidate.menu_path.len() > path.len()
                    && candidate
                        .menu_path
                        .iter()
                        .zip(&path)
                        .all(|(left, right)| left == right)
            })
        });
        if !has_children {
            return false;
        }
        self.menu_path = Some(path);
        self.selected = 0;
        self.rebuild_rows();
        true
    }

    pub fn back_menu(&mut self) -> bool {
        let Some(path) = self.menu_path.clone() else {
            return false;
        };
        self.menu_path = if path.is_empty() {
            None
        } else {
            Some(path[..path.len() - 1].to_vec())
        };
        self.selected = 0;
        self.rebuild_rows();
        true
    }

    pub fn menu_path(&self) -> Option<&[String]> {
        self.menu_path.as_deref()
    }

    pub fn show_object_actions(&mut self) {
        if self.object_actions_active {
            return;
        }
        self.object_action_parent = self.selection_id().map(str::to_owned);
        self.object_actions_active = true;
        self.selected = 0;
        self.rebuild_rows();
    }

    pub fn hide_object_actions(&mut self) {
        if !self.object_actions_active {
            return;
        }
        let parent = self.object_action_parent.take();
        self.object_actions_active = false;
        self.rebuild_rows();
        if let Some(index) = parent
            .as_deref()
            .and_then(|id| self.rows.iter().position(|row| row.id == id))
        {
            self.selected = index;
        }
    }

    pub fn workspace(&self) -> LauncherWorkspace {
        LauncherWorkspace {
            query: self.query.clone(),
            selected_scope: self.selected_scope.clone(),
            selection_id: self.selection_id().map(str::to_owned),
            object_actions_active: self.object_actions_active,
            menu_path: self.menu_path.clone(),
            object_action_parent: self.object_action_parent.clone(),
        }
    }

    pub fn restore_workspace(&mut self, workspace: LauncherWorkspace) {
        self.query = workspace.query;
        self.selected_scope = workspace.selected_scope;
        self.object_actions_active = workspace.object_actions_active;
        self.menu_path = workspace.menu_path;
        self.object_action_parent = workspace.object_action_parent;
        self.rebuild_rows();
        if let Some(index) = workspace
            .selection_id
            .as_deref()
            .and_then(|id| self.rows.iter().position(|row| row.id == id))
        {
            self.selected = index;
        }
        self.skip_disabled(1);
    }

    pub fn apply(&mut self, state: LauncherState) {
        self.apply_shared(Arc::new(state));
    }

    pub fn apply_shared(&mut self, state: Arc<LauncherState>) -> LauncherApply {
        let input_changed = self.query != state.input;
        let scopes_changed = self.scopes.len() != state.scopes.len()
            || self
                .scopes
                .iter()
                .zip(&state.scopes)
                .any(|(current, next)| {
                    current.id != next.id
                        || current.label != next.label
                        || current.is_current_app != next.is_current_app
                });
        let rows_changed = self.source.as_ref().is_none_or(|current| {
            self.query != state.input
                || self.selected_scope != state.selected_scope
                || !Arc::ptr_eq(&current.candidates, &state.candidates)
                || current.context_candidates != state.context_candidates
                || (self.object_actions_active && current.result_actions != state.result_actions)
                || current.context_summary != state.context_summary
        });
        let previous_id = self.selection_id().map(str::to_owned);
        self.loading = false;
        self.revision = state.revision;
        self.query = state.input.clone();
        self.selected_scope = state.selected_scope.clone();
        self.scopes = state
            .scopes
            .iter()
            .map(|scope| LauncherScope {
                id: scope.id.clone(),
                label: scope.label.clone(),
                is_current_app: scope.is_current_app,
            })
            .collect();
        self.diagnostics = state.diagnostics.clone();
        self.source = Some(state);
        if rows_changed {
            self.rebuild_rows();
        }
        if let Some(index) = previous_id
            .as_deref()
            .and_then(|id| self.rows.iter().position(|row| row.id == id))
        {
            self.selected = index;
        }
        LauncherApply {
            input_changed,
            rows_changed,
            scopes_changed,
        }
    }

    fn refresh_preview(&mut self) {
        self.rebuild_rows();
    }

    fn rebuild_rows(&mut self) {
        let rows = if let Some(source) = &self.source {
            if self.object_actions_active {
                source
                    .result_actions
                    .iter()
                    .filter(|candidate| candidate.availability.is_visible())
                    .take(MAX_QUERY_ROWS)
                    .map(|candidate| {
                        LauncherRow::from_protocol(candidate, Some(source), self.locale)
                    })
                    .collect()
            } else if self.query.trim().is_empty() && self.menu_path.is_some() {
                menu_rows(
                    source,
                    self.selected_scope.as_deref(),
                    self.menu_path.as_deref().unwrap_or_default(),
                    self.locale,
                )
            } else {
                snapshot_rows(source, self.selected_scope.as_deref(), self.locale)
            }
        } else {
            let query = self.query.trim().to_lowercase();
            self.preview_rows
                .iter()
                .filter(|row| {
                    query.is_empty()
                        || row.title.to_lowercase().contains(&query)
                        || row
                            .subtitle
                            .as_deref()
                            .is_some_and(|value| value.to_lowercase().contains(&query))
                })
                .take(if query.is_empty() {
                    MAX_HOME_ROWS
                } else {
                    MAX_QUERY_ROWS
                })
                .cloned()
                .collect()
        };
        self.rows = rows;
        if self.selected >= self.rows.len() {
            self.selected = 0;
        }
        self.skip_disabled(1);
    }

    fn skip_disabled(&mut self, direction: i8) {
        if self.rows.len() > 1 && self.rows.iter().any(|row| row.enabled) {
            while self.rows.get(self.selected).is_some_and(|row| !row.enabled) {
                self.selected = if direction < 0 {
                    (self.selected + self.rows.len() - 1) % self.rows.len()
                } else {
                    (self.selected + 1) % self.rows.len()
                };
            }
        }
    }
}

fn snapshot_rows(
    state: &LauncherState,
    selected_scope: Option<&str>,
    locale: Locale,
) -> Vec<LauncherRow> {
    let candidates: Vec<&CandidatePresentation> = if !state.input.trim().is_empty() {
        state
            .candidates
            .iter()
            .filter(|candidate| candidate.availability.is_visible())
            .collect()
    } else {
        match selected_scope {
            Some(id) => state.scoped_candidates(&Scope::Group(id.to_owned())),
            None => state
                .candidates
                .iter()
                .filter(|candidate| candidate.availability.is_visible())
                .collect(),
        }
    };
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .take(if state.input.trim().is_empty() {
            MAX_HOME_ROWS
        } else {
            MAX_QUERY_ROWS
        })
        .filter(|candidate| seen.insert(candidate.id.clone()))
        .map(|candidate| LauncherRow::from_protocol(candidate, Some(state), locale))
        .collect()
}

fn preview_rows(locale: Locale) -> Vec<LauncherRow> {
    let values = match locale {
        Locale::En => [
            (
                "open-finder",
                "Open in Finder",
                "Selected folder",
                "folder",
                Some("⌘↩"),
                true,
            ),
            (
                "git-status",
                "Git Status",
                "Anywhere repository",
                "git",
                None,
                true,
            ),
            (
                "file-search",
                "File Search",
                "Search files and folders",
                "search",
                None,
                true,
            ),
            (
                "current-menu",
                "Browse Menus…",
                "Current App",
                "menu",
                None,
                true,
            ),
            (
                "clipboard",
                "Clipboard History",
                "Unavailable until recorded",
                "clipboard",
                None,
                false,
            ),
        ],
        Locale::Zh => [
            (
                "open-finder",
                "在 Finder 中打开",
                "所选文件夹",
                "folder",
                Some("⌘↩"),
                true,
            ),
            ("git-status", "Git 状态", "Anywhere 仓库", "git", None, true),
            (
                "file-search",
                "文件搜索",
                "搜索文件和文件夹",
                "search",
                None,
                true,
            ),
            ("current-menu", "浏览菜单…", "当前应用", "menu", None, true),
            (
                "clipboard",
                "剪贴板历史",
                "开始记录后可用",
                "clipboard",
                None,
                false,
            ),
        ],
    };
    values
        .into_iter()
        .map(
            |(id, title, subtitle, icon, shortcut, enabled)| LauncherRow {
                id: id.to_owned(),
                title: title.to_owned(),
                subtitle: Some(subtitle.to_owned()),
                shortcut: shortcut.map(str::to_owned),
                icon: icon.to_owned(),
                kind: CandidateKind::Action,
                enabled,
                is_menu: false,
                menu_path: Vec::new(),
                command_id: None,
                file_kind: None,
                source_label: None,
                icon_identity: None,
                execution_context_id: None,
                primary_action: false,
            },
        )
        .collect()
}

fn menu_rows(
    state: &LauncherState,
    selected_scope: Option<&str>,
    path: &[String],
    locale: Locale,
) -> Vec<LauncherRow> {
    let candidates: Vec<&CandidatePresentation> = match selected_scope {
        Some(id) => state.scoped_candidates(&Scope::Group(id.to_owned())),
        None => state
            .candidates
            .iter()
            .filter(|candidate| candidate.availability.is_visible())
            .collect(),
    };
    let mut folders = std::collections::BTreeMap::<String, Vec<String>>::new();
    let mut actions = Vec::new();
    for candidate in candidates {
        if !candidate.is_menu
            || candidate.menu_path.len() < path.len()
            || !candidate
                .menu_path
                .iter()
                .zip(path)
                .all(|(left, right)| left == right)
        {
            continue;
        }
        if candidate.menu_path.len() > path.len() {
            let next = candidate.menu_path[path.len()].clone();
            let mut next_path = path.to_vec();
            next_path.push(next.clone());
            folders.entry(next).or_insert(next_path);
        } else {
            actions.push(candidate);
        }
    }
    folders
        .into_iter()
        .map(|(title, menu_path)| LauncherRow {
            id: format!("menu:{}", title.to_ascii_lowercase()),
            title,
            subtitle: None,
            shortcut: None,
            icon: "menu".to_owned(),
            kind: CandidateKind::Action,
            enabled: true,
            is_menu: true,
            menu_path,
            command_id: None,
            file_kind: None,
            source_label: None,
            icon_identity: None,
            execution_context_id: None,
            primary_action: false,
        })
        .chain(
            actions
                .into_iter()
                .map(|candidate| LauncherRow::from_protocol(candidate, Some(state), locale)),
        )
        .take(MAX_QUERY_ROWS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anywhere_protocol::{
        Availability, CandidatePresentation, ContextPresentation, ContextSource, GroupId,
        IconDescriptor, LauncherState, ScopeEntry,
    };

    fn candidate(id: &str, title: &str, enabled: bool) -> CandidatePresentation {
        CandidatePresentation {
            id: id.into(),
            kind: CandidateKind::Action,
            is_menu: false,
            file_kind: None,
            title: title.into(),
            localized_titles: Default::default(),
            subtitle: None,
            shortcut: None,
            icon: IconDescriptor {
                key: "action".into(),
            },
            score: 1.0,
            group_id: "tools".into(),
            availability: if enabled {
                Availability::enabled()
            } else {
                Availability::disabled("offline")
            },
            command_id: Some(id.into()),
            bundle_id: None,
            owning_app: None,
            menu_path: Vec::new(),
            context_keys: Vec::new(),
            execution_context_id: None,
            execution_target_label: None,
            primary_action: false,
            section_label: None,
        }
    }

    #[test]
    fn authority_rows_are_scoped_and_preserve_selection_by_stable_id() {
        let one = candidate("action:1", "One", true);
        let mut two = candidate("action:2", "Two", true);
        two.group_id = "files".into();
        let mut model = LauncherModel::preview(Locale::En);
        model.apply(LauncherState {
            candidates: std::sync::Arc::new(vec![one.clone(), two.clone()]),
            context_candidates: vec![one, two.clone()],
            scopes: vec![ScopeEntry {
                id: "files".into(),
                label: "Files".into(),
                is_current_app: false,
                action_ids: vec![two.id.clone()],
                menu_ids: Vec::new(),
            }],
            ..Default::default()
        });
        model.select_row(1);
        assert_eq!(model.selection_id(), Some("action:2"));
        model.select_scope(Some("files".into()));
        assert_eq!(model.rows.len(), 1);
        assert_eq!(model.selection_id(), Some("action:2"));
    }

    #[test]
    fn a_scoped_query_uses_the_filtered_candidates_instead_of_full_scope_offers() {
        let offers = (0..4)
            .map(|index| candidate(&format!("offer:{index}"), "Full scope offer", true))
            .collect::<Vec<_>>();
        let matches = ["open-finder", "open-terminal"]
            .into_iter()
            .map(|id| candidate(id, "Filtered query result", true))
            .collect::<Vec<_>>();
        let state = LauncherState {
            input: "open".into(),
            candidates: std::sync::Arc::new(matches),
            context_candidates: offers.clone(),
            scopes: vec![ScopeEntry {
                id: "current-app".into(),
                label: "Current App".into(),
                is_current_app: true,
                action_ids: offers
                    .iter()
                    .map(|candidate| candidate.id.clone())
                    .collect(),
                menu_ids: Vec::new(),
            }],
            selected_scope: Some("current-app".into()),
            ..Default::default()
        };
        let mut model = LauncherModel::preview(Locale::En);
        model.apply(state.clone());
        assert_eq!(model.rows.len(), 2);
        assert_eq!(model.selection_id(), Some("open-finder"));

        model.apply(LauncherState {
            input: String::new(),
            ..state
        });
        assert_eq!(model.rows.len(), 4);
        assert_eq!(model.selection_id(), Some("offer:0"));
    }

    #[test]
    fn metadata_only_updates_reuse_the_existing_rows() {
        let candidates = Arc::new(vec![candidate("action:1", "One", true)]);
        let mut model = LauncherModel::preview(Locale::En);
        let initial = model.apply_shared(Arc::new(LauncherState {
            candidates: candidates.clone(),
            input: "one".into(),
            ..Default::default()
        }));
        assert!(initial.rows_changed);
        let row_id = model.rows[0].id.clone();
        let update = model.apply_shared(Arc::new(LauncherState {
            candidates,
            input: "one".into(),
            snapshot_seq: 1,
            diagnostics: vec!["late metadata".into()],
            ..Default::default()
        }));
        assert!(!update.rows_changed);
        assert!(!update.input_changed);
        assert_eq!(model.rows[0].id, row_id);
        assert_eq!(model.diagnostics, vec!["late metadata"]);
    }

    #[test]
    fn preview_search_is_case_insensitive_and_never_leaves_disabled_selected() {
        let mut model = LauncherModel::preview(Locale::En);
        model.set_query("clipboard".into());
        assert_eq!(model.rows.len(), 1);
        assert_eq!(model.selected, 0);
        assert!(model.selected_row().is_some_and(|row| !row.enabled));
    }

    #[test]
    fn previous_selection_skips_a_trailing_disabled_row_backward() {
        let mut model = LauncherModel::preview(Locale::En);
        model.select_previous();
        assert_eq!(model.selection_id(), Some("current-menu"));
    }

    #[test]
    fn scope_cycle_wraps_across_visible_scopes_without_an_unfiltered_stop() {
        let mut model = LauncherModel::preview(Locale::En);
        model.scopes = vec![
            LauncherScope {
                id: GroupId::from("current"),
                label: "Current App".into(),
                is_current_app: true,
            },
            LauncherScope {
                id: GroupId::from("tools"),
                label: "Tools".into(),
                is_current_app: false,
            },
            LauncherScope {
                id: GroupId::from("files"),
                label: "Files".into(),
                is_current_app: false,
            },
        ];
        model.cycle_scope();
        assert_eq!(model.selected_scope.as_deref(), Some("current"));
        model.cycle_scope();
        assert_eq!(model.selected_scope.as_deref(), Some("tools"));
        model.cycle_scope();
        assert_eq!(model.selected_scope.as_deref(), Some("files"));
        model.cycle_scope();
        assert_eq!(model.selected_scope.as_deref(), Some("current"));
        model.cycle_scope_previous();
        assert_eq!(model.selected_scope.as_deref(), Some("files"));
    }

    #[test]
    fn scope_cycle_without_a_selection_starts_at_the_visible_edge() {
        let mut model = LauncherModel::preview(Locale::En);
        model.scopes = vec![
            LauncherScope {
                id: GroupId::from("first"),
                label: "First".into(),
                is_current_app: false,
            },
            LauncherScope {
                id: GroupId::from("last"),
                label: "Last".into(),
                is_current_app: false,
            },
        ];
        model.cycle_scope();
        assert_eq!(model.selected_scope.as_deref(), Some("first"));
        model.select_scope(None);
        model.cycle_scope_previous();
        assert_eq!(model.selected_scope.as_deref(), Some("last"));
    }

    #[test]
    fn source_labels_follow_the_flutter_candidate_row_contract() {
        let mut contextual = candidate("context-action", "Summarize", true);
        contextual.execution_context_id = Some("selected-text".into());
        contextual.context_keys = vec!["selected-text".into()];
        let state = LauncherState {
            candidates: std::sync::Arc::new(vec![contextual]),
            context_summary: vec![ContextPresentation {
                id: "selected-text".into(),
                type_id: "core.text".into(),
                label: "Selected text".into(),
                detail: "A paragraph".into(),
                source: ContextSource::SelectedText,
                key: "selected-text".into(),
                root_key: "selected-text".into(),
                icon_key: "text".into(),
                capabilities: Vec::new(),
                candidate_ids: vec!["context-action".into()],
                primary_candidate_id: Some("context-action".into()),
            }],
            ..Default::default()
        };
        let mut model = LauncherModel::preview(Locale::En);
        model.apply(state);
        assert_eq!(model.rows[0].source_label.as_deref(), Some("Selected text"));

        let mut menu = candidate("menu-action", "Layout", true);
        menu.is_menu = true;
        menu.section_label = Some("Editor".into());
        let mut model = LauncherModel::preview(Locale::En);
        model.apply(LauncherState {
            candidates: std::sync::Arc::new(vec![menu]),
            ..Default::default()
        });
        assert_eq!(model.rows[0].source_label.as_deref(), Some("Editor"));

        let mut file = candidate("file", "report.pdf", true);
        file.kind = CandidateKind::File;
        file.file_kind = Some(FileObjectKind::File);
        let mut model = LauncherModel::preview(Locale::En);
        model.apply(LauncherState {
            candidates: std::sync::Arc::new(vec![file]),
            ..Default::default()
        });
        assert_eq!(model.rows[0].source_label.as_deref(), Some("Files"));
    }

    #[test]
    fn object_actions_require_the_original_primary_target_and_matching_actions() {
        let mut application = candidate("app", "Application", true);
        application.kind = CandidateKind::Application;
        application.command_id = None;
        let mut model = LauncherModel::preview(Locale::En);
        model.apply(LauncherState {
            candidates: std::sync::Arc::new(vec![application]),
            ..Default::default()
        });
        assert!(!model.can_show_object_actions(&model.rows[0], false));

        let mut target = candidate("action", "Website", true);
        target.primary_action = true;
        target.execution_context_id = Some("context:url".into());
        let mut object_action = candidate("open-url", "Open URL", true);
        object_action.execution_context_id = Some("context:url".into());
        model.apply(LauncherState {
            candidates: std::sync::Arc::new(vec![target]),
            result_actions: vec![object_action],
            ..Default::default()
        });
        assert!(model.can_show_object_actions(&model.rows[0], false));

        let mut file = candidate("file", "Report", true);
        file.kind = CandidateKind::File;
        file.primary_action = true;
        file.execution_context_id = Some("context:file".into());
        model.apply(LauncherState {
            candidates: std::sync::Arc::new(vec![file]),
            ..Default::default()
        });
        assert!(model.can_show_object_actions(&model.rows[0], true));
    }
}
