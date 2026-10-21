use super::{tests::*, *};
use crate::application::web_plugins::WebPackageStore;
use anywhere_protocol::web_plugin::WebPlatform;
use std::fs;
fn archive(package: &anywhere_plugin::web::VerifiedWebPackage, path: &std::path::Path) {
    package
        .write_archive(fs::File::create(path).unwrap())
        .unwrap();
}
#[test]
fn import_approves_exact_snapshot_persists_and_serves_only_verified_assets() {
    let root = tempfile::tempdir().unwrap();
    let app = app();
    let file = root.path().join("source.awplugin");
    let package = package(serde_json::json!([]), serde_json::json!([]), "<p>old</p>");
    archive(&package, &file);
    let location = root.path().join("managed");
    let mut store = WebPackageStore::open(location.clone(), WebPlatform::Macos, &app).unwrap();
    let review = store.inspect(&file).unwrap();
    fs::write(&file, b"changed after review").unwrap();
    assert!(store.approve(&app, "wrong token").is_err());
    store.approve(&app, &review.token).unwrap();
    assert!(store.approve(&app, &review.token).is_err());
    assert_eq!(
        store.resource(package.digest(), "index.html").unwrap(),
        b"<p>old</p>"
    );
    assert!(store
        .resource(package.digest(), "../receipts.json")
        .is_err());
    drop(store);
    let mut store = WebPackageStore::open(location.clone(), WebPlatform::Macos, &app).unwrap();
    assert_eq!(store.statuses().len(), 1);
    fs::write(
        location.join(package.digest()).join("assets/index.html"),
        b"tampered",
    )
    .unwrap();
    assert!(store.resource(package.digest(), "index.html").is_err());
    store.set_enabled(&app, &review.id, false).unwrap();
    assert!(store.resource(package.digest(), "index.html").is_err());
    store.uninstall(&app, &review.id).unwrap();
    assert!(store.statuses().is_empty());
    assert!(!location.join(package.digest()).exists());
}

#[test]
fn a_failed_new_inspection_releases_the_previous_review() {
    let root = tempfile::tempdir().unwrap();
    let app = app();
    let mut store =
        WebPackageStore::open(root.path().join("store"), WebPlatform::Macos, &app).unwrap();
    let path = root.path().join("import.awplugin");
    archive(
        &package(serde_json::json!([]), serde_json::json!([]), "body"),
        &path,
    );
    let review = store.inspect(&path).unwrap();
    fs::write(&path, b"not a package").unwrap();
    assert!(store.inspect(&path).is_err());
    assert!(store.approve(&app, &review.token).is_err());
    assert!(store.statuses().is_empty());
}
#[test]
fn updates_require_review_and_rollback_restores_previously_approved_bytes() {
    let root = tempfile::tempdir().unwrap();
    let app = app();
    let path = root.path().join("import.awplugin");
    let mut store =
        WebPackageStore::open(root.path().join("store"), WebPlatform::Macos, &app).unwrap();
    let old = package(serde_json::json!([]), serde_json::json!([]), "old");
    archive(&old, &path);
    let review = store.inspect(&path).unwrap();
    store.approve(&app, &review.token).unwrap();
    let next = package(
        serde_json::json!([{"name":"clipboard.writeText"}]),
        serde_json::json!([]),
        "new",
    );
    archive(&next, &path);
    let review = store.inspect(&path).unwrap();
    assert!(review.updating);
    assert_eq!(store.resource(old.digest(), "index.html").unwrap(), b"old");
    store.approve(&app, &review.token).unwrap();
    assert!(store.resource(old.digest(), "index.html").is_err());
    assert_eq!(store.resource(next.digest(), "index.html").unwrap(), b"new");
    store.rollback(&app, &review.id).unwrap();
    assert_eq!(store.resource(old.digest(), "index.html").unwrap(), b"old");
    assert!(store.resource(next.digest(), "index.html").is_err());
}
#[test]
fn persistence_failure_restores_existing_catalog_and_permissions() {
    let root = tempfile::tempdir().unwrap();
    let app = app();
    let location = root.path().join("store");
    let mut store = WebPackageStore::open(location.clone(), WebPlatform::Macos, &app).unwrap();
    let path = root.path().join("import.awplugin");
    let package = package(serde_json::json!([]), serde_json::json!([]), "old");
    archive(&package, &path);
    let review = store.inspect(&path).unwrap();
    store.approve(&app, &review.token).unwrap();
    fs::remove_file(location.join("receipts.json")).unwrap();
    fs::create_dir(location.join("receipts.json")).unwrap();
    assert!(store.set_enabled(&app, &review.id, false).is_err());
    assert!(store.statuses()[0].enabled);
    assert_eq!(
        store.resource(package.digest(), "index.html").unwrap(),
        b"old"
    );
}

