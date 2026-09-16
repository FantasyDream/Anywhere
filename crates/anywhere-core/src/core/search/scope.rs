//! Scope bar construction and ordering.
//!
//! A scope is a view over the matched candidates: the unified list, the current
//! application's actions, and one entry per action group. The core builds and orders them
//! so the host only renders what it is given — otherwise frecency, late scope arrival and
//! current-app priority would all have to be reimplemented in the UI.
use crate::core::action::group::groups;
use crate::core::search::candidate::Candidate;
use anywhere_protocol::*;
use std::collections::BTreeMap;

/// Rows the host shows for an empty query in the unified scope.
///
/// A *view* limit, deliberately not applied here: truncating the candidate set left each
/// scope's id list pointing at entries that no longer existed, so a group could look
/// populated and then render empty, and a row pushed past the limit lost the identity the
/// selection was holding on to. The core returns every visible candidate and the host
/// decides how many rows fit.
pub const HOME_LIMIT: usize = 7;

/// Scope ids reserved by the host: the unified list first, the current application second.
pub const SCOPE_ALL: &str = "all";
pub const SCOPE_APP: &str = "app";

/// Usage total per group, used to order the bar.
pub type ScopeUse = BTreeMap<GroupId, u32>;

/// Build the scope bar from every visible candidate, before any view limit.
pub fn build(
    candidates: &[Candidate],
    current_app: &Option<String>,
    app_label: &str,
    scope_use: &ScopeUse,
) -> Vec<ScopeEntry> {
    let all_ids = candidates
        .iter()
        .map(|candidate| candidate.presentation.id.clone())
        .collect::<Vec<_>>();
    let mut scopes = vec![
        ScopeEntry {
            id: SCOPE_ALL.into(),
            kind: ScopeKind::All,
            label: "All".into(),
            candidate_ids: all_ids,
        },
        ScopeEntry {
            id: SCOPE_APP.into(),
            kind: ScopeKind::App,
            label: app_label.to_owned(),
            candidate_ids: Vec::new(),
        },
    ];
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
            scopes[1]
                .candidate_ids
                .push(candidate.presentation.id.clone());
        }
        groups
            .entry(candidate.presentation.group_id.clone())
            .or_default()
            .push(candidate.presentation.id.clone());
    }
    // Presentation order is fixed for the built-in groups so the bar reads the same as the
    // documented examples (`Git` before `Folder`); anything else follows in id order.
    let mut ordered = groups.into_iter().collect::<Vec<_>>();
    // Most-used scope first. `group_rank` then the id break ties, so a fresh install still
    // reads like the documented example and the order never depends on the luck of
    // iteration.
    let use_of = |group: &str| scope_use.get(group).copied().unwrap_or_default();
    ordered.sort_by(|(a, _), (b, _)| {
        use_of(b)
            .cmp(&use_of(a))
            .then_with(|| group_rank(a).cmp(&group_rank(b)))
            .then_with(|| a.cmp(b))
    });
    for (group, ids) in ordered {
        // The Current App scope already merges app-owned groups into slot two.
        if group == groups::APP {
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

/// Usage total per group across the matched candidates.
pub fn usage_by_group(
    candidates: &[Candidate],
    usage: &dyn Fn(&str, &str) -> crate::core::ranking::score::UsageStats,
) -> ScopeUse {
    let mut scope_use = ScopeUse::new();
    for candidate in candidates {
        let (Some(action), Some(context)) = (
            candidate.presentation.command_id.as_ref(),
            candidate.context.as_ref(),
        ) else {
            continue;
        };
        let count = usage(action, &context.type_id).execute_count;
        *scope_use
            .entry(candidate.presentation.group_id.clone())
            .or_default() += count;
    }
    scope_use
}

/// Display order of the built-in dynamic scopes.
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

/// Built-in labels for known groups; unknown groups fall back to an id-derived label.
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
