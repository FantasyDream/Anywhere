use super::*;
use crate::{
    features::application_history::{model::Notice, service::HistoryService},
    persistence::module_storage::ModuleStorageHost,
};
use serde_json::json;
use std::path::Path;

fn storage() -> Arc<dyn crate::core::storage::ExtensionStorage> {
    ModuleStorageHost::memory()
        .unwrap()
        .bind("application_history")
        .unwrap()
}
fn idea(path: &Path) -> String {
    format!(
        r#"<application><component name="RecentProjectsManager"><option name="additionalInfo"><map><entry key="{}"><value><RecentProjectMetaInfo><option name="displayName" value="配置验收项目"/><option name="projectOpenTimestamp" value="1700000000000"/></RecentProjectMetaInfo></value></entry></map></option></component></application>"#,
        path.display()
    )
}
#[test]
fn official_idea_xml_to_searchable_history_is_configured_not_special_cased() {
    let temp = tempfile::tempdir().unwrap();
    let project = temp.path().join("项目 with spaces");
    std::fs::create_dir(&project).unwrap();
    let source = temp.path().join("recentProjects.xml");
    std::fs::write(&source, idea(&project)).unwrap();
    let service = HistoryService::new(Ok(storage()), OfficialSources::bundled().unwrap());
    service.refresh().unwrap();
    assert_eq!(
        service.search(0, "", "").unwrap().notice,
        Some(Notice::Unconfigured)
    );
    service
        .configure("idea", Some(source.to_str().unwrap()))
        .unwrap();
    let first = service.search(0, "pzys", "").unwrap();
    assert_eq!(first.items.len(), 1);
    assert_eq!(
        first.items[0].entry.target,
        Target::Directory { path: project }
    );
    assert_eq!(first.items[0].entry.last_used_at, Some(1700000000000));
    assert!(service
        .search(0, "does not match", "")
        .unwrap()
        .items
        .is_empty());
    service.configure("idea", None).unwrap();
    assert!(service.search(0, "", "").unwrap().items.is_empty());
}

#[test]
fn vscode_uses_the_same_sqlite_reader_and_preserves_workspace_and_remote_types() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("state.vscdb");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch(
        "PRAGMA journal_mode=WAL; CREATE TABLE ItemTable(key TEXT PRIMARY KEY, value BLOB)",
    )
    .unwrap();
    let workspace = temp.path().join("多根.code-workspace");
    let value = json!({"entries":[
        {"workspace":{"id":"workspace","configPath":url::Url::from_file_path(&workspace).unwrap().as_str()}},
        {"folderUri":"vscode-remote://ssh-remote+example/project"}
    ]}).to_string();
    db.execute(
        "INSERT INTO ItemTable VALUES (?1,?2)",
        rusqlite::params!["history.recentlyOpenedPathsList", value],
    )
    .unwrap();
    let loader = OfficialSources::bundled().unwrap();
    let entries = loader
        .read(
            &SourcePreference {
                id: "vscode".into(),
                path: path.clone(),
            },
            Arc::new(|| false),
        )
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].target, Target::Workspace { path: workspace });
    assert!(matches!(entries[1].target, Target::Remote { .. }));
    assert_eq!(
        db.query_row("SELECT value FROM ItemTable", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        value
    );
    assert_eq!(
        db.query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "wal"
    );
}

#[test]
fn scripts_cannot_access_io_and_invalid_outputs_are_rejected() {
    let input = json!({});
    assert!(script::transform("export function transform() { if (typeof fetch !== 'undefined' || typeof require !== 'undefined' || typeof process !== 'undefined') throw Error(); return []; }", &input, Arc::new(|| false)).is_ok());
    for source in [
        "export function transform() { return [{key:'x',title:'x',target:{kind:'directory',path:'relative'}}]; }",
        "export function transform() { return [{key:'x',title:'x',source_id:'spoof',target:{kind:'directory',path:'/tmp'}}]; }",
        "export function transform() { const x={key:'x',title:'x',target:{kind:'directory',path:'/tmp'}}; return [x,x]; }",
        "export function transform() { return Promise.resolve([]); }",
    ] { assert!(script::transform(source, &input, Arc::new(|| false)).is_err()); }
    assert!(script::transform(
        "export function transform(){return [];}",
        &input,
        Arc::new(|| true)
    )
    .is_err());
}

#[test]
fn runaway_scripts_are_interrupted_and_the_next_runtime_still_works() {
    let started = Instant::now();
    assert!(script::transform(
        "export function transform() { while(true) {} }",
        &json!({}),
        Arc::new(|| false)
    )
    .is_err());
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(script::transform(
        "export function transform() { return Array(100000000).fill('too large'); }",
        &json!({}),
        Arc::new(|| false)
    )
    .is_err());
    assert!(script::transform(
        "export function transform() { return []; }",
        &json!({}),
        Arc::new(|| false)
    )
    .is_ok());
}

