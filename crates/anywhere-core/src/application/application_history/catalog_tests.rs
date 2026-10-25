use super::*;
use crate::{
    features::application_history::service::HistoryService,
    persistence::module_storage::ModuleStorageHost,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn definition(id: &str) -> Value {
    json!({"schema":1,"id":id,"title":"Example Editor","applications":["example.editor"],"file_hint":"history.json","action":{"title":"Example History"},"reader":{"format":"json"}})
}
fn write(root: &Path, folder: &str, value: &Value, script: Option<&str>) {
    let path = root.join(folder);
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("source.json"), value.to_string()).unwrap();
    if let Some(script) = script {
        fs::write(path.join("transform.js"), script).unwrap();
    }
}
fn storage() -> Arc<dyn crate::core::storage::ExtensionStorage> {
    ModuleStorageHost::memory()
        .unwrap()
        .bind("application_history")
        .unwrap()
}

#[test]
fn user_sources_share_the_bundled_contract_and_are_captured_once() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("history.json");
    fs::write(&source, json!({"path":root.path()}).to_string()).unwrap();
    let script = "export function transform(input) { return [{key:input.path,title:'User adapter',target:{kind:'directory',path:input.path}}]; }";
    write(root.path(), "example", &definition("example"), Some(script));
    let catalog = SourceCatalog::with_directories(None, root.path()).unwrap();
    assert!(catalog.configuration_error().is_none());
    assert_eq!(
        catalog.descriptors().len(),
        BUNDLED_HISTORY_SOURCES.len() + 1
    );
    let service = HistoryService::new(Ok(storage()), catalog.clone());
    service.configure("example", source.to_str()).unwrap();
    assert_eq!(
        service.search(0, "", "example").unwrap().items[0]
            .entry
            .title,
        "User adapter"
    );
    fs::write(
        root.path().join("example/transform.js"),
        "throw Error('changed');",
    )
    .unwrap();
    assert_eq!(
        service.search(0, "", "example").unwrap().items.len(),
        1,
        "editing files must not hot-load code into this process"
    );
}

