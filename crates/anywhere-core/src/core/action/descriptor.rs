use super::availability::AvailabilityRule;
use super::handler::ActionHandlerRef;
use crate::core::context::ResolvedContext;
use crate::core::search::query::SearchFields;
use anyhow::{bail, Result};
use anywhere_protocol::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub enum ActionExecutor {
    Native(ActionHandlerRef),
    Declarative(DeclarativeAction),
}

#[derive(Debug, Clone)]
pub struct ActionDescriptor {
    pub id: ActionId,
    pub title: String,
    pub subtitle: Option<String>,
    pub shortcut: Option<String>,
    pub accepts: Vec<ContextMatcher>,
    pub executor: ActionExecutor,
    pub availability: AvailabilityRule,
    pub base_priority: i32,
    pub icon: String,
    pub group_id: GroupId,
    pub aliases: Vec<String>,
    pub keywords: Vec<String>,
    /// App that owns this action exclusively, beyond ambient provenance.
    ///
    /// No built-in or v1 plugin sets this yet: the Current App scope is currently
    /// driven by which context an action accepts. It stays so an app-specific action
    /// can declare itself without a protocol change.
    pub owning_app: Option<String>,
    /// Precomputed lowercase search fields.
    pub search: SearchFields,
}

impl ActionDescriptor {
    pub fn matches(&self, context: &ResolvedContext) -> bool {
        self.accepts
            .iter()
            .any(|matcher| matcher.type_id == context.type_id)
    }
}

/// The set of actions available to a session, keyed by id.
///
/// Registration and lookup only. Matching lives in `ActionMatcher`, availability in
/// `availability`, execution in `execute` and registered feature handlers.
#[derive(Clone, Default)]
pub struct ActionRegistry {
    actions: BTreeMap<ActionId, ActionDescriptor>,
}

