//! Known transient locations, without excluding arbitrary project/cache names.
use std::path::{Path, PathBuf};

pub struct TemporaryDirectories {
    temporary_roots: Vec<PathBuf>,
    updater_cache: Option<PathBuf>,
    search_roots: Vec<PathBuf>,
}
impl TemporaryDirectories {
    pub fn for_search(search_roots: &[PathBuf]) -> Self {
        let temporary_roots = vec![std::env::temp_dir()];
        #[cfg(target_os = "macos")]
        let temporary_roots = {
            let mut roots = temporary_roots;
            roots.extend([
                PathBuf::from("/private/tmp"),
                PathBuf::from("/private/var/tmp"),
            ]);
            roots
        };
        let updater_cache = if cfg!(target_os = "macos") {
            crate::paths::home_directory().map(|home| home.join("Library/Caches"))
        } else {
            None
        };
        Self::with_locations(search_roots, temporary_roots, updater_cache)
    }

    /// Host path facts are resolved once per query; entries require no extra IO.
    pub fn with_locations(
        search_roots: &[PathBuf],
        temporary_roots: Vec<PathBuf>,
        updater_cache: Option<PathBuf>,
    ) -> Self {
        let normalize = |path: &Path| {
            path.canonicalize()
                .unwrap_or_else(|_| crate::search_storage::lexical_scope_path(path))
        };
        Self {
            temporary_roots: temporary_roots
                .iter()
                .map(|p| normalize(p))
                .filter(|p| p.is_absolute() && p.parent().is_some())
                .collect(),
            updater_cache: updater_cache.as_deref().map(normalize),
            search_roots: search_roots.iter().map(|p| normalize(p)).collect(),
        }
    }

    pub fn excludes(&self, path: &Path, is_directory: bool) -> bool {
        let temporary_root = self
            .temporary_roots
            .iter()
            .find(|root| path.starts_with(root));
        let updater_root = self.updater_cache.as_ref().and_then(|cache| {
            let mut parts = path.strip_prefix(cache).ok()?.components();
            let name = parts.next()?;
            if !is_directory && parts.next().is_none() {
                return None;
            }
            name.as_os_str()
                .to_str()?
                .ends_with(".ShipIt")
                .then(|| cache.join(name))
        });
        let Some(root) = temporary_root.or(updater_root.as_ref()) else {
            return false;
        };
        // Selecting a root inside this location, including an explicit path
        // query, is an intentional override. Existing authorization still applies.
        !self
            .search_roots
            .iter()
            .any(|selected| selected.starts_with(root) && path.starts_with(selected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_known_transient_locations_are_excluded() {
        let fixture = tempfile::tempdir().unwrap();
        let home = fixture.path().canonicalize().unwrap();
        let temp = home.join("system-temp");
        let caches = home.join("Library/Caches");
        let policy = TemporaryDirectories::with_locations(
            std::slice::from_ref(&home),
            vec![temp.clone()],
            Some(caches.clone()),
        );
        for path in [
            temp.join("report.txt"),
            caches.join("example.ShipIt/update.123/report.txt"),
        ] {
            assert!(policy.excludes(&path, false));
        }
        for path in [
            home.join("project/tmp/report.txt"),
            home.join("project/temp/report.txt"),
            home.join("node_modules/report.txt"),
            home.join(".m2/report.txt"),
            home.join(".cache/report.txt"),
            caches.join("ordinary/report.txt"),
            home.join("project/example.ShipIt/report.txt"),
        ] {
            assert!(!policy.excludes(&path, false), "{}", path.display());
        }
        assert!(!policy.excludes(&caches.join("report.ShipIt"), false));
        assert!(policy.excludes(&caches.join("example.ShipIt"), true));
    }
    #[test]
    fn explicit_roots_inside_transient_locations_remain_searchable() {
        let fixture = tempfile::tempdir().unwrap();
        let home = fixture.path().canonicalize().unwrap();
        let temp = home.join("system-temp");
        let selected = temp.join("project");
        let policy = TemporaryDirectories::with_locations(
            &[home.clone(), selected.clone()],
            vec![temp.clone()],
            None,
        );
        assert!(!policy.excludes(&selected.join("report.txt"), false));
        assert!(policy.excludes(&temp.join("other/report.txt"), false));
        let caches = home.join("Library/Caches");
        let selected = caches.join("example.ShipIt/update.123");
        let policy = TemporaryDirectories::with_locations(
            std::slice::from_ref(&selected),
            vec![],
            Some(caches),
        );
        assert!(!policy.excludes(&selected.join("report.txt"), false));
    }
}
