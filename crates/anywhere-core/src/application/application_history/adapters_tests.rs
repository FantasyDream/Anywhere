//! Synthetic files only: exercise shared parsers and detection for the bundled families.
use super::*;
use serde_json::json;
use std::{fs, path::Path};

fn read(catalog: &SourceCatalog, id: &str, path: &Path) -> Vec<Entry> {
    catalog
        .read(
            &SourcePreference {
                id: id.into(),
                path: path.into(),
            },
            Arc::new(|| false),
        )
        .unwrap()
}

#[test]
fn bundled_families_detect_only_their_own_configuration_and_keep_unique_app_identities() {
    let temp = tempfile::tempdir().unwrap();
    let cases = [
        (
            "idea",
            "JetBrains/IntelliJIdea2026.2/options/recentProjects.xml",
        ),
        (
            "pycharm",
            "JetBrains/PyCharmCE2026.2/options/recentProjects.xml",
        ),
        (
            "webstorm",
            "JetBrains/WebStorm2026.2/options/recentProjects.xml",
        ),
        (
            "goland",
            "JetBrains/GoLand2026.2/options/recentProjects.xml",
        ),
        ("clion", "JetBrains/CLion2026.2/options/recentProjects.xml"),
        (
            "phpstorm",
            "JetBrains/PhpStorm2026.2/options/recentProjects.xml",
        ),
        (
            "rubymine",
            "JetBrains/RubyMine2026.2/options/recentProjects.xml",
        ),
        (
            "datagrip",
            "JetBrains/DataGrip2026.2/options/recentProjects.xml",
        ),
        ("rider", "JetBrains/Rider2026.2/options/recentSolutions.xml"),
        (
            "rustrover",
            "JetBrains/RustRover2026.2/options/recentProjects.xml",
        ),
        (
            "android_studio",
            "Google/AndroidStudio2026.2/options/recentProjects.xml",
        ),
        ("vscode", "Code/User/globalStorage/state.vscdb"),
        (
            "vscode_insiders",
            "Code - Insiders/User/globalStorage/state.vscdb",
        ),
        ("cursor", "Cursor/User/globalStorage/state.vscdb"),
        ("windsurf", "Windsurf/User/globalStorage/state.vscdb"),
        ("vscodium", "VSCodium/User/globalStorage/state.vscdb"),
        ("sublime", "Sublime Text/Local/Session.sublime_session"),
        ("obsidian", "obsidian/obsidian.json"),
    ];
    let catalog = SourceCatalog::bundled_with_config_directory(Some(temp.path().into())).unwrap();
    assert_eq!(catalog.descriptors().len(), cases.len());
    let mut apps = BTreeSet::new();
    for descriptor in catalog.descriptors() {
        for app in descriptor.applications {
            assert!(
                apps.insert(app),
                "one application must not get two history routes"
            );
        }
    }
    for (id, relative) in cases {
        assert_eq!(
            catalog.detect(id),
            None,
            "{id} must not guess another product's file"
        );
        let path = temp.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "metadata only; not a valid document").unwrap();
        assert_eq!(catalog.detect(id), Some(path.canonicalize().unwrap()));
    }
}

#[test]
fn jetbrains_variants_share_schema_parsing_and_preserve_solution_file_targets() {
    let temp = tempfile::tempdir().unwrap();
    let catalog = SourceCatalog::bundled_with_config_directory(None).unwrap();
    for id in [
        "pycharm",
        "webstorm",
        "goland",
        "clion",
        "phpstorm",
        "rubymine",
        "datagrip",
        "rider",
        "rustrover",
        "android_studio",
    ] {
        let path = temp.path().join(if id == "rider" {
            "项目.sln"
        } else {
            "项目"
        });
        let xml = temp.path().join("recent.xml");
        fs::write(&xml, format!("<application><component name=\"{id}RecentProjects\"><option name=\"additionalInfo\"><map><entry key=\"{}\"><value><RecentProjectMetaInfo><option name=\"displayName\" value=\"配置项目\"/><option name=\"projectOpenTimestamp\" value=\"1700000000000\"/></RecentProjectMetaInfo></value></entry></map></option></component></application>", path.display())).unwrap();
        let rows = read(&catalog, id, &xml);
        assert_eq!(rows.len(), 1, "{id}");
        assert_eq!(rows[0].title, "配置项目");
        assert_eq!(rows[0].last_used_at, Some(1700000000000));
        assert_eq!(
            rows[0].target,
            if id == "rider" {
                Target::File { path }
            } else {
                Target::Directory { path }
            }
        );
    }
    let xml = temp.path().join("ambiguous.xml");
    fs::write(&xml, "<application><component><option name='recentPaths'/></component><component><option name='recentPaths'/></component></application>").unwrap();
    assert!(catalog
        .read(
            &SourcePreference {
                id: "idea".into(),
                path: xml
            },
            Arc::new(|| false)
        )
        .is_err());
}

