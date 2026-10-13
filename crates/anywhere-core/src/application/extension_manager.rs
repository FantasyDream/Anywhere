//! Host-owned module selection and declarative approvals, independent of UI/platform.
use crate::{
    application::{composition::configured_builtins, Application},
    core::{action::descriptor::ActionRegistry, context::registry::ContextProviderRegistry},
    persistence::extensions::ExtensionPreferences,
};
use anyhow::{ensure, Context, Result};
use anywhere_protocol::{extension::ExtensionCapability, PluginManifest};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Clone, Debug)]
pub struct ExtensionStatus {
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
    pub capabilities: Vec<String>,
    pub approval_token: String,
    pub detail: String,
}

pub enum ExtensionChange {
    Enable { key: String, enabled: bool },
    Approve { key: String, token: String },
    Revoke { key: String },
}
struct InspectedPlugin {
    identity: String,
    manifest: PluginManifest,
}
struct PreparedRuntime {
    registry: ActionRegistry,
    providers: ContextProviderRegistry,
    statuses: Vec<ExtensionStatus>,
    inspected: BTreeMap<String, InspectedPlugin>,
    generation: u64,
    first_party_registry: ActionRegistry,
    sources: crate::core::search::candidate_source::SourceSet,
}

/// Serialize callers through one host-owned manager. Files are read only on open/reload/change.
pub struct ExtensionManager {
    path: PathBuf,
    directories: Vec<PathBuf>,
    preferences: ExtensionPreferences,
    statuses: Vec<ExtensionStatus>,
    inspected: BTreeMap<String, InspectedPlugin>,
    generation: u64,
    storage: Result<crate::persistence::module_storage::ModuleStorageHost, String>,
    storage_path: PathBuf,
    states: BTreeMap<String, std::sync::Arc<dyn crate::extensions::ExtensionState>>,
    policy_error: Option<String>,
}
impl ExtensionManager {
    pub fn open(path: PathBuf, directories: Vec<PathBuf>, app: &Application) -> Result<Self> {
        let storage_path = path.with_file_name("extension-storage.sqlite3");
        let storage = crate::persistence::module_storage::ModuleStorageHost::open(&storage_path)
            .map_err(|e| format!("{e:#}"));
        let mut manager = Self {
            path,
            directories,
            preferences: ExtensionPreferences::default(),
            statuses: vec![],
            inspected: BTreeMap::new(),
            generation: 0,
            storage,
            storage_path,
            states: BTreeMap::new(),
            policy_error: None,
        };
        manager.reload(app)?;
        Ok(manager)
    }
    pub fn statuses(&self) -> Vec<ExtensionStatus> {
        self.statuses.clone()
    }
    pub fn reload(&mut self, app: &Application) -> Result<Vec<ExtensionStatus>> {
        let preferences = match ExtensionPreferences::load(&self.path) {
            Ok(prefs) => {
                self.policy_error = None;
                prefs
            }
            Err(error) => {
                self.policy_error = Some(format!("Extension settings could not be read: {error:#}. Repair {} and Reload. The saved file has not been changed.", self.path.display()));
                ExtensionPreferences {
                    recovery: true,
                    ..Default::default()
                }
            }
        };
        if let Ok(storage) = &self.storage {
            if let Err(error) = storage.reopen() {
                self.storage = Err(format!("{error:#}"));
            }
        }
        if self.storage.is_err() {
            // A new host cannot reuse state bound to a previous failed connection.
            self.states.retain(|id, _| {
                !crate::extensions::modules().iter().any(|m| {
                    m.manifest().id == *id
                        && m.manifest()
                            .capabilities
                            .contains(&ExtensionCapability::Storage)
                })
            });
            self.storage =
                crate::persistence::module_storage::ModuleStorageHost::open(&self.storage_path)
                    .map_err(|e| format!("{e:#}"));
        }
        self.commit(app, preferences, false)
    }
    pub fn change(
        &mut self,
        app: &Application,
        change: ExtensionChange,
    ) -> Result<Vec<ExtensionStatus>> {
        ensure!(
            self.policy_error.is_none(),
            "repair extension preferences and reload before changing settings"
        );
        let mut next = self.preferences.clone();
        let key = match &change {
            ExtensionChange::Enable { key, .. }
            | ExtensionChange::Approve { key, .. }
            | ExtensionChange::Revoke { key } => key,
        };
        let status = self
            .statuses
            .iter()
            .find(|s| &s.key == key && s.configurable)
            .context("extension is unavailable; reload extensions")?;
        match change {
            ExtensionChange::Enable { key, enabled } => {
                if enabled {
                    next.disabled.remove(&key);
                } else {
                    next.disabled.insert(key);
                }
            }
            ExtensionChange::Approve { key, token } => {
                ensure!(
                    !status.first_party && !status.legacy && token == status.approval_token,
                    "approval is stale; reload and review the extension"
                );
                let old = self
                    .inspected
                    .get(&key)
                    .context("extension is unavailable")?;
                // Approval binds the UI's inspected definition, never whatever replaced it on disk.
                let fresh = prepare(
                    &next,
                    &self.directories,
                    self.generation + 1,
                    &self.storage,
                    &mut self.states,
                )?;
                let inspected = fresh
                    .inspected
                    .get(&key)
                    .context("extension changed; reload and review it")?;
                ensure!(
                    old.identity == inspected.identity,
                    "extension changed; reload and review it"
                );
                next.approvals
                    .insert(key.clone(), inspected.identity.clone());
                next.disabled.remove(&key);
                // Install the exact inspected snapshot, avoiding a second discovery TOCTOU window.
                return self.commit_prepared(
                    app,
                    next.clone(),
                    prepare_from_inspected(next, fresh)?,
                    true,
                );
            }
            ExtensionChange::Revoke { key } => {
                ensure!(
                    !status.first_party && !status.legacy,
                    "this extension has no user capability grant"
                );
                next.approvals.remove(&key);
            }
        }
        self.commit(app, next, true)
    }
    fn commit(
        &mut self,
        app: &Application,
        preferences: ExtensionPreferences,
        save: bool,
    ) -> Result<Vec<ExtensionStatus>> {
        let prepared = prepare(
            &preferences,
            &self.directories,
            self.generation + 1,
            &self.storage,
            &mut self.states,
        )?;
        self.commit_prepared(app, preferences, prepared, save)
    }
    fn commit_prepared(
        &mut self,
        app: &Application,
        preferences: ExtensionPreferences,
        prepared: PreparedRuntime,
        save: bool,
    ) -> Result<Vec<ExtensionStatus>> {
        app.replace_extension_runtime(
            prepared.registry,
            prepared.providers,
            prepared.sources,
            || {
                if save {
                    preferences.save(&self.path)?;
                }
                Ok(())
            },
        )?;
        self.preferences = preferences;
        self.statuses = prepared.statuses;
        if let Some(error) = &self.policy_error {
            self.statuses
                .push(invalid("Extension settings recovery".into(), error.clone()));
        }
        self.inspected = prepared.inspected;
        self.generation += 1;
        Ok(self.statuses())
    }
}

