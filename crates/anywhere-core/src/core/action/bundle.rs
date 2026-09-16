//! The registration unit for actions.
//!
//! A feature registers a bundle: the group its actions appear under, plus the actions
//! themselves. Registration is the only wiring — the engine, the search pipeline and the
//! launcher service never learn the feature's name.
use super::descriptor::{ActionDescriptor, BuiltinExecutor};
use super::group::ActionGroup;
use crate::core::search::query::SearchFields;
use anywhere_protocol::ContextMatcher;

/// One action declaration inside a bundle.
///
/// A compact table row rather than a `ActionDescriptor` literal: the descriptor is derived
/// from it, search fields included, so a feature never assembles matching state by hand and
/// cannot forget to keep the two in step.
pub struct ActionSpec {
    pub id: &'static str,
    pub title: &'static str,
    /// Legacy V0.1 type ids are normalized at registration time.
    pub accepts: &'static str,
    pub executor: BuiltinExecutor,
    pub base_priority: i32,
    pub icon: &'static str,
    pub aliases: &'static [&'static str],
    pub keywords: &'static [&'static str],
}

impl ActionSpec {
    /// The descriptor this row stands for, in the bundle's group.
    pub fn descriptor(&self, group_id: &str) -> ActionDescriptor {
        let aliases = self
            .aliases
            .iter()
            .map(|alias| (*alias).to_owned())
            .collect::<Vec<_>>();
        let keywords = self
            .keywords
            .iter()
            .map(|keyword| (*keyword).to_owned())
            .collect::<Vec<_>>();
        ActionDescriptor {
            id: self.id.into(),
            title: self.title.into(),
            subtitle: None,
            accepts: vec![ContextMatcher {
                type_id: anywhere_protocol::canonical_context_type(self.accepts).into(),
            }],
            executor: super::descriptor::ActionExecutor::Builtin(self.executor),
            base_priority: self.base_priority,
            icon: self.icon.into(),
            group_id: group_id.into(),
            aliases: aliases.clone(),
            keywords: keywords.clone(),
            owning_app: None,
            search: SearchFields::new(self.title, &aliases, &keywords),
        }
    }
}

/// A group and the actions that belong to it.
pub struct ActionBundle {
    pub group: ActionGroup,
    pub actions: Vec<ActionSpec>,
}

impl ActionBundle {
    pub fn descriptors(&self) -> Vec<ActionDescriptor> {
        self.actions
            .iter()
            .map(|spec| spec.descriptor(&self.group.id))
            .collect()
    }
}
