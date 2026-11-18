//! Settings coordination: native policy and typed data refresh.
//! Contributions are captured once at startup; changes never construct a feature again.
use super::{
    native_features::{BuiltinCatalog, FeatureSettings},
    Application,
};
use crate::{
    features::{policy::FeaturePolicySnapshot, services::FeatureServices},
    persistence::module_storage::ModuleStorageHost,
};
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

#[derive(Clone, Debug)]
pub struct ModuleStatus {
    pub key: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub first_party: bool,
    pub enabled: bool,
    pub active: bool,
    pub configurable: bool,
    pub capabilities: Vec<String>,
    pub detail: String,
}

fn recovery_status(name: String, detail: String) -> ModuleStatus {
    ModuleStatus {
        key: format!("invalid:{name}"),
        id: name.clone(),
        name,
        version: String::new(),
        first_party: false,
        enabled: false,
        active: false,
        configurable: false,
        capabilities: vec![],
        detail,
    }
}

pub enum SettingsChange {
    Enable { key: String, enabled: bool },
}
impl SettingsChange {
    pub(super) fn key(&self) -> &str {
        match self {
            Self::Enable { key, .. } => key,
        }
    }
}

pub struct SettingsService {
    native: FeatureSettings,
    catalog: BuiltinCatalog,
    services: FeatureServices,
    storage: Result<ModuleStorageHost, String>,
    storage_path: PathBuf,
    data_errors: BTreeMap<String, String>,
}
impl SettingsService {
    pub fn open(legacy_path: PathBuf, app: &Application) -> Result<Self> {
        let installation = app.resolver.catalog.begin_install()?;
        let settings = Self::assemble(legacy_path, app)?;
        installation.commit();
        Ok(settings)
    }
    fn assemble(legacy_path: PathBuf, app: &Application) -> Result<Self> {
        app.previews
            .lock()
            .map_err(|_| anyhow::anyhow!("preview service unavailable"))?
            .configure(legacy_path.with_file_name("preview-resources"));
        let feature_path = legacy_path.with_file_name("features.json");
        ensure!(
            legacy_path != feature_path,
            "external and feature preference paths must differ"
        );
        let storage_path = legacy_path.with_file_name("extension-storage.sqlite3");
        let storage = ModuleStorageHost::open(&storage_path).map_err(|e| format!("{e:#}"));
        let mut services = FeatureServices::new(binding(&storage));
        let clipboard = Arc::new(crate::features::clipboard::service::HistoryService::new(
            crate::persistence::clipboard::SqliteHistoryRepository::open(
                &legacy_path.with_file_name("clipboard-history.sqlite3"),
            )
            .map(|repo| {
                Box::new(repo) as Box<dyn crate::features::clipboard::history::HistoryRepository>
            })
            .map_err(|_| "clipboard history storage unavailable".into()),
        ));
        services.bind_clipboard(clipboard.clone());
        let history_storage = storage.as_ref().map_err(Clone::clone).and_then(|host| {
            host.bind("application_history")
                .map_err(|_| "history preferences unavailable".into())
        });
        let installed = if app.resolver.application_index.has_completed_scan() {
            app.resolver
                .application_index
                .full_catalog()
                .map(|catalog| {
                    catalog
                        .applications()
                        .iter()
                        .map(|application| application.bundle_id.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        } else {
            let scanned = crate::features::applications::index::scan(app.platform.as_ref());
            let ids = scanned
                .applications
                .iter()
                .map(|application| application.bundle_id.clone())
                .collect::<Vec<_>>();
            app.resolver.application_index.seed(scanned);
            ids
        };
        services.bind_application_history(Arc::new(
            crate::features::application_history::service::HistoryService::new(
                history_storage,
                crate::application::application_history::SourceCatalog::with_user_directory(
                    &legacy_path.with_file_name("application-history-sources"),
                )?,
            )
            .with_application_lookup({
                let index = app.resolver.application_index.clone();
                Arc::new(move |ids| {
                    index
                        .full_catalog()
                        .map(|catalog| {
                            catalog
                                .applications()
                                .iter()
                                .filter(|application| ids.contains(&application.bundle_id))
                                .map(|application| anywhere_platform::BundleApplication {
                                    bundle_id: application.bundle_id.clone(),
                                    name: application.name.clone(),
                                    path: Some(application.path.clone()),
                                    icon: None,
                                    aliases: application.aliases.clone(),
                                    localized_names: application.localized_names.clone(),
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                })
            })
            .with_installed_applications(installed),
        ));
        let catalog = BuiltinCatalog::assemble(&mut services)?;
        let native = FeatureSettings::open(feature_path, legacy_path);
        let mut settings = Self {
            native,
            catalog,
            services,
            storage,
            storage_path,
            data_errors: Default::default(),
        };
        settings.refresh_business_data();
        settings.publish(app, settings.native.policy.clone(), || Ok(()))?;
        clipboard.start(app.platform.clone())?;
        Ok(settings)
    }
    pub fn statuses(&self) -> Vec<ModuleStatus> {
        let mut statuses = self
            .catalog
            .statuses(&self.native.policy, &self.data_errors);
        if let Some(e) = &self.native.error {
            statuses.push(recovery_status(
                "Feature settings recovery".into(),
                e.clone(),
            ));
        }
        statuses
    }
    pub fn feature_policy(&self) -> Arc<FeaturePolicySnapshot> {
        self.native.policy.clone()
    }
    pub fn clipboard(&self) -> Arc<crate::features::clipboard::service::HistoryService> {
        self.services.clipboard()
    }
    pub fn application_history(
        &self,
    ) -> Result<Arc<crate::features::application_history::service::HistoryService>> {
        self.services.application_history()
    }
    pub fn application_history_directory(&self) -> PathBuf {
        self.storage_path
            .with_file_name("application-history-sources")
    }
    /// A successful configuration change includes publication and revokes old targets.
    pub fn configure_application_history(
        &mut self,
        app: &Application,
        id: &str,
        mode: crate::features::application_history::model::SourceMode,
        path: Option<&str>,
    ) -> Result<()> {
        self.application_history()?.configure_mode(id, mode, path)?;
        self.publish_application_history(app)
    }
    pub fn set_application_history_action_enabled(
        &mut self,
        app: &Application,
        id: &str,
        enabled: bool,
    ) -> Result<()> {
        self.application_history()?
            .set_action_enabled(id, enabled)?;
        self.publish_application_history(app)
    }
    fn publish_application_history(&mut self, app: &Application) -> Result<()> {
        self.refresh_application_history();
        self.publish(app, self.native.policy.clone(), || Ok(()))
    }
    fn refresh_application_history(&mut self) {
        match self
            .services
            .application_history()
            .and_then(|service| service.refresh())
        {
            Ok(()) => {
                self.data_errors.remove("application_history");
            }
            Err(_) => {
                self.data_errors.insert(
                    "application_history".into(),
                    "Application history preferences unavailable".into(),
                );
            }
        }
    }
    fn refresh_business_data(&mut self) {
        self.refresh_application_history();
        if self.native.policy.is_enabled("saved_directories") {
            if let Ok(host) = &self.storage {
                if let Err(e) = host.reopen() {
                    self.storage = Err(format!("{e:#}"));
                }
            }
            if self.storage.is_err() {
                self.storage =
                    ModuleStorageHost::open(&self.storage_path).map_err(|e| format!("{e:#}"));
            }
            match self
                .services
                .refresh_saved_directories(binding(&self.storage))
            {
                Ok(()) => {
                    self.data_errors.remove("saved_directories");
                }
                Err(e) => {
                    self.data_errors.insert(
                        "saved_directories".into(),
                        format!(
                            "Saved directory data unavailable: {e:#}. Repair data and Refresh."
                        ),
                    );
                }
            }
        }
    }
    /// Refresh preference files and business data. Does not discover or bind code.
    pub fn refresh(&mut self, app: &Application) -> Result<Vec<ModuleStatus>> {
        self.native.read()?;
        self.refresh_business_data();
        self.publish(app, self.native.policy.clone(), || Ok(()))?;
        Ok(self.statuses())
    }
    pub fn change(
        &mut self,
        app: &Application,
        change: SettingsChange,
    ) -> Result<Vec<ModuleStatus>> {
        let key = change.key();
        let status = self
            .statuses()
            .into_iter()
            .find(|s| s.key == key && s.configurable && s.first_party)
            .ok_or_else(|| anyhow::anyhow!("feature unavailable; Refresh settings"))?;
        let SettingsChange::Enable { enabled, .. } = change;
        let mut next = self.native.preferences.clone();
        if enabled {
            next.disabled.remove(&status.id);
        } else {
            next.disabled.insert(status.id.clone());
        }
        let policy = self.native.policy_for(&next)?;
        // Re-enabling a feature may refresh its data; it never rebinds its handlers.
        if enabled && status.id == "saved_directories" {
            match self
                .services
                .refresh_saved_directories(binding(&self.storage))
            {
                Ok(()) => {
                    self.data_errors.remove(&status.id);
                }
                Err(e) => {
                    self.data_errors.insert(status.id.clone(), format!("{e:#}"));
                }
            }
        }
        let prepared = next.prepare_save(&self.native.path)?;
        self.publish(app, policy.clone(), || {
            next.commit_prepared(&self.native.path, &self.native.preferences, prepared)
        })?;
        self.native.preferences = next;
        self.native.policy = policy;
        Ok(self.statuses())
    }
    fn publish(
        &self,
        app: &Application,
        policy: Arc<FeaturePolicySnapshot>,
        persist: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let excluded: std::collections::BTreeSet<String> =
            self.data_errors.keys().cloned().collect();
        let clipboard_enabled = policy.is_enabled("clipboard");
        let history_enabled = policy.is_enabled("application_history")
            && !self.data_errors.contains_key("application_history");
        let mut registry = self.catalog.registry.clone();
        let hidden = self.services.application_history()?.disabled_action_ids();
        registry.retain_actions(|action| !hidden.contains(&action.id));
        app.publish_selected_catalog(
            super::feature_catalog::CatalogSelection {
                registry,
                providers: self.catalog.providers.clone(),
                sources: self.catalog.sources.clone(),
                policy,
                excluded,
            },
            persist,
        )?;
        self.services
            .application_history()?
            .set_enabled(history_enabled);
        self.services
            .clipboard()
            .set_feature_enabled(clipboard_enabled, app.platform.as_ref())
    }
}
fn binding(
    storage: &Result<ModuleStorageHost, String>,
) -> Result<Arc<dyn crate::core::storage::ExtensionStorage>, String> {
    storage.as_ref().map_err(Clone::clone).and_then(|host| {
        crate::features::host_services::grant(crate::features::saved_directories::descriptor())
            .and_then(|grant| {
                grant.require(anywhere_protocol::extension::ExtensionCapability::Storage)
            })
            .and_then(|_| host.bind("saved_directories"))
            .map_err(|e| format!("{e:#}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::source_catalog_tests::QuietPlatform, core::ranking::usage::MemoryUsage,
    };
    fn app() -> Application {
        Application::new(
            Arc::new(QuietPlatform),
            super::super::builtin_registry(),
            Box::<MemoryUsage>::default(),
            vec![],
        )
    }
    #[test]
    fn switches_refresh_and_data_repair_reuse_startup_actions_and_sources() {
        let directory = tempfile::tempdir().unwrap();
        let app = app();
        let path = directory.path().join("extensions.json");
        let mut settings = SettingsService::open(path.clone(), &app).unwrap();
        let action = app
            .resolver
            .base_registry()
            .get_shared("saved_directories.save")
            .unwrap();
        let source = app.resolver.catalog.synchronous_sources().unwrap()[0].clone();
        settings
            .change(
                &app,
                SettingsChange::Enable {
                    key: "first-party:saved_directories".into(),
                    enabled: false,
                },
            )
            .unwrap();
        assert!(app
            .resolver
            .base_registry()
            .get_shared(&action.id)
            .is_none());
        settings
            .change(
                &app,
                SettingsChange::Enable {
                    key: "first-party:saved_directories".into(),
                    enabled: true,
                },
            )
            .unwrap();
        settings.refresh(&app).unwrap();
        let storage = ModuleStorageHost::open(&directory.path().join("extension-storage.sqlite3"))
            .unwrap()
            .bind("saved_directories")
            .unwrap();
        storage.put("directories", b"broken data").unwrap();
        settings.refresh(&app).unwrap();
        assert!(app.resolver.base_registry().get(&action.id).is_none());
        storage.remove("directories").unwrap();
        settings.refresh(&app).unwrap();
        assert!(Arc::ptr_eq(
            &action,
            &app.resolver.base_registry().get_shared(&action.id).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &source,
            &app.resolver.catalog.synchronous_sources().unwrap()[0]
        ));
        assert!(SettingsService::open(path, &app).is_err());
    }
    #[test]
    fn conflicting_feature_path_is_rejected_and_failed_startup_can_retry() {
        let directory = tempfile::tempdir().unwrap();
        let app = app();
        assert!(SettingsService::open(directory.path().join("features.json"), &app).is_err());
        let settings =
            SettingsService::open(directory.path().join("extensions.json"), &app).unwrap();
        assert!(settings
            .statuses()
            .iter()
            .any(|s| s.id == "url" && s.active));
    }
}
