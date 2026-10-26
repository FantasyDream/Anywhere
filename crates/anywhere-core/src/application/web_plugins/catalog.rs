use crate::core::action::{
    availability::AvailabilityRule,
    bundle::ActionBundle,
    descriptor::{ActionDefinition, ActionExecutor, ActionRegistry},
    group::{groups, ActionGroup},
    web_command::WebCommandRef,
};
use anyhow::{ensure, Result};
use anywhere_plugin::web::VerifiedWebPackage;
use anywhere_protocol::{
    web_plugin::{WebPermission, WebPlatform, WebPluginManifest},
    ContextMatcher,
};
use std::{collections::BTreeMap, sync::Arc};

/// Trusted Host approval, not a Manifest field or a browser request. Callers must
/// supply the exact content and permissions accepted by the package review use case.
pub struct WebPackageApproval {
    pub plugin_id: String,
    pub content_digest: String,
    pub permissions: Vec<WebPermission>,
    pub source_id: String,
}

pub(super) struct Registration {
    pub manifest: Arc<WebPluginManifest>,
    pub digest: String,
    pub source_id: String,
    pub token: String,
}

#[derive(Default)]
pub struct WebPluginCatalog {
    pub(super) entries: BTreeMap<String, Registration>,
}

impl WebPluginCatalog {
    /// Pure preparation: failure changes neither installed definitions nor the registry.
    /// Only bounded metadata survives; compressed package bytes are not cached here.
    pub fn prepare<'a>(
        platform: WebPlatform,
        packages: impl IntoIterator<Item = (&'a VerifiedWebPackage, WebPackageApproval)>,
    ) -> Result<Self> {
        let mut entries = BTreeMap::new();
        for (package, approval) in packages {
            let manifest = package.manifest();
            anywhere_plugin::web::validate_manifest(manifest)?;
            ensure!(entries.len() < 64, "too many Web packages");
            ensure!(
                manifest.platforms.contains(&platform),
                "unsupported Web package platform"
            );
            ensure!(
                approval.plugin_id == manifest.id
                    && approval.content_digest == package.digest()
                    && approval.permissions == manifest.permissions,
                "Web approval does not match package"
            );
            ensure!(
                !approval.source_id.trim().is_empty()
                    && approval.source_id.len() <= 512
                    && !approval.source_id.chars().any(char::is_control),
                "invalid Web source identity"
            );
            ensure!(
                !entries.contains_key(&manifest.id),
                "duplicate Web package id"
            );
            entries.insert(
                manifest.id.clone(),
                Registration {
                    manifest: Arc::new(manifest.clone()),
                    digest: package.digest().into(),
                    source_id: approval.source_id,
                    token: super::token()?,
                },
            );
        }
        Ok(Self { entries })
    }

    /// Merge independently verified metadata without retaining a batch of archives.
    pub fn merge(&mut self, next: Self) -> Result<()> {
        ensure!(
            self.entries.len() + next.entries.len() <= 64,
            "too many Web packages"
        );
        ensure!(
            next.entries.keys().all(|id| !self.entries.contains_key(id)),
            "duplicate Web package id"
        );
        self.entries.extend(next.entries);
        Ok(())
    }

    pub(super) fn append_to(&self, base: &ActionRegistry) -> Result<ActionRegistry> {
        let mut next = base.clone();
        next.retain_contributors(|id| !id.starts_with("web:"));
        for entry in self.entries.values() {
            for (index, command) in entry.manifest.commands.iter().enumerate() {
                let reference = WebCommandRef {
                    plugin_id: entry.manifest.id.clone(),
                    command_id: command.id.clone(),
                    registration: entry.token.clone(),
                    input_matchers: command.input_matchers.clone().into(),
                    require_input_match: command.require_input_match,
                };
                let action = ActionDefinition {
                    localized_titles: Default::default(),
                    id: reference.action_id(),
                    title: command.title.clone(),
                    subtitle: Some(entry.manifest.name.clone()),
                    shortcut: None,
                    accepts: command
                        .accepts
                        .iter()
                        .map(String::as_str)
                        .chain(
                            (!command.require_input_match)
                                .then_some(crate::features::tools::ENTRY_CONTEXT),
                        )
                        .map(|type_id| ContextMatcher {
                            type_id: type_id.into(),
                        })
                        .collect(),
                    executor: ActionExecutor::WebCommand(reference),
                    availability: AvailabilityRule::Enabled,
                    // Preserve the author's declaration order when match and
                    // usage are equal; adding helper commands must not displace
                    // the package's default entry just because of lexical IDs.
                    base_priority: -(index as i32),
                    icon: "code".into(),
                    group_id: groups::TOOLS.into(),
                    aliases: vec![],
                    keywords: command.keywords.clone(),
                    owning_app: None,
                    menu_path: vec![],
                    primary_action: false,
                    search_title: None,
                };
                next.register_owned(action, Some(&format!("web:{}", entry.manifest.id)))?;
            }
            next.register_owned_bundle(
                ActionBundle {
                    group: ActionGroup {
                        id: groups::TOOLS.into(),
                        title: "Tools".into(),
                        icon: None,
                    },
                    actions: vec![],
                },
                None,
            )?;
        }
        Ok(next)
    }
}
