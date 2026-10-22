//! Installed-application catalog. Deliberately outside the context graph.
//!
//! Discovery belongs to [`anywhere_platform::Platform::installed_applications`]. This
//! module turns that list into searchable rows (aliases, dedup) and holds the
//! in-memory snapshot. The first snapshot never waits for a scan.
use anywhere_platform::{BundleApplication, Platform};
use std::collections::BTreeMap;
use std::sync::Arc;

/// An installed application available for direct launch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndexedApplication {
    pub bundle_id: String,
    pub name: String,
    pub localized_names: BTreeMap<String, String>,
    /// Canonical bundle path: both the launch target and the icon source.
    pub path: String,
    pub aliases: Vec<String>,
    pub keywords: Vec<String>,
    pub search: crate::core::search::query::SearchFields,
}

impl IndexedApplication {
    /// Icon source for this application.
    ///
    /// The bytes are fetched lazily for visible rows only; holding them all in the index
    /// would cost far more memory than the index itself.
    pub fn icon_source(&self) -> &str {
        &self.path
    }

    /// Usage row this application's history is recorded and read under.
    ///
    /// Derived from `bundle_id` so recording and ranking can never drift apart.
    pub fn usage_id(&self) -> String {
        crate::core::ranking::usage::application_usage_id(&self.bundle_id)
    }

    pub fn prepared(mut self) -> Self {
        for name in self.localized_names.values() {
            push_unique(&mut self.aliases, name.clone(), &self.name);
        }
        let identifier =
            crate::features::applications::source::searchable_bundle_id(&self.bundle_id);
        self.search = crate::core::search::query::SearchFields::new(
            &self.name,
            &self.aliases,
            &self.keywords,
        )
        .with_metadata(&[identifier.as_str()]);
        self
    }
}

/// Collapse duplicate installations of the same bundle id.
///
/// Shorter paths win, then lexicographically smaller paths, so the
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

fn push_unique(aliases: &mut Vec<String>, value: String, canonical: &str) {
    if value.is_empty() {
        return;
    }
    if value.eq_ignore_ascii_case(canonical) {
        return;
    }
    if aliases
        .iter()
        .any(|alias| alias.eq_ignore_ascii_case(&value))
    {
        return;
    }
    aliases.push(value);
}

/// Turn platform entries into catalog rows. Display names are kept as given.
pub fn index_from(entries: Vec<BundleApplication>) -> Vec<IndexedApplication> {
    let mut applications = Vec::new();
    for entry in entries {
        let name = entry.name;
        let path = entry.path.unwrap_or_default();
        if name.is_empty() || path.is_empty() {
            continue;
        }
        let mut aliases = Vec::new();
        for alias in entry.aliases {
            push_unique(&mut aliases, alias, &name);
        }
        applications.push(
            IndexedApplication {
                bundle_id: entry.bundle_id,
                name,
                localized_names: entry.localized_names,
                path,
                aliases,
                keywords: Vec::new(),
                search: crate::core::search::query::SearchFields::default(),
            }
            .prepared(),
        );
    }
    deduplicate(applications)
}

/// A finished catalog walk. `skipped == 0` and an empty list is an empty machine.
/// `skipped > 0` is a partial index: the applications that could be read are still usable.
pub struct ScanReport {
    pub applications: Vec<IndexedApplication>,
    pub skipped: usize,
}

pub fn scan(platform: &dyn Platform) -> ScanReport {
    let found = platform.installed_applications();
    let input = found.applications.len();
    let applications = index_from(found.applications);
    ScanReport {
        skipped: found.skipped + input.saturating_sub(applications.len()),
        applications,
    }
}

/// The scan task finished, or it failed and the previous index must stay.
pub enum ScanStatus {
    Complete(ScanReport),
    Failed,
}

/// Background scan. A failed task is not an empty catalog.
pub async fn scan_async(platform: Arc<dyn Platform>) -> ScanStatus {
    let started = std::time::Instant::now();
    match tokio::task::spawn_blocking(move || scan(platform.as_ref())).await {
        Ok(report) => {
            tracing::info!(
                duration_us = started.elapsed().as_micros() as u64,
                count = report.applications.len(),
                skipped = report.skipped,
                "app_index_ready"
            );
            ScanStatus::Complete(report)
        }
        Err(error) => {
            tracing::error!(error = %error, "app_index_failed");
            ScanStatus::Failed
        }
    }
}

