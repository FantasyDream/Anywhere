//! Installed-application index. Deliberately outside the context graph.
//!
//! V0.2 builds the index once per process, in the background, and never blocks
//! the first snapshot on it. No filesystem watcher and no persisted index.
use crate::candidates::IndexedApplication;

/// Local alias so tests can name the crate the way callers do.
#[cfg(test)]
use crate as anywhere_core_alias;
use anywhere_platform::Platform;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Root order also decides which duplicate installation wins.
pub fn standard_roots(home: Option<&Path>) -> Vec<PathBuf> {
    if cfg!(target_os = "windows") {
        return windows_roots();
    }
    let mut roots = Vec::new();
    if let Some(home) = home {
        roots.push(home.join("Applications"));
    }
    roots.push(PathBuf::from("/Applications"));
    roots.push(PathBuf::from("/System/Applications"));
    roots
}

/// Start Menu locations, per-user first.
///
/// These are where the shell lists installed applications, and `%ProgramData%` covers the
/// machine-wide installs. Roots that do not exist are skipped by the walk.
#[cfg(target_os = "windows")]
fn windows_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        roots.push(
            PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    if let Some(program_data) = std::env::var_os("ProgramData") {
        roots.push(
            PathBuf::from(program_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    roots
}

/// Windows builds without the Windows target have no Start Menu to read.
#[cfg(not(target_os = "windows"))]
fn windows_roots() -> Vec<PathBuf> {
    Vec::new()
}

/// Depth is bounded so a pathological tree cannot stall the scan.
const MAX_DEPTH: usize = 3;
const MAX_APPS: usize = 4000;

/// Extensions that mark one entry as an application.
///
/// Both are recognized on every platform, deliberately: the decision is name-based, so it
/// can be tested where it is written, and the platform call that reads the entry is what
/// decides whether the name means anything there (`bundle_application` returns `None` for
/// a `.lnk` on macOS and for anything that is not a bundle there).
fn is_application_path(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("app") || extension.eq_ignore_ascii_case("lnk")
    })
}

/// Find `.app` bundles without descending into them.
pub fn discover_bundles(roots: &[PathBuf]) -> Vec<PathBuf> {
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
    // Deterministic order keeps duplicate resolution reproducible.
    paths.sort();
    for path in paths {
        // An application entry may be a file, not a directory: on Windows every
        // application is a `.lnk`, so testing for a directory first skipped all of them
        // and the index came back empty on the one platform that needed it.
        if is_application_path(&path) {
            bundles.push(path);
        } else if path.is_dir() {
            walk(&path, depth + 1, bundles);
        }
    }
}

/// Collapse duplicate installations of the same bundle id.
///
/// Earlier roots win, then shorter and lexicographically smaller paths, so the
/// choice is stable across scans. Distinct bundle ids stay distinct even when
/// they share a display name.
pub fn deduplicate(mut applications: Vec<IndexedApplication>) -> Vec<IndexedApplication> {
    applications.sort_by(|a, b| {
        a.bundle_id
            .cmp(&b.bundle_id)
            .then_with(|| a.path.len().cmp(&b.path.len()))
            .then_with(|| a.path.cmp(&b.path))
    });
    let mut best: BTreeMap<String, IndexedApplication> = BTreeMap::new();
    for application in applications {
        best.entry(application.bundle_id.clone())
            .or_insert(application);
    }
    let mut result = best.into_values().collect::<Vec<_>>();
    result.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.bundle_id.cmp(&b.bundle_id))
    });
    result
}

