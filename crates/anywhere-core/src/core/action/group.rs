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
/// Shared group identities. Product labels and declarations live in features/groups.rs.
pub mod groups {

    pub const GIT: &str = "git";
    pub const FOLDER: &str = "folder";
    pub const FILE: &str = "file";
    pub const WEB: &str = "web";
    pub const JSON: &str = "json";
    pub const RUST: &str = "rust";
    pub const MAVEN: &str = "maven";
    pub const GRADLE: &str = "gradle";
    pub const SPRING_BOOT: &str = "spring_boot";
    pub const NODE: &str = "node";
    pub const TOOLS: &str = "tools";
    /// Launch candidates; never a scope of their own.
    pub const APP: &str = "app";

    pub fn for_app(bundle_id: &str) -> String {
        format!("app.{bundle_id}")
    }
}

/// Title and tie-break rank recorded with a group.
pub trait GroupMeta {
    fn label(&self, group_id: &str) -> String;
    fn rank(&self, group_id: &str) -> u16;
}

/// Display label for a group id, falling back to a label derived from the id.
///
/// Plugin groups are not in the catalogue, so they are titled from their id rather than
/// showing a raw `plugin.foo_bar`.
pub fn label_for(group_id: &str) -> String {
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
        assert_eq!(label_for(groups::JSON), "Json");
        assert_eq!(label_for("plugin.my_tool"), "My Tool");
        assert_eq!(label_for("custom"), "Custom");
    }

    /// A group carries no context type: applicability belongs to the action.
    #[test]
    fn a_group_is_presentation_only() {
        for group in crate::features::groups::builtin() {
            assert!(!group.id.is_empty());
            assert!(!group.title.is_empty());
        }
    }
}
