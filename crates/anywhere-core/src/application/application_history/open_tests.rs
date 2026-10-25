//! Open dispatch tests deliberately never launch an application or touch the OS clipboard.
use super::SourceCatalog;
use crate::{
    features::application_history::{model::*, service::HistoryService},
    persistence::module_storage::ModuleStorageHost,
};
use anyhow::{bail, Result};
use anywhere_platform::{ApplicationInfo, BundleApplication, InstalledApplications, Platform};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

struct OpenRecorder {
    apps: Vec<BundleApplication>,
    opened: Mutex<Vec<(PathBuf, PathBuf)>>,
    copied: Mutex<Vec<String>>,
    urls: Mutex<Vec<(PathBuf, String)>>,
}
impl OpenRecorder {
    fn new(ids: &[(&str, &str)]) -> Self {
        Self {
            apps: ids
                .iter()
                .map(|(id, path)| BundleApplication {
                    bundle_id: (*id).into(),
                    name: (*id).into(),
                    path: Some((*path).into()),
                    localized_names: Default::default(),
                    aliases: vec![],
                    icon: None,
                })
                .collect(),
            opened: Default::default(),
            copied: Default::default(),
            urls: Default::default(),
        }
    }
}
impl Platform for OpenRecorder {
    fn installed_applications(&self) -> InstalledApplications {
        InstalledApplications {
            applications: self.apps.clone(),
            skipped: 0,
        }
    }
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        Ok(None)
    }
    fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<PathBuf>> {
        Ok(None)
    }
    fn open_url(&self, _: &str) -> Result<()> {
        bail!("default URL opening must not be used")
    }
    fn open_file(&self, _: &Path) -> Result<()> {
        bail!("default file opening must not be used")
    }
    fn reveal_file(&self, _: &Path) -> Result<()> {
        bail!("reveal must not be used")
    }
    fn open_directory(&self, _: &Path) -> Result<()> {
        bail!("default folder opening must not be used")
    }
    fn open_terminal(&self, _: &Path) -> Result<()> {
        bail!("terminal must not be used")
    }
    fn open_file_in_application(&self, application: &Path, target: &Path) -> Result<()> {
        self.opened
            .lock()
            .unwrap()
            .push((application.into(), target.into()));
        Ok(())
    }
    fn copy_text(&self, text: &str) -> Result<()> {
        self.copied.lock().unwrap().push(text.into());
        Ok(())
    }
    fn open_url_in_application(&self, application: &Path, url: &str) -> Result<()> {
        self.urls
            .lock()
            .unwrap()
            .push((application.into(), url.into()));
        Ok(())
    }
}
fn service() -> HistoryService {
    let storage = ModuleStorageHost::memory()
        .unwrap()
        .bind("application_history")
        .unwrap();
    HistoryService::new(
        Ok(storage),
        SourceCatalog::bundled_with_config_directory(None).unwrap(),
    )
    .with_installed_applications([
        "com.jetbrains.intellij".into(),
        "com.microsoft.VSCode".into(),
        "md.obsidian".into(),
    ])
}
fn write_idea(source: &Path, project: &Path) {
    std::fs::write(source, format!("<application><component name=\"RecentProjectsManager\"><option name=\"recentPaths\"><list><option value=\"{}\"/></list></option></component></application>", project.display())).unwrap();
}

#[test]
fn obsidian_opens_the_exact_registered_vault_in_the_selected_app_and_revalidates_it() {
    let temp = tempfile::tempdir().unwrap();
    let vault = temp.path().join("合成仓库");
    std::fs::create_dir(&vault).unwrap();
    let source = temp.path().join("obsidian.json");
    let write = |id: &str| {
        std::fs::write(
            &source,
            json!({"vaults":{id:{"path":vault,"ts":1700000000000_i64}}}).to_string(),
        )
        .unwrap()
    };
    write("0123456789abcdef");
    let service = service();
    service
        .configure_mode("obsidian", SourceMode::Custom, source.to_str())
        .unwrap();
    let row = service.search(0, "", "obsidian").unwrap().items.remove(0);
    let platform = OpenRecorder::new(&[("md.obsidian", "/mock/Obsidian.app")]);
    service.use_item(&row.id, false, &platform).unwrap();
    assert_eq!(
        *platform.urls.lock().unwrap(),
        vec![(
            PathBuf::from("/mock/Obsidian.app"),
            "obsidian://open?vault=0123456789abcdef".into()
        )]
    );
    assert!(platform.opened.lock().unwrap().is_empty());
    write("fedcba9876543210");
    assert!(service.use_item(&row.id, false, &platform).is_err());
    assert_eq!(platform.urls.lock().unwrap().len(), 1);
    let row = service.search(0, "", "obsidian").unwrap().items.remove(0);
    let duplicate = OpenRecorder::new(&[
        ("md.obsidian", "/mock/A.app"),
        ("md.obsidian", "/mock/B.app"),
    ]);
    assert!(service.use_item(&row.id, false, &duplicate).is_err());
    assert!(duplicate.urls.lock().unwrap().is_empty());
}

