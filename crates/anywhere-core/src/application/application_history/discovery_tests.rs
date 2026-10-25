#[cfg(test)]
mod cases {
    // Only synthetic configuration roots are inspected by these tests.
    use super::super::*;
    use crate::{
        features::application_history::service::HistoryService,
        persistence::module_storage::ModuleStorageHost,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        time::SystemTime,
    };

    fn write(root: &Path, relative: &str, body: &str) -> PathBuf {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, body).unwrap();
        path.canonicalize().unwrap()
    }
    fn xml(root: &Path, relative: &str) -> PathBuf {
        write(root, relative, &format!("<application><component name=\"RecentProjectsManager\"><option name=\"recentPaths\"><list><option value=\"{}\"/></list></option></component></application>", root.display()))
    }
    fn storage() -> Arc<dyn crate::core::storage::ExtensionStorage> {
        ModuleStorageHost::memory()
            .unwrap()
            .bind("application_history")
            .unwrap()
    }
    fn status(service: &HistoryService, id: &str) -> SourceStatus {
        service
            .sources()
            .into_iter()
            .find(|s| s.descriptor.id == id)
            .unwrap()
    }

    #[test]
    fn detects_supported_files_using_metadata_and_prefers_recent_configuration() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let older = write(
            root,
            "JetBrains/IntelliJIdea2026.2/options/recentProjects.xml",
            "not parsed during discovery",
        );
        let newer = write(
            root,
            "JetBrains/IdeaIC2026.1/options/recentProjects.xml",
            "also not parsed",
        );
        for (path, seconds) in [(&older, 10), (&newer, 20)] {
            fs::File::open(path)
                .unwrap()
                .set_times(
                    fs::FileTimes::new()
                        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)),
                )
                .unwrap();
        }
        let code = write(
            root,
            "Code/User/globalStorage/state.vscdb",
            "not a database",
        );
        let loader = SourceCatalog::bundled_with_config_directory(Some(root.into())).unwrap();
        assert_eq!(loader.detect("idea"), Some(newer));
        assert_eq!(loader.detect("vscode"), Some(code));
        assert_eq!(loader.detect("unknown"), None);
    }

    #[test]
    fn automatic_custom_and_disabled_modes_persist_without_overriding_the_user() {
        let temp = tempfile::tempdir().unwrap();
        let auto = xml(
            temp.path(),
            "JetBrains/IntelliJIdea2026.2/options/recentProjects.xml",
        );
        let custom = xml(temp.path(), "custom.xml");
        let loader =
            SourceCatalog::bundled_with_config_directory(Some(temp.path().into())).unwrap();
        let storage = storage();
        let service = HistoryService::new(Ok(storage.clone()), loader.clone());
        service.refresh().unwrap();
        assert_eq!(status(&service, "idea").mode, SourceMode::Automatic);
        assert_eq!(status(&service, "idea").path, Some(auto.clone()));
        assert!(
            storage.get("sources").unwrap().is_none(),
            "auto paths are not pinned in storage"
        );
        let first = service.search(0, "", "idea").unwrap().items[0].id.clone();
        service.refresh().unwrap();
        assert!(
            service
                .use_item(
                    &first,
                    true,
                    &crate::application::source_catalog_tests::QuietPlatform
                )
                .is_ok(),
            "unchanged detection must not revoke selections"
        );

        service.configure("idea", custom.to_str()).unwrap();
        assert!(service
            .use_item(
                &first,
                true,
                &crate::application::source_catalog_tests::QuietPlatform
            )
            .is_err());
        service.refresh().unwrap();
        assert_eq!(status(&service, "idea").mode, SourceMode::Custom);
        assert_eq!(status(&service, "idea").path, Some(custom.clone()));
        fs::remove_file(custom).unwrap();
        service.refresh().unwrap();
        assert_eq!(status(&service, "idea").mode, SourceMode::Custom);
        assert_eq!(
            service.search(0, "", "idea").unwrap().notice,
            Some(Notice::ReadFailed),
            "invalid custom paths must not silently fall back"
        );

        service
            .configure_mode("idea", SourceMode::Automatic, None)
            .unwrap();
        assert_eq!(status(&service, "idea").path, Some(auto.clone()));
        let automatic = HistoryService::new(Ok(storage.clone()), loader.clone());
        automatic.refresh().unwrap();
        assert_eq!(status(&automatic, "idea").mode, SourceMode::Automatic);
        assert_eq!(status(&automatic, "idea").path, Some(auto));
        let selected = service.search(0, "", "idea").unwrap().items[0].id.clone();
        service.configure("idea", None).unwrap();
        assert!(service
            .use_item(
                &selected,
                true,
                &crate::application::source_catalog_tests::QuietPlatform
            )
            .is_err());
        let reopened = HistoryService::new(Ok(storage), loader);
        reopened.refresh().unwrap();
        assert_eq!(status(&reopened, "idea").mode, SourceMode::Disabled);
        assert_eq!(status(&reopened, "idea").path, None);
        assert_eq!(
            reopened.search(0, "", "idea").unwrap().notice,
            Some(Notice::Unconfigured)
        );
    }

    #[test]
    fn an_automatic_source_created_later_is_found_when_its_page_opens() {
        let temp = tempfile::tempdir().unwrap();
        let service = HistoryService::new(
            Ok(storage()),
            SourceCatalog::bundled_with_config_directory(Some(temp.path().into())).unwrap(),
        );
        service.refresh().unwrap();
        assert_eq!(status(&service, "idea").mode, SourceMode::Automatic);
        assert!(status(&service, "idea").path.is_none());
        assert_eq!(
            service.search(0, "", "idea").unwrap().notice,
            Some(Notice::Unconfigured)
        );
        let path = xml(
            temp.path(),
            "JetBrains/IntelliJIdea2026.2/options/recentProjects.xml",
        );
        let owner = service.open_session().unwrap();
        assert_eq!(status(&service, "idea").path, Some(path));
        assert_eq!(service.search(owner, "", "idea").unwrap().items.len(), 1);
    }

    #[test]
    fn old_custom_paths_and_explicitly_disabled_sources_survive_migration() {
        let temp = tempfile::tempdir().unwrap();
        let path = xml(temp.path(), "legacy.xml");
        write(
            temp.path(),
            "Code/User/globalStorage/state.vscdb",
            "metadata only",
        );
        let storage = storage();
        storage
            .put(
                "sources",
                &serde_json::to_vec(
                    &serde_json::json!({"schema":1,"sources":[{"id":"idea","path":path}]}),
                )
                .unwrap(),
            )
            .unwrap();
        let service = HistoryService::new(
            Ok(storage.clone()),
            SourceCatalog::bundled_with_config_directory(Some(temp.path().into())).unwrap(),
        );
        service.refresh().unwrap();
        assert_eq!(status(&service, "idea").mode, SourceMode::Custom);
        assert_eq!(status(&service, "idea").path, Some(path));
        assert_eq!(status(&service, "vscode").mode, SourceMode::Disabled);
        service
            .configure_mode("vscode", SourceMode::Automatic, None)
            .unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&storage.get("sources").unwrap().unwrap()).unwrap();
        assert_eq!(value["schema"], 2);
        assert_eq!(status(&service, "idea").mode, SourceMode::Custom);
        assert_eq!(status(&service, "vscode").mode, SourceMode::Automatic);
    }

    #[test]
    fn discovery_rejects_unbounded_patterns_and_paths_outside_the_root() {
        use crate::persistence::application_history::discovery::{detect, validate};
        for invalid in [
            "/absolute/file",
            "../escape",
            "nested/../escape",
            "**/state.vscdb",
            "nested//file",
        ] {
            assert!(validate(&[invalid.into()]).is_err(), "{invalid}");
        }
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("Code/User/globalStorage/state.vscdb")).unwrap();
        assert_eq!(
            detect(root.path(), &["Code/User/globalStorage/state.vscdb".into()]),
            None
        );
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            write(outside.path(), "recentProjects.xml", "outside");
            fs::create_dir_all(root.path().join("JetBrains/IntelliJIdea2026.2")).unwrap();
            std::os::unix::fs::symlink(
                outside.path(),
                root.path().join("JetBrains/IntelliJIdea2026.2/options"),
            )
            .unwrap();
            assert_eq!(
                detect(
                    root.path(),
                    &["JetBrains/IntelliJIdea*/options/recentProjects.xml".into()]
                ),
                None
            );
        }
    }
}
