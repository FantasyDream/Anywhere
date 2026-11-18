//! Native feature configuration, independent of external package approval.
use super::config_file;
use crate::features::descriptor::valid_id;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

const MAX_BYTES: usize = 128 * 1024;
const LEGACY_MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize)]
struct LegacyExtensionPreferences {
    schema_version: u32,
    #[serde(default)]
    disabled: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeaturePreferences {
    pub schema_version: u32,
    /// Bare, stable feature IDs; valid unknown IDs survive module removal / reintroduction.
    pub disabled: BTreeSet<String>,
    #[serde(skip)]
    pub recovery: bool,
}

impl Default for FeaturePreferences {
    fn default() -> Self {
        Self {
            schema_version: 1,
            disabled: BTreeSet::new(),
            recovery: false,
        }
    }
}

impl FeaturePreferences {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1,
            "unsupported feature preference version"
        );
        ensure!(
            self.disabled.len() <= 1024 && self.disabled.iter().all(|id| valid_id(id)),
            "invalid disabled feature IDs"
        );
        Ok(())
    }

    fn load(path: &Path) -> Result<Option<Self>> {
        let Some(raw) = config_file::read_bounded(path, MAX_BYTES)? else {
            return Ok(None);
        };
        ensure!(raw.len() <= MAX_BYTES, "feature preferences exceed 128 KiB");
        let prefs: Self = serde_json::from_slice(&raw).context("invalid feature preferences")?;
        prefs.validate()?;
        Ok(Some(prefs))
    }

    fn load_legacy_disabled(path: &Path) -> Result<BTreeSet<String>> {
        let Some(raw) = config_file::read_bounded(path, LEGACY_MAX_BYTES)? else {
            return Ok(BTreeSet::new());
        };
        ensure!(
            raw.len() <= LEGACY_MAX_BYTES,
            "extension preferences exceed 4 MiB"
        );
        let prefs: LegacyExtensionPreferences =
            serde_json::from_slice(&raw).context("invalid extension preferences")?;
        ensure!(
            prefs.schema_version == 1,
            "unsupported extension preference version"
        );
        Ok(prefs.disabled)
    }

    /// Never re-import legacy keys once a valid new configuration exists. The old
    /// file is retained unchanged, including if this migration cannot be persisted.
    pub fn load_or_migrate(path: &Path, legacy_path: &Path) -> Result<Self> {
        if let Some(prefs) = Self::load(path)? {
            return Ok(prefs);
        }
        let legacy_disabled = Self::load_legacy_disabled(legacy_path)
            .map_err(|error| anyhow::anyhow!("legacy feature preferences unavailable: {error}"))?;
        let prefs = Self {
            disabled: legacy_disabled
                .iter()
                .filter_map(|key| key.strip_prefix("first-party:"))
                .filter(|id| valid_id(id))
                .map(str::to_owned)
                .collect(),
            ..Self::default()
        };
        prefs.validate()?;
        if !config_file::create_json(path, &prefs, MAX_BYTES)? {
            return Self::load(path)?
                .context("feature preferences changed during migration; retry");
        }
        Ok(prefs)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        ensure!(
            !self.recovery,
            "repair feature preferences and refresh before changing settings"
        );
        self.validate()?;
        config_file::write_json(path, self, MAX_BYTES)
    }

    /// A setting opened before a file was damaged / changed must not overwrite it.
    pub(crate) fn prepare_save(&self, path: &Path) -> Result<tempfile::NamedTempFile> {
        ensure!(
            !self.recovery,
            "repair feature preferences and refresh before changing settings"
        );
        self.validate()?;
        config_file::prepare_json(path, self, MAX_BYTES)
    }

    pub(crate) fn commit_prepared(
        &self,
        path: &Path,
        expected: &Self,
        file: tempfile::NamedTempFile,
    ) -> Result<()> {
        let current = Self::load(path)?
            .context("feature preferences were removed; refresh before changing settings")?;
        ensure!(
            current == *expected,
            "feature preferences changed; refresh before changing settings"
        );
        file.persist(path).context("could not save preferences")?;
        Ok(())
    }

    pub fn save_checked(&self, path: &Path, expected: &Self) -> Result<()> {
        self.commit_prepared(path, expected, self.prepare_save(path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_migration_is_idempotent_and_new_settings_are_authoritative() {
        let temporary = tempfile::tempdir().unwrap();
        let legacy_path = temporary.path().join("extensions.json");
        let path = temporary.path().join("features.json");
        let raw_legacy = serde_json::json!({
            "schema_version": 1,
            "disabled": [
                "first-party:json",
                "first-party:future_tool",
                "first-party:Bad",
                "plugin:example"
            ],
            "approvals": {
                "plugin:example": "exact inspected definition"
            }
        });
        std::fs::write(
            &legacy_path,
            serde_json::to_vec_pretty(&raw_legacy).unwrap(),
        )
        .unwrap();
        let original = std::fs::read(&legacy_path).unwrap();
        let mut migrated = FeaturePreferences::load_or_migrate(&path, &legacy_path).unwrap();
        assert_eq!(
            migrated.disabled,
            ["future_tool".into(), "json".into()].into()
        );
        assert_eq!(std::fs::read(&legacy_path).unwrap(), original);
        migrated.disabled.remove("json");
        migrated.save(&path).unwrap();
        let unreadable_legacy = temporary.path().join("broken_extensions.json");
        std::fs::write(&unreadable_legacy, "not json").unwrap();
        let reloaded = FeaturePreferences::load_or_migrate(&path, &unreadable_legacy).unwrap();
        assert_eq!(reloaded, migrated);
        assert_eq!(
            FeaturePreferences::load_or_migrate(&path, &legacy_path).unwrap(),
            migrated
        );
        assert_eq!(std::fs::read(&legacy_path).unwrap(), original);
        assert!(!std::fs::read_to_string(path).unwrap().contains("approvals"));
    }

    #[test]
    fn unknown_malformed_and_oversized_feature_files_are_never_replaced() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("features.json");
        let legacy_path = temporary.path().join("extensions.json");
        for raw in [
            "not json",
            r#"{"schema_version":99,"disabled":[]}"#,
            r#"{"schema_version":1,"disabled":["core.tool"]}"#,
            r#"{"schema_version":1,"disabled":["first-party:json"]}"#,
            r#"{"schema_version":1,"disabled":[],"approvals":{}}"#,
        ] {
            std::fs::write(&path, raw).unwrap();
            assert!(FeaturePreferences::load_or_migrate(&path, &legacy_path).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), raw);
        }
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(8 * 1024 * 1024).unwrap();
        assert!(FeaturePreferences::load_or_migrate(&path, &legacy_path)
            .unwrap_err()
            .to_string()
            .contains("128 KiB"));
        assert_eq!(std::fs::metadata(path).unwrap().len(), 8 * 1024 * 1024);
    }

    #[test]
    fn failed_migration_can_be_retried_without_losing_legacy_keys() {
        let temporary = tempfile::tempdir().unwrap();
        let legacy_path = temporary.path().join("extensions.json");
        let path = temporary.path().join("missing_directory/features.json");
        std::fs::write(
            &legacy_path,
            r#"{"schema_version":1,"disabled":["first-party:json"]}"#,
        )
        .unwrap();
        let original = std::fs::read(&legacy_path).unwrap();
        assert!(FeaturePreferences::load_or_migrate(&path, &legacy_path).is_err());
        assert!(!path.exists());
        assert_eq!(std::fs::read(&legacy_path).unwrap(), original);
        std::fs::create_dir(path.parent().unwrap()).unwrap();
        assert!(FeaturePreferences::load_or_migrate(&path, &legacy_path)
            .unwrap()
            .disabled
            .contains("json"));
        assert_eq!(std::fs::read(legacy_path).unwrap(), original);
    }

    #[test]
    fn migration_creation_cannot_replace_existing_authoritative_configuration() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("features.json");
        let current = FeaturePreferences {
            disabled: ["url".into()].into(),
            ..Default::default()
        };
        current.save(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let legacy = FeaturePreferences {
            disabled: ["json".into()].into(),
            ..Default::default()
        };
        assert!(!config_file::create_json(&path, &legacy, MAX_BYTES).unwrap());
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
}
