//! Startup-only declarative compatibility. No code discovery or registration on Refresh.
use super::settings_service::{ModuleStatus, SettingsChange};
use crate::{core::action::descriptor::ActionRegistry, persistence::extensions::PluginPreferences};
use anyhow::{ensure, Context, Result};
use anywhere_protocol::PluginManifest;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

struct InspectedPlugin {
    path: PathBuf,
    identity: String,
    manifest: PluginManifest,
    loaded: bool,
    token: String,
    error: Option<String>,
}
pub(super) struct LegacyPluginSettings {
    pub path: PathBuf,
    pub preferences: PluginPreferences,
    pub error: Option<String>,
    inspected: BTreeMap<String, InspectedPlugin>,
    diagnostics: Vec<ModuleStatus>,
}
fn identity(path: &std::path::Path, manifest: &PluginManifest) -> Result<String> {
    Ok(serde_json::to_string(&(path.canonicalize()?, manifest))?)
}
impl LegacyPluginSettings {
    pub fn open(
        path: PathBuf,
        directories: Vec<PathBuf>,
        registry: &mut ActionRegistry,
    ) -> Result<Self> {
        static BOOT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let boot = BOOT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut settings = Self {
            path,
            preferences: Default::default(),
            error: None,
            inspected: Default::default(),
            diagnostics: vec![],
        };
        settings.read();
        let mut found = Vec::new();
        let mut origins = BTreeSet::new();
        for directory in directories {
            let discovery = anywhere_plugin::discover_with_paths(&directory);
            for detail in discovery.diagnostics {
                settings
                    .diagnostics
                    .push(invalid(directory.display().to_string(), detail));
            }
            for (path, manifest) in discovery.manifests {
                let origin = path.canonicalize()?;
                if origins.insert(origin) {
                    found.push((path, manifest));
                }
            }
        }
        let mut counts = BTreeMap::new();
        for (_, manifest) in &found {
            *counts.entry(manifest.id.clone()).or_insert(0) += 1;
        }
        for (path, manifest) in found {
            if counts[&manifest.id] > 1 {
                if !settings.diagnostics.iter().any(|s| s.id == manifest.id) {
                    settings.diagnostics.push(invalid(
                        manifest.id,
                        "Duplicate plugin id; all installations are blocked".into(),
                    ));
                }
                continue;
            }
            let key = format!("plugin:{}", manifest.id);
            let identity = identity(&path, &manifest)?;
            // Register captured definitions at startup only, including currently disabled
            // approved definitions. Their settings select from this immutable catalogue.
            let approved = manifest.protocol_version == 1
                || settings.preferences.approvals.get(&key) == Some(&identity);
            let result = if approved && !settings.preferences.recovery {
                registry.register_owned_plugin(manifest.clone(), &manifest.capabilities, Some(&key))
            } else {
                Ok(())
            };
            settings.inspected.insert(
                key.clone(),
                InspectedPlugin {
                    path,
                    token: format!("{boot}:{key}"),
                    identity,
                    manifest,
                    loaded: approved && !settings.preferences.recovery && result.is_ok(),
                    error: result.err().map(|e| format!("{e:#}")),
                },
            );
        }
        Ok(settings)
    }
    pub fn read(&mut self) {
        let loaded = PluginPreferences::load(&self.path);
        self.error = loaded.as_ref().err().map(|e| format!("Plugin settings could not be read: {e:#}. Repair {} and Refresh. The saved file has not been changed.", self.path.display()));
        self.preferences = loaded.unwrap_or_else(|_| PluginPreferences {
            recovery: true,
            ..Default::default()
        });
    }
    fn unchanged(plugin: &InspectedPlugin) -> bool {
        anywhere_plugin::read_manifest(&plugin.path)
            .and_then(|manifest| identity(&plugin.path, &manifest))
            .is_ok_and(|fresh| fresh == plugin.identity)
    }
    pub fn verify_definitions(&mut self) {
        for plugin in self.inspected.values_mut() {
            if !Self::unchanged(plugin) {
                plugin.error =
                    Some("Definition changed. Restart Anywhere to inspect the new version.".into());
            }
        }
    }
    pub fn selected(&self, key: &str) -> bool {
        self.available_at_startup(key, &self.preferences)
    }
    pub fn available_at_startup(&self, key: &str, preferences: &PluginPreferences) -> bool {
        self.inspected.get(key).is_some_and(|p| {
            p.loaded
                && p.error.is_none()
                && !preferences.recovery
                && !preferences.disabled.contains(key)
                && (p.manifest.protocol_version == 1
                    || preferences.approvals.get(key) == Some(&p.identity))
        })
    }
    pub fn prepare_change(&self, change: &SettingsChange) -> Result<PluginPreferences> {
        ensure!(
            !self.preferences.recovery,
            "plugin settings are read-only; repair and Refresh"
        );
        let key = change.key();
        let plugin = self
            .inspected
            .get(key)
            .context("plugin unavailable; restart to discover changes")?;
        ensure!(
            Self::unchanged(plugin),
            "plugin definition changed; restart and review it"
        );
        ensure!(
            plugin.error.is_none(),
            "plugin unavailable; restart and review it"
        );
        let mut next = self.preferences.clone();
        match change {
            SettingsChange::Enable { enabled, .. } => {
                if *enabled {
                    next.disabled.remove(key);
                } else {
                    next.disabled.insert(key.into());
                }
            }
            SettingsChange::Approve { token, .. } => {
                ensure!(
                    plugin.manifest.protocol_version != 1 && *token == plugin.token,
                    "approval is stale; restart and review the plugin"
                );
                next.approvals.insert(key.into(), plugin.identity.clone());
                next.disabled.remove(key);
            }
            SettingsChange::Revoke { .. } => {
                ensure!(
                    plugin.manifest.protocol_version != 1,
                    "legacy plugin has no revocable grant"
                );
                next.approvals.remove(key);
            }
        }
        Ok(next)
    }
    pub fn statuses(&self) -> Vec<ModuleStatus> {
        let mut statuses = self.diagnostics.clone();
        for (key, p) in &self.inspected {
            let enabled = !self.preferences.recovery && !self.preferences.disabled.contains(key);
            let approved = (p.manifest.protocol_version == 1
                || self.preferences.approvals.get(key) == Some(&p.identity))
                && p.error.is_none();
            let active = self.selected(key);
            let detail = p.error.clone().unwrap_or_else(|| {
                if !enabled {
                    "Disabled"
                } else if !approved {
                    "Needs permission approval"
                } else if !active {
                    "Restart Anywhere to activate this approved plugin"
                } else {
                    "Enabled · Startup definition"
                }
                .into()
            });
            statuses.push(ModuleStatus {
                key: key.clone(),
                id: p.manifest.id.clone(),
                name: p.manifest.name.clone(),
                version: p.manifest.version.clone().unwrap_or_else(|| "v1".into()),
                first_party: false,
                legacy: p.manifest.protocol_version == 1,
                enabled,
                active,
                needs_approval: !approved,
                configurable: !self.preferences.recovery && p.error.is_none(),
                restart_required: enabled && approved && !active,
                capabilities: p
                    .manifest
                    .capabilities
                    .iter()
                    .map(|c| c.as_str().into())
                    .collect(),
                approval_token: p.token.clone(),
                detail,
            });
        }
        if let Some(error) = &self.error {
            statuses.push(invalid("Plugin settings recovery".into(), error.clone()));
        }
        statuses
    }
}
pub(super) fn invalid(id: String, detail: String) -> ModuleStatus {
    ModuleStatus {
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
        restart_required: false,
        capabilities: vec![],
        approval_token: String::new(),
        detail,
    }
}
