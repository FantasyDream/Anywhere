mod applications;
mod clipboard;
mod context;
mod everything;
mod file_search;
mod menu;
mod shell;
mod windows_search;

pub(crate) use file_search::{
    DEFAULT_ENGINE as DEFAULT_FILE_SEARCH_ENGINE,
    SUPPORTED_ENGINES as SUPPORTED_FILE_SEARCH_ENGINES,
};

use crate::{
    ApplicationCatalog, ApplicationHost, ApplicationInfo, BundleApplication, ClipboardHost,
    ClipboardSnapshot, ContextReader, FileIndexResults, FileSearchHost, FileSearchIndex, GitHost,
    InstalledApplications, MenuDiscovery, MenuHost, MenuLocator, ResourceOpener,
};
use anyhow::{ensure, Result};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

#[derive(Default)]
pub(super) struct WindowsPlatform {
    clipboard_tracker: OnceLock<Arc<clipboard::Tracker>>,
}

impl ApplicationCatalog for WindowsPlatform {
    fn application_roots(&self) -> Vec<PathBuf> {
        applications::start_menu_roots()
    }

    fn is_application_entry(&self, path: &Path) -> bool {
        path.is_file() && applications::is_launch_file(path)
    }

    fn bundle_application(&self, path: &Path) -> Result<Option<BundleApplication>> {
        Ok(self
            .is_application_entry(path)
            .then(|| applications::file_entry(path))
            .flatten())
    }

    fn installed_applications(&self) -> InstalledApplications {
        self.installed_applications_with_paths(&[])
    }

    fn installed_applications_with_paths(&self, paths: &[PathBuf]) -> InstalledApplications {
        applications::discover(&self.application_roots(), paths)
    }
}

impl ApplicationHost for WindowsPlatform {
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        Ok(shell::frontmost_application())
    }

    fn launch_application(&self, path: &str) -> Result<bool> {
        if let Some(id) = applications::packaged_target(path) {
            applications::launch_packaged(id)?;
            return Ok(true);
        }
        let path = Path::new(path);
        ensure!(
            path.is_absolute()
                && (path.is_dir() || path.is_file() && applications::is_launch_file(path)),
            "Application search target is unavailable"
        );
        shell::open(path.as_os_str())?;
        Ok(true)
    }

    fn application_icon_bgra(&self, path: &str, size: u32) -> Result<Option<crate::RawIconImage>> {
        applications::icon(path, size)
    }

    fn file_icon_bgra(&self, path: &str, size: u32) -> Result<Option<crate::RawIconImage>> {
        applications::icon(path, size)
    }

    fn web_platform_id(&self) -> Option<&'static str> {
        Some("windows")
    }
}

impl ContextReader for WindowsPlatform {
    fn application_directory(&self, application: &ApplicationInfo) -> Result<Option<PathBuf>> {
        context::application_directory(application)
    }

    fn selected_text(&self, application: &ApplicationInfo) -> Result<Option<String>> {
        context::selected_text(application)
    }

    fn file_selection_supported(&self, application: &ApplicationInfo) -> bool {
        context::is_explorer(application)
    }

    fn selected_file_paths(&self, application: &ApplicationInfo) -> Result<Vec<PathBuf>> {
        context::selected_file_paths(application)
    }

    fn browser_current_url_supported(&self, application: &ApplicationInfo) -> bool {
        context::browser_supported(application)
    }

    fn browser_current_url(&self, application: &ApplicationInfo) -> Result<Option<String>> {
        context::browser_current_url(application)
    }
}

impl ClipboardHost for WindowsPlatform {
    fn copy_text(&self, text: &str) -> Result<()> {
        ensure!(
            !text.contains('\0'),
            "clipboard text contains an embedded NUL"
        );
        crate::clipboard::copy_text(text)
    }

    fn start_clipboard_tracking(&self) {
        self.clipboard_tracker
            .get_or_init(clipboard::Tracker::start);
    }

    fn clipboard_generation(&self) -> Result<i64> {
        Ok(i64::from(clipboard::generation()?))
    }