#[test]
fn users_can_reuse_a_bundled_parser_and_override_a_bundled_source() {
    let root = tempfile::tempdir().unwrap();
    let mut value = definition("example");
    value.as_object_mut().unwrap().remove("reader");
    value["extends"] = json!("idea");
    write(root.path(), "example", &value, None);
    let (manifest, _) = BUNDLED_HISTORY_SOURCES
        .iter()
        .find(|(m, _)| serde_json::from_str::<Value>(m).unwrap()["id"] == "idea")
        .unwrap();
    let mut original: Value = serde_json::from_str(manifest).unwrap();
    original["action"]["title"] = json!("My IDEA History");
    write(root.path(), "override", &original, None);
    let catalog = SourceCatalog::with_directories(None, root.path()).unwrap();
    assert!(catalog.configuration_error().is_none());
    let descriptors = catalog.descriptors();
    assert_eq!(descriptors.iter().filter(|s| s.id == "idea").count(), 1);
    assert_eq!(
        descriptors
            .iter()
            .find(|s| s.id == "idea")
            .unwrap()
            .action
            .title,
        "My IDEA History"
    );
    let path = root.path().join("recent.xml");
    fs::write(&path, format!("<application><component name=\"RecentProjectsManager\"><option name=\"recentPaths\"><list><option value=\"{}\"/></list></option></component></application>", root.path().display())).unwrap();
    assert_eq!(
        catalog
            .read(
                &SourcePreference {
                    id: "example".into(),
                    path
                },
                Arc::new(|| false)
            )
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn malformed_or_ambiguous_user_definitions_fail_closed_without_partial_overrides() {
    for kind in [
        "unknown-template",
        "duplicate",
        "missing-script",
        "wrong-identity",
        "future-schema",
        "oversize",
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut value = definition("example");
        let mut script = Some("export function transform() { return []; }");
        match kind {
            "unknown-template" => value["extends"] = json!("unknown"),
            "future-schema" => value["schema"] = json!(99),
            "missing-script" => script = None,
            "wrong-identity" => value["id"] = json!("idea"),
            _ => {}
        }
        write(root.path(), "first", &value, script);
        if kind == "duplicate" {
            write(root.path(), "second", &value, script);
        }
        if kind == "oversize" {
            fs::write(
                root.path().join("first/transform.js"),
                "x".repeat(64 * 1024 + 1),
            )
            .unwrap();
        }
        let catalog = SourceCatalog::with_directories(None, root.path()).unwrap();
        assert!(catalog.configuration_error().is_some(), "{kind}");
        assert_eq!(
            catalog.descriptors().len(),
            BUNDLED_HISTORY_SOURCES.len(),
            "invalid input must not partially replace the built-ins"
        );
        assert!(HistoryService::new(Ok(storage()), catalog)
            .refresh()
            .is_err());
    }
}

#[test]
fn action_switch_preserves_custom_paths_and_revokes_old_selections() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("history.json");
    fs::write(&path, json!({"path":root.path()}).to_string()).unwrap();
    write(root.path(), "example", &definition("example"), Some("export function transform(input) {return [{key:input.path,title:'Item',target:{kind:'directory',path:input.path}}];}"));
    let catalog = SourceCatalog::with_directories(None, root.path()).unwrap();
    let storage = storage();
    let service = HistoryService::new(Ok(storage.clone()), catalog.clone());
    service.configure("example", path.to_str()).unwrap();
    let id = service.search(0, "", "example").unwrap().items[0]
        .id
        .clone();
    service.set_action_enabled("example", false).unwrap();
    assert!(service.page_input("example").is_err());
    assert!(service
        .disabled_action_ids()
        .contains("core.application_history.example.open"));
    assert!(service
        .use_item(
            &id,
            true,
            &crate::application::source_catalog_tests::QuietPlatform
        )
        .is_err());
    let restarted = HistoryService::new(Ok(storage), catalog);
    restarted.refresh().unwrap();
    let status = restarted
        .sources()
        .into_iter()
        .find(|s| s.descriptor.id == "example")
        .unwrap();
    assert!(!status.action_enabled);
    assert_eq!(status.path, Some(path.canonicalize().unwrap()));
    restarted.set_action_enabled("example", true).unwrap();
    assert!(restarted.disabled_action_ids().is_empty());
    assert_eq!(restarted.search(0, "", "example").unwrap().items.len(), 1);
}

#[test]
fn removing_a_user_adapter_preserves_inactive_preferences_without_disabling_builtins() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("history.json");
    fs::write(&source, "{}").unwrap();
    write(
        root.path(),
        "example",
        &definition("example"),
        Some("export function transform(){return [];}"),
    );
    let storage = storage();
    let first = HistoryService::new(
        Ok(storage.clone()),
        SourceCatalog::with_directories(None, root.path()).unwrap(),
    );
    first.configure("example", source.to_str()).unwrap();
    fs::remove_dir_all(root.path().join("example")).unwrap();
    let restarted = HistoryService::new(
        Ok(storage.clone()),
        SourceCatalog::with_directories(None, root.path()).unwrap(),
    );
    restarted.refresh().unwrap();
    assert_eq!(restarted.sources().len(), BUNDLED_HISTORY_SOURCES.len());
    assert!(String::from_utf8(storage.get("sources").unwrap().unwrap())
        .unwrap()
        .contains("example"));
    assert!(restarted.page_input("example").is_err());
    assert!(restarted.page_input("idea").is_ok());
}

#[test]
fn legacy_disabled_builtins_do_not_implicitly_disable_new_user_adapters() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "example",
        &definition("example"),
        Some("export function transform(){return [];}"),
    );
    let storage = storage();
    storage
        .put("sources", br#"{"schema":1,"sources":[]}"#)
        .unwrap();
    let service = HistoryService::new(
        Ok(storage),
        SourceCatalog::with_directories(None, root.path()).unwrap(),
    );
    service.refresh().unwrap();
    let statuses = service.sources();
    assert!(
        statuses
            .iter()
            .find(|s| s.descriptor.id == "cursor")
            .unwrap()
            .action_enabled,
        "new bundled sources were unknown to schema 1 and must not inherit its omissions"
    );
    assert!(
        !statuses
            .iter()
            .find(|s| s.descriptor.id == "idea")
            .unwrap()
            .action_enabled
    );
    assert!(
        statuses
            .iter()
            .find(|s| s.descriptor.id == "example")
            .unwrap()
            .action_enabled
    );
}