#[test]
fn generic_readers_reject_writes_external_tables_unknown_xml_and_oversize_files() {
    use crate::persistence::application_history::{read, Reader};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("db");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE permitted(value TEXT); CREATE TABLE secret(value TEXT); INSERT INTO permitted VALUES ('kept')").unwrap();
    for query in [
        "DELETE FROM permitted",
        "SELECT * FROM secret",
        "ATTACH DATABASE '/tmp/no-write' AS another",
    ] {
        let reader = Reader::Sqlite {
            query: query.into(),
            parameters: vec![],
            tables: vec!["permitted".into()],
        };
        assert!(read(&path, &reader, Arc::new(|| false)).is_err());
    }
    assert_eq!(
        db.query_row("SELECT value FROM permitted", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "kept"
    );
    let document = temp.path().join("json");
    std::fs::write(&document, r#"{"projects":[]}"#).unwrap();
    assert_eq!(
        read(&document, &Reader::Json, Arc::new(|| false)).unwrap(),
        json!({"projects":[]})
    );
    std::fs::OpenOptions::new()
        .write(true)
        .open(&document)
        .unwrap()
        .set_len((MAX_INPUT + 1) as u64)
        .unwrap();
    assert!(read(&document, &Reader::Json, Arc::new(|| false)).is_err());
    let xml = temp.path().join("empty.xml");
    std::fs::write(&xml, "<unrelated />").unwrap();
    assert!(OfficialSources::bundled()
        .unwrap()
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
fn source_preferences_persist_without_mirroring_history_and_disable_invalidates_selection() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("recent.xml");
    std::fs::write(&path, idea(temp.path())).unwrap();
    let storage = storage();
    let loader = OfficialSources::bundled().unwrap();
    let first = HistoryService::new(Ok(storage.clone()), loader.clone());
    first.refresh().unwrap();
    first
        .configure("idea", Some(path.to_str().unwrap()))
        .unwrap();
    let second = HistoryService::new(Ok(storage.clone()), loader);
    second.refresh().unwrap();
    assert_eq!(second.search(0, "", "idea").unwrap().items.len(), 1);
    let saved = String::from_utf8(storage.get("sources").unwrap().unwrap()).unwrap();
    assert!(!saved.contains("配置验收项目") && !saved.contains("1700000000000"));
    second.set_enabled(false);
    assert_eq!(
        second.search(0, "", "").unwrap().notice,
        Some(Notice::Disabled)
    );
    second.set_enabled(true);
    assert_eq!(second.search(0, "", "").unwrap().items.len(), 1);
}

#[test]
fn corrupted_or_future_preferences_are_preserved_on_save() {
    let storage = storage();
    let original = br#"{"schema":99,"sources":[]}"#;
    storage.put("sources", original).unwrap();
    let service = HistoryService::new(Ok(storage.clone()), OfficialSources::bundled().unwrap());
    assert!(service.refresh().is_err());
    assert!(service.configure("idea", None).is_err());
    assert_eq!(storage.get("sources").unwrap().unwrap(), original);
}

#[test]
fn delayed_old_page_cancellation_cannot_invalidate_a_new_page() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("recent.xml");
    std::fs::write(&path, idea(temp.path())).unwrap();
    let service = HistoryService::new(Ok(storage()), OfficialSources::bundled().unwrap());
    service
        .configure("idea", Some(path.to_str().unwrap()))
        .unwrap();
    let old = service.open_session().unwrap();
    let old_id = service.search(old, "", "").unwrap().items[0].id.clone();
    let current = service.open_session().unwrap();
    let new_id = service.search(current, "", "").unwrap().items[0].id.clone();
    service.cancel_session(old);
    assert_eq!(
        service.search(old, "", "").unwrap().notice,
        Some(Notice::Stale)
    );
    let platform = crate::application::source_catalog_tests::QuietPlatform;
    assert!(service.use_item(&old_id, true, &platform).is_err());
    assert!(service.use_item(&new_id, true, &platform).is_ok());
    std::fs::write(&path, "<unrelated />").unwrap();
    assert!(service.use_item(&new_id, true, &platform).is_err());
}

#[test]
fn cancellation_during_source_work_rejects_publication_and_retains_admission() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Barrier,
    };
    struct Gated {
        first: AtomicBool,
        entered: Barrier,
        release: Barrier,
        inner: Arc<OfficialSources>,
    }
    impl SourceLoader for Gated {
        fn descriptors(&self) -> Vec<SourceDescriptor> {
            self.inner.descriptors()
        }
        fn read(&self, source: &SourcePreference, cancelled: Cancelled) -> Result<Vec<Entry>> {
            if self.first.swap(false, Ordering::AcqRel) {
                self.entered.wait();
                self.release.wait();
            }
            self.inner.read(source, cancelled)
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("recent.xml");
    std::fs::write(&path, idea(temp.path())).unwrap();
    let loader = Arc::new(Gated {
        first: AtomicBool::new(true),
        entered: Barrier::new(2),
        release: Barrier::new(2),
        inner: OfficialSources::bundled().unwrap(),
    });
    let service = Arc::new(HistoryService::new(Ok(storage()), loader.clone()));
    service
        .configure("idea", Some(path.to_str().unwrap()))
        .unwrap();
    let owner = service.open_session().unwrap();
    let running = service.clone();
    let worker = std::thread::spawn(move || running.search(owner, "", "").unwrap());
    loader.entered.wait();
    service.cancel_session(owner);
    assert_eq!(
        service.search(owner, "new", "").unwrap().notice,
        Some(Notice::Busy)
    );
    loader.release.wait();
    assert_eq!(worker.join().unwrap().notice, Some(Notice::Stale));
    assert_eq!(service.search(owner, "", "").unwrap().items.len(), 1);
}

#[test]
fn a_json_source_only_needs_a_manifest_and_a_transform() {
    let reader = crate::persistence::application_history::Reader::Json;
    let loader = OfficialSources { packages: vec![Package {
        manifest: Manifest { schema:1, id:"example".into(), title:"Example".into(), file_hint:"recent.json".into(), applications:vec!["example.editor".into()], reader },
        script: "export function transform(input) { return input.projects.map(p => ({key:p.path,title:p.name,target:{kind:'directory',path:p.path}})); }",
    }] };
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("recent.json");
    std::fs::write(
        &path,
        json!({"projects":[{"name":"Example","path":temp.path()}]}).to_string(),
    )
    .unwrap();
    let entries = loader
        .read(
            &SourcePreference {
                id: "example".into(),
                path,
            },
            Arc::new(|| false),
        )
        .unwrap();
    assert_eq!(entries[0].title, "Example");
}
