use crate::core::context::ResolvedContext;
use crate::core::search::query::SearchFields;
use anyhow::{bail, Result};
use anywhere_protocol::*;
use std::collections::BTreeMap;

/// Actions that only a repository context can serve.
pub fn is_git_action(command_id: &str) -> bool {
    matches!(
        command_id,
        "core.git.status" | "core.git.log" | "core.git.diff"
    )
}

#[derive(Debug, Clone, Copy)]
pub enum BuiltinExecutor {
    OpenDirectory,
    OpenTerminal,
    CopyPath,
    OpenFile,
    RevealFile,
    OpenBrowser,
    CopyUrl,
    FormatJson,
    MinifyJson,
    CopyFormattedJson,
    GitStatus,
    GitLog,
    GitDiff,
}

#[derive(Debug, Clone)]
pub enum CommandExecutor {
    Builtin(BuiltinExecutor),
    Declarative(DeclarativeAction),
}

#[derive(Debug, Clone)]
pub struct CommandDescriptor {
    pub id: CommandId,
    pub title: String,
    pub subtitle: Option<String>,
    pub accepts: Vec<ContextMatcher>,
    pub executor: CommandExecutor,
    pub base_priority: i32,
    pub icon: String,
    pub group_id: GroupId,
    pub aliases: Vec<String>,
    pub keywords: Vec<String>,
    /// App that owns this action exclusively, beyond ambient provenance.
    ///
    /// No built-in or v1 plugin sets this yet: the Current App scope is currently
    /// driven by which context a command accepts. It stays so an app-specific action
    /// can declare itself without a protocol change.
    pub owning_app: Option<String>,
    /// Precomputed lowercase search fields.
    pub search: SearchFields,
}

impl CommandDescriptor {
    pub fn matches(&self, context: &ResolvedContext) -> bool {
        self.accepts
            .iter()
            .any(|matcher| matcher.type_id == context.type_id)
    }
}

/// The set of actions available to a session, keyed by id.
///
/// Registration and lookup only. Matching lives in `ActionMatcher`, availability in
/// `availability`, execution in `executor`.
#[derive(Clone, Default)]
pub struct CommandRegistry {
    commands: BTreeMap<CommandId, CommandDescriptor>,
}

impl CommandRegistry {
    /// Every first-party action, registered from the feature bundles.
    pub fn builtin() -> Self {
        let mut registry = Self::default();
        for bundle in crate::features::builtin_action_bundles() {
            for descriptor in bundle.descriptors() {
                // A duplicate here is a programming error, not a user-facing condition.
                let _ = registry.register(descriptor);
            }
        }
        registry
    }

    pub fn register(&mut self, command: CommandDescriptor) -> Result<()> {
        if self.commands.contains_key(&command.id) {
            bail!("duplicate command id: {}", command.id);
        }
        if command.group_id.trim().is_empty() {
            bail!("command {} requires a group id", command.id);
        }
        self.commands.insert(command.id.clone(), command);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&CommandDescriptor> {
        self.commands.get(id)
    }

    pub fn matching<'a>(
        &'a self,
        context: &'a ResolvedContext,
    ) -> impl Iterator<Item = &'a CommandDescriptor> {
        self.commands
            .values()
            .filter(|command| command.matches(context))
    }

    pub fn commands(&self) -> impl Iterator<Item = &CommandDescriptor> {
        self.commands.values()
    }

    /// Validate all commands first; a bad plugin never leaves half a registration.
    pub fn register_plugin(&mut self, plugin: PluginManifest) -> Result<()> {
        let default_group = format!("plugin.{}", plugin.id);
        let plugin_name = plugin.name.clone();
        let mut commands = Vec::new();
        for command in plugin.commands {
            let id = format!("{}.{}", plugin.id, command.id);
            if self.commands.contains_key(&id)
                || commands.iter().any(|c: &CommandDescriptor| c.id == id)
            {
                bail!("duplicate command id: {id}");
            }
            // V0.1 manifests use `core.json`; normalize before the compatibility check.
            let accepts = command
                .accepts
                .iter()
                .map(|type_id| canonical_context_type(type_id).to_owned())
                .collect::<Vec<_>>();
            if let DeclarativeAction::ExecuteBuiltin { command_id } = &command.action {
                let Some(builtin) = self.get(command_id) else {
                    bail!("unknown builtin: {command_id}");
                };
                if !matches!(builtin.executor, CommandExecutor::Builtin(_))
                    || accepts
                        .iter()
                        .any(|kind| !builtin.accepts.iter().any(|m| &m.type_id == kind))
                {
                    bail!("plugin matcher is incompatible with builtin: {command_id}");
                }
            }
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
            commands.push(CommandDescriptor {
                id,
                search: SearchFields::new(&command.title, &aliases, &keywords),
                title: command.title,
                subtitle: Some(command.subtitle.unwrap_or_else(|| plugin_name.clone())),
                accepts: accepts
                    .into_iter()
                    .map(|type_id| ContextMatcher { type_id })
                    .collect(),
                executor: CommandExecutor::Declarative(command.action),
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
        let registry = CommandRegistry::builtin();
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
        let registry = CommandRegistry::builtin();
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
        let registry = CommandRegistry::builtin();
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
        let mut registry = CommandRegistry::builtin();
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
        let mut fresh = CommandRegistry::builtin();
        assert!(fresh.register_plugin(bad).is_err());
    }

    #[test]
    fn git_actions_and_dispatch_agree_on_the_same_set() {
        let registry = CommandRegistry::builtin();
        for command in registry.commands() {
            let builtin_git = matches!(
                command.executor,
                CommandExecutor::Builtin(
                    BuiltinExecutor::GitStatus | BuiltinExecutor::GitLog | BuiltinExecutor::GitDiff
                )
            );
            assert_eq!(
                is_git_action(&command.id),
                builtin_git,
                "{} disagrees between the id predicate and its executor",
                command.id
            );
            // Every git action lives in the git group, so the scope filter agrees too.
            assert_eq!(
                builtin_git,
                command.group_id == groups::GIT,
                "{} is in the wrong group",
                command.id
            );
        }
    }

    #[test]
    fn copy_path_actions_share_one_group_across_object_types() {
        let registry = CommandRegistry::builtin();
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
        let registry = CommandRegistry::builtin();
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
        let mut registry = CommandRegistry::builtin();
        registry
            .register_plugin(
                anywhere_plugin::parse_manifest(anywhere_plugin::EXAMPLE_PLUGIN).unwrap(),
            )
            .unwrap();
        assert_eq!(registry.get("example.search").unwrap().group_id, "web");
    }
}
