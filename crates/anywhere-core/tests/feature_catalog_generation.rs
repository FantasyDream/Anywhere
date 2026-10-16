#[path = "../build_support/feature_catalog.rs"]
mod feature_catalog;

use std::{fs, path::Path, process::Command};

fn registration(root: &Path, name: &str) {
    let directory = root.join("src/features").join(name);
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("registration.rs"),
        format!(
            "pub const REGISTRATIONS: &[crate::BuiltinFeatureRegistration] = &[crate::BuiltinFeatureRegistration {{ id: {name:?} }}];"
        ),
    )
    .unwrap();
}

#[test]
fn discovery_tracks_additions_removals_and_compiles_without_central_edits() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("workspace with spaces 项目");
    registration(&root, "zeta");
    registration(&root, "alpha");
    fs::write(root.join("src/features/business.rs"), "").unwrap();
    fs::create_dir(root.join("src/features/unregistered")).unwrap();
    let original = feature_catalog::discover(&root).unwrap();
    assert_eq!(
        original,
        [
            "src/features/alpha/registration.rs",
            "src/features/zeta/registration.rs"
        ]
    );
    registration(&root, "middle");
    let added = feature_catalog::discover(&root).unwrap();
    assert_eq!(added.len(), 3);
    let generated = feature_catalog::render(&added);
    let source = root.join("main.rs");
    fs::write(
        &source,
        format!(
            "pub struct BuiltinFeatureRegistration {{ pub id: &'static str }}\n{generated}\nfn main() {{ assert_eq!(BUILTIN_FEATURE_GROUPS.iter().flat_map(|g|g.iter().map(|e|e.id)).collect::<Vec<_>>(), [\"alpha\", \"middle\", \"zeta\"]); }}"
        ),
    )
    .unwrap();
    let executable = root.join(if cfg!(windows) {
        "catalog.exe"
    } else {
        "catalog"
    });
    let compiled = Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .env("CARGO_MANIFEST_DIR", &root)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    assert!(Command::new(executable).status().unwrap().success());
    fs::remove_file(root.join("src/features/middle/registration.rs")).unwrap();
    assert_eq!(feature_catalog::discover(&root).unwrap(), original);
}

#[test]
fn check_refuses_missing_or_stale_snapshot_without_modifying_it() {
    let temporary = tempfile::tempdir().unwrap();
    let snapshot = temporary.path().join("catalog.rs");
    assert!(feature_catalog::write_or_check(&snapshot, "new", true).is_err());
    assert!(!snapshot.exists());
    fs::write(&snapshot, "old").unwrap();
    assert!(feature_catalog::write_or_check(&snapshot, "new", true).is_err());
    assert_eq!(fs::read_to_string(&snapshot).unwrap(), "old");
    feature_catalog::write_or_check(&snapshot, "new", false).unwrap();
    feature_catalog::write_or_check(&snapshot, "new", true).unwrap();
}

#[test]
fn check_accepts_windows_line_endings_but_rejects_stale_content_without_writing() {
    let temporary = tempfile::tempdir().unwrap();
    let snapshot = temporary.path().join("catalog.rs");
    let expected = "first\nsecond\n";
    let checkout = "first\r\nsecond\r\n";
    fs::write(&snapshot, checkout).unwrap();
    feature_catalog::write_or_check(&snapshot, expected, true).unwrap();
    assert_eq!(fs::read_to_string(&snapshot).unwrap(), checkout);
    assert!(feature_catalog::write_or_check(&snapshot, "first\nchanged\n", true).is_err());
    assert_eq!(fs::read_to_string(&snapshot).unwrap(), checkout);
}

#[test]
fn discovery_rejects_missing_empty_and_malformed_registrations() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    assert!(feature_catalog::discover(root).is_err());
    fs::create_dir_all(root.join("src/features")).unwrap();
    assert!(feature_catalog::discover(root).is_err());
    registration(root, "Bad-name");
    assert!(feature_catalog::discover(root).is_err());
    fs::remove_dir_all(root.join("src/features/Bad-name")).unwrap();
    fs::create_dir_all(root.join("src/features/valid/registration.rs")).unwrap();
    assert!(feature_catalog::discover(root).is_err());
}

#[cfg(unix)]
#[test]
fn discovery_rejects_symlinked_modules_and_registration_files() {
    use std::os::unix::fs::symlink;
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    registration(root, "valid");
    symlink(
        root.join("src/features/valid"),
        root.join("src/features/alias"),
    )
    .unwrap();
    assert!(feature_catalog::discover(root).is_err());
    fs::remove_file(root.join("src/features/alias")).unwrap();
    fs::create_dir(root.join("src/features/linked")).unwrap();
    symlink(
        root.join("src/features/valid/registration.rs"),
        root.join("src/features/linked/registration.rs"),
    )
    .unwrap();
    assert!(feature_catalog::discover(root).is_err());
}
