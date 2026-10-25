//! Defaults use only synthetic installed-app snapshots, never this machine's apps.
use super::*;
use crate::{
    features::application_history::service::HistoryService,
    persistence::module_storage::ModuleStorageHost,
};

fn storage() -> Arc<dyn crate::core::storage::ExtensionStorage> {
    ModuleStorageHost::memory()
        .unwrap()
        .bind("application_history")
        .unwrap()
}
fn loader() -> Arc<dyn SourceLoader> {
    SourceCatalog::bundled_with_config_directory(None).unwrap()
}
fn enabled(service: &HistoryService, id: &str) -> bool {
    service
        .sources()
        .iter()
        .find(|s| s.descriptor.id == id)
        .unwrap()
        .action_enabled
}

#[test]
fn absent_apps_are_off_and_installed_variants_share_one_default() {
    let storage = storage();
    let missing = HistoryService::new(Ok(storage.clone()), loader());
    missing.refresh().unwrap();
    assert!(missing.sources().iter().all(|s| !s.action_enabled));
    assert_eq!(
        missing.disabled_action_ids().len(),
        BUNDLED_HISTORY_SOURCES.len()
    );
    assert!(missing.page_input("idea").is_err());
    let installed =
        HistoryService::new(Ok(storage.clone()), loader()).with_installed_applications([
            "com.jetbrains.intellij.ce".into(),
            "com.todesktop.230313mzl4w4u92".into(),
        ]);
    installed.refresh().unwrap();
    assert!(enabled(&installed, "idea"));
    assert!(enabled(&installed, "cursor"));
    assert!(!enabled(&installed, "vscode"));
    assert!(
        storage.get("sources").unwrap().is_none(),
        "defaults must not become user choices"
    );
}

#[test]
fn implicit_defaults_follow_installation_on_restart_without_becoming_persisted() {
    let storage = storage();
    let first = HistoryService::new(Ok(storage.clone()), loader())
        .with_installed_applications(["com.jetbrains.intellij".into()]);
    first.refresh().unwrap();
    assert!(enabled(&first, "idea"));
    first.set_action_enabled("vscode", true).unwrap();
    let absent = HistoryService::new(Ok(storage.clone()), loader());
    absent.refresh().unwrap();
    assert!(!enabled(&absent, "idea"));
    assert!(
        enabled(&absent, "vscode"),
        "an explicit user enable survives a missing app"
    );
    let again = HistoryService::new(Ok(storage), loader())
        .with_installed_applications(["com.jetbrains.intellij".into()]);
    again.refresh().unwrap();
    assert!(enabled(&again, "idea"));
}

#[test]
fn explicit_disable_and_custom_paths_survive_changes_in_installed_apps() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("recent.xml");
    std::fs::write(
        &path,
        "<application><component><option name='recentPaths'/></component></application>",
    )
    .unwrap();
    let storage = storage();
    let first = HistoryService::new(Ok(storage.clone()), loader());
    first.configure("idea", path.to_str()).unwrap();
    assert!(
        enabled(&first, "idea"),
        "setting a custom path is an explicit enable"
    );
    first.set_action_enabled("idea", false).unwrap();
    let installed = HistoryService::new(Ok(storage.clone()), loader())
        .with_installed_applications(["com.jetbrains.intellij".into()]);
    installed.refresh().unwrap();
    assert!(!enabled(&installed, "idea"));
    assert_eq!(
        installed
            .sources()
            .into_iter()
            .find(|s| s.descriptor.id == "idea")
            .unwrap()
            .path,
        Some(path.canonicalize().unwrap())
    );
    installed.set_action_enabled("idea", true).unwrap();
    let absent = HistoryService::new(Ok(storage), loader());
    absent.refresh().unwrap();
    assert!(enabled(&absent, "idea"));
}

#[test]
fn leftover_metadata_does_not_count_as_an_installed_app() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Code/User/globalStorage/state.vscdb");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "not a database; must not be read").unwrap();
    let catalog = SourceCatalog::bundled_with_config_directory(Some(root.path().into())).unwrap();
    assert!(catalog.detect("vscode").is_some());
    let service = HistoryService::new(Ok(storage()), catalog);
    service.refresh().unwrap();
    assert!(!enabled(&service, "vscode"));
    assert!(service
        .sources()
        .iter()
        .find(|s| s.descriptor.id == "vscode")
        .unwrap()
        .path
        .is_none());
    assert!(service
        .search(0, "", "vscode")
        .unwrap()
        .failed_sources
        .is_empty());
}
