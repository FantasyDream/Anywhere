//! Durable extension preferences. Failed writes leave the previous policy intact.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::Path,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionPreferences {
    pub schema_version: u32,
    pub disabled: BTreeSet<String>,
    /// Exact canonical manifest plus installation origin; no grant by plugin id alone.
    pub approvals: BTreeMap<String, String>,
    #[serde(skip)]
    pub recovery: bool,
}

impl Default for ExtensionPreferences {
    fn default() -> Self {
        Self {
            schema_version: 1,
            disabled: Default::default(),
            approvals: Default::default(),
            recovery: false,
        }
    }
}
impl ExtensionPreferences {
    pub fn load(path: &Path) -> Result<Self> {
        let mut raw = Vec::new();
        match std::fs::File::open(path) {
            Ok(file) => {
                file.take(4 * 1024 * 1024 + 1).read_to_end(&mut raw)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default())
            }
            Err(error) => return Err(error).context("extension preferences unavailable"),
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
            "repair extension preferences and reload before changing settings"
        );
        let raw = serde_json::to_vec_pretty(self)?;
        ensure!(
            raw.len() <= 4 * 1024 * 1024,
            "extension preferences exceed 4 MiB"
        );
        let parent = path
            .parent()
            .context("extension preferences need a directory")?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(&raw)?;
        file.as_file().sync_all()?;
        file.persist(path)
            .context("could not save extension preferences")?;
        Ok(())
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
        assert!(ExtensionPreferences::load(&path)
            .unwrap_err()
            .to_string()
            .contains("4 MiB"));
        assert_eq!(std::fs::metadata(path).unwrap().len(), 64 * 1024 * 1024);
    }
}
