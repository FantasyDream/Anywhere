use super::availability::AvailabilityRule;
use super::bundle::ActionBundle;
use super::group::{label_for, ActionGroup, GroupMeta};
use super::handler::ActionHandlerRef;
use crate::core::context::ResolvedContext;
use crate::core::search::query::SearchFields;
use anyhow::{bail, Result};
use anywhere_protocol::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub enum ActionExecutor {
    Native(ActionHandlerRef),
    Declarative(DeclarativeAction),
}

impl ActionExecutor {
    pub fn is_menu(&self) -> bool {
        match self {
            Self::Native(handler) => handler.is_menu(),
            Self::Declarative(_) => false,
        }
    }
    pub fn records_usage(&self) -> bool {
        match self {
            Self::Native(handler) => handler.records_usage(),
            Self::Declarative(_) => true,
        }
    }

    pub fn execution_token(&self) -> Option<u64> {
        match self {
            Self::Native(handler) => handler.execution_token(),
            Self::Declarative(_) => None,
        }
    }
}

/// Author-supplied action. Search text is derived when this is registered.
#[derive(Debug, Clone)]
pub struct ActionDefinition {
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
    pub owning_app: Option<String>,
    pub menu_path: Vec<String>,
    pub home_eligible: bool,
    pub primary_action: bool,
    /// Visible title used for search when it differs from the row title.
    pub search_title: Option<String>,
}

/// An action after registration. `search` is computed from the definition and cannot be set.
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
    /// Menu discoveries set this to their captured application. Other actions use
    /// ambient context provenance. Scope membership is always determined by group_id.
    pub owning_app: Option<String>,
    /// Ancestor menu titles. The row title is the leaf; display joins the two.
    pub menu_path: Vec<String>,
    pub home_eligible: bool,
    pub primary_action: bool,
    search: SearchFields,
}

impl ActionDescriptor {
    fn from_definition(definition: ActionDefinition) -> Self {
        let primary = definition
            .search_title
            .as_deref()
            .unwrap_or(definition.title.as_str());
        let search = SearchFields::new(primary, &definition.aliases, &definition.keywords);
        Self {
            id: definition.id,
            title: definition.title,
            subtitle: definition.subtitle,
            shortcut: definition.shortcut,
            accepts: definition.accepts,
            executor: definition.executor,
            availability: definition.availability,
            base_priority: definition.base_priority,
            icon: definition.icon,
            group_id: definition.group_id,
            aliases: definition.aliases,
            keywords: definition.keywords,
            owning_app: definition.owning_app,
            menu_path: definition.menu_path,
            home_eligible: definition.home_eligible,
            primary_action: definition.primary_action,
            search,
        }
    }

    pub fn search(&self) -> &SearchFields {
        &self.search
    }

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
#[derive(Clone, Debug)]
struct RegisteredGroup {
    title: String,
    rank: u16,
}

#[derive(Clone, Default)]
pub struct ActionRegistry {
    actions: BTreeMap<ActionId, std::sync::Arc<ActionDescriptor>>,
    groups: BTreeMap<GroupId, RegisteredGroup>,
    next_rank: u16,
    factories: Vec<std::sync::Arc<dyn super::bundle::ContextActionFactory>>,
}

impl GroupMeta for ActionRegistry {
    fn label(&self, group_id: &str) -> String {
        self.groups
            .get(group_id)
            .map(|group| group.title.clone())
            .unwrap_or_else(|| label_for(group_id))
    }