    fn request_clipboard_history_access(&self) -> Result<()> {
        clipboard::generation().map(|_| ())
    }

    fn clipboard_history_payload(
        &self,
        generation: i64,
    ) -> Result<Option<crate::clipboard::ClipboardPayload>> {
        clipboard::read_history(u32::try_from(generation)?)
    }

    fn write_clipboard(&self, payload: &crate::clipboard::ClipboardPayload) -> Result<()> {
        clipboard::write_payload(payload)
    }

    fn watch_clipboard(
        &self,
        changed: Box<dyn Fn() + Send>,
    ) -> Result<Box<dyn crate::clipboard::ClipboardWatch>> {
        crate::clipboard::watch(changed)
    }

    fn clipboard_text(&self) -> Result<Option<String>> {
        clipboard::read_text(16 * 1024, None)
    }

    fn clipboard_snapshot(&self) -> Result<ClipboardSnapshot> {
        let generation = clipboard::generation()?;
        let text = clipboard::read_text(16 * 1024, Some(generation))?;
        let age_ms = self
            .clipboard_tracker
            .get()
            .and_then(|tracker| tracker.age_ms(generation));
        Ok(ClipboardSnapshot { text, age_ms })
    }
}

impl FileSearchHost for WindowsPlatform {
    fn file_search_supported(&self) -> bool {
        true
    }

    fn supported_file_search_engines(&self) -> &'static [crate::FileSearchEngine] {
        file_search::SUPPORTED_ENGINES
    }

    fn default_file_search_engine(&self) -> crate::FileSearchEngine {
        file_search::DEFAULT_ENGINE
    }

    fn file_search_index(&self) -> Option<&dyn FileSearchIndex> {
        Some(self)
    }

    fn search_provisional(
        &self,
        query: &str,
        roots: &[PathBuf],
        options: &crate::FileSearchOptions,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<FileIndexResults> {
        file_search::search_provisional(query, roots, options, cancelled)
    }
}

impl FileSearchIndex for WindowsPlatform {
    fn search_index(
        &self,
        query: &str,
        roots: &[PathBuf],
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<FileIndexResults> {
        everything::search(query, roots, cancelled)
    }
}

impl ResourceOpener for WindowsPlatform {
    fn open_url(&self, value: &str) -> Result<()> {
        let url = url::Url::parse(value)?;
        ensure!(
            matches!(url.scheme(), "http" | "https") && url.host_str().is_some(),
            "only absolute http/https URLs are permitted"
        );
        shell::open(url.as_str().as_ref())
    }

    fn open_file(&self, path: &Path) -> Result<()> {
        ensure!(
            path.is_absolute() && path.is_file(),
            "file no longer exists"
        );
        shell::open(dunce::simplified(path).as_os_str())
    }

    fn reveal_file(&self, path: &Path) -> Result<()> {
        shell::reveal(path)
    }

    fn open_directory(&self, path: &Path) -> Result<()> {
        ensure!(
            path.is_absolute() && path.is_dir(),
            "directory no longer exists"
        );
        shell::open(dunce::simplified(path).as_os_str())
    }

    fn open_terminal(&self, path: &Path) -> Result<()> {
        shell::open_terminal(path)
    }

    fn open_file_in_application(&self, application: &Path, target: &Path) -> Result<()> {
        shell::open_file_in_application(application, target)
    }

    fn open_url_in_application(&self, application: &Path, url: &str) -> Result<()> {
        shell::open_url_in_application(application, url)
    }
}

impl MenuHost for WindowsPlatform {
    fn accessibility_trusted(&self, _prompt: bool) -> bool {
        true
    }

    fn discover_menu_actions(&self, application: &ApplicationInfo) -> Result<MenuDiscovery> {
        menu::discover(application)
    }

    fn perform_menu_action(
        &self,
        application: &ApplicationInfo,
        locator: &MenuLocator,
    ) -> Result<()> {
        menu::perform(application, locator)
    }
}
impl GitHost for WindowsPlatform {}
