use super::availability::AvailabilityRule;
use super::bundle::ActionBundle;
use super::group::{label_for, ActionGroup, GroupMeta};
use super::handler::ActionHandlerRef;
use crate::core::context::ResolvedContext;
use crate::core::search::query::{Query, SearchFields};
use anyhow::{bail, Result};
use anywhere_protocol::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub enum ActionExecutor {
    WebCommand(super::web_command::WebCommandRef),
    Native(ActionHandlerRef),
}

impl ActionExecutor {
    pub fn is_menu(&self) -> bool {
        match self {
            Self::Native(handler) => handler.is_menu(),
            Self::WebCommand(_) => false,
        }
    }
    pub fn records_usage(&self) -> bool {
        match self {
            Self::Native(handler) => handler.records_usage(),
            Self::WebCommand(_) => false, // Recorded once after the qualified document is ready.
        }
    }

    /// Web navigation records later at ready, but still consumes its existing frecency.
    pub fn uses_usage(&self) -> bool {
        matches!(self, Self::WebCommand(_)) || self.records_usage()
    }

    pub fn execution_token(&self) -> Option<u64> {
        match self {
            Self::Native(handler) => handler.execution_token(),
            Self::WebCommand(_) => None,
        }
    }
}

/// Author-supplied action. Search text is derived when this is registered.
#[derive(Debug, Clone)]
pub struct ActionDefinition {
    pub id: ActionId,
    pub title: String,
    pub localized_titles: BTreeMap<String, String>,
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
    pub primary_action: bool,
    /// Visible title used for search when it differs from the row title.
    pub search_title: Option<String>,
}

/// An action after registration. `search` is computed from the definition and cannot be set.
#[derive(Debug, Clone)]
pub struct ActionDescriptor {
    pub id: ActionId,
    pub title: String,
    pub localized_titles: BTreeMap<String, String>,
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
    pub primary_action: bool,
    search: SearchFields,
    // Usually the display title is already the first cached search name. Only
    // retain a separate normalized title when it differs (or the title is empty).
    display_title_search: Option<String>,
    contributor: Option<std::sync::Arc<str>>,
}

impl ActionDescriptor {
    fn from_definition(mut definition: ActionDefinition) -> Self {
        for title in definition.localized_titles.values() {
            if title != &definition.title && !definition.aliases.contains(title) {
                definition.aliases.push(title.clone());
            }
        }
        let primary = definition
            .search_title
            .as_deref()
            .unwrap_or(definition.title.as_str());
        let search = SearchFields::new(primary, &definition.aliases, &definition.keywords);
        let display_title_search = (primary != definition.title || definition.title.is_empty())
            .then(|| definition.title.to_lowercase());
        Self {
            id: definition.id,
            title: definition.title,
            localized_titles: definition.localized_titles,
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
            primary_action: definition.primary_action,
            search,
            display_title_search,
            contributor: None,
        }
    }

    /// Registration ownership, independent of display groups and business action ids.
    pub fn contributor(&self) -> Option<&str> {
        self.contributor.as_deref()
    }

    pub fn search(&self) -> &SearchFields {
        &self.search
    }

    pub(crate) fn title_matches_word_prefix(&self, query: &Query) -> bool {
        let title = self
            .display_title_search
            .as_deref()
            .or_else(|| self.search.names().first().map(String::as_str))
            .unwrap_or_default();
        SearchFields::name_matches_word_prefix(title, query)
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
    factories: Vec<RegisteredFactory>,
}

#[derive(Clone)]
struct RegisteredFactory {
    factory: std::sync::Arc<dyn super::bundle::ContextActionFactory>,
    contributor: Option<std::sync::Arc<str>>,
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
            ActionExecutor::WebCommand(_) => BindingPolicy::Persistent,
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
            .any(|factory| factory.factory.supports_persistent_binding(id))
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
        self.register_owned_factory(factory, None);
    }

    pub fn register_owned_factory(
        &mut self,
        factory: std::sync::Arc<dyn super::bundle::ContextActionFactory>,
        contributor: Option<&str>,
    ) {
        self.factories.push(RegisteredFactory {
            factory,
            contributor: contributor.map(Into::into),
        });
    }

    /// Filter once at a configuration boundary, including not-yet-expanded factories.
    pub fn retain_contributors(&mut self, enabled: impl Fn(&str) -> bool) {
        self.factories
            .retain(|factory| factory.contributor.as_deref().is_none_or(&enabled));
        self.retain_actions(|action| action.contributor().is_none_or(&enabled));
    }
    /// Select configured commands without reconstructing their contributors.
    pub fn retain_actions(&mut self, enabled: impl Fn(&ActionDescriptor) -> bool) {
        self.actions.retain(|_, action| enabled(action));
    }

