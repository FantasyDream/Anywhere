//! External plugin approvals. Legacy native disable keys are inert migration backups.
use super::config_file;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginPreferences {
    pub schema_version: u32,
    /// Plugin keys plus retained legacy first-party keys, which are never used as native policy.
    pub disabled: BTreeSet<String>,
    /// Exact canonical manifest plus installation origin; no grant by plugin id alone.
    pub approvals: BTreeMap<String, String>,
    #[serde(skip)]
    pub recovery: bool,
}

impl Default for PluginPreferences {
    fn default() -> Self {
        Self {
            schema_version: 1,
            disabled: Default::default(),
            approvals: Default::default(),
            recovery: false,
        }
    }
}
impl PluginPreferences {
    pub fn load(path: &Path) -> Result<Self> {
        let Some(raw) = config_file::read_bounded(path, 4 * 1024 * 1024)? else {
            return Ok(Self::default());
        };
        ensure!(
            raw.len() <= 4 * 1024 * 1024,
            "extension preferences exceed 4 MiB"
        );
        let prefs: Self = serde_json::from_slice(&raw).context("invalid extension preferences")?;
        ensure!(
            prefs.schema_version == 1,
            "unsupported extension preference version"
        );
        Ok(prefs)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        ensure!(
            !self.recovery,
            "repair extension preferences and refresh before changing settings"
        );
        ensure!(
            self.schema_version == 1,
            "unsupported extension preference version"
        );
        config_file::write_json(path, self, 4 * 1024 * 1024)
    }

    pub fn save_checked(&self, path: &Path, expected: &Self) -> Result<()> {
        let current = Self::load(path)?;
        ensure!(
            current == *expected,
            "plugin preferences changed; refresh before changing settings"
        );
        self.save(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_preferences_are_rejected_and_never_rewritten() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("extensions.json");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(64 * 1024 * 1024).unwrap();
        assert!(PluginPreferences::load(&path)
            .unwrap_err()
            .to_string()
            .contains("4 MiB"));
        assert_eq!(std::fs::metadata(path).unwrap().len(), 64 * 1024 * 1024);
    }
}
