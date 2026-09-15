use crate::context::ResolvedContext;
use crate::query::SearchFields;
use anyhow::{bail, Result};
use anywhere_protocol::*;
use std::collections::BTreeMap;

/// Group ids of the built-in actions.
///
/// Grouping is a presentation concern owned by the registry, so every literal
/// lives here: ranking bonuses, scope membership and labels all refer to these.
pub mod groups {
    pub const GIT: &str = "git";
    pub const FOLDER: &str = "folder";
    pub const FILE: &str = "file";
    pub const WEB: &str = "web";
    pub const JSON: &str = "json";
    /// Launch candidates; never a scope of their own.
    pub const APP: &str = "app";
}

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

/// Built-in command metadata shared by registration and tests.
struct Builtin {
    id: &'static str,
    title: &'static str,
    /// Legacy V0.1 type ids are normalized at registration time.
    accepts: &'static str,
    executor: BuiltinExecutor,
    base_priority: i32,
    icon: &'static str,
    group_id: &'static str,
    aliases: &'static [&'static str],
    keywords: &'static [&'static str],
}

const BUILTINS: &[Builtin] = &[
    Builtin {
        id: "core.git.status",
        title: "Git Status",
        accepts: GIT_REPOSITORY,
        executor: BuiltinExecutor::GitStatus,
        base_priority: 50,
        icon: "git",
        group_id: groups::GIT,
        aliases: &["status"],
        keywords: &["repository", "branch"],
    },
    Builtin {
        id: "core.git.log",
        title: "Git Log",
        accepts: GIT_REPOSITORY,
        executor: BuiltinExecutor::GitLog,
        base_priority: 25,
        icon: "git",
        group_id: groups::GIT,
        aliases: &["log", "history", "commits"],
        keywords: &["repository"],
    },
    Builtin {
        id: "core.git.diff",
        title: "Git Diff",
        accepts: GIT_REPOSITORY,
        executor: BuiltinExecutor::GitDiff,
        base_priority: 15,
        icon: "git",
        group_id: groups::GIT,
        aliases: &["diff", "changes"],
        keywords: &["repository", "worktree"],
    },
    Builtin {
        id: "core.directory.terminal",
        title: "Open Terminal Here",
        accepts: DIRECTORY,
        executor: BuiltinExecutor::OpenTerminal,
        base_priority: 30,
        icon: "terminal",
        group_id: groups::FOLDER,
        aliases: &["terminal", "shell", "console"],
        keywords: &["folder"],
    },
    Builtin {
        id: "core.directory.copy",
        title: "Copy Path",
        accepts: DIRECTORY,
        executor: BuiltinExecutor::CopyPath,
        base_priority: 20,
        icon: "copy",
        group_id: groups::FOLDER,
        aliases: &["copy", "path", "clipboard"],
        keywords: &["folder"],
    },
    Builtin {
        id: "core.directory.open",
        title: "Open Directory",
        accepts: DIRECTORY,
        executor: BuiltinExecutor::OpenDirectory,
        base_priority: 10,
        icon: "folder",
        group_id: groups::FOLDER,
        aliases: &["open", "reveal", "finder"],
        keywords: &["folder"],
    },
    Builtin {
        id: "core.file.open",
        title: "Open File",
        accepts: FILE,
        executor: BuiltinExecutor::OpenFile,
        base_priority: 30,
        icon: "file",
        group_id: groups::FILE,
        aliases: &["open", "edit"],
        keywords: &["document"],
    },
    Builtin {
        id: "core.file.reveal",
        title: "Reveal in File Manager",
        accepts: FILE,
        executor: BuiltinExecutor::RevealFile,
        base_priority: 20,
        icon: "folder",
        group_id: groups::FILE,
        aliases: &["reveal", "finder", "show"],
        keywords: &["document"],
    },
    Builtin {
        id: "core.file.copy",
        title: "Copy Path",
        accepts: FILE,
        executor: BuiltinExecutor::CopyPath,
        base_priority: 10,
        icon: "copy",
        // Both "Copy Path" actions share one group: they do the same thing to a path.
        group_id: groups::FOLDER,
        aliases: &["copy", "path", "clipboard"],
        keywords: &["document"],
    },
    Builtin {
        id: "core.url.open",
        title: "Open in Default Browser",
        accepts: URL,
        executor: BuiltinExecutor::OpenBrowser,
        base_priority: 30,
        icon: "globe",
        group_id: groups::WEB,
        aliases: &["browser", "open"],
        keywords: &["link"],
    },
    Builtin {
        id: "core.url.copy",
        title: "Copy URL",
        accepts: URL,
        executor: BuiltinExecutor::CopyUrl,
        base_priority: 20,
        icon: "copy",
        group_id: groups::WEB,
        aliases: &["copy", "link", "clipboard"],
        keywords: &["url"],
    },
    Builtin {
        id: "core.json.format",
        title: "Format JSON",
        accepts: JSON_LEGACY,
        executor: BuiltinExecutor::FormatJson,
        base_priority: 30,
        icon: "code",
        group_id: groups::JSON,
        aliases: &["pretty", "beautify"],
        keywords: &["document"],
    },
    Builtin {
        id: "core.json.minify",
        title: "Minify JSON",
        accepts: JSON_LEGACY,
        executor: BuiltinExecutor::MinifyJson,
        base_priority: 20,
        icon: "code",
        group_id: groups::JSON,
        aliases: &["compact"],
        keywords: &["document"],
    },
    Builtin {
        id: "core.json.copy",
        title: "Copy Formatted JSON",
        accepts: JSON_LEGACY,
        executor: BuiltinExecutor::CopyFormattedJson,
        base_priority: 10,
        icon: "copy",
        group_id: groups::JSON,
        aliases: &["copy", "clipboard"],
        keywords: &["document"],
    },
];

