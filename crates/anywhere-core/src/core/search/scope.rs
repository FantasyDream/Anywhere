//! Available UX domains, derived only from visible action offers.
use super::candidate::Candidate;
use crate::core::action::descriptor::ActionRegistry;
use crate::core::action::group::{groups, GroupMeta};
use crate::core::ranking::score::{usage_terms, UsageStats};
use anywhere_platform::ApplicationInfo;
use anywhere_protocol::*;
use std::collections::{BTreeMap, HashMap, HashSet};

pub fn build(
    offers: &[Candidate],
    registry: &ActionRegistry,
    usage: &HashMap<(String, String), UsageStats>,
    current_app: Option<&ApplicationInfo>,
    now: i64,
) -> Vec<ScopeEntry> {
    let current_group = current_app.map(|app| groups::for_app(&app.bundle_id));
    let mut groups: BTreeMap<String, (ScopeEntry, i32, i32)> = BTreeMap::new();
    let mut counted = HashSet::new();
    for offer in offers {
        let candidate = &offer.presentation;
        if candidate.kind != CandidateKind::Action || !candidate.availability.is_visible() {
            continue;
        }
        let entry = groups.entry(candidate.group_id.clone()).or_insert_with(|| {
            (
                ScopeEntry {
                    id: candidate.group_id.clone(),
                    label: candidate
                        .section_label
                        .clone()
                        .unwrap_or_else(|| registry.label(&candidate.group_id)),
                    is_current_app: current_group.as_ref() == Some(&candidate.group_id),
                    action_ids: Vec::new(),
                    menu_ids: Vec::new(),
                },
                0,
                i32::MIN,
            )
        });
        if let (Some(command), Some(context)) = (candidate.command_id.as_ref(), offer.context()) {
            let key = (command.clone(), context.type_id.clone());
            if offer.target.records_usage() && counted.insert(key.clone()) {
                let stats = usage.get(&key).copied().unwrap_or_default();
                let (frequency, recency) =
                    usage_terms(stats.execute_count, stats.last_executed_at, now);
                entry.1 = entry.1.saturating_add(frequency + recency);
            }
            if let Some(action) = registry.get(command) {
                entry.2 = entry.2.max(action.base_priority);
            }
        }
    }
    // Current App is the action home for this captured environment. Its children
    // keep their original domain groups; menus are only one of its action sources.
    if !groups.is_empty() {
        if let Some(app) = current_app {
            let entry = groups
                .entry(groups::for_app(&app.bundle_id))
                .or_insert_with(|| {
                    (
                        ScopeEntry {
                            id: groups::for_app(&app.bundle_id),
                            label: app.name.clone(),
                            is_current_app: true,
                            action_ids: Vec::new(),
                            menu_ids: Vec::new(),
                        },
                        0,
                        i32::MIN,
                    )
                });
            entry.0.label.clone_from(&app.name);
            entry.0.is_current_app = true;
        }
    }
    let mut entries: Vec<_> = groups.into_values().collect();
    entries.sort_by(|a, b| {
        b.0.is_current_app
            .cmp(&a.0.is_current_app)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| a.0.id.cmp(&b.0.id))
    });
    entries
        .into_iter()
        .map(|entry| {
            let mut scope = entry.0;
            for offer in offers {
                let c = &offer.presentation;
                if scope.contains(c) {
                    scope.action_ids.push(c.id.clone());
                }
                if c.is_menu && c.availability.is_visible() && c.group_id == scope.id {
                    scope.menu_ids.push(c.id.clone());
                }
            }
            scope
        })
        .collect()
}

/// Current App includes every applicable action. Other scopes filter one domain.
/// The caller must still enforce Action kind for any selected scope, so this
/// never admits application launch candidates into Current App.
pub fn group_filter<'a>(
    scope_id: Option<&'a str>,
    current_app: Option<&ApplicationInfo>,
) -> Option<&'a str> {
    scope_id.filter(|scope| current_app.is_none_or(|app| *scope != groups::for_app(&app.bundle_id)))
}

