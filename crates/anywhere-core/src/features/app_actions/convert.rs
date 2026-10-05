use super::handler::MenuAction;
use crate::core::action::{
    availability::AvailabilityRule,
    descriptor::{ActionDefinition, ActionExecutor, ActionRegistry},
    group::groups,
    handler::ActionHandlerRef,
};
use anywhere_platform::{ApplicationInfo, DiscoveredMenuItem, MenuLocator};
use anywhere_protocol::{ContextMatcher, APPLICATION};
use std::collections::HashMap;

/// Merge unambiguous discoveries through normal registration and availability validation.
pub fn registry(
    base: &ActionRegistry,
    app: &ApplicationInfo,
    items: Vec<DiscoveredMenuItem>,
) -> anyhow::Result<ActionRegistry> {
    let mut registry = base.clone();
    for action in descriptors(app, items) {
        registry.register(action)?;
    }
    Ok(registry)
}

pub fn descriptors(app: &ApplicationInfo, items: Vec<DiscoveredMenuItem>) -> Vec<ActionDefinition> {
    let mut counts = HashMap::new();
    for item in &items {
        if let Some(Some(id)) = item.identifiers.last() {
            *counts.entry(id.clone()).or_insert(0usize) += 1;
        }
    }
    let actions: Vec<ActionDefinition> = items
        .into_iter()
        .map(|item| {
            // A unique AX identifier survives localization and path movement. Otherwise use
            // the full semantic path, substituting AX identifiers wherever available.
            let unique_leaf = item
                .identifiers
                .last()
                .and_then(Option::as_ref)
                .filter(|id| counts.get(*id) == Some(&1));
            let identity = if let Some(id) = unique_leaf {
                vec![format!("ax:{id}")]
            } else {
                item.menu_path
                    .iter()
                    .enumerate()
                    .map(|(i, title)| {
                        item.identifiers
                            .get(i)
                            .and_then(Option::as_ref)
                            .map(|id| format!("ax:{id}"))
                            .unwrap_or_else(|| format!("title:{title}"))
                    })
                    .collect()
            };
            let id = action_id(&app.bundle_id, &identity);
            let path = item.menu_path.clone();
            let leaf = path.last().cloned().unwrap_or_default();
            let parents = if path.len() > 1 {
                path[..path.len() - 1].to_vec()
            } else {
                Vec::new()
            };
            let search_title = path.join(" › ");
            let mut aliases = vec![path.join(" ")];
            let mut keywords = path.clone();

            if let Some(shortcut) = &item.shortcut {
                aliases.push(shortcut.clone());
                keywords.push(shortcut.clone());
            }
            ActionDefinition {
                id,
                title: leaf.clone(),
                subtitle: None,
                shortcut: item.shortcut,
                accepts: vec![ContextMatcher {
                    type_id: APPLICATION.into(),
                }],
                executor: ActionExecutor::Native(ActionHandlerRef::owned(MenuAction::new(
                    app.clone(),
                    MenuLocator {
                        bundle_id: app.bundle_id.clone(),
                        titles: item.menu_path,
                        identifiers: item.identifiers,
                    },
                ))),
                availability: if item.enabled {
                    AvailabilityRule::Enabled
                } else {
                    AvailabilityRule::Hidden
                },
                base_priority: if recommend(&leaf) { 20 } else { 0 },
                icon: "app".into(),
                group_id: groups::for_app(&app.bundle_id),
                aliases,
                keywords,
                owning_app: Some(app.bundle_id.clone()),
                menu_path: parents,
                home_eligible: recommend(&leaf),
                primary_action: false,
                search_title: Some(search_title),
            }
        })
        .collect();

    // One discovered identity must not resolve to multiple executable menu items.
    // A duplicate used to make registration fail and prevent every menu action from
    // being published. Omit both ambiguous items, preserving unrelated actions.
    let mut id_counts = HashMap::new();
    for action in &actions {
        *id_counts.entry(action.id.clone()).or_insert(0usize) += 1;
    }
    let ambiguous_count = actions
        .iter()
        .filter(|action| id_counts.get(&action.id) != Some(&1))
        .count();
    if ambiguous_count > 0 {
        tracing::warn!(
            bundle_id = %app.bundle_id,
            ambiguous_count,
            "ambiguous_app_actions_omitted"
        );
    }
    actions
        .into_iter()
        .filter(|action| id_counts.get(&action.id) == Some(&1))
        .collect()
}