/// Enumerate and identify every installed application. Best-effort by design: an
/// unreadable bundle is skipped instead of failing the whole index.
/// The user's preferred languages, in order, for localized name lookup.
///
/// Only these are read: loading every localization an app ships would add thousands of
/// names to the index, while a user searches in the languages they actually read.
pub fn preferred_languages(platform: &dyn Platform) -> Vec<String> {
    let from_system = platform.preferred_languages();
    if !from_system.is_empty() {
        return from_system;
    }
    // Fallback for hosts that do not report it: the environment, then English.
    let raw = std::env::var("AppleLanguages").unwrap_or_default();
    let parsed = raw
        .split(['\n', ',', '(', ')', '"'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if parsed.is_empty() {
        vec!["en".to_owned()]
    } else {
        parsed
    }
}

pub fn scan(platform: &dyn Platform, roots: &[PathBuf]) -> Vec<IndexedApplication> {
    let languages = preferred_languages(platform);
    scan_in_languages(platform, roots, &languages)
}

/// Same as [`scan`], with explicit languages so the behaviour is testable.
pub fn scan_in_languages(
    platform: &dyn Platform,
    roots: &[PathBuf],
    languages: &[String],
) -> Vec<IndexedApplication> {
    let mut applications = Vec::new();
    let mut unreadable = 0usize;
    for bundle in discover_bundles(roots) {
        match platform.bundle_application(&bundle) {
            Ok(Some(application)) => {
                let mut localized = platform
                    .bundle_localized_names(&bundle, languages)
                    .unwrap_or_default();
                // The name the user sees in the Dock is the localized one; the
                // canonical `CFBundleName` is often an unrelated codename (Lark for
                // 飞书). Show the system's name, and keep the canonical one searchable
                // by moving it into the aliases.
                let canonical = application.name;
                let display = if localized.is_empty() {
                    canonical.clone()
                } else {
                    let first = localized.remove(0);
                    if first.to_lowercase() != canonical.to_lowercase() {
                        localized.push(canonical.clone());
                    }
                    first
                };
                // Names the platform knows but did not choose as the title — on Windows, a
                // shortcut's description. They are searchable only: a description is not a
                // name, so it must not become the title even when it is the only one.
                localized.extend(application.aliases);
                applications.push(IndexedApplication {
                    bundle_id: application.bundle_id,
                    name: display,
                    path: application
                        .path
                        .unwrap_or_else(|| bundle.to_string_lossy().into_owned()),
                    aliases: localized,
                    keywords: Vec::new(),
                });
            }
            Ok(None) => unreadable += 1,
            Err(error) => {
                unreadable += 1;
                tracing::debug!(bundle = %bundle.display(), error = %error, "app_index_skip");
            }
        }
    }
    if unreadable > 0 {
        tracing::info!(unreadable, "app_index_incomplete");
    }
    let mut applications = deduplicate(applications);
    for application in &mut applications {
        // `aliases` holds visible names collected during the scan (localized names).
        // Only de-duplicate here: identifiers must never be added, or `app.hiddify.com`
        // contributes the word `com` and a two-letter query matches dozens of unrelated
        // apps. Identifiers are matched through `SearchFields::with_metadata` instead,
        // which only matches them as a whole.
        let canonical = application.name.to_lowercase();
        application
            .aliases
            .retain(|alias| !alias.is_empty() && *alias != canonical);
        // The case is kept for display, so equal names can differ in case; compare folded.
        let mut seen = std::collections::HashSet::new();
        application
            .aliases
            .retain(|alias| seen.insert(alias.to_lowercase()));
    }
    applications
}

/// Background index handle shared with the application state.
pub struct ApplicationCatalog {
    applications: Vec<IndexedApplication>,
}

impl ApplicationCatalog {
    pub fn empty() -> Self {
        Self {
            applications: Vec::new(),
        }
    }
    pub fn new(applications: Vec<IndexedApplication>) -> Self {
        Self { applications }
    }
    pub fn applications(&self) -> &[IndexedApplication] {
        &self.applications
    }
    pub fn is_empty(&self) -> bool {
        self.applications.is_empty()
    }
}

/// One-shot background scan. The caller stores the catalog when this resolves.
pub async fn scan_async(platform: Arc<dyn Platform>, roots: Vec<PathBuf>) -> ApplicationCatalog {
    let started = std::time::Instant::now();
    let applications = tokio::task::spawn_blocking(move || scan(platform.as_ref(), &roots))
        .await
        .unwrap_or_default();
    tracing::info!(
        duration_us = started.elapsed().as_micros() as u64,
        count = applications.len(),
        "app_index_ready"
    );
    ApplicationCatalog::new(applications)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anywhere_platform::ApplicationInfo;
    use std::sync::Mutex;
    #[derive(Default)]
    struct FakePlatform {
        missing: bool,
        /// Stands in for data read out of `<lang>.lproj/InfoPlist.strings`.
        localized: Vec<String>,
        /// Names the platform offers that are searchable but not the title — on Windows,
        /// a shortcut's description.
        platform_aliases: Vec<String>,
    }
    impl Platform for FakePlatform {
        fn frontmost_application(&self) -> anyhow::Result<Option<ApplicationInfo>> {
            Ok(None)
        }
        fn application_directory(&self, _: &ApplicationInfo) -> anyhow::Result<Option<PathBuf>> {
            Ok(None)
        }
        fn bundle_application(
            &self,
            bundle: &Path,
        ) -> anyhow::Result<Option<anywhere_platform::BundleApplication>> {
            if self.missing {
                return Ok(None);
            }
            let stem = bundle
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Shaped like a real reversed-DNS id, including the `com` segment that
            // caused the reported leak.
            let bundle_id = format!("app.{}.com", stem.replace(' ', "").to_lowercase());
            Ok(Some(anywhere_platform::BundleApplication {
                bundle_id,
                name: stem,
                aliases: self.platform_aliases.clone(),
                path: Some(bundle.to_string_lossy().into_owned()),
                icon: None,
            }))
        }
        fn preferred_languages(&self) -> Vec<String> {
            if self.localized.is_empty() {
                Vec::new()
            } else {
                vec!["zh-Hans-CN".into()]
            }
        }
        fn bundle_localized_names(
            &self,
            _: &Path,
            languages: &[String],
        ) -> anyhow::Result<Vec<String>> {
            Ok(if languages.is_empty() {
                Vec::new()
            } else {
                self.localized.clone()
            })
        }
        fn launch_application(&self, _: &str) -> anyhow::Result<bool> {
            Ok(true)
        }
        fn application_icon(&self, _: &str, _: u32) -> anyhow::Result<Option<Vec<u8>>> {
            Ok(None)
        }
        fn clipboard_text(&self) -> anyhow::Result<Option<String>> {
            Ok(None)
        }
        fn git_available(&self) -> bool {
            false
        }
        fn run_git(
            &self,
            _: &Path,
            _: anywhere_platform::GitOperation,
        ) -> anyhow::Result<anywhere_platform::GitOutput> {
            anyhow::bail!("git is not supported")
        }
        fn open_url(&self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn open_file(&self, _: &Path) -> anyhow::Result<()> {
            Ok(())
        }
        fn reveal_file(&self, _: &Path) -> anyhow::Result<()> {
            Ok(())
        }
        fn open_directory(&self, _: &Path) -> anyhow::Result<()> {
            Ok(())
        }
        fn open_terminal(&self, _: &Path) -> anyhow::Result<()> {
            Ok(())
        }
        fn copy_text(&self, _: &str) -> anyhow::Result<()> {
            Ok(())
        }
    }

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
        // A nested bundle inside an app must not be enumerated separately.
        std::fs::create_dir_all(root.path().join("Alpha.app/Contents/Helpers/Nested.app")).unwrap();
        // Utilities-style grouping is still found.
        std::fs::create_dir_all(root.path().join("Utilities")).unwrap();
        make_apps(&root.path().join("Utilities"), &["Gamma"]);
        let roots = vec![root.path().to_path_buf()];
        let bundles = discover_bundles(&roots);
        let names = bundles
            .iter()
            .map(|bundle| bundle.file_stem().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Alpha", "Beta", "Gamma"]);
        assert!(!names.iter().any(|name| name == "Nested"));
    }

    /// Entries are recognized before they are descended into.
    ///
    /// The walk used to skip anything that was not a directory first. Every Windows
    /// application is a `.lnk` *file*, so the whole index came back empty there while the
    /// platform reader that understands shortcuts was never even called.
    #[test]
    fn discovery_finds_application_entries_that_are_files() {
        let root = tempfile::tempdir().unwrap();
        let programs = root.path().join("Start Menu").join("Programs");
        std::fs::create_dir_all(&programs).unwrap();
        std::fs::write(programs.join("Google Chrome.lnk"), b"stub").unwrap();
        std::fs::create_dir_all(programs.join("Company")).unwrap();
        std::fs::write(programs.join("Company").join("Tool.LNK"), b"stub").unwrap();
        // A plain file must not be treated as an application.
        std::fs::write(programs.join("notes.txt"), b"stub").unwrap();
        make_apps(&programs, &["Mac Only"]);

        let mut names = discover_bundles(&[root.path().to_path_buf()])
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

    #[test]
    fn duplicate_bundle_ids_prefer_the_first_root_and_canonical_order() {
        let applications = vec![
            IndexedApplication {
                bundle_id: "com.example.tool".into(),
                name: "Tool".into(),
                path: "/Applications/Tool.app".into(),
                ..Default::default()
            },
            IndexedApplication {
                bundle_id: "com.example.tool".into(),
                name: "Tool".into(),
                path: "/Users/x/Applications/Tool.app".into(),
                ..Default::default()
            },
            IndexedApplication {
                bundle_id: "com.example.other".into(),
                name: "Tool".into(),
                path: "/Applications/Tool Other.app".into(),
                ..Default::default()
            },
        ];
        let result = deduplicate(applications);
        assert_eq!(
            result.len(),
            2,
            "same bundle id collapses, same name does not"
        );
        let tool = result
            .iter()
            .find(|app| app.bundle_id == "com.example.tool")
            .unwrap();
        assert_eq!(tool.path, "/Applications/Tool.app");
    }

    #[test]
    fn unreadable_bundles_are_skipped_without_failing_the_scan() {
        let root = tempfile::tempdir().unwrap();
        make_apps(root.path(), &["Alpha"]);
        let platform = FakePlatform::default();
        let applications = scan(&platform, &[root.path().to_path_buf()]);
        assert_eq!(applications.len(), 1);
        assert_eq!(applications[0].name, "Alpha");
        assert!(
            applications[0].aliases.is_empty(),
            "the index must not invent aliases from the path or the bundle id: {:?}",
            applications[0].aliases
        );

        let platform = FakePlatform {
            missing: true,
            ..FakePlatform::default()
        };
        assert!(scan(&platform, &[root.path().to_path_buf()]).is_empty());
    }

    /// Reported: an app whose display name is Chinese could not be found by that name.
    ///
    /// macOS keeps localized names in `<lang>.lproj/InfoPlist.strings`, which the
    /// canonical `Info.plist` does not contain, so the index only ever held the English
    /// name ("Lark" for 飞书, "Safari" for Safari浏览器).
    #[test]
    fn localized_names_are_searchable_and_are_not_overwritten() {
        let root = tempfile::tempdir().unwrap();
        make_apps(root.path(), &["Lark"]);
        let platform = FakePlatform {
            localized: vec!["飞书".into()],
            ..FakePlatform::default()
        };
        let languages = vec!["zh-Hans-CN".to_owned()];
        let applications = scan_in_languages(&platform, &[root.path().to_path_buf()], &languages);
        assert_eq!(applications.len(), 1);
        // The system's name is what the user sees in the Dock, so it becomes the
        // display name; the canonical one stays searchable as an alias.
        assert_eq!(applications[0].name, "飞书");
        assert_eq!(applications[0].aliases, vec!["Lark".to_owned()]);
        let fields = crate::query::SearchFields::new(
            &applications[0].name,
            &applications[0].aliases,
            &applications[0].keywords,
        )
        .with_metadata(&[
            crate::candidates::searchable_bundle_id(&applications[0].bundle_id).as_str(),
        ]);
        assert!(fields
            .matched(&crate::query::Query::parse("飞书"))
            .is_some());
        assert!(fields
            .matched(&crate::query::Query::parse("Lark"))
            .is_some());

        // No languages configured: keep the canonical name and invent no aliases.
        let none = scan_in_languages(&platform, &[root.path().to_path_buf()], &[]);
        assert_eq!(none[0].name, "Lark");
        assert!(none[0].aliases.is_empty());
    }

    /// A platform's extra names are searchable but never replace the title.
    ///
    /// Windows reports a shortcut's file name as the display name and its free-text
    /// description separately: `Google Chrome.lnk` describes itself as "Access the
    /// Internet". Using the description as the title would lose the name the user reads and
    /// types, so it is kept as an alias — searchable, and shown as nothing.
    #[test]
    fn platform_aliases_are_searchable_and_never_replace_the_title() {
        let root = tempfile::tempdir().unwrap();
        make_apps(root.path(), &["Google Chrome"]);
        let platform = FakePlatform {
            platform_aliases: vec!["Access the Internet".into()],
            ..FakePlatform::default()
        };
        let applications = scan_in_languages(&platform, &[root.path().to_path_buf()], &[]);
        assert_eq!(applications.len(), 1);
        assert_eq!(applications[0].name, "Google Chrome");
        assert_eq!(
            applications[0].aliases,
            vec!["Access the Internet".to_owned()]
        );

        let fields = crate::query::SearchFields::new(
            &applications[0].name,
            &applications[0].aliases,
            &applications[0].keywords,
        );
        for query in ["chrome", "Google", "internet"] {
            assert!(
                fields.matched(&crate::query::Query::parse(query)).is_some(),
                "{query} must find the application"
            );
        }
    }

    /// A bundle id must never become a searchable alias.
    ///
    /// Reported: typing `co` listed apps whose visible name has no "co". The index had
    /// put `app.hiddify.com` into `aliases`, and `com` matched as a word start.
    #[test]
    fn bundle_ids_are_not_searchable_aliases() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("Hiddify.app/Contents")).unwrap();
        let platform = FakePlatform::default();
        let applications = scan(&platform, &[root.path().to_path_buf()]);
        assert_eq!(applications.len(), 1);
        let application = &applications[0];
        assert!(
            application.bundle_id.to_lowercase().contains("com"),
            "the fixture must reproduce the reported id: {}",
            application.bundle_id
        );
        for alias in &application.aliases {
            assert!(
                !alias.contains(&application.bundle_id.to_lowercase()),
                "the bundle id leaked into a searchable alias: {alias}"
            );
        }
        // And the matcher agrees: a "com" fragment is not a match.
        let fields = crate::query::SearchFields::new(
            &application.name,
            &application.aliases,
            &application.keywords,
        )
        .with_metadata(&[crate::candidates::searchable_bundle_id(&application.bundle_id).as_str()]);
        assert_eq!(
            fields.matched(&crate::query::Query::parse("co")),
            None,
            "name={:?} aliases={:?} metadata={:?}",
            application.name,
            application.aliases,
            fields.metadata()
        );
    }

    /// The index must carry everything the spec's application index lists.
    #[test]
    fn indexed_application_exposes_the_spec_fields() {
        let root = tempfile::tempdir().unwrap();
        make_apps(root.path(), &["Alpha"]);
        let platform = FakePlatform::default();
        let applications = scan(&platform, &[root.path().to_path_buf()]);
        let application = &applications[0];
        assert_eq!(application.name, "Alpha");
        assert_eq!(application.bundle_id, "app.alpha.com");
        assert!(application.path.ends_with("Alpha.app"));
        // Icon is addressed by the bundle path and resolved lazily through Platform.
        assert_eq!(application.icon_source(), application.path);
        // Usage is addressed by the same id the launch path records under.
        assert_eq!(
            application.usage_id(),
            anywhere_core_alias::history::application_usage_id("app.alpha.com")
        );
    }

    #[test]
    fn catalog_handle_starts_empty() {
        let catalog: Mutex<ApplicationCatalog> = Mutex::new(ApplicationCatalog::empty());
        assert!(catalog.lock().unwrap().is_empty());
    }
}
