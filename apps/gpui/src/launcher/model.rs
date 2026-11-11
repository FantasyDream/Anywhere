use crate::l10n::Locale;
use anywhere_protocol::{
    AvailabilityState, CandidateKind, CandidatePresentation, FileSearchProgress, LauncherState,
    Scope, SearchSourcePresentation,
};

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
}

impl LauncherRow {
    fn from_protocol(candidate: &CandidatePresentation, locale: Locale) -> Self {
        let localized = match locale {
            Locale::En => candidate.localized_titles.get("en"),
            Locale::Zh => candidate.localized_titles.get("zh"),
        };
        let menu = candidate.menu_path.join(" › ");
        let subtitle = (!menu.is_empty())
            .then_some(menu)
            .or_else(|| candidate.subtitle.clone())
            .filter(|value| !value.trim().is_empty() && value != &candidate.title);
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
        }
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
    source: Option<LauncherState>,
    preview_rows: Vec<LauncherRow>,
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

    pub fn loading(locale: Locale) -> Self {
        Self {
            loading: true,
            ..Self::preview(locale)
        }
    }

    pub fn is_preview(&self) -> bool {
        self.source.is_none()
    }

    pub fn selection_id(&self) -> Option<&str> {
        self.rows.get(self.selected).map(|row| row.id.as_str())
    }

    pub fn selected_row(&self) -> Option<&LauncherRow> {
        self.rows.get(self.selected)
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
        if self.scopes.is_empty() {
            return;
        }
        let next = match self
            .selected_scope
            .as_ref()
            .and_then(|id| self.scopes.iter().position(|scope| &scope.id == id))
        {
            None => Some(self.scopes[0].id.clone()),
            Some(index) if index + 1 < self.scopes.len() => Some(self.scopes[index + 1].id.clone()),
            Some(_) => None,
        };
        self.select_scope(next);
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

    pub fn apply(&mut self, state: LauncherState) {
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
        self.rebuild_rows();
        if let Some(index) = previous_id
            .as_deref()
            .and_then(|id| self.rows.iter().position(|row| row.id == id))
        {
            self.selected = index;
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
                    .map(|candidate| LauncherRow::from_protocol(candidate, self.locale))
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
    let candidates: Vec<&CandidatePresentation> = match selected_scope {
        Some(id) => state.scoped_candidates(&Scope::Group(id.to_owned())),
        None => state
            .candidates
            .iter()
            .filter(|candidate| candidate.availability.is_visible())
            .collect(),
    };
    candidates
        .into_iter()
        .take(if state.input.trim().is_empty() {
            MAX_HOME_ROWS
        } else {
            MAX_QUERY_ROWS
        })
        .map(|candidate| LauncherRow::from_protocol(candidate, locale))
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
        })
        .chain(
            actions
                .into_iter()
                .map(|candidate| LauncherRow::from_protocol(candidate, locale)),
        )
        .take(MAX_QUERY_ROWS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anywhere_protocol::{
        Availability, CandidatePresentation, GroupId, IconDescriptor, LauncherState, ScopeEntry,
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
    fn scope_cycle_returns_to_universal() {
        let mut model = LauncherModel::preview(Locale::En);
        model.scopes = vec![LauncherScope {
            id: GroupId::from("tools"),
            label: "Tools".into(),
            is_current_app: false,
        }];
        model.cycle_scope();
        assert_eq!(model.selected_scope.as_deref(), Some("tools"));
        model.cycle_scope();
        assert_eq!(model.selected_scope, None);
    }
}