#[test]
fn each_local_kind_dispatches_to_its_source_app_without_using_default_openers() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let idea = root.join("IDEA 测试 项目");
    let folder = root.join("VS Code 目录");
    let file = root.join("文本 文件.txt");
    let workspace = root.join("多根.code-workspace");
    std::fs::create_dir(&idea).unwrap();
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(&file, "synthetic fixture").unwrap();
    std::fs::write(&workspace, "{\"folders\":[]}").unwrap();
    let xml = root.join("recentProjects.xml");
    write_idea(&xml, &idea);
    let db_path = root.join("state.vscdb");
    let db = rusqlite::Connection::open(&db_path).unwrap();
    db.execute_batch("CREATE TABLE ItemTable(key TEXT PRIMARY KEY,value TEXT)")
        .unwrap();
    let uri = |path: &Path| url::Url::from_file_path(path).unwrap().to_string();
    let document = json!({"entries":[
        {"folderUri":uri(&folder)}, {"fileUri":uri(&file)},
        {"workspace":{"configPath":uri(&workspace)}},
        {"folderUri":"vscode-remote://ssh-remote+synthetic/project"}
    ]});
    db.execute(
        "INSERT INTO ItemTable VALUES (?1,?2)",
        rusqlite::params!["history.recentlyOpenedPathsList", document.to_string()],
    )
    .unwrap();
    let service = service();
    service
        .configure_mode("idea", SourceMode::Custom, Some(xml.to_str().unwrap()))
        .unwrap();
    service
        .configure_mode(
            "vscode",
            SourceMode::Custom,
            Some(db_path.to_str().unwrap()),
        )
        .unwrap();
    let platform = OpenRecorder::new(&[
        ("com.jetbrains.intellij", "/mock/IDEA.app"),
        ("com.microsoft.VSCode", "/mock/Code.app"),
    ]);
    for source in ["idea", "vscode"] {
        let rows = service.search(0, "", source).unwrap().items;
        for item in rows {
            if matches!(item.entry.target, Target::Remote { .. }) {
                assert!(service.use_item(&item.id, false, &platform).is_err());
                service.use_item(&item.id, true, &platform).unwrap();
            } else {
                service.use_item(&item.id, false, &platform).unwrap();
            }
        }
    }
    assert_eq!(
        *platform.opened.lock().unwrap(),
        [
            (PathBuf::from("/mock/IDEA.app"), idea),
            (PathBuf::from("/mock/Code.app"), folder),
            (PathBuf::from("/mock/Code.app"), file),
            (PathBuf::from("/mock/Code.app"), workspace),
        ]
    );
    assert_eq!(
        *platform.copied.lock().unwrap(),
        ["vscode-remote://ssh-remote+synthetic/project"]
    );
}

#[test]
fn ambiguous_apps_changed_targets_and_revoked_sources_fail_before_dispatch() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let project = root.join("project");
    let changed = root.join("other-project");
    std::fs::create_dir(&project).unwrap();
    std::fs::create_dir(&changed).unwrap();
    let xml = root.join("recentProjects.xml");
    write_idea(&xml, &project);
    let service = service();
    service
        .configure_mode("idea", SourceMode::Custom, Some(xml.to_str().unwrap()))
        .unwrap();
    let row = service.search(0, "", "idea").unwrap().items.remove(0);
    for apps in [
        vec![],
        vec![
            ("com.jetbrains.intellij", "/mock/One.app"),
            ("com.jetbrains.intellij.ce", "/mock/Two.app"),
        ],
    ] {
        let platform = OpenRecorder::new(&apps);
        assert!(service.use_item(&row.id, false, &platform).is_err());
        assert!(platform.opened.lock().unwrap().is_empty());
    }
    let platform = OpenRecorder::new(&[("com.jetbrains.intellij", "/mock/IDEA.app")]);
    write_idea(&xml, &changed);
    assert!(service.use_item(&row.id, false, &platform).is_err());
    write_idea(&xml, &project);
    // It still has the same source identity, but is no longer a directory.
    std::fs::remove_dir(&project).unwrap();
    std::fs::write(&project, "changed type").unwrap();
    assert!(service.use_item(&row.id, false, &platform).is_err());
    service
        .configure_mode("idea", SourceMode::Disabled, None)
        .unwrap();
    assert!(service.use_item(&row.id, false, &platform).is_err());
    assert!(platform.opened.lock().unwrap().is_empty());
}
