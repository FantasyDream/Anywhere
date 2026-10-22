//! Directory-based application discovery.
//!
//! Default strategy for [`crate::Platform::installed_applications`]: walk known roots
//! and identify each entry through [`crate::Platform::bundle_application`]. A later
//! indexer replaces `installed_applications` and does not have to use this walk.
use crate::{InstalledApplications, Platform};
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 3;
const MAX_APPS: usize = 4000;

pub(crate) fn from_roots<P: Platform + ?Sized>(platform: &P) -> InstalledApplications {
    let mut applications = Vec::new();
    let mut skipped = 0usize;
    for path in discover(&platform.application_roots(), &|path| {
        platform.is_application_entry(path)
    }) {
        match platform.bundle_application(&path) {
            Ok(Some(application)) => applications.push(application),
            Ok(None) => skipped += 1,
            Err(error) => {
                skipped += 1;
                tracing::debug!(bundle = %path.display(), error = %error, "app_index_skip");
            }
        }
    }
    if skipped > 0 {
        tracing::info!(skipped, "app_index_incomplete");
    }
    InstalledApplications {
        applications,
        skipped,
    }
}

pub(crate) fn discover(roots: &[PathBuf], is_application: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut bundles = Vec::new();
    for root in roots {
        walk(root, 0, &mut bundles, is_application);
        if bundles.len() >= MAX_APPS {
            break;
        }
    }
    bundles.truncate(MAX_APPS);
    bundles
}

fn walk(
    directory: &Path,
    depth: usize,
    bundles: &mut Vec<PathBuf>,
    is_application: &dyn Fn(&Path) -> bool,
) {
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
        if bundles.len() >= MAX_APPS {
            break;
        }
        if is_application(&path) {
            bundles.push(path);
        } else if path.is_dir() {
            walk(&path, depth + 1, bundles, is_application);
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
        let bundles = discover(&[root.path().to_path_buf()], &|path| {
            path.extension().is_some_and(|e| e == "app")
        });
        let names = bundles
            .iter()
            .map(|bundle| bundle.file_stem().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Alpha", "Beta", "Gamma"]);
        assert!(!names.iter().any(|name| name == "Nested"));
    }

    #[test]
    fn discovery_stops_identifying_entries_at_the_catalog_limit() {
        use std::cell::Cell;
        let root = tempfile::tempdir().unwrap();
        for index in 0..MAX_APPS + 3 {
            std::fs::File::create(root.path().join(format!("{index:05}.app"))).unwrap();
        }
        let identified = Cell::new(0);
        let bundles = discover(&[root.path().to_path_buf()], &|_| {
            identified.set(identified.get() + 1);
            true
        });
        assert_eq!(bundles.len(), MAX_APPS);
        assert_eq!(identified.get(), MAX_APPS);
    }
}