fn prepare(
    prefs: &ExtensionPreferences,
    directories: &[PathBuf],
    generation: u64,
    storage: &Result<crate::persistence::module_storage::ModuleStorageHost, String>,
    states: &mut BTreeMap<String, std::sync::Arc<dyn crate::extensions::ExtensionState>>,
) -> Result<PreparedRuntime> {
    let modules = crate::extensions::modules();
    let core_only: BTreeSet<_> = modules
        .iter()
        .map(|m| format!("first-party:{}", m.manifest().id))
        .collect();
    let (mut registry, mut providers) = configured_builtins(&core_only);
    let mut sources = crate::core::search::candidate_source::SourceSet::default();
    let mut statuses = Vec::new();
    for module in modules {
        let manifest = module.manifest();
        let key = format!("first-party:{}", manifest.id);
        let enabled = !prefs.recovery && !prefs.disabled.contains(&key);
        let runtime = if enabled {
            let runtime = if manifest
                .capabilities
                .contains(&ExtensionCapability::Storage)
                && storage.is_err()
            {
                Err(anyhow::anyhow!(
                    "Storage unavailable: {}",
                    storage.as_ref().err().expect("checked storage error")
                ))
            } else {
                let state = states.get(&manifest.id).cloned();
                let loaded = if manifest
                    .capabilities
                    .contains(&ExtensionCapability::Storage)
                {
                    storage
                        .as_ref()
                        .map_err(|e| anyhow::anyhow!("Storage unavailable: {e}"))
                        .and_then(|host| crate::extensions::runtime(module.as_ref(), host, state))
                } else {
                    module.runtime_with_state(None, state)
                };
                if let Ok(runtime) = &loaded {
                    if let Some(state) = &runtime.state {
                        states.insert(manifest.id.clone(), state.clone());
                    }
                }
                loaded
            };
            runtime.and_then(|runtime| {
                let mut ids: BTreeSet<_> = sources.ids().collect();
                for id in runtime.sources.ids() {
                    ensure!(
                        id.len() <= 100
                            && (id == manifest.id || id.starts_with(&format!("{}.", manifest.id)))
                            && ids.insert(id),
                        "duplicate or unowned candidate source id"
                    );
                }
                let mut updated = registry.clone();
                let mut updated_providers = providers.clone();
                super::composition::append_contribution(
                    &mut updated,
                    &mut updated_providers,
                    module.contribution(),
                )?;
                for action in runtime.actions {
                    updated.register(action)?;
                }
                registry = updated;
                providers = updated_providers;
                sources.synchronous.extend(runtime.sources.synchronous);
                sources.asynchronous.extend(runtime.sources.asynchronous);
                Ok(())
            })
        } else {
            Ok(())
        };
        let detail = runtime.as_ref().err().map(|e| format!("{e:#}"));
        statuses.push(ExtensionStatus {
            key,
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            first_party: true,
            legacy: false,
            enabled,
            active: enabled && runtime.is_ok(),
            needs_approval: false,
            configurable: !prefs.recovery,
            capabilities: manifest
                .capabilities
                .iter()
                .map(|c| c.as_str().into())
                .collect(),
            approval_token: String::new(),
            detail: detail.unwrap_or_else(|| {
                if enabled {
                    "Built in · Enabled"
                } else {
                    "Built in · Disabled"
                }
                .into()
            }),
        });
    }
    let mut found = Vec::new();
    let mut origins = BTreeSet::new();
    for directory in directories {
        let discovery = anywhere_plugin::discover_with_paths(directory);
        for diagnostic in discovery.diagnostics {
            statuses.push(invalid(directory.display().to_string(), diagnostic));
        }
        for (path, manifest) in discovery.manifests {
            let origin = path
                .canonicalize()
                .context("extension origin unavailable")?;
            if !origins.insert(origin.clone()) {
                continue;
            }
            let identity = serde_json::to_string(&(origin, &manifest))?;
            found.push(InspectedPlugin { identity, manifest });
        }
    }
    let mut counts = BTreeMap::new();
    for plugin in &found {
        *counts.entry(&plugin.manifest.id).or_insert(0) += 1;
    }
    let duplicates: BTreeSet<_> = counts
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(id, _)| id.clone())
        .collect();
    let mut inspected = BTreeMap::new();
    for plugin in found {
        if duplicates.contains(&plugin.manifest.id) {
            if !statuses
                .iter()
                .any(|s| s.id == plugin.manifest.id && !s.first_party)
            {
                statuses.push(invalid(
                    plugin.manifest.id,
                    "Duplicate plugin id; all installations are blocked".into(),
                ));
            }
            continue;
        }
        inspected.insert(format!("plugin:{}", plugin.manifest.id), plugin);
    }
    assemble_external(
        prefs,
        PreparedRuntime {
            first_party_registry: registry.clone(),
            registry,
            providers,
            statuses,
            inspected,
            generation,
            sources,
        },
        generation,
    )
}

