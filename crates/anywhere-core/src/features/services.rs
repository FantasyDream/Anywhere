//! App-owned, typed service bindings. Modules cannot access the host database.
use crate::{
    core::storage::ExtensionStorage, features::saved_directories::service::SavedDirectoriesService,
};
use anyhow::Result;
use std::sync::Arc;

pub struct FeatureServices {
    saved_directories: Arc<SavedDirectoriesService>,
    clipboard: Arc<crate::features::clipboard::service::HistoryService>,
    application_history: Option<Arc<crate::features::application_history::service::HistoryService>>,
}

impl FeatureServices {
    /// A typed binding exists even when its persistent data needs repair.
    pub fn new(storage: Result<Arc<dyn ExtensionStorage>, String>) -> Self {
        Self {
            application_history: None,
            saved_directories: Arc::new(SavedDirectoriesService::pending(storage)),
            clipboard: Arc::new(crate::features::clipboard::service::HistoryService::new(
                Err("not bound".into()),
            )),
        }
    }
    pub(crate) fn bind_clipboard(
        &mut self,
        service: Arc<crate::features::clipboard::service::HistoryService>,
    ) {
        self.clipboard = service;
    }
    pub fn clipboard(&self) -> Arc<crate::features::clipboard::service::HistoryService> {
        self.clipboard.clone()
    }
    pub(crate) fn bind_application_history(
        &mut self,
        service: Arc<crate::features::application_history::service::HistoryService>,
    ) {
        self.application_history = Some(service);
    }
    pub fn application_history(
        &self,
    ) -> Result<Arc<crate::features::application_history::service::HistoryService>> {
        self.application_history
            .clone()
            .ok_or_else(|| anyhow::anyhow!("application history not bound"))
    }
    pub(crate) fn saved_directories(&self) -> Arc<SavedDirectoriesService> {
        self.saved_directories.clone()
    }
    pub(crate) fn refresh_saved_directories(
        &self,
        storage: Result<Arc<dyn ExtensionStorage>, String>,
    ) -> Result<()> {
        self.saved_directories.refresh_binding(storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::module_storage::ModuleStorageHost;

    #[test]
    fn service_is_shared_and_failed_refresh_preserves_data_until_repaired() {
        let storage = ModuleStorageHost::memory()
            .unwrap()
            .bind("saved_directories")
            .unwrap();
        let services = FeatureServices::new(Ok(storage.clone()));
        let first = services.saved_directories();
        first.refresh().unwrap();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap();
        first.save(&path).unwrap();
        let second = services.saved_directories();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            second.paths().unwrap().as_slice(),
            std::slice::from_ref(&path)
        );
        storage
            .put("directories", br#"{"version":99,"paths":[]}"#)
            .unwrap();
        assert!(first.refresh().is_err());
        assert_eq!(first.paths().unwrap(), [path]);
        assert_eq!(
            storage.get("directories").unwrap().unwrap(),
            br#"{"version":99,"paths":[]}"#
        );
        storage.remove("directories").unwrap();
        first.refresh().unwrap();
        let repaired = services.saved_directories();
        assert!(Arc::ptr_eq(&first, &repaired));
        assert!(repaired.paths().unwrap().is_empty());
    }
}
