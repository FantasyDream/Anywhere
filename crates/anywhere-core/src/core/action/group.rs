//! Action groups: how actions are organised for the user.
//!
//! A group is presentation only — an id, a label and an icon. It deliberately does *not*
//! carry a context type: applicability always belongs to the action's own `accepts`, so
//! "which group is this in" and "does this apply here" can never disagree.

pub type ActionGroupId = String;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionGroup {
    pub id: ActionGroupId,
    pub title: String,
    pub icon: Option<String>,
}

/// Group ids of the built-in actions.
///
/// Grouping is a presentation concern, so every literal lives here: scope labels, scope
/// ordering and group membership all refer to these.
pub mod groups {
    use super::ActionGroup;

    pub const GIT: &str = "git";
    pub const FOLDER: &str = "folder";
    pub const FILE: &str = "file";
    pub const WEB: &str = "web";
    pub const JSON: &str = "json";
    /// Launch candidates; never a scope of their own.
    pub const APP: &str = "app";

    pub fn git() -> ActionGroup {
        ActionGroup {
            id: GIT.into(),
            title: "Git".into(),
            icon: Some("git".into()),
        }
    }

    pub fn folder() -> ActionGroup {
        ActionGroup {
            id: FOLDER.into(),
            title: "Folder".into(),
            icon: Some("folder".into()),
        }
    }

    pub fn file() -> ActionGroup {
        ActionGroup {
            id: FILE.into(),
            title: "File".into(),
            icon: Some("file".into()),
        }
    }

    pub fn web() -> ActionGroup {
        ActionGroup {
            id: WEB.into(),
            title: "Web".into(),
            icon: Some("globe".into()),
        }
    }

    pub fn json() -> ActionGroup {
        ActionGroup {
            id: JSON.into(),
            title: "JSON".into(),
            icon: Some("code".into()),
        }
    }

    pub fn app() -> ActionGroup {
        ActionGroup {
            id: APP.into(),
            title: "Current App".into(),
            icon: None,
        }
    }

    /// The built-in catalogue, in the order the scope bar falls back to.
    pub fn builtin() -> Vec<ActionGroup> {
        vec![git(), folder(), file(), web(), json(), app()]
    }
}

/// Display label for a group id, falling back to a label derived from the id.
///
/// Plugin groups are not in the catalogue, so they are titled from their id rather than
/// showing a raw `plugin.foo_bar`.
pub fn label_for(group_id: &str) -> String {
    if let Some(group) = groups::builtin()
        .into_iter()
        .find(|group| group.id == group_id)
    {
        return group.title;
    }
    match group_id.strip_prefix("plugin.") {
        Some(plugin) => plugin
            .split(['.', '_', '-'])
            .map(capitalize)
            .collect::<Vec<_>>()
            .join(" "),
        None => capitalize(group_id),
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

    #[test]
    fn labels_come_from_the_catalogue_and_plugin_ids_are_titled() {
        assert_eq!(label_for(groups::GIT), "Git");
        assert_eq!(label_for(groups::JSON), "JSON");
        assert_eq!(label_for("plugin.my_tool"), "My Tool");
        assert_eq!(label_for("custom"), "Custom");
    }

    /// A group carries no context type: applicability belongs to the action.
    #[test]
    fn a_group_is_presentation_only() {
        for group in groups::builtin() {
            assert!(!group.id.is_empty());
            assert!(!group.title.is_empty());
        }
    }
}
