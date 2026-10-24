//! Settings coordination: native policy, legacy preferences and typed data refresh.
//! Contributions are captured once at startup; changes never construct a feature again.
use super::{
    legacy_plugins::{invalid, LegacyPluginSettings},
    native_features::{BuiltinCatalog, FeatureSettings},
    Application,
};
use crate::{
    features::{policy::FeaturePolicySnapshot, services::FeatureServices},
    persistence::{extensions::PluginPreferences, module_storage::ModuleStorageHost},
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
    pub legacy: bool,
    pub enabled: bool,
    pub active: bool,
    pub needs_approval: bool,
    pub configurable: bool,
    /// An approved startup-only plugin is waiting for a fresh process.
    /// Host presentation must not infer this from diagnostic prose.
    pub restart_required: bool,
    pub capabilities: Vec<String>,
    pub approval_token: String,
    pub detail: String,
}
pub enum SettingsChange {
    Enable { key: String, enabled: bool },
    Approve { key: String, token: String },
    Revoke { key: String },
}
impl SettingsChange {
    pub(super) fn key(&self) -> &str {
        match self {
            Self::Enable { key, .. } | Self::Approve { key, .. } | Self::Revoke { key } => key,
        }
    }
}

pub struct SettingsService {
    native: FeatureSettings,
    legacy: LegacyPluginSettings,
    catalog: BuiltinCatalog,
    services: FeatureServices,
    storage: Result<ModuleStorageHost, String>,
    storage_path: PathBuf,
    data_errors: BTreeMap<String, String>,
}
impl SettingsService {
    pub fn open(path: PathBuf, directories: Vec<PathBuf>, app: &Application) -> Result<Self> {
        ensure!(
            app.feature_catalog_installed
                .compare_exchange(
                    false,
                    true,
                    std::sync::atomic::Ordering::AcqRel,
                    std::sync::atomic::Ordering::Acquire
                )
                .is_ok(),
            "feature catalogue already installed; restart instead of rebinding native code"
        );
        let result = Self::assemble(path, directories, app);
        if result.is_err() {
            app.feature_catalog_installed
                .store(false, std::sync::atomic::Ordering::Release);
        }
        result
    }
    fn assemble(path: PathBuf, directories: Vec<PathBuf>, app: &Application) -> Result<Self> {
        let feature_path = path.with_file_name("features.json");
        ensure!(
            path != feature_path,
            "external and feature preference paths must differ"
        );
        let storage_path = path.with_file_name("extension-storage.sqlite3");
        let storage = ModuleStorageHost::open(&storage_path).map_err(|e| format!("{e:#}"));
        let mut services = FeatureServices::new(binding(&storage));
        let clipboard = Arc::new(crate::features::clipboard::service::HistoryService::new(
            crate::persistence::clipboard::SqliteHistoryRepository::open(
                &path.with_file_name("clipboard-history.sqlite3"),
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
        services.bind_application_history(Arc::new(
            crate::features::application_history::service::HistoryService::new(
                history_storage,
                crate::application::application_history::OfficialSources::bundled()?,
            ),
        ));
        let mut catalog = BuiltinCatalog::assemble(&mut services)?;
        let legacy = LegacyPluginSettings::open(path, directories, &mut catalog.registry)?;
        let native = FeatureSettings::open(
            feature_path,
            if let Some(e) = &legacy.error {
                Err(e)
            } else {
                Ok(&legacy.preferences)
            },
        );
        let mut settings = Self {
            native,
            legacy,
            catalog,
            services,
            storage,
            storage_path,
            data_errors: Default::default(),
        };
        settings.refresh_business_data();
        settings.publish(
            app,
            settings.native.policy.clone(),
            &settings.legacy.preferences,
            || Ok(()),
        )?;
        clipboard.start(app.platform.clone())?;
        Ok(settings)
    }
    pub fn statuses(&self) -> Vec<ModuleStatus> {
        let mut statuses = self
            .catalog
            .statuses(&self.native.policy, &self.data_errors);
        statuses.extend(self.legacy.statuses());
        if let Some(e) = &self.native.error {
            statuses.push(invalid("Feature settings recovery".into(), e.clone()));
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
    fn refresh_business_data(&mut self) {
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
        self.legacy.read();
        self.legacy.verify_definitions();
        self.native.read(if let Some(e) = &self.legacy.error {
            Err(e)
        } else {
            Ok(&self.legacy.preferences)
        })?;
        self.refresh_business_data();
        self.publish(
            app,
            self.native.policy.clone(),
            &self.legacy.preferences,
            || Ok(()),
        )?;
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
            .find(|s| s.key == key && s.configurable)
            .ok_or_else(|| anyhow::anyhow!("feature or plugin unavailable; Refresh settings"))?;
        if status.first_party {
            let SettingsChange::Enable { enabled, .. } = change else {
                anyhow::bail!("built-in features do not require user plugin approval");
            };
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
            self.publish(app, policy.clone(), &self.legacy.preferences, || {
                next.save_checked(&self.native.path, &self.native.preferences)
            })?;
            self.native.preferences = next;
            self.native.policy = policy;
        } else {
            let next = self.legacy.prepare_change(&change)?;
            self.publish(app, self.native.policy.clone(), &next, || {
                next.save_checked(&self.legacy.path, &self.legacy.preferences)
            })?;
            self.legacy.preferences = next;
        }
        Ok(self.statuses())
    }
    fn publish(
        &self,
        app: &Application,
        policy: Arc<FeaturePolicySnapshot>,
        plugins: &PluginPreferences,
        persist: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let mut excluded: std::collections::BTreeSet<String> =
            self.data_errors.keys().cloned().collect();
        for status in self.legacy.statuses() {
            if !self.legacy.available_at_startup(&status.key, plugins) {
                excluded.insert(status.key);
            }
        }
        let clipboard_enabled = policy.is_enabled("clipboard");
        let history_enabled = policy.is_enabled("application_history")
            && !self.data_errors.contains_key("application_history");
        app.publish_selected_catalog(
            self.catalog.registry.clone(),
            self.catalog.providers.clone(),
            self.catalog.sources.clone(),
            policy,
            excluded,
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
        let mut settings = SettingsService::open(path.clone(), vec![], &app).unwrap();
        let action = app
            .resolver
            .base_registry()
            .get_shared("saved_directories.save")
            .unwrap();
        let source = app.resolver.sources.lock().unwrap().extensions.synchronous[0].clone();
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
            &app.resolver.sources.lock().unwrap().extensions.synchronous[0]
        ));
        assert!(SettingsService::open(path, vec![], &app).is_err());
    }
    #[test]
    fn refresh_never_discovers_new_plugins_and_restart_is_the_only_loading_boundary() {
        let directory = tempfile::tempdir().unwrap();
        let app = app();
        let path = directory.path().join("extensions.json");
        let mut settings =
            SettingsService::open(path.clone(), vec![directory.path().into()], &app).unwrap();
        std::fs::write(
            directory.path().join("new.toml"),
            anywhere_plugin::EXAMPLE_PLUGIN,
        )
        .unwrap();
        settings.refresh(&app).unwrap();
        assert!(!settings.statuses().iter().any(|s| s.id == "example"));
        assert!(app.resolver.base_registry().get("example.search").is_none());
        let restarted = super::tests::app();
        let loaded =
            SettingsService::open(path, vec![directory.path().into()], &restarted).unwrap();
        assert!(loaded
            .statuses()
            .iter()
            .any(|s| s.id == "example" && s.active));
    }
    #[test]
    fn disabled_native_target_hides_startup_delegate_and_failed_startup_can_retry() {
        let directory = tempfile::tempdir().unwrap();
        let app = app();
        assert!(
            SettingsService::open(directory.path().join("features.json"), vec![], &app).is_err()
        );
        std::fs::write(
            directory.path().join("delegate.toml"),
            r#"protocol_version=1
id="delegate"
name="Delegating action"
[[commands]]
id="url"
title="Copy URL"
accepts=["core.url"]
action={type="execute_builtin",command_id="core.url.copy"}
"#,
        )
        .unwrap();
        let mut settings = SettingsService::open(
            directory.path().join("extensions.json"),
            vec![directory.path().into()],
            &app,
        )
        .unwrap();
        assert!(app.resolver.base_registry().get("delegate.url").is_some());
        settings
            .change(
                &app,
                SettingsChange::Enable {
                    key: "first-party:url".into(),
                    enabled: false,
                },
            )
            .unwrap();
        assert!(app.resolver.base_registry().get("delegate.url").is_none());
        settings
            .change(
                &app,
                SettingsChange::Enable {
                    key: "first-party:url".into(),
                    enabled: true,
                },
            )
            .unwrap();
        assert!(app.resolver.base_registry().get("delegate.url").is_some());
    }
}