#[derive(Default)]
pub struct CommandRegistry {
    commands: BTreeMap<CommandId, CommandDescriptor>,
}

impl CommandRegistry {
    pub fn builtin() -> Self {
        let commands = BUILTINS
            .iter()
            .map(|builtin| {
                let descriptor = CommandDescriptor {
                    id: builtin.id.into(),
                    title: builtin.title.into(),
                    subtitle: None,
                    accepts: vec![ContextMatcher {
                        type_id: canonical_context_type(builtin.accepts).into(),
                    }],
                    executor: CommandExecutor::Builtin(builtin.executor),
                    base_priority: builtin.base_priority,
                    icon: builtin.icon.into(),
                    group_id: builtin.group_id.into(),
                    aliases: builtin.aliases.iter().map(|s| (*s).to_owned()).collect(),
                    keywords: builtin.keywords.iter().map(|s| (*s).to_owned()).collect(),
                    owning_app: None,
                    search: SearchFields::new(
                        builtin.title,
                        &builtin
                            .aliases
                            .iter()
                            .map(|s| (*s).to_owned())
                            .collect::<Vec<_>>(),
                        &builtin
                            .keywords
                            .iter()
                            .map(|s| (*s).to_owned())
                            .collect::<Vec<_>>(),
                    ),
                };
                (descriptor.id.clone(), descriptor)
            })
            .collect();
        Self { commands }
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
                if !crate::query::valid_search_field(field) {
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
    use crate::input::text_context;
    use crate::input::InputEngine;

    fn seed(input: &str) -> Vec<ResolvedContext> {
        let mut contexts = InputEngine.resolve(input);
        if let Some(text) = text_context(input) {
            contexts.insert(0, text);
        }
        contexts
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
            crate::context::ContextValue::Directory("/tmp".into()),
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
            crate::context::ContextValue::Directory("/tmp".into()),
            ContextSource::ApplicationContext,
            "test",
        );
        assert!(registry
            .matching(&directory)
            .all(|command| !command.id.starts_with("core.git.")));
        let repository = ResolvedContext::new(
            GIT_REPOSITORY,
            crate::context::ContextValue::Repository("/tmp".into()),
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
