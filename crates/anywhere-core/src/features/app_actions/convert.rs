use super::handler::MenuAction;
use crate::core::action::{
    availability::AvailabilityRule,
    descriptor::{ActionDescriptor, ActionExecutor, ActionRegistry},
    group::groups,
    handler::ActionHandlerRef,
};
use crate::core::search::query::SearchFields;
use anywhere_platform::{ApplicationInfo, DiscoveredMenuItem, MenuLocator};
use anywhere_protocol::{ContextMatcher, APPLICATION};
use std::collections::HashMap;

/// Merge through normal registration, including duplicate-ID and availability validation.
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

pub fn descriptors(app: &ApplicationInfo, items: Vec<DiscoveredMenuItem>) -> Vec<ActionDescriptor> {
    let mut counts = HashMap::new();
    for item in &items {
        if let Some(Some(id)) = item.identifiers.last() {
            *counts.entry(id.clone()).or_insert(0usize) += 1;
        }
    }
    items
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
            let title = item.menu_path.join(" › ");
            let mut aliases = vec![item.menu_path.join(" ")];
            let mut keywords = item.menu_path.clone();
            if let Some(shortcut) = &item.shortcut {
                aliases.push(shortcut.clone());
                keywords.push(shortcut.clone());
            }
            ActionDescriptor {
                id,
                title: title.clone(),
                subtitle: None,
                shortcut: item.shortcut,
                accepts: vec![ContextMatcher {
                    type_id: APPLICATION.into(),
                }],
                executor: ActionExecutor::Native(ActionHandlerRef::owned(MenuAction {
                    app: app.clone(),
                    locator: MenuLocator {
                        bundle_id: app.bundle_id.clone(),
                        titles: item.menu_path,
                        identifiers: item.identifiers,
                    },
                })),
                availability: if item.enabled {
                    AvailabilityRule::Enabled
                } else {
                    AvailabilityRule::Hidden
                },
                base_priority: 0,
                icon: "app".into(),
                group_id: groups::APP.into(),
                aliases: aliases.clone(),
                keywords: keywords.clone(),
                owning_app: Some(app.bundle_id.clone()),
                search: SearchFields::new(&title, &aliases, &keywords),
            }
        })
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
    fn unavailable_actions_are_registered_as_hidden_not_dropped() {
        let registry = registry(
            &ActionRegistry::default(),
            &app(),
            vec![item(&["File", "Save"], None, false)],
        )
        .unwrap();
        let action = registry.commands().next().unwrap();
        assert_eq!(action.availability, AvailabilityRule::Hidden);
        assert_eq!(action.group_id, "app");
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
    fn dynamic_actions_use_registry_collision_validation() {
        let same = item(&["File", "Save"], None, true);
        assert!(registry(&ActionRegistry::default(), &app(), vec![same.clone(), same]).is_err());
    }
}
