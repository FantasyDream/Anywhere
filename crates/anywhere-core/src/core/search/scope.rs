//! Scope bar construction and ordering.
//!
//! A scope is a hard filter over matched candidates: the current application's actions
//! and one entry per action group. Global search is `scope = None` on the host, not a
//! synthetic All row. The core builds membership; the session freezes presentation order.
use crate::core::action::group::groups;
use crate::core::search::candidate::Candidate;
use anywhere_protocol::*;
use std::collections::{BTreeMap, HashMap};

/// Rows the host shows for an empty query.
///
/// A *view* limit, deliberately not applied here: truncating the candidate set left each
/// scope's id list pointing at entries that no longer existed, so a group could look
/// populated and then render empty, and a row pushed past the limit lost the identity the
/// selection was holding on to. The core returns every visible candidate and the host
/// decides how many rows fit.
pub const HOME_LIMIT: usize = 7;

/// Reserved id for the current application's visible actions.
pub const SCOPE_APP: &str = "app";

/// Bounded frequency plus recency per distinct action, used to order scopes.
pub type ScopeUse = BTreeMap<GroupId, u32>;

/// Build the scope bar from every visible candidate, before any view limit.
pub fn build(
    candidates: &[Candidate],
    current_app: &Option<String>,
    app_label: &str,
    scope_use: &ScopeUse,
) -> Vec<ScopeEntry> {
    let mut app_ids = Vec::new();
    let mut groups: BTreeMap<GroupId, Vec<ActionId>> = BTreeMap::new();
    for candidate in candidates {
        if candidate.presentation.kind != CandidateKind::Action {
            continue;
        }
        let owns_current_app = match (&candidate.presentation.owning_app, current_app.as_deref()) {
            (Some(owning), Some(current)) => owning == current,
            _ => false,
        };
        if owns_current_app {
            app_ids.push(candidate.presentation.id.clone());
        }
        groups
            .entry(candidate.presentation.group_id.clone())
            .or_default()
            .push(candidate.presentation.id.clone());
    }
    let mut scopes = Vec::new();
    if !app_ids.is_empty() {
        scopes.push(ScopeEntry {
            id: SCOPE_APP.into(),
            kind: ScopeKind::App,
            label: app_label.to_owned(),
            candidate_ids: app_ids,
        });
    }
    let mut ordered = groups.into_iter().collect::<Vec<_>>();
    let use_of = |group: &str| scope_use.get(group).copied().unwrap_or_default();
    ordered.sort_by(|(a, _), (b, _)| {
        use_of(b)
            .cmp(&use_of(a))
            .then_with(|| group_rank(a).cmp(&group_rank(b)))
            .then_with(|| a.cmp(b))
    });
    for (group, ids) in ordered {
        if group == groups::APP || ids.is_empty() {
            continue;
        }
        scopes.push(ScopeEntry {
            id: group.clone(),
            kind: ScopeKind::Group,
            label: group_label(&group),
            candidate_ids: ids,
        });
    }
    scopes
}

/// Keep session order: Current App first, already-seen groups in place, newcomers append.
pub fn apply_session_order(
    built: Vec<ScopeEntry>,
    frozen_ids: &mut Vec<ScopeId>,
) -> Vec<ScopeEntry> {
    let mut by_id = HashMap::with_capacity(built.len());
    let mut built_ids = Vec::with_capacity(built.len());
    for scope in built {
        built_ids.push(scope.id.clone());
        by_id.insert(scope.id.clone(), scope);
    }
    if by_id.contains_key(SCOPE_APP) && frozen_ids.first().map(String::as_str) != Some(SCOPE_APP) {
        frozen_ids.retain(|id| id != SCOPE_APP);
        frozen_ids.insert(0, SCOPE_APP.into());
    }
    for id in &built_ids {
        if id == SCOPE_APP {
            continue;
        }
        if !frozen_ids.iter().any(|frozen| frozen == id) {
            frozen_ids.push(id.clone());
        }
    }
    frozen_ids
        .iter()
        .filter_map(|id| by_id.remove(id))
        .collect()
}

/// Frecency per group across distinct visible actions.
pub fn usage_by_group(
    candidates: &[Candidate],
    usage: &dyn Fn(&str, &str) -> crate::core::ranking::score::UsageStats,
    now: i64,
) -> ScopeUse {
    let mut scope_use = ScopeUse::new();
    let mut seen = std::collections::HashSet::new();
    for candidate in candidates {
        let (Some(action), Some(context)) = (
            candidate.presentation.command_id.as_ref(),
            candidate.context(),
        ) else {
            continue;
        };
        if !seen.insert((action, &context.type_id)) {
            continue;
        }
        let stats = usage(action, &context.type_id);
        let (frequency, recency) = crate::core::ranking::score::usage_terms(
            stats.execute_count,
            stats.last_executed_at,
            now,
        );
        let score = scope_use
            .entry(candidate.presentation.group_id.clone())
            .or_default();
        *score = score.saturating_add((frequency + recency) as u32);
    }
    scope_use
}

fn group_rank(group: &str) -> u8 {
    match group {
        groups::GIT => 0,
        groups::FOLDER => 1,
        groups::FILE => 2,
        groups::WEB => 3,
        groups::JSON => 4,
        _ => 5,
    }
}

fn group_label(group: &str) -> String {
    match group {
        groups::GIT => "Git".into(),
        groups::FOLDER => "Folder".into(),
        groups::FILE => "File".into(),
        groups::WEB => "Web".into(),
        groups::JSON => "JSON".into(),
        groups::APP => "Current App".into(),
        other => match other.strip_prefix("plugin.") {
            Some(plugin) => plugin
                .split(['.', '_', '-'])
                .map(capitalize)
                .collect::<Vec<_>>()
                .join(" "),
            None => capitalize(other),
        },
    }
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, kind: ScopeKind) -> ScopeEntry {
        ScopeEntry {
            id: id.into(),
            kind,
            label: id.into(),
            candidate_ids: vec!["x".into()],
        }
    }

    fn ids(scopes: &[ScopeEntry]) -> Vec<&str> {
        scopes.iter().map(|scope| scope.id.as_str()).collect()
    }

    #[test]
    fn late_groups_append_and_current_app_inserts_first() {
        let mut frozen = Vec::new();
        let first = apply_session_order(vec![entry("folder", ScopeKind::Group)], &mut frozen);
        assert_eq!(ids(&first), ["folder"]);
        let late = apply_session_order(
            vec![
                entry("app", ScopeKind::App),
                entry("git", ScopeKind::Group),
                entry("folder", ScopeKind::Group),
            ],
            &mut frozen,
        );
        assert_eq!(ids(&late), ["app", "folder", "git"]);
    }

    #[test]
    fn hidden_groups_keep_their_frozen_slot() {
        let mut frozen = Vec::new();
        apply_session_order(
            vec![
                entry("app", ScopeKind::App),
                entry("git", ScopeKind::Group),
                entry("folder", ScopeKind::Group),
            ],
            &mut frozen,
        );
        let hidden = apply_session_order(
            vec![
                entry("app", ScopeKind::App),
                entry("folder", ScopeKind::Group),
            ],
            &mut frozen,
        );
        assert_eq!(ids(&hidden), ["app", "folder"]);
        let restored = apply_session_order(
            vec![
                entry("app", ScopeKind::App),
                entry("folder", ScopeKind::Group),
                entry("git", ScopeKind::Group),
            ],
            &mut frozen,
        );
        assert_eq!(ids(&restored), ["app", "git", "folder"]);
    }
}
