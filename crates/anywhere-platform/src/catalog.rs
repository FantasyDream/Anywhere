//! Directory-based application discovery.
//!
//! Default strategy for [`crate::Platform::installed_applications`]: walk known roots
//! and identify each entry through [`crate::Platform::bundle_application`]. A later
//! indexer replaces `installed_applications` and does not have to use this walk.
use crate::{BundleApplication, Platform};
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 3;
const MAX_APPS: usize = 4000;

pub(crate) fn from_roots<P: Platform + ?Sized>(platform: &P) -> Vec<BundleApplication> {
    let mut applications = Vec::new();
    let mut unreadable = 0usize;
    for path in discover(&platform.application_roots()) {
        match platform.bundle_application(&path) {
            Ok(Some(application)) => applications.push(application),
            Ok(None) => unreadable += 1,
            Err(error) => {
                unreadable += 1;
                tracing::debug!(bundle = %path.display(), error = %error, "app_index_skip");
            }
        }
    }
    if unreadable > 0 {
        tracing::info!(unreadable, "app_index_incomplete");
    }
    applications
}

pub(crate) fn discover(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut bundles = Vec::new();
    for root in roots {
        walk(root, 0, &mut bundles);
        if bundles.len() >= MAX_APPS {
            break;
        }
    }
    bundles.truncate(MAX_APPS);
    bundles
}

fn is_application_path(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("app") || extension.eq_ignore_ascii_case("lnk")
    })
}

fn walk(directory: &Path, depth: usize, bundles: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || bundles.len() >= MAX_APPS {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut paths = Vec::new();
    for entry in entries.flatten() {
        paths.push(entry.path());
    }
    paths.sort();
    for path in paths {
        if is_application_path(&path) {
            bundles.push(path);
        } else if path.is_dir() {
            walk(&path, depth + 1, bundles);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_apps(root: &Path, names: &[&str]) -> Vec<PathBuf> {
        let mut created = Vec::new();
        for name in names {
            let bundle = root.join(format!("{name}.app"));
            std::fs::create_dir_all(bundle.join("Contents")).unwrap();
            created.push(bundle);
        }
        created
    }

    #[test]
    fn discovery_is_bounded_and_never_descends_into_bundles() {
        let root = tempfile::tempdir().unwrap();
        make_apps(root.path(), &["Alpha", "Beta"]);
        std::fs::create_dir_all(root.path().join("Alpha.app/Contents/Helpers/Nested.app")).unwrap();
        std::fs::create_dir_all(root.path().join("Utilities")).unwrap();
        make_apps(&root.path().join("Utilities"), &["Gamma"]);
        let bundles = discover(&[root.path().to_path_buf()]);
        let names = bundles
            .iter()
            .map(|bundle| bundle.file_stem().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Alpha", "Beta", "Gamma"]);
        assert!(!names.iter().any(|name| name == "Nested"));
    }

    #[test]
    fn discovery_finds_application_entries_that_are_files() {
        let root = tempfile::tempdir().unwrap();
        let programs = root.path().join("Start Menu").join("Programs");
        std::fs::create_dir_all(&programs).unwrap();
        std::fs::write(programs.join("Google Chrome.lnk"), b"stub").unwrap();
        std::fs::create_dir_all(programs.join("Company")).unwrap();
        std::fs::write(programs.join("Company").join("Tool.LNK"), b"stub").unwrap();
        std::fs::write(programs.join("notes.txt"), b"stub").unwrap();
        make_apps(&programs, &["Mac Only"]);

        let mut names = discover(&[root.path().to_path_buf()])
            .iter()
            .map(|entry| entry.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            ["Google Chrome.lnk", "Mac Only.app", "Tool.LNK"],
            "shortcut files are applications; other files are not"
        );
    }
}