fn prepare_from_inspected(
    prefs: ExtensionPreferences,
    mut prepared: PreparedRuntime,
) -> Result<PreparedRuntime> {
    let generation = prepared.generation;
    prepared.registry = prepared.first_party_registry.clone();
    prepared
        .statuses
        .retain(|s| s.first_party || s.key.starts_with("invalid:"));
    assemble_external(&prefs, prepared, generation)
}

fn assemble_external(
    prefs: &ExtensionPreferences,
    mut prepared: PreparedRuntime,
    generation: u64,
) -> Result<PreparedRuntime> {
    for (key, inspected) in &prepared.inspected {
        let manifest = &inspected.manifest;
        let enabled = !prefs.recovery && !prefs.disabled.contains(key);
        let legacy = manifest.protocol_version == 1;
        let approved = legacy || prefs.approvals.get(key) == Some(&inspected.identity);
        let capabilities: Vec<ExtensionCapability> = if approved {
            manifest.capabilities.clone()
        } else {
            vec![]
        };
        let mut status = ExtensionStatus {
            key: key.clone(),
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            version: manifest.version.clone().unwrap_or_else(|| "v1".into()),
            first_party: false,
            legacy,
            enabled,
            active: false,
            needs_approval: !approved,
            configurable: !prefs.recovery,
            capabilities: manifest
                .capabilities
                .iter()
                .map(|c| c.as_str().into())
                .collect(),
            approval_token: format!("{generation}:{key}"),
            detail: if !enabled {
                "Disabled"
            } else if !approved {
                "Needs permission approval"
            } else if legacy {
                "Legacy manifest · Enabled"
            } else {
                "Enabled"
            }
            .into(),
        };
        if enabled && approved {
            match prepared
                .registry
                .register_plugin_with_capabilities(manifest.clone(), &capabilities)
            {
                Ok(()) => status.active = true,
                Err(error) => {
                    status.configurable = false;
                    status.detail = format!("{error:#}");
                }
            }
        }
        prepared.statuses.push(status);
    }
    Ok(prepared)
}
fn invalid(id: String, detail: String) -> ExtensionStatus {
    ExtensionStatus {
        key: format!("invalid:{id}"),
        name: id.clone(),
        id,
        version: String::new(),
        first_party: false,
        legacy: false,
        enabled: false,
        active: false,
        needs_approval: false,
        configurable: false,
        capabilities: vec![],
        approval_token: String::new(),
        detail,
    }
}