pub fn action_id(bundle_id: &str, identity: &[String]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for value in std::iter::once(bundle_id).chain(identity.iter().map(String::as_str)) {
        for byte in (value.len() as u64)
            .to_le_bytes()
            .iter()
            .chain(value.as_bytes())
        {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("app.{bundle_id}.{hash:016x}")
}

/// Small generic home heuristic while unverified menu usage is quarantined.
fn recommend(title: &str) -> bool {
    let title = title.to_lowercase();
    [
        "toggle",
        "terminal",
        "command palette",
        "format",
        "new window",
        "save",
        "find",
        "undo",
        "redo",
        "copy",
        "paste",
        "新建",
        "保存",
        "查找",
        "复制",
        "粘贴",
        "终端",
    ]
    .iter()
    .any(|word| title.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> ApplicationInfo {
        ApplicationInfo {
            bundle_id: "test.editor".into(),
            name: "Editor".into(),
            pid: 42,
            instance_id: Some("one".into()),
        }
    }
    fn item(path: &[&str], leaf_id: Option<&str>, enabled: bool) -> DiscoveredMenuItem {
        let mut identifiers = vec![None; path.len()];
        *identifiers.last_mut().unwrap() = leaf_id.map(str::to_owned);
        DiscoveredMenuItem {
            title: path.last().unwrap().to_string(),
            menu_path: path.iter().map(|s| s.to_string()).collect(),
            identifiers,
            shortcut: Some("⌘S".into()),
            enabled,
        }
    }
    #[test]
    fn menu_identity_is_not_eligible_for_a_persistent_binding() {
        let registry = registry(
            &ActionRegistry::default(),
            &app(),
            vec![item(&["File", "Save"], Some("save"), true)],
        )
        .unwrap();
        let id = registry.commands().next().unwrap().id.clone();
        assert!(registry
            .persistent_binding(&id)
            .unwrap_err()
            .to_string()
            .contains("not verified"));
    }
    #[test]
    fn unavailable_actions_are_registered_as_hidden_not_dropped() {
        let registry = registry(
            &ActionRegistry::default(),
            &app(),
            vec![item(&["File", "Save"], None, false)],
        )
        .unwrap();
        let action = registry.commands().next().unwrap();
        assert_eq!(action.availability, AvailabilityRule::Hidden);
        assert_eq!(action.group_id, "app.test.editor");
        assert_eq!(action.shortcut.as_deref(), Some("⌘S"));
    }
    #[test]
    fn inserting_a_sibling_does_not_change_existing_identity() {
        let original = descriptors(&app(), vec![item(&["File", "Save"], None, true)]);
        let inserted = descriptors(
            &app(),
            vec![
                item(&["File", "New"], None, true),
                item(&["File", "Save"], None, true),
            ],
        );
        assert_eq!(original[0].id, inserted[1].id);
    }
    #[test]
    fn rescans_and_new_application_instances_keep_the_same_identity() {
        let original = descriptors(
            &app(),
            vec![
                item(&["File", "Save"], Some("save-document"), true),
                item(&["File", "Close"], None, true),
            ],
        );
        let mut restarted = app();
        restarted.pid = 99;
        restarted.instance_id = Some("two".into());
        let mut changed_metadata = item(&["File", "Save"], Some("save-document"), false);
        changed_metadata.shortcut = Some("⌘⇧S".into());
        let rescan = descriptors(
            &restarted,
            vec![changed_metadata, item(&["File", "Close"], None, false)],
        );
        assert_eq!(original[0].id, rescan[0].id);
        assert_eq!(original[1].id, rescan[1].id);
        assert_eq!(rescan[0].availability, AvailabilityRule::Hidden);
    }
    #[test]
    fn reordering_siblings_does_not_change_their_identities() {
        let before = descriptors(
            &app(),
            vec![
                item(&["File", "New"], None, true),
                item(&["File", "Save"], None, true),
            ],
        );
        let after = descriptors(
            &app(),
            vec![
                item(&["File", "Save"], None, true),
                item(&["File", "New"], None, true),
            ],
        );
        assert_eq!(before[0].id, after[1].id);
        assert_eq!(before[1].id, after[0].id);
    }
    #[test]
    fn ax_identity_survives_localization_and_path_changes() {
        let old = descriptors(
            &app(),
            vec![item(&["File", "Save"], Some("save-document"), true)],
        );
        let new = descriptors(
            &app(),
            vec![item(&["文件", "保存"], Some("save-document"), false)],
        );
        assert_eq!(old[0].id, new[0].id);
        assert_ne!(old[0].availability, new[0].availability);
    }
    #[test]
    fn duplicate_titles_remain_distinct_with_paths_or_identifiers() {
        let actions = descriptors(
            &app(),
            vec![
                item(&["File", "New"], None, true),
                item(&["Window", "New"], None, true),
            ],
        );
        assert_ne!(actions[0].id, actions[1].id);
        let actions = descriptors(
            &app(),
            vec![
                item(&["File", "New"], Some("new-a"), true),
                item(&["File", "New"], Some("new-b"), true),
            ],
        );
        assert_ne!(actions[0].id, actions[1].id);
    }
    #[test]
    fn a_new_duplicate_ax_identifier_never_inherits_the_old_identity() {
        let original = descriptors(
            &app(),
            vec![item(&["File", "Save"], Some("reused-id"), true)],
        )[0]
        .id
        .clone();
        let duplicated = descriptors(
            &app(),
            vec![
                item(&["File", "Save"], Some("reused-id"), true),
                item(&["Window", "Save"], Some("reused-id"), true),
            ],
        );
        assert_eq!(duplicated.len(), 2);
        assert_ne!(duplicated[0].id, duplicated[1].id);
        assert!(duplicated.iter().all(|action| action.id != original));
        let unambiguous = descriptors(
            &app(),
            vec![item(&["File", "Save"], Some("reused-id"), true)],
        );
        assert_eq!(unambiguous[0].id, original);
    }
    #[test]
    fn title_fallback_changes_identity_when_its_path_changes() {
        let original = descriptors(&app(), vec![item(&["File", "Save"], None, true)]);
        let renamed = descriptors(&app(), vec![item(&["File", "Save As"], None, true)]);
        assert_ne!(original[0].id, renamed[0].id);
    }
    #[test]
    fn identity_parts_and_bundle_ids_do_not_collide() {
        assert_ne!(
            action_id("test.editor", &["title:File".into(), "title:Save".into()]),
            action_id("test.editor", &["title:FileSave".into()]),
        );
        assert_ne!(
            action_id("test.editor", &["ax:save".into()]),
            action_id("test.other", &["ax:save".into()]),
        );
    }
    #[test]
    fn ambiguous_identity_omits_only_colliding_actions() {
        let same = item(&["File", "Save"], None, true);
        let actions = descriptors(
            &app(),
            vec![
                same.clone(),
                same.clone(),
                item(&["File", "Open"], None, true),
            ],
        );
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Open");
        let registry = registry(
            &ActionRegistry::default(),
            &app(),
            vec![same.clone(), same, item(&["File", "Open"], None, true)],
        )
        .unwrap();
        assert_eq!(registry.commands().count(), 1);
        assert_eq!(registry.commands().next().unwrap().title, "Open");
    }
}
