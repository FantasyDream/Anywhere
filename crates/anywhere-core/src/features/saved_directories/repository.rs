//! The only bookmark component that knows the storage key and wire format.
use crate::core::storage::ExtensionStorage;
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

pub(super) const LIMIT: usize = 100;
pub(super) const KEY: &str = "directories";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    version: u32,
    paths: Vec<PathBuf>,
}

pub(super) struct SavedDirectoryRepository {
    storage: Arc<dyn ExtensionStorage>,
}

impl SavedDirectoryRepository {
    pub fn new(storage: Arc<dyn ExtensionStorage>) -> Self {
        Self { storage }
    }

    fn decode(raw: Option<&[u8]>) -> Result<Saved> {
        let saved = match raw {
            Some(raw) => serde_json::from_slice(raw)?,
            None => Saved {
                version: 1,
                paths: vec![],
            },
        };
        ensure!(
            saved.version == 1
                && saved.paths.len() <= LIMIT
                && saved.paths.iter().all(|path| path.is_absolute()),
            "invalid saved directories"
        );
        Ok(saved)
    }

    pub fn load(&self) -> Result<Vec<PathBuf>> {
        Ok(Self::decode(self.storage.get(KEY)?.as_deref())?.paths)
    }

    /// Mutate the latest committed data, never the service's in-memory snapshot.
    pub fn change(&self, path: &std::path::Path, save: bool) -> Result<Vec<PathBuf>> {
        let committed = self.storage.update(KEY, &mut |raw| {
            let mut next = Self::decode(raw)?;
            next.paths.retain(|entry| entry != path);
            if save {
                ensure!(
                    next.paths.len() < LIMIT,
                    "saved directory limit is 100; forget an existing directory first"
                );
                next.paths.push(path.to_owned());
                next.paths.sort();
            }
            Ok(Some(serde_json::to_vec(&next)?))
        })?;
        Ok(Self::decode(committed.as_deref())?.paths)
    }
}