    fn rank(&self, group_id: &str) -> u16 {
        self.groups
            .get(group_id)
            .map(|group| group.rank)
            .unwrap_or(u16::MAX)
    }
}

impl ActionRegistry {
    /// Return only a stable business identity, never a scan token or candidate row ID.
    pub fn persistent_binding(&self, id: &str) -> Result<super::identity::PersistentActionBinding> {
        use super::identity::{BindingPolicy, BusinessActionId, PersistentActionBinding};
        let action = self
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("action no longer exists"))?;
        let policy = match &action.executor {
            ActionExecutor::Native(handler) => handler.binding_policy(),
            ActionExecutor::Declarative(DeclarativeAction::ExecuteBuiltin { command_id }) => {
                let target = self
                    .get(command_id)
                    .ok_or_else(|| anyhow::anyhow!("built-in action no longer exists"))?;
                match &target.executor {
                    ActionExecutor::Native(handler) => handler.binding_policy(),
                    _ => bail!("execute_builtin target is not built-in"),
                }
            }
            ActionExecutor::Declarative(_) => BindingPolicy::Persistent,
        };
        if let BindingPolicy::Unavailable { reason } = policy {
            bail!("{reason}");
        }
        Ok(PersistentActionBinding {
            action_id: BusinessActionId::new(action.id.clone()),
            context_types: action
                .accepts
                .iter()
                .map(|matcher| matcher.type_id.clone())
                .collect(),
        })
    }
    /// An absent generated action can still belong to an installed factory. This
    /// does not grant an execution offer or bypass the handler's binding policy.
    pub fn supports_persistent_binding(&self, id: &str) -> bool {
        if self.get(id).is_some() {
            return self.persistent_binding(id).is_ok();
        }
        self.factories
            .iter()
            .any(|factory| factory.supports_persistent_binding(id))
    }
    pub fn register_factory(
        &mut self,
        factory: impl super::bundle::ContextActionFactory + 'static,
    ) {
        self.register_factory_ref(std::sync::Arc::new(factory));
    }

    pub fn register_factory_ref(
        &mut self,
        factory: std::sync::Arc<dyn super::bundle::ContextActionFactory>,
    ) {
        self.factories.push(factory);
    }

    /// Build an immutable snapshot registry. Generated handlers travel with candidates.
    pub fn for_contexts(&self, contexts: &[ResolvedContext]) -> (Self, Vec<String>) {
        let mut registry = self.clone();
        registry.factories.clear();
        let mut diagnostics = Vec::new();
        for factory in &self.factories {
            let expansion = factory.actions(contexts);
            diagnostics.extend(
                expansion
                    .diagnostics
                    .into_iter()
                    .map(|message| format!("context actions [{}]: {message}", factory.id())),
            );
            for action in expansion.actions {
                if let Err(error) = registry.register(action) {
                    diagnostics.push(format!("context actions [{}]: {error}", factory.id()));
                }
            }
        }
        (registry, diagnostics)
    }
    /// Check every action, then insert. A failure leaves the registry unchanged.
    fn commit_batch(&mut self, actions: Vec<ActionDefinition>) -> Result<()> {
        let mut seen = BTreeSet::new();
        for action in &actions {
            if !seen.insert(action.id.clone()) || self.actions.contains_key(&action.id) {
                bail!("duplicate action id: {}", action.id);
            }
            if action.group_id.trim().is_empty() {
                bail!("action {} requires a group id", action.id);
            }
            action.availability.validate()?;
        }
        for action in actions {
            let action = ActionDescriptor::from_definition(action);
            self.actions
                .insert(action.id.clone(), std::sync::Arc::new(action));
        }
        Ok(())
    }

    pub fn register(&mut self, action: ActionDefinition) -> Result<()> {
        self.commit_batch(vec![action])
    }

    /// Actions and the group's display title, or neither.
    pub fn register_bundle(&mut self, bundle: ActionBundle) -> Result<()> {
        let group = bundle.group.clone();
        self.commit_batch(bundle.descriptors())?;
        self.remember_group(group, true);
        Ok(())
    }

    fn remember_group(&mut self, group: ActionGroup, ranked: bool) {
        if group.id.trim().is_empty() || self.groups.contains_key(&group.id) {
            return;
        }
        let rank = if ranked {
            let rank = self.next_rank;
            self.next_rank = self.next_rank.saturating_add(1);
            rank
        } else {
            u16::MAX
        };
        let title = if group.title.trim().is_empty() {
            label_for(&group.id)
        } else {
            group.title
        };
        self.groups
            .insert(group.id, RegisteredGroup { title, rank });
    }

    pub fn get(&self, id: &str) -> Option<&ActionDescriptor> {
        self.actions.get(id).map(std::sync::Arc::as_ref)
    }

    pub fn get_shared(&self, id: &str) -> Option<std::sync::Arc<ActionDescriptor>> {
        self.actions.get(id).cloned()
    }