    /// Build an immutable snapshot registry. Generated handlers travel with candidates.
    pub fn for_contexts(
        &self,
        contexts: &[ResolvedContext],
    ) -> (std::borrow::Cow<'_, Self>, Vec<String>) {
        if self.factories.is_empty() {
            return (std::borrow::Cow::Borrowed(self), Vec::new());
        }
        let mut diagnostics = Vec::new();
        let mut generated = Vec::new();
        for registered in &self.factories {
            let factory = &registered.factory;
            let expansion = factory.actions(contexts);
            diagnostics.extend(
                expansion
                    .diagnostics
                    .into_iter()
                    .map(|message| format!("context actions [{}]: {message}", factory.id())),
            );
            for action in expansion.actions {
                generated.push((factory.id(), action, registered.contributor.clone()));
            }
        }
        if generated.is_empty() {
            return (std::borrow::Cow::Borrowed(self), diagnostics);
        }
        let mut registry = self.clone();
        registry.factories.clear();
        for (factory_id, action, contributor) in generated {
            if let Err(error) = registry.register_owned(action, contributor.as_deref()) {
                diagnostics.push(format!("context actions [{factory_id}]: {error}"));
            }
        }
        (std::borrow::Cow::Owned(registry), diagnostics)
    }
    /// Check every action, then insert. A failure leaves the registry unchanged.
    pub(crate) fn commit_batch(
        &mut self,
        actions: Vec<ActionDefinition>,
        contributor: Option<&str>,
    ) -> Result<()> {
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
            let mut action = ActionDescriptor::from_definition(action);
            action.contributor = contributor.map(Into::into);
            self.actions
                .insert(action.id.clone(), std::sync::Arc::new(action));
        }
        Ok(())
    }

    pub fn register(&mut self, action: ActionDefinition) -> Result<()> {
        self.register_owned(action, None)
    }

    pub fn register_owned(
        &mut self,
        action: ActionDefinition,
        contributor: Option<&str>,
    ) -> Result<()> {
        self.commit_batch(vec![action], contributor)
    }

    /// Actions and the group's display title, or neither.
    pub fn register_bundle(&mut self, bundle: ActionBundle) -> Result<()> {
        self.register_owned_bundle(bundle, None)
    }

    pub fn register_owned_bundle(
        &mut self,
        bundle: ActionBundle,
        contributor: Option<&str>,
    ) -> Result<()> {
        let group = bundle.group.clone();
        self.commit_batch(bundle.descriptors(), contributor)?;
        self.remember_group(group, true);
        Ok(())
    }

    pub(crate) fn remember_group(&mut self, group: ActionGroup, ranked: bool) {
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
    ) -> impl Iterator<Item = &'a std::sync::Arc<ActionDescriptor>> {
        self.actions
            .values()
            .filter(|command| command.matches(context))
    }

    pub fn commands(&self) -> impl Iterator<Item = &ActionDescriptor> {
        self.actions.values().map(std::sync::Arc::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::action::bundle::ActionSpec;
    use crate::core::action::group::groups;
    use crate::core::action::handler::{ActionHandler, ExecutionOutput};
    use crate::features::git::actions::is_git_action;
    use crate::features::recognized_contexts;

    struct NoopHandler;
    impl ActionHandler for NoopHandler {
        fn execute(
            &self,
            _: &ResolvedContext,
            _: &dyn anywhere_platform::Platform,
        ) -> anyhow::Result<ExecutionOutput> {
            Ok(ExecutionOutput::dismiss())
        }
    }

    fn json_context() -> ResolvedContext {
        ResolvedContext::new(
            JSON_DOCUMENT,
            crate::core::context::ContextValue::Text("{}".into()),
            ContextSource::UserInput,
            "legacy-fixture",
        )
    }

    #[test]
    fn direct_title_matching_excludes_alternate_fields_and_preserves_literal_tiers() {
        use crate::core::search::query::MatchLevel;
        for (title, search_title) in [
            ("Find", None),
            ("Find", Some("Finder Command")),
            ("Find", Some("")),
            ("", None),
            ("...", None),
            ("打开文件…", None),
            ("İnfo", None),
        ] {
            let action = ActionDescriptor::from_definition(ActionDefinition {
                id: "test.title".into(),
                title: title.into(),
                localized_titles: BTreeMap::from([("zh".into(), "查找".into())]),
                subtitle: None,
                shortcut: None,
                accepts: vec![ContextMatcher {
                    type_id: APPLICATION.into(),
                }],
                executor: ActionExecutor::Native(ActionHandlerRef::Static(&NoopHandler)),
                availability: AvailabilityRule::Enabled,
                base_priority: 0,
                icon: "".into(),
                group_id: "test".into(),
                aliases: vec!["Alias".into()],
                keywords: vec!["Keyword".into()],
                owning_app: Some("test.app".into()),
                menu_path: vec![],
                primary_action: false,
                search_title: search_title.map(str::to_owned),
            });
            let title_only = SearchFields::new(title, &[], &[]);
            for input in [
                "",
                "Fi",
                "find",
                "ind",
                "finder",
                "finder command",
                "alias",
                "keyword",
                "查找",
                "chazhao",
                "打开",
                "dk",
                "...",
                "İnfo",
            ] {
                let query = Query::parse(input);
                assert_eq!(
                    action.title_matches_word_prefix(&query),
                    title_only
                        .matched(&query)
                        .is_some_and(|level| level >= MatchLevel::WordPrefix),
                    "title={title:?}, search_title={search_title:?}, query={input:?}"
                );
            }
        }
    }

    #[test]
    fn bundled_actions_supply_chinese_names_and_share_literal_and_pinyin_search() {
        use crate::core::search::query::Query;
        let registry = crate::application::builtin_registry();
        for action in registry.commands() {
            let chinese = action
                .localized_titles
                .get("zh")
                .unwrap_or_else(|| panic!("missing Chinese action label: {}", action.id));
            assert!(
                action.search().matched(&Query::parse(chinese)).is_some(),
                "{}",
                action.id
            );
            assert!(
                action
                    .search()
                    .matched(&Query::parse(&action.title))
                    .is_some(),
                "{}",
                action.id
            );
        }
        let action = registry.get("core.git.copy_branch_name").unwrap();
        for query in ["复制分支名", "fuzhifenzhiming", "fzfzm", "Copy Branch Name"] {
            assert!(
                action.search().matched(&Query::parse(query)).is_some(),
                "{query}"
            );
        }
        assert_eq!(action.id, "core.git.copy_branch_name");
        assert_eq!(action.localized_titles["zh"], "复制分支名");
        assert!(action
            .search()
            .matched(&Query::parse("not-a-branch-action"))
            .is_none());
    }

    #[test]
    fn exact_type_match_and_builtin_command_count() {
        let registry = crate::application::builtin_registry();
        assert_eq!(registry.commands().count(), 31);
        let url = recognized_contexts("https://github.com")
            .into_iter()
            .find(|c| c.type_id == URL)
            .unwrap();
        let json = json_context();
        assert!(registry.get("core.url.open").unwrap().matches(&url));
        assert!(!registry.get("core.url.open").unwrap().matches(&json));
        assert_eq!(registry.matching(&url).count(), 2);
        assert_eq!(registry.matching(&json).count(), 0);
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
    fn legacy_matcher_normalization_remains_for_action_specs() {
        let mut registry = crate::application::builtin_registry();
        let definition = ActionSpec {
            localized_titles: &[],
            id: "legacy.pretty",
            title: "Pretty JSON",
            accepts: JSON_LEGACY,
            handler: &NoopHandler,
            availability: AvailabilityRule::Enabled,
            base_priority: 0,
            icon: "copy",
            aliases: &[],
            keywords: &[],
            primary_action: false,
        }
        .descriptor("plugin.legacy");
        registry.register(definition).unwrap();
        let command = registry.get("legacy.pretty").unwrap();
        assert_eq!(command.accepts[0].type_id, JSON_DOCUMENT);
        assert_eq!(command.group_id, "plugin.legacy");
        assert!(command.matches(&json_context()));
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
    fn register_rejects_unknown_and_mistyped_availability_facts() {
        use crate::core::session::facts::{CLIPBOARD, GIT};

        let spec = |id: &'static str, availability: AvailabilityRule| {
            ActionSpec {
                localized_titles: &[],
                id,
                title: "Bad",
                accepts: TEXT,
                handler: &NoopHandler,
                availability,
                base_priority: 0,
                icon: "copy",
                aliases: &[],
                keywords: &[],
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

    #[test]
    fn a_later_invalid_command_in_bundle_does_not_keep_the_earlier_one() {
        use crate::core::action::group::ActionGroup;
        let mut registry = ActionRegistry::default();
        let ok = ActionSpec {
            localized_titles: &[],
            id: "partial.ok",
            title: "Ok",
            accepts: TEXT,
            handler: &NoopHandler,
            availability: AvailabilityRule::Enabled,
            base_priority: 0,
            icon: "copy",
            aliases: &[],
            keywords: &[],
            primary_action: false,
        }
        .descriptor("tools");
        let mut bad = ok.clone();
        bad.id = "partial.bad".into();
        bad.group_id = "".into();
        assert!(registry.commit_batch(vec![ok, bad], None).is_err());
        assert!(registry.get("partial.ok").is_none());
        assert!(registry.get("partial.bad").is_none());
        let _ = ActionGroup {
            id: "tools".into(),
            title: "Tools".into(),
            icon: None,
        };
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