#[test]
fn code_family_reuses_sqlite_reader_and_keeps_source_order_and_types() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("state.vscdb");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE ItemTable(key TEXT PRIMARY KEY,value TEXT)")
        .unwrap();
    let file = temp.path().join("配置.txt");
    let uri = url::Url::from_file_path(&file).unwrap().to_string();
    db.execute("INSERT INTO ItemTable VALUES (?1,?2)", rusqlite::params!["history.recentlyOpenedPathsList",json!({"entries":[{"fileUri":uri,"label":"配置文件"},{"folderUri":"vscode-remote://ssh-remote+synthetic/project"}]}).to_string()]).unwrap();
    let catalog = SourceCatalog::bundled_with_config_directory(None).unwrap();
    for id in ["cursor", "windsurf", "vscodium", "vscode_insiders"] {
        let rows = read(&catalog, id, &path);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].target, Target::File { path: file.clone() });
        assert_eq!(rows[0].title, "配置文件");
        assert!(matches!(rows[1].target, Target::Remote { .. }));
    }
}

#[test]
fn sublime_reads_recorded_history_without_exporting_buffers_or_duplicates() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("项目");
    let file = temp.path().join("配置.md");
    let workspace = temp.path().join("项目.sublime-workspace");
    let source = temp.path().join("Session.sublime_session");
    fs::write(&source, json!({"folder_history":[folder,folder],"file_history":[file],"workspaces":{"recent_workspaces":[workspace]},"buffers":[{"contents":"must not appear in history"}]}).to_string()).unwrap();
    let catalog = SourceCatalog::bundled_with_config_directory(None).unwrap();
    let rows = read(&catalog, "sublime", &source);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].target, Target::Directory { path: folder });
    assert_eq!(rows[1].target, Target::File { path: file });
    assert_eq!(rows[2].target, Target::File { path: workspace });
    assert!(!serde_json::to_string(&rows)
        .unwrap()
        .contains("must not appear"));
}

#[test]
fn obsidian_preserves_distinct_vault_ids_and_rejects_unknown_or_unsafe_entries() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("obsidian.json");
    let document = json!({"vaults":{
        "0123456789abcdef":{"path":temp.path().join("first/笔记"),"ts":1700000000000_i64},
        "fedcba9876543210":{"path":temp.path().join("second/笔记"),"ts":1700000001000_i64}
    }});
    fs::write(&source, document.to_string()).unwrap();
    let catalog = SourceCatalog::bundled_with_config_directory(None).unwrap();
    let rows = read(&catalog, "obsidian", &source);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].title, rows[1].title);
    assert_ne!(rows[0].key, rows[1].key);
    assert!(
        matches!(&rows[0].target,Target::ObsidianVault {vault_id,..} if vault_id=="0123456789abcdef")
    );
    for bad in [
        json!({}),
        json!({"vaults":{"../bad?new=1":{"path":temp.path()}}}),
        json!({"vaults":{"0123456789abcdef":{"path":"relative"}}}),
    ] {
        fs::write(&source, bad.to_string()).unwrap();
        assert!(catalog
            .read(
                &SourcePreference {
                    id: "obsidian".into(),
                    path: source.clone()
                },
                Arc::new(|| false)
            )
            .is_err());
    }
}