/// Background index handle shared with the application state.
#[derive(Clone)]
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

    pub fn excluding(&self, excluded: &std::collections::BTreeSet<String>) -> Self {
        if excluded.is_empty() {
            return self.clone();
        }
        Self {
            applications: self
                .applications
                .iter()
                .filter(|application| !excluded.contains(&application.bundle_id))
                .cloned()
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anywhere_platform::{ApplicationInfo, BundleApplication, GitOperation, GitOutput};
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    #[cfg(test)]
    use crate as anywhere_core_alias;

    #[derive(Default)]
    struct FakePlatform {
        missing: bool,
        display: Option<String>,
        aliases: Vec<String>,
        roots: Vec<PathBuf>,
    }
    impl Platform for FakePlatform {
        fn is_application_entry(&self, path: &Path) -> bool {
            path.extension().is_some_and(|extension| extension == "app")
        }
        fn application_roots(&self) -> Vec<PathBuf> {
            self.roots.clone()
        }
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
            let bundle_id = format!("app.{}.com", stem.replace(' ', "").to_lowercase());
            Ok(Some(anywhere_platform::BundleApplication {
                localized_names: Default::default(),
                bundle_id,
                name: self.display.clone().unwrap_or(stem),
                aliases: self.aliases.clone(),
                path: Some(bundle.to_string_lossy().into_owned()),
                icon: None,
            }))
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
        fn run_git(&self, _: &Path, _: GitOperation) -> anyhow::Result<GitOutput> {
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

    fn make_apps(root: &Path, names: &[&str]) {
        for name in names {
            std::fs::create_dir_all(root.join(format!("{name}.app/Contents"))).unwrap();
        }
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
        let platform = FakePlatform {
            roots: vec![root.path().to_path_buf()],
            ..FakePlatform::default()
        };
        let applications = scan(&platform).applications;
        assert_eq!(applications.len(), 1);
        assert_eq!(applications[0].name, "Alpha");
        assert!(
            applications[0].aliases.is_empty(),
            "the index must not invent aliases from the path or the bundle id: {:?}",
            applications[0].aliases
        );

        let platform = FakePlatform {
            missing: true,
            roots: vec![root.path().to_path_buf()],
            ..FakePlatform::default()
        };
        let missing = scan(&platform);
        assert!(missing.applications.is_empty());
        assert!(missing.skipped > 0, "unreadable bundles are a partial scan");
    }

    #[test]
    fn display_name_is_kept_and_platform_aliases_remain_searchable() {
        let applications = index_from(vec![BundleApplication {
            localized_names: Default::default(),
            bundle_id: "com.lark.suite".into(),
            name: "飞书".into(),
            aliases: vec!["Lark".into()],
            path: Some("/Applications/Lark.app".into()),
            icon: None,
        }]);
        assert_eq!(applications.len(), 1);
        assert_eq!(applications[0].name, "飞书");
        assert!(
            applications[0].aliases.iter().any(|alias| alias == "Lark"),
            "{:?}",
            applications[0].aliases
        );
        let fields = crate::core::search::query::SearchFields::new(
            &applications[0].name,
            &applications[0].aliases,
            &applications[0].keywords,
        )
        .with_metadata(
            &[crate::features::applications::source::searchable_bundle_id(
                &applications[0].bundle_id,
            )
            .as_str()],
        );
        for query in ["飞书", "Lark"] {
            assert!(
                fields
                    .matched(&crate::core::search::query::Query::parse(query))
                    .is_some(),
                "{query} must find the application"
            );
        }
    }

    #[test]
    fn localized_application_names_match_chinese_english_and_pinyin_without_changing_identity() {
        use crate::core::search::{
            candidate::CandidateTarget,
            candidate_source::{CandidateSource, SearchContext},
            query::Query,
        };
        use crate::features::applications::source::ApplicationSource;

        for (english, chinese, full, initials) in [
            ("Reminders", "提醒事项", "tixingshixiang", "txsx"),
            (
                "Activity Monitor",
                "活动监视器",
                "huodongjianshiqi",
                "hdjsq",
            ),
        ] {
            let make = |name: &str| {
                index_from(vec![BundleApplication {
                    bundle_id: format!("test.{full}"),
                    name: name.into(),
                    localized_names: BTreeMap::from([
                        ("en".into(), english.into()),
                        ("zh".into(), chinese.into()),
                    ]),
                    aliases: Vec::new(),
                    path: Some(format!("/System/Applications/{english}.app")),
                    icon: None,
                }])
            };
            let english_catalog = make(english);
            let chinese_catalog = make(chinese);
            assert_eq!(english_catalog[0].usage_id(), chinese_catalog[0].usage_id());
            let usage = |_: &str, _: &str| Default::default();
            for input in [english, chinese, full, initials] {
                let query = Query::parse(input);
                let search = SearchContext {
                    query: &query,
                    contexts: &[],
                    scope_id: None,
                    ranking: Default::default(),
                };
                let rows = |applications: &[IndexedApplication]| {
                    ApplicationSource {
                        applications,
                        app_usage: &usage,
                        now: 0,
                    }
                    .candidates(&search)
                };
                let en = rows(&english_catalog);
                let zh = rows(&chinese_catalog);
                assert_eq!(en.len(), 1, "{input}");
                assert_eq!(zh.len(), 1, "{input}");
                assert_eq!(en[0].presentation.id, zh[0].presentation.id);
                assert_eq!(en[0].presentation.bundle_id, zh[0].presentation.bundle_id);
                assert_eq!(en[0].presentation.localized_titles["zh"], chinese);
                assert_eq!(en[0].presentation.localized_titles["en"], english);
                for row in [en, zh] {
                    let CandidateTarget::LaunchApplication { path, icon_source } = &row[0].target
                    else {
                        panic!("application candidate must retain its launch target");
                    };
                    assert_eq!(path, &english_catalog[0].path);
                    assert_eq!(icon_source, path);
                }
            }
            for input in ["邮件", "nomatch"] {
                assert!(english_catalog[0]
                    .search
                    .matched(&Query::parse(input))
                    .is_none());
            }
        }
    }

    #[test]
    fn platform_aliases_are_searchable_and_never_replace_the_title() {
        let applications = index_from(vec![BundleApplication {
            localized_names: Default::default(),
            bundle_id: "com.google.Chrome".into(),
            name: "Google Chrome".into(),
            aliases: vec!["Access the Internet".into(), "谷歌浏览器".into()],
            path: Some("/Applications/Google Chrome.app".into()),
            icon: None,
        }]);
        assert_eq!(applications.len(), 1);
        assert_eq!(applications[0].name, "Google Chrome");
        assert!(
            applications[0]
                .aliases
                .iter()
                .any(|alias| alias == "Access the Internet"),
            "{:?}",
            applications[0].aliases
        );
        assert!(
            applications[0]
                .aliases
                .iter()
                .any(|alias| alias == "谷歌浏览器"),
            "{:?}",
            applications[0].aliases
        );

        let fields = crate::core::search::query::SearchFields::new(
            &applications[0].name,
            &applications[0].aliases,
            &applications[0].keywords,
        );
        for query in ["chrome", "Google", "internet"] {
            assert!(
                fields
                    .matched(&crate::core::search::query::Query::parse(query))
                    .is_some(),
                "{query} must find the application"
            );
        }
    }

    #[test]
    fn bundle_ids_are_not_searchable_aliases() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("Hiddify.app/Contents")).unwrap();
        let platform = FakePlatform {
            roots: vec![root.path().to_path_buf()],
            ..FakePlatform::default()
        };
        let applications = scan(&platform).applications;
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
        let fields =
            crate::core::search::query::SearchFields::new(
                &application.name,
                &application.aliases,
                &application.keywords,
            )
            .with_metadata(&[
                crate::features::applications::source::searchable_bundle_id(&application.bundle_id)
                    .as_str(),
            ]);
        assert_eq!(
            fields.matched(&crate::core::search::query::Query::parse("co")),
            None,
            "name={:?} aliases={:?} metadata={:?}",
            application.name,
            application.aliases,
            fields.metadata()
        );
    }

    #[test]
    fn indexed_application_exposes_the_spec_fields() {
        let applications = index_from(vec![BundleApplication {
            localized_names: Default::default(),
            bundle_id: "app.alpha.com".into(),
            name: "Alpha".into(),
            aliases: Vec::new(),
            path: Some("/Applications/Alpha.app".into()),
            icon: None,
        }]);
        let application = &applications[0];
        assert_eq!(application.name, "Alpha");
        assert_eq!(application.bundle_id, "app.alpha.com");
        assert!(application.path.ends_with("Alpha.app"));
        assert_eq!(application.icon_source(), application.path);
        assert_eq!(
            application.usage_id(),
            anywhere_core_alias::core::ranking::usage::application_usage_id("app.alpha.com")
        );
    }

    #[test]
    fn catalog_handle_starts_empty() {
        let catalog: Mutex<ApplicationCatalog> = Mutex::new(ApplicationCatalog::empty());
        assert!(catalog.lock().unwrap().is_empty());
    }
}