/// Remember slots for the entire session. Hidden/removed groups vacate their slot;
/// query changes cannot reorder it, and a returning group keeps the same position.
pub fn freeze(scopes: &mut [ScopeEntry], order: &mut Vec<GroupId>) {
    for scope in scopes.iter() {
        if !order.contains(&scope.id) {
            order.push(scope.id.clone());
        }
    }
    scopes.sort_by_key(|scope| order.iter().position(|id| id == &scope.id).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope(id: &str) -> ScopeEntry {
        ScopeEntry {
            id: id.into(),
            label: id.into(),
            is_current_app: false,
            action_ids: Vec::new(),
            menu_ids: Vec::new(),
        }
    }
    #[test]
    fn late_groups_append_and_hidden_groups_keep_their_slot() {
        let mut order = vec!["app.editor".into()];
        let mut initial = vec![scope("folder"), scope("web")];
        freeze(&mut initial, &mut order);
        let mut later = vec![
            scope("git"),
            scope("folder"),
            scope("web"),
            scope("app.editor"),
        ];
        freeze(&mut later, &mut order);
        assert_eq!(
            later
                .iter()
                .map(|scope| scope.id.as_str())
                .collect::<Vec<_>>(),
            ["app.editor", "folder", "web", "git"]
        );
        let mut hidden = vec![scope("git"), scope("folder")];
        freeze(&mut hidden, &mut order);
        assert_eq!(hidden[0].id, "folder");
        let mut restored = vec![scope("git"), scope("web"), scope("folder")];
        freeze(&mut restored, &mut order);
        assert_eq!(
            restored
                .iter()
                .map(|scope| scope.id.as_str())
                .collect::<Vec<_>>(),
            ["folder", "web", "git"]
        );
    }
    #[test]
    fn group_order_uses_execution_history_instead_of_candidate_rank() {
        use crate::core::context::{ContextValue, ResolvedContext};
        use crate::core::search::{engine::ActionSearch, query::Query};
        use crate::core::session::facts::{FactValue, SessionFacts, GIT};
        let registry = crate::application::builtin_registry();
        let directory = tempfile::tempdir().unwrap();
        let contexts = vec![
            ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory(directory.path().into()),
                ContextSource::ApplicationContext,
                "test",
            ),
            ResolvedContext::new(
                GIT_REPOSITORY,
                ContextValue::Repository(directory.path().into()),
                ContextSource::ApplicationContext,
                "test",
            ),
        ];
        let facts = SessionFacts::default().with(GIT, FactValue::Flag(true));
        let no_usage = |_: &str, _: &str| UsageStats::default();
        let search = ActionSearch::new(
            &registry,
            &no_usage,
            &facts,
            1000,
            crate::features::context_semantic,
        );
        let offers = search.search(&Query::parse(""), &contexts, &[]).candidates;
        assert_eq!(offers[0].presentation.group_id, "git");
        let mut history = HashMap::new();
        history.insert(
            ("core.directory.copy".into(), DIRECTORY.into()),
            UsageStats {
                execute_count: 10,
                last_executed_at: 1000,
            },
        );
        let scopes = build(&offers, &registry, &history, None, 1000);
        assert_eq!(
            scopes[0].id, "folder",
            "Group frecency comes from executions, not the leading candidate's context/rank"
        );
    }
    #[test]
    fn unverified_menu_history_cannot_change_scope_order() {
        use crate::core::context::{ContextValue, ResolvedContext};
        use crate::core::search::{engine::ActionSearch, query::Query};
        use crate::core::session::facts::SessionFacts;
        use anywhere_platform::{ApplicationInfo, DiscoveredMenuItem};
        use anywhere_protocol::APPLICATION;
        let app = ApplicationInfo {
            bundle_id: "test.editor".into(),
            name: "Editor".into(),
            pid: 42,
            instance_id: Some("one".into()),
        };
        let registry = crate::features::app_actions::convert::registry(
            &crate::application::builtin_registry(),
            &app,
            vec![DiscoveredMenuItem {
                title: "Uncommon".into(),
                menu_path: vec!["Tools".into(), "Uncommon".into()],
                identifiers: vec![None, Some("reused".into())],
                shortcut: None,
                enabled: true,
            }],
        )
        .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let contexts = vec![
            ResolvedContext::new(
                DIRECTORY,
                ContextValue::Directory(directory.path().into()),
                ContextSource::ApplicationContext,
                "test",
            ),
            ResolvedContext::new(
                APPLICATION,
                ContextValue::Application {
                    bundle_id: app.bundle_id.clone(),
                    name: app.name.clone(),
                },
                ContextSource::ApplicationContext,
                "test",
            ),
        ];
        let facts = SessionFacts::default();
        let no_usage = |_: &str, _: &str| UsageStats::default();
        let search = ActionSearch::new(
            &registry,
            &no_usage,
            &facts,
            1000,
            crate::features::context_semantic,
        );
        let offers = search.search(&Query::parse(""), &contexts, &[]).candidates;
        let menu = offers
            .iter()
            .find(|offer| offer.presentation.title == "Uncommon")
            .unwrap();
        let baseline = build(&offers, &registry, &HashMap::new(), None, 1000);
        let history = HashMap::from([(
            (
                menu.presentation.command_id.clone().unwrap(),
                APPLICATION.into(),
            ),
            UsageStats {
                execute_count: u32::MAX,
                last_executed_at: 1000,
            },
        )]);
        let actual = build(&offers, &registry, &history, None, 1000);
        assert_eq!(
            actual.iter().map(|scope| &scope.id).collect::<Vec<_>>(),
            baseline.iter().map(|scope| &scope.id).collect::<Vec<_>>()
        );
    }
}