#[tokio::test]
async fn failed_package_save_does_not_revoke_an_open_document() {
    let root = tempfile::tempdir().unwrap();
    let app = app();
    let location = root.path().join("store");
    let mut store = WebPackageStore::open(location.clone(), WebPlatform::Macos, &app).unwrap();
    let path = root.path().join("import.awplugin");
    let package = package(
        serde_json::json!([]),
        serde_json::json!(["core.text"]),
        "body",
    );
    archive(&package, &path);
    let review = store.inspect(&path).unwrap();
    store.approve(&app, &review.token).unwrap();
    app.begin_activation().unwrap();
    let invocation = {
        let mut web = app.web_plugins.lock().unwrap();
        let registry = web.catalog.append_to(&Default::default()).unwrap();
        start(&mut web, &reference(&registry), &context("input"))
    };
    let binding = app
        .bind_web_document(&invocation, WebPlatform::Macos)
        .unwrap();
    app.validate_web_document(&binding, source(&binding))
        .unwrap();
    fs::remove_file(location.join("receipts.json")).unwrap();
    fs::create_dir(location.join("receipts.json")).unwrap();
    assert!(store.set_enabled(&app, &review.id, false).is_err());
    app.validate_web_document(&binding, source(&binding))
        .unwrap();
    assert_eq!(
        store.resource(package.digest(), "index.html").unwrap(),
        b"body"
    );
    app.close_web_document(&binding).unwrap();
    assert!(app
        .validate_web_document(&binding, source(&binding))
        .is_err());
}
#[cfg(test)]
#[cfg(unix)]
#[test]
fn substituted_resource_symlinks_are_rejected() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let app = app();
    let location = root.path().join("store");
    let path = root.path().join("import.awplugin");
    let package = package(serde_json::json!([]), serde_json::json!([]), "same bytes");
    archive(&package, &path);
    let mut store = WebPackageStore::open(location.clone(), WebPlatform::Macos, &app).unwrap();
    let review = store.inspect(&path).unwrap();
    store.approve(&app, &review.token).unwrap();
    let resource = location.join(package.digest()).join("assets/index.html");
    let outside = root.path().join("outside");
    fs::write(&outside, b"same bytes").unwrap();
    fs::remove_file(&resource).unwrap();
    symlink(&outside, &resource).unwrap();
    assert!(store.resource(package.digest(), "index.html").is_err());
}
#[tokio::test(start_paused = true)]
async fn retained_document_preserves_origin_but_invalidates_old_epoch() {
    let package = package(
        serde_json::json!([]),
        serde_json::json!(["core.text"]),
        "body",
    );
    let mut service = WebPluginService::default();
    service.replace_catalog(catalog(&package));
    let registry = service.catalog.append_to(&Default::default()).unwrap();
    let command = reference(&registry);
    let invocation = start(&mut service, &command, &context("text"));
    let old = service.bind(&invocation, WebPlatform::Macos).unwrap();
    service.suspend();
    let next = service.resume(&old, WebPlatform::Macos).unwrap();
    assert_eq!(old.origin(), next.origin());
    assert_ne!(old.runtime_id(), next.runtime_id());
    assert!(service.invocation(&old, source(&old)).is_err());
    assert!(service.invocation(&next, source(&next)).is_ok());
    service.suspend();
    tokio::time::advance(std::time::Duration::from_secs(15)).await;
    assert!(service.resume(&next, WebPlatform::Macos).is_err());
}

#[tokio::test]
async fn unchanged_enable_preserves_document_and_execution_catalog() {
    let root = tempfile::tempdir().unwrap();
    let app = app();
    let mut store =
        WebPackageStore::open(root.path().join("store"), WebPlatform::Macos, &app).unwrap();
    let path = root.path().join("import.awplugin");
    let package = package(
        serde_json::json!([]),
        serde_json::json!(["core.text"]),
        "body",
    );
    archive(&package, &path);
    let review = store.inspect(&path).unwrap();
    store.approve(&app, &review.token).unwrap();
    app.begin_activation().unwrap();
    let invocation = {
        let mut web = app.web_plugins.lock().unwrap();
        let registry = web.catalog.append_to(&Default::default()).unwrap();
        start(&mut web, &reference(&registry), &context("input"))
    };
    let binding = app
        .bind_web_document(&invocation, WebPlatform::Macos)
        .unwrap();
    store.set_enabled(&app, &review.id, true).unwrap();
    app.validate_web_document(&binding, source(&binding))
        .unwrap();
    assert!(store.set_enabled(&app, "missing", true).is_err());
}

#[test]
fn reinstall_cannot_silently_reuse_damaged_extracted_assets() {
    let root = tempfile::tempdir().unwrap();
    let app = app();
    let location = root.path().join("store");
    let mut store = WebPackageStore::open(location.clone(), WebPlatform::Macos, &app).unwrap();
    let path = root.path().join("import.awplugin");
    let package = package(serde_json::json!([]), serde_json::json!([]), "body");
    archive(&package, &path);
    let review = store.inspect(&path).unwrap();
    store.approve(&app, &review.token).unwrap();
    fs::write(
        location.join(package.digest()).join("assets/index.html"),
        b"broken",
    )
    .unwrap();
    let review = store.inspect(&path).unwrap();
    assert!(store.approve(&app, &review.token).is_err());
    assert!(store.resource(package.digest(), "index.html").is_err());
    // Normal uninstall/reinstall removes the damaged cache and restores verified bytes.
    store.uninstall(&app, &review.id).unwrap();
    let review = store.inspect(&path).unwrap();
    store.approve(&app, &review.token).unwrap();
    assert_eq!(
        store.resource(package.digest(), "index.html").unwrap(),
        b"body"
    );
}
