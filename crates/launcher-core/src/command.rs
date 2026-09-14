use crate::context::ResolvedContext;
use anyhow::{bail, Result};
use launcher_protocol::*;
use std::collections::BTreeMap;

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
    WebSearch,
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
}

impl CommandDescriptor {
    pub fn matches(&self, context: &ResolvedContext) -> bool {
        self.accepts
            .iter()
            .any(|matcher| matcher.type_id == context.type_id)
    }
}

#[derive(Default)]
pub struct CommandRegistry {
    commands: BTreeMap<CommandId, CommandDescriptor>,
}

impl CommandRegistry {
    pub fn builtin() -> Self {
        use BuiltinExecutor::*;
        let definitions = [
            (
                "core.directory.terminal",
                "Open Terminal Here",
                DIRECTORY,
                OpenTerminal,
                30,
                "terminal",
            ),
            (
                "core.directory.copy",
                "Copy Path",
                DIRECTORY,
                CopyPath,
                20,
                "copy",
            ),
            (
                "core.directory.open",
                "Open Directory",
                DIRECTORY,
                OpenDirectory,
                10,
                "folder",
            ),
            ("core.file.open", "Open File", FILE, OpenFile, 30, "file"),
            (
                "core.file.reveal",
                "Reveal in File Manager",
                FILE,
                RevealFile,
                20,
                "folder",
            ),
            ("core.file.copy", "Copy Path", FILE, CopyPath, 10, "copy"),
            (
                "core.url.open",
                "Open in Default Browser",
                URL,
                OpenBrowser,
                30,
                "globe",
            ),
            ("core.url.copy", "Copy URL", URL, CopyUrl, 20, "copy"),
            (
                "core.json.format",
                "Format JSON",
                JSON,
                FormatJson,
                30,
                "code",
            ),
            (
                "core.json.minify",
                "Minify JSON",
                JSON,
                MinifyJson,
                20,
                "code",
            ),
            (
                "core.json.copy",
                "Copy Formatted JSON",
                JSON,
                CopyFormattedJson,
                10,
                "copy",
            ),
            (
                "core.text.search",
                "Web Search",
                TEXT,
                WebSearch,
                0,
                "search",
            ),
        ];
        let commands = definitions
            .into_iter()
            .map(|(id, title, kind, executor, priority, icon)| {
                (
                    id.into(),
                    CommandDescriptor {
                        id: id.into(),
                        title: title.into(),
                        subtitle: None,
                        accepts: vec![ContextMatcher {
                            type_id: kind.into(),
                        }],
                        executor: CommandExecutor::Builtin(executor),
                        base_priority: priority,
                        icon: icon.into(),
                    },
                )
            })
            .collect();
        Self { commands }
    }
    pub fn register(&mut self, command: CommandDescriptor) -> Result<()> {
        if self.commands.contains_key(&command.id) {
            bail!("duplicate command id: {}", command.id);
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
        let mut commands = Vec::new();
        for command in plugin.commands {
            let id = format!("{}.{}", plugin.id, command.id);
            if self.commands.contains_key(&id)
                || commands.iter().any(|c: &CommandDescriptor| c.id == id)
            {
                bail!("duplicate command id: {id}");
            }
            if let DeclarativeAction::ExecuteBuiltin { command_id } = &command.action {
                let Some(builtin) = self.get(command_id) else {
                    bail!("unknown builtin: {command_id}");
                };
                if !matches!(builtin.executor, CommandExecutor::Builtin(_))
                    || command
                        .accepts
                        .iter()
                        .any(|kind| !builtin.accepts.iter().any(|m| &m.type_id == kind))
                {
                    bail!("plugin matcher is incompatible with builtin: {command_id}");
                }
            }
            let icon = match command.action {
                DeclarativeAction::OpenUrl { .. } => "globe",
                DeclarativeAction::OpenFile { .. } => "file",
                DeclarativeAction::Copy { .. } => "copy",
                DeclarativeAction::ExecuteBuiltin { .. } => "bolt",
            };
            commands.push(CommandDescriptor {
                id,
                title: command.title,
                subtitle: Some(command.subtitle.unwrap_or_else(|| plugin.name.clone())),
                accepts: command
                    .accepts
                    .into_iter()
                    .map(|type_id| ContextMatcher { type_id })
                    .collect(),
                executor: CommandExecutor::Declarative(command.action),
                base_priority: command.base_priority,
                icon: icon.into(),
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
    use crate::input::InputEngine;
    #[test]
    fn exact_type_match_and_builtin_command_count() {
        let registry = CommandRegistry::builtin();
        assert_eq!(registry.commands().count(), 12);
        let url = InputEngine.resolve("https://github.com").pop().unwrap();
        let json = InputEngine.resolve("{}").pop().unwrap();
        assert!(registry.get("core.url.open").unwrap().matches(&url));
        assert!(!registry.get("core.url.open").unwrap().matches(&json));
        assert_eq!(registry.matching(&url).count(), 2);
        assert_eq!(registry.matching(&json).count(), 3);
        let directory = crate::context::ResolvedContext::new(
            DIRECTORY,
            crate::context::ContextValue::Directory("/tmp".into()),
            ContextSource::ApplicationContext,
            "test",
        );
        assert_eq!(registry.matching(&directory).count(), 3);
    }
}