impl ActionRegistry {
    pub fn register(&mut self, action: ActionDescriptor) -> Result<()> {
        if self.actions.contains_key(&action.id) {
            bail!("duplicate action id: {}", action.id);
        }
        if action.group_id.trim().is_empty() {
            bail!("action {} requires a group id", action.id);
        }
        action.availability.validate()?;
        self.actions.insert(action.id.clone(), action);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ActionDescriptor> {
        self.actions.get(id)
    }

    pub fn matching<'a>(
        &'a self,
        context: &'a ResolvedContext,
    ) -> impl Iterator<Item = &'a ActionDescriptor> {
        self.actions
            .values()
            .filter(|command| command.matches(context))
    }

    pub fn commands(&self) -> impl Iterator<Item = &ActionDescriptor> {
        self.actions.values()
    }

    /// Validate all commands first; a bad plugin never leaves half a registration.
    pub fn register_plugin(&mut self, plugin: PluginManifest) -> Result<()> {
        let default_group = format!("plugin.{}", plugin.id);
        let plugin_name = plugin.name.clone();
        let mut commands = Vec::new();
        for command in plugin.commands {
            let id = format!("{}.{}", plugin.id, command.id);
            if self.actions.contains_key(&id)
                || commands.iter().any(|c: &ActionDescriptor| c.id == id)
            {
                bail!("duplicate command id: {id}");
            }
            // V0.1 manifests use `core.json`; normalize before the compatibility check.
            let accepts = command
                .accepts
                .iter()
                .map(|type_id| canonical_context_type(type_id).to_owned())
                .collect::<Vec<_>>();
            let availability =
                if let DeclarativeAction::ExecuteBuiltin { command_id } = &command.action {
                    let Some(builtin) = self.get(command_id) else {
                        bail!("unknown builtin: {command_id}");
                    };
                    if !matches!(builtin.executor, ActionExecutor::Native(_))
                        || accepts
                            .iter()
                            .any(|kind| !builtin.accepts.iter().any(|m| &m.type_id == kind))
                    {
                        bail!("plugin matcher is incompatible with builtin: {command_id}");
                    }
                    builtin.availability.clone()
                } else {
                    AvailabilityRule::Enabled
                };
            availability.validate()?;
            for field in command
                .aliases
                .iter()
                .chain(command.keywords.iter())
                .chain(std::iter::once(&command.title))
            {
                if !crate::core::search::query::valid_search_field(field) {
                    bail!("command {id} search field exceeds 200 bytes");
                }
            }
            let icon = match command.action {
                DeclarativeAction::OpenUrl { .. } => "globe",
                DeclarativeAction::OpenFile { .. } => "file",
                DeclarativeAction::Copy { .. } => "copy",
                DeclarativeAction::ExecuteBuiltin { .. } => "bolt",
            };
            let aliases = command.aliases.clone();
            let keywords = command.keywords.clone();
            commands.push(ActionDescriptor {
                shortcut: None,
                id,
                search: SearchFields::new(&command.title, &aliases, &keywords),
                title: command.title,
                subtitle: Some(command.subtitle.unwrap_or_else(|| plugin_name.clone())),
                accepts: accepts
                    .into_iter()
                    .map(|type_id| ContextMatcher { type_id })
                    .collect(),
                executor: ActionExecutor::Declarative(command.action),
                availability,
                base_priority: command.base_priority,
                icon: icon.into(),
                group_id: command.group_id.unwrap_or_else(|| default_group.clone()),
                owning_app: None,
                aliases,
                keywords,
            });
        }
        for command in commands {
            self.register(command)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::action::group::groups;
    use crate::features::git::actions::is_git_action;
    use crate::features::recognized_contexts;

    fn seed(input: &str) -> Vec<ResolvedContext> {
        recognized_contexts(input)
    }

    fn json_context() -> ResolvedContext {
        seed("{}")
            .into_iter()
            .find(|c| c.type_id == JSON_DOCUMENT)
            .unwrap()
    }

    #[test]
    fn exact_type_match_and_builtin_command_count() {
        let registry = crate::application::composition::builtin_registry();
        assert_eq!(registry.commands().count(), 14);
        let url = seed("https://github.com")
            .into_iter()
            .find(|c| c.type_id == URL)
            .unwrap();
        let json = json_context();
        assert!(registry.get("core.url.open").unwrap().matches(&url));
        assert!(!registry.get("core.url.open").unwrap().matches(&json));
        assert_eq!(registry.matching(&url).count(), 2);
        assert_eq!(registry.matching(&json).count(), 3);
        let directory = ResolvedContext::new(
            DIRECTORY,
            crate::core::context::ContextValue::Directory("/tmp".into()),
            ContextSource::ApplicationContext,
            "test",
        );
        assert_eq!(registry.matching(&directory).count(), 3);
    }

    #[test]
    fn every_builtin_has_a_group_and_search_fields() {
        let registry = crate::application::composition::builtin_registry();
        for command in registry.commands() {
            assert!(!command.group_id.is_empty(), "{}", command.id);
            assert!(
                !command.search.names().is_empty(),
                "{} needs searchable fields",
                command.id
            );
        }
    }

    #[test]
    fn git_actions_only_accept_repository_context() {
        let registry = crate::application::composition::builtin_registry();
        let directory = ResolvedContext::new(
            DIRECTORY,
            crate::core::context::ContextValue::Directory("/tmp".into()),
            ContextSource::ApplicationContext,
            "test",
        );
        assert!(registry
            .matching(&directory)
            .all(|command| !command.id.starts_with("core.git.")));
        let repository = ResolvedContext::new(
            GIT_REPOSITORY,
            crate::core::context::ContextValue::Repository("/tmp".into()),
            ContextSource::ApplicationContext,
            "test",
        );
        // Registry iteration is id-ordered; ordering itself belongs to the matcher.
        let mut ids = registry
            .matching(&repository)
            .map(|command| command.id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, ["core.git.diff", "core.git.log", "core.git.status"]);
    }

    #[test]
    fn v0_1_json_plugin_is_normalized_and_keeps_working() {
        let mut registry = crate::application::composition::builtin_registry();
        let raw = r#"
protocol_version = 1
id = "legacy"
name = "Legacy"

[[commands]]
id = "pretty"
title = "Pretty JSON"
accepts = ["core.json"]
action = { type = "execute_builtin", command_id = "core.json.format" }
"#;
        let manifest = anywhere_plugin::parse_manifest(raw).unwrap();
        registry.register_plugin(manifest.clone()).unwrap();
        let command = registry.get("legacy.pretty").unwrap();
        assert_eq!(command.accepts[0].type_id, JSON_DOCUMENT);
        assert_eq!(command.group_id, "plugin.legacy");
        // The legacy matcher from the manifest must match the canonical context.
        assert!(command.matches(&json_context()));

        // A mismatched builtin is still rejected after normalization.
        let mut bad = manifest;
        bad.commands[0].action = DeclarativeAction::ExecuteBuiltin {
            command_id: "core.url.open".into(),
        };
        let mut fresh = crate::application::composition::builtin_registry();
        assert!(fresh.register_plugin(bad).is_err());
    }

    #[test]
    fn git_actions_and_dispatch_agree_on_the_same_set() {
        let registry = crate::application::composition::builtin_registry();
        for command in registry.commands() {
            assert_eq!(
                is_git_action(&command.id),
                command.group_id == groups::GIT,
                "{} disagrees between the id predicate and its group",
                command.id
            );
        }
    }

    #[test]
    fn copy_path_actions_share_one_group_across_object_types() {
        let registry = crate::application::composition::builtin_registry();
        for id in ["core.directory.copy", "core.file.copy"] {
            assert_eq!(
                registry.get(id).unwrap().group_id,
                groups::FOLDER,
                "{id} must appear in the same scope as the other Copy Path action"
            );
        }
    }

    #[test]
    fn known_groups_are_the_only_group_ids_in_the_builtin_registry() {
        let registry = crate::application::composition::builtin_registry();
        for command in registry.commands() {
            assert!(
                [
                    groups::GIT,
                    groups::FOLDER,
                    groups::FILE,
                    groups::WEB,
                    groups::JSON,
                ]
                .contains(&command.group_id.as_str()),
                "{} uses an unknown group {}",
                command.id,
                command.group_id
            );
        }
    }

    #[test]
    fn plugins_without_group_metadata_get_an_explicit_default() {
        let mut registry = crate::application::composition::builtin_registry();
        registry
            .register_plugin(
                anywhere_plugin::parse_manifest(anywhere_plugin::EXAMPLE_PLUGIN).unwrap(),
            )
            .unwrap();
        assert_eq!(registry.get("example.search").unwrap().group_id, "web");
    }

    #[test]
    fn register_rejects_unknown_and_mistyped_availability_facts() {
        use crate::core::action::bundle::ActionSpec;
        use crate::core::action::handler::{ActionHandler, ExecutionOutput};
        use crate::core::session::facts::{CLIPBOARD, GIT};

        struct Noop;
        impl ActionHandler for Noop {
            fn execute(
                &self,
                _: &ResolvedContext,
                _: &dyn anywhere_platform::Platform,
            ) -> anyhow::Result<ExecutionOutput> {
                Ok(ExecutionOutput::dismiss())
            }
        }

        let spec = |id: &'static str, availability: AvailabilityRule| {
            ActionSpec {
                id,
                title: "Bad",
                accepts: TEXT,
                handler: &Noop,
                availability,
                base_priority: 0,
                icon: "copy",
                aliases: &[],
                keywords: &[],
            }
            .descriptor("test")
        };

        let mut registry = ActionRegistry::default();
        assert!(registry
            .register(spec("test.unknown", AvailabilityRule::hide_if("nope")))
            .is_err());
        assert!(registry.get("test.unknown").is_none());
        assert!(registry
            .register(spec("test.hide-flag", AvailabilityRule::hide_if(GIT)))
            .is_err());
        assert!(registry
            .register(spec(
                "test.require-text",
                AvailabilityRule::require(CLIPBOARD, "x"),
            ))
            .is_err());
        assert!(registry
            .register(spec(
                "test.require-git",
                AvailabilityRule::require(GIT, "x"),
            ))
            .is_ok());
        assert!(registry.get("test.require-git").is_some());
    }

    const WRAP_PLUGIN: &str = r#"
protocol_version = 1
id = "wrap"
name = "Wrap"

[[commands]]
id = "status"
title = "Wrapped Git Status"
accepts = ["git.repository"]
action = { type = "execute_builtin", command_id = "core.git.status" }

[[commands]]
id = "copy"
title = "Wrapped Copy"
accepts = ["core.directory"]
action = { type = "execute_builtin", command_id = "core.directory.copy" }
"#;

    #[test]
    fn execute_builtin_inherits_the_target_availability_rule() {
        let mut registry = crate::application::composition::builtin_registry();
        registry
            .register_plugin(anywhere_plugin::parse_manifest(WRAP_PLUGIN).unwrap())
            .unwrap();
        assert_eq!(
            registry.get("wrap.status").unwrap().availability,
            registry.get("core.git.status").unwrap().availability
        );
        assert_eq!(
            registry.get("wrap.copy").unwrap().availability,
            registry.get("core.directory.copy").unwrap().availability
        );
        registry
            .register_plugin(
                anywhere_plugin::parse_manifest(anywhere_plugin::EXAMPLE_PLUGIN).unwrap(),
            )
            .unwrap();
        assert_eq!(
            registry.get("example.search").unwrap().availability,
            AvailabilityRule::Enabled
        );
    }

    #[test]
    fn a_bad_wrapping_plugin_does_not_leave_a_partial_registration() {
        let mut registry = crate::application::composition::builtin_registry();
        let raw = format!(
            "{WRAP_PLUGIN}

[[commands]]
id = \"missing\"
title = \"Missing\"
accepts = [\"git.repository\"]
action = {{ type = \"execute_builtin\", command_id = \"core.missing.action\" }}
"
        );
        assert!(anywhere_plugin::parse_manifest(&raw)
            .and_then(|manifest| registry.register_plugin(manifest))
            .is_err());
        assert!(registry.get("wrap.status").is_none());
        assert!(registry.get("wrap.copy").is_none());
        assert!(registry.get("wrap.missing").is_none());
        assert!(registry.get("core.git.status").is_some());
    }
}
