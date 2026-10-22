//! Native code is assembled once. Preference changes only select captured contributions.
use super::settings_service::ModuleStatus;
use crate::{
    core::{
        action::descriptor::ActionRegistry, context::registry::ContextProviderRegistry,
        search::candidate_source::SourceSet,
    },
    features::{
        descriptor::{FeatureDescriptor, APP_VERSION},
        policy::FeaturePolicySnapshot,
        services::FeatureServices,
    },
    persistence::{extensions::PluginPreferences, features::FeaturePreferences},
};
use anyhow::{ensure, Context, Result};
use std::{path::PathBuf, sync::Arc};

pub(super) struct BuiltinCatalog {
    pub registry: ActionRegistry,
    pub providers: ContextProviderRegistry,
    pub sources: SourceSet,
    pub descriptors: Vec<&'static FeatureDescriptor>,
}
impl BuiltinCatalog {
    pub fn assemble(services: &mut FeatureServices) -> Result<Self> {
        let mut catalog = Self {
            registry: Default::default(),
            providers: Default::default(),
            sources: Default::default(),
            descriptors: vec![],
        };
        for contribution in crate::features::contribution::foundations() {
            super::composition::append_contribution(
                &mut catalog.registry,
                &mut catalog.providers,
                contribution,
            )?;
        }
        // This is the sole service-binding pass. No module constructor is retained for reload.
        for module in crate::features::catalog::instantiate()? {
            let descriptor = module.descriptor();
            crate::features::host_services::grant(descriptor)?;
            let contribution = module.contribution();
            ensure!(
                contribution.id == descriptor.id && contribution.configurable,
                "native contribution ownership differs from registration"
            );
            super::composition::append_contribution(
                &mut catalog.registry,
                &mut catalog.providers,
                contribution,
            )?;
            let mut bindings = module.bind_services(services)?;
            let mut ids: std::collections::BTreeSet<_> = catalog.sources.ids().collect();
            for id in bindings.sources.ids() {
                ensure!(
                    !id.is_empty() && ids.insert(id),
                    "duplicate or empty source id: {id}"
                );
            }
            for action in bindings.actions {
                catalog
                    .registry
                    .register_owned(action, Some(descriptor.id))?;
            }
            bindings.sources.assign_contributor(descriptor.id);
            catalog.sources.extend(bindings.sources);
            catalog.descriptors.push(descriptor);
        }
        Ok(catalog)
    }
    pub fn statuses(
        &self,
        policy: &FeaturePolicySnapshot,
        errors: &std::collections::BTreeMap<String, String>,
    ) -> Vec<ModuleStatus> {
        self.descriptors
            .iter()
            .map(|descriptor| {
                let enabled = policy.is_enabled(descriptor.id);
                let error = errors.get(descriptor.id);
                ModuleStatus {
                    key: format!("first-party:{}", descriptor.id),
                    id: descriptor.id.into(),
                    name: descriptor.name.into(),
                    version: APP_VERSION.into(),
                    first_party: true,
                    legacy: false,
                    enabled,
                    active: enabled && error.is_none(),
                    needs_approval: false,
                    configurable: policy.is_configurable(),
                    restart_required: false,
                    capabilities: descriptor
                        .capabilities
                        .iter()
                        .map(|c| c.as_str().into())
                        .collect(),
                    approval_token: String::new(),
                    detail: error.cloned().unwrap_or_else(|| {
                        if enabled {
                            "Built in · Enabled"
                        } else {
                            "Built in · Disabled"
                        }
                        .into()
                    }),
                }
            })
            .collect()
    }
}

pub(super) struct FeatureSettings {
    pub path: PathBuf,
    pub preferences: FeaturePreferences,
    pub policy: Arc<FeaturePolicySnapshot>,
    pub error: Option<String>,
}
impl FeatureSettings {
    pub fn open(path: PathBuf, legacy: Result<&PluginPreferences, &str>) -> Self {
        let mut settings = Self {
            path,
            preferences: Default::default(),
            policy: Arc::new(FeaturePolicySnapshot::new(0, Default::default(), false)),
            error: None,
        };
        settings
            .read(legacy)
            .expect("initial generation is available");
        settings
    }
    pub fn read(&mut self, legacy: Result<&PluginPreferences, &str>) -> Result<()> {
        let loaded = FeaturePreferences::load_or_migrate(&self.path, legacy);
        self.error = loaded.as_ref().err().map(|e| format!("Feature settings could not be read: {e:#}. Repair {} and Refresh. Existing files have not been overwritten.", self.path.display()));
        let next = loaded.unwrap_or_else(|_| FeaturePreferences {
            recovery: true,
            ..Default::default()
        });
        self.policy = self.policy_for(&next)?;
        self.preferences = next;
        Ok(())
    }
    pub fn policy_for(
        &self,
        preferences: &FeaturePreferences,
    ) -> Result<Arc<FeaturePolicySnapshot>> {
        if self.policy.generation() != 0 && *preferences == self.preferences {
            return Ok(self.policy.clone());
        }
        Ok(Arc::new(FeaturePolicySnapshot::new(
            self.policy
                .generation()
                .checked_add(1)
                .context("feature policy generation exhausted")?,
            preferences.disabled.clone(),
            preferences.recovery,
        )))
    }
}