    pub fn matching<'a>(
        &'a self,
        context: &'a ResolvedContext,
    ) -> impl Iterator<Item = &'a ActionDescriptor> {
        self.actions
            .values()
            .map(std::sync::Arc::as_ref)
            .filter(|command| command.matches(context))
    }

    pub fn commands(&self) -> impl Iterator<Item = &ActionDescriptor> {
        self.actions.values().map(std::sync::Arc::as_ref)
    }

    /// Validate all commands first; a bad plugin never leaves half a registration.
    pub fn register_plugin(&mut self, plugin: PluginManifest) -> Result<()> {
        let default_group = format!("plugin.{}", plugin.id);
        let plugin_name = plugin.name.clone();
        let mut commands = Vec::new();
        for command in plugin.commands {
            let id = format!("{}.{}", plugin.id, command.id);
            if self.actions.contains_key(&id)
                || commands.iter().any(|c: &ActionDefinition| c.id == id)
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
            commands.push(ActionDefinition {
                shortcut: None,
                id,
                title: command.title.clone(),
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
                menu_path: Vec::new(),
                home_eligible: true,
                primary_action: false,
                aliases,
                keywords,
                search_title: None,
            });
        }
        let group_ids = commands
            .iter()
            .map(|command| command.group_id.clone())
            .collect::<Vec<_>>();
        self.commit_batch(commands)?;
        for id in group_ids {
            self.remember_group(
                ActionGroup {
                    id: id.clone(),
                    title: label_for(&id),
                    icon: None,
                },
                false,
            );
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
        let registry = crate::application::builtin_registry();
        assert_eq!(registry.commands().count(), 31);
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
        let registry = crate::application::builtin_registry();
        for command in registry.commands() {
            assert!(!command.group_id.is_empty(), "{}", command.id);
            assert!(
                !command.search().names().is_empty(),
                "{} needs searchable fields",
                command.id
            );
        }
    }

    #[test]
    fn git_actions_only_accept_repository_context() {
        let registry = crate::application::builtin_registry();
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
        assert_eq!(
            ids,
            [
                "core.git.branches",
                "core.git.copy_branch_name",
                "core.git.copy_commit_hash",
                "core.git.copy_remote_url",
                "core.git.copy_repository_path",
                "core.git.copy_short_commit_hash",
                "core.git.diff",
                "core.git.log",
                "core.git.staged_diff",
                "core.git.status"
            ]
        );
    }

    #[test]
    fn v0_1_json_plugin_is_normalized_and_keeps_working() {
        let mut registry = crate::application::builtin_registry();
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
        let mut fresh = crate::application::builtin_registry();
        assert!(fresh.register_plugin(bad).is_err());
    }

    #[test]
    fn git_actions_and_dispatch_agree_on_the_same_set() {
        let registry = crate::application::builtin_registry();
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
        let registry = crate::application::builtin_registry();
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
        let registry = crate::application::builtin_registry();
        for command in registry.commands() {
            assert!(
                [
                    groups::GIT,
                    groups::FOLDER,
                    groups::FILE,
                    groups::WEB,
                    groups::JSON,
                    groups::RUST,
                    groups::MAVEN,
                    groups::GRADLE,
                    groups::NODE,
                    groups::SPRING_BOOT,
                    groups::TOOLS,
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
        let mut registry = crate::application::builtin_registry();
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
                home_eligible: true,
                primary_action: false,
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
        let mut registry = crate::application::builtin_registry();
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
        let mut registry = crate::application::builtin_registry();
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

    #[test]
    fn a_later_invalid_command_does_not_keep_the_earlier_one() {
        let mut registry = ActionRegistry::default();
        let raw = r#"
protocol_version = 1
id = "partial"
name = "Partial"

[[commands]]
id = "ok"
title = "Ok"
accepts = ["core.text"]
group_id = "tools"
action = { type = "copy", text = "x" }

[[commands]]
id = "bad"
title = "Bad"
accepts = ["core.text"]
group_id = ""
action = { type = "copy", text = "y" }
"#;
        assert!(anywhere_plugin::parse_manifest(raw)
            .and_then(|manifest| registry.register_plugin(manifest))
            .is_err());
        assert!(registry.get("partial.ok").is_none());
        assert!(registry.get("partial.bad").is_none());
    }

    #[test]
    fn a_bundle_title_is_the_action_section_label() {
        use crate::core::action::bundle::ActionBundle;
        use crate::core::action::group::ActionGroup;
        let mut registry = ActionRegistry::default();
        registry
            .register_bundle(ActionBundle {
                group: ActionGroup {
                    id: "custom_tools".into(),
                    title: "Custom tools display title".into(),
                    icon: None,
                },
                actions: Vec::new(),
            })
            .unwrap();
        assert_eq!(registry.label("custom_tools"), "Custom tools display title");
    }
}
