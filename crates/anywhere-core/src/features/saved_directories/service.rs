//! Bounded bookmark cache and use cases, independent of module reloads.
use super::repository::SavedDirectoryRepository;
use crate::core::storage::ExtensionStorage;
use anyhow::{ensure, Result};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub(crate) struct SavedDirectoriesService {
    repository: Mutex<Result<SavedDirectoryRepository, String>>,
    paths: Mutex<Vec<PathBuf>>,
    writes: Mutex<()>,
}

impl SavedDirectoriesService {
    #[cfg(test)]
    pub fn open(storage: Arc<dyn ExtensionStorage>) -> Result<Self> {
        let repository = SavedDirectoryRepository::new(storage);
        let paths = repository.load()?;
        Ok(Self {
            repository: Mutex::new(Ok(repository)),
            paths: Mutex::new(paths),
            writes: Mutex::new(()),
        })
    }

    /// Bind handlers at startup without reading disabled feature data.
    pub fn pending(storage: Result<Arc<dyn ExtensionStorage>, String>) -> Self {
        Self {
            repository: Mutex::new(storage.map(SavedDirectoryRepository::new)),
            paths: Mutex::new(Vec::new()),
            writes: Mutex::new(()),
        }
    }

    /// Data repair swaps a typed repository inside the same service. No action is rebuilt.
    pub fn refresh_binding(
        &self,
        storage: Result<Arc<dyn ExtensionStorage>, String>,
    ) -> Result<()> {
        let _write = self
            .writes
            .lock()
            .map_err(|_| anyhow::anyhow!("saved directory writes unavailable"))?;
        let mut repository = self
            .repository
            .lock()
            .map_err(|_| anyhow::anyhow!("saved directories unavailable"))?;
        *repository = storage.map(SavedDirectoryRepository::new);
        let paths = repository
            .as_ref()
            .map_err(|e| anyhow::anyhow!(e.clone()))?
            .load()?;
        self.publish(paths)
    }

    /// Queries never touch disk or wait for a transaction / refresh to finish.
    pub fn paths(&self) -> Result<Vec<PathBuf>> {
        Ok(self
            .paths
            .lock()
            .map_err(|_| anyhow::anyhow!("saved directories unavailable"))?
            .clone())
    }

    fn publish(&self, paths: Vec<PathBuf>) -> Result<()> {
        *self
            .paths
            .lock()
            .map_err(|_| anyhow::anyhow!("saved directories unavailable"))? = paths;
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        ensure!(
            path.is_absolute(),
            "saved directory must be an absolute path"
        );
        self.change(path, true)
    }

    pub fn forget(&self, path: &Path) -> Result<()> {
        self.change(path, false)
    }

    fn change(&self, path: &Path, save: bool) -> Result<()> {
        let _write = self
            .writes
            .lock()
            .map_err(|_| anyhow::anyhow!("saved directory writes unavailable"))?;
        let repository = self
            .repository
            .lock()
            .map_err(|_| anyhow::anyhow!("saved directories unavailable"))?;
        self.publish(
            repository
                .as_ref()
                .map_err(|e| anyhow::anyhow!(e.clone()))?
                .change(path, save)?,
        )
    }

    pub fn refresh(&self) -> Result<()> {
        let _write = self
            .writes
            .lock()
            .map_err(|_| anyhow::anyhow!("saved directory writes unavailable"))?;
        let repository = self
            .repository
            .lock()
            .map_err(|_| anyhow::anyhow!("saved directories unavailable"))?;
        self.publish(
            repository
                .as_ref()
                .map_err(|e| anyhow::anyhow!(e.clone()))?
                .load()?,
        )
    }

    pub fn resolve_for_open(&self, path: &Path) -> Result<PathBuf> {
        // Revalidate independent host-instance commits, including removal and corruption.
        self.refresh()?;
        ensure!(
            self.paths()?.iter().any(|entry| entry == path),
            "directory is no longer saved"
        );
        let canonical = path.canonicalize()?;
        ensure!(
            canonical == path && canonical.is_dir(),
            "saved directory changed or no longer exists"
        );
        Ok(canonical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::module_storage::ModuleStorageHost;

    #[test]
    fn independent_services_preserve_all_transactional_saves() {
        let temporary = tempfile::tempdir().unwrap();
        let database = temporary.path().join("storage.sqlite3");
        let root = temporary.path().canonicalize().unwrap();
        let services: Vec<_> = (0..2)
            .map(|_| {
                SavedDirectoriesService::open(
                    ModuleStorageHost::open(&database)
                        .unwrap()
                        .bind("saved_directories")
                        .unwrap(),
                )
                .unwrap()
            })
            .collect();
        let writes: Vec<_> = services
            .into_iter()
            .enumerate()
            .map(|(writer, service)| {
                let root = root.clone();
                std::thread::spawn(move || {
                    for index in 0..50 {
                        service
                            .save(&root.join(format!("writer_{writer}_{index:02}")))
                            .unwrap();
                    }
                })
            })
            .collect();
        for write in writes {
            write.join().unwrap();
        }
        let reloaded = SavedDirectoriesService::open(
            ModuleStorageHost::open(&database)
                .unwrap()
                .bind("saved_directories")
                .unwrap(),
        )
        .unwrap();
        let paths = reloaded.paths().unwrap();
        assert_eq!(paths.len(), 100);
        for writer in 0..2 {
            for index in 0..50 {
                assert!(paths.contains(&root.join(format!("writer_{writer}_{index:02}"))));
            }
        }
    }
}
