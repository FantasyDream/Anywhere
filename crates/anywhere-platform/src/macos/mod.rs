//! macOS platform capabilities.
//!
//! Each capability is its own module, and the `Platform` impl below is only a delegation
//! layer: it holds no logic, so a change to how Git is run or how an icon is read touches one
//! file and cannot reach the others.
mod aliases;
mod application;
mod browser;
mod clipboard;
mod editor;
mod finder;
pub(crate) mod native;
mod shell;

use crate::{
    ApplicationInfo, BundleApplication, GitOperation, GitOutput, InstalledApplications, Platform,
};
use anyhow::{Context, Result};
use std::{
    ffi::CString,
    path::{Path, PathBuf},
};

/// The macOS implementation of the platform port.
pub struct MacOsPlatform;

mod file_search;

impl Platform for MacOsPlatform {
    fn file_search_supported(&self) -> bool {
        true
    }

    fn search_files(
        &self,
        query: &str,
        limit: usize,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<crate::FileSearchHit>> {
        file_search::search(query, limit, cancelled)
    }
    fn search_files_with_options(
        &self,
        query: &str,
        limit: usize,
        options: &crate::FileSearchOptions,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<crate::FileSearchHit>> {
        file_search::search_with_options(query, limit, options, cancelled)
    }
    fn application_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Applications"));
        }
        roots.push(PathBuf::from("/Applications"));
        roots.push(PathBuf::from("/System/Applications"));
        roots
    }

    fn is_application_entry(&self, path: &Path) -> bool {
        path.extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
    }
    fn installed_applications(&self) -> InstalledApplications {
        application::installed_applications(&self.application_roots())
    }

    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        application::frontmost_application()
    }
    fn application_directory(&self, application: &ApplicationInfo) -> Result<Option<PathBuf>> {
        if application.bundle_id == "com.apple.finder" {
            finder::application_directory(application)
        } else {
            editor::application_directory(application)
        }
    }
    fn bundle_application(&self, bundle: &Path) -> Result<Option<BundleApplication>> {
        application::bundle_application(bundle)
    }
    fn launch_application(&self, path: &str) -> Result<bool> {
        application::launch_application(path)
    }
    fn application_icon(&self, path: &str, size: u32) -> Result<Option<Vec<u8>>> {
        application::application_icon(path, size)
    }
    fn start_clipboard_tracking(&self) {
        clipboard::start_tracking();
    }
    fn clipboard_snapshot(&self) -> Result<crate::ClipboardSnapshot> {
        clipboard::snapshot()
    }
    fn clipboard_generation(&self) -> Result<i64> {
        let count = clipboard::change_count();
        anyhow::ensure!(count >= 0, crate::ContextReadError::Unavailable);
        Ok(count)
    }
    fn request_clipboard_history_access(&self) -> Result<()> {
        crate::clipboard::request_access()
    }
    fn clipboard_history_payload(
        &self,
        generation: i64,
    ) -> Result<Option<crate::clipboard::ClipboardPayload>> {
        crate::clipboard::read(generation)
    }
    fn write_clipboard(&self, payload: &crate::clipboard::ClipboardPayload) -> Result<()> {
        crate::clipboard::write(payload)
    }
    fn watch_clipboard(
        &self,
        changed: Box<dyn Fn() + Send>,
    ) -> Result<Box<dyn crate::clipboard::ClipboardWatch>> {
        crate::clipboard::watch(changed)
    }
    fn clipboard_text(&self) -> Result<Option<String>> {
        clipboard::clipboard_text()
    }
    fn selected_text(&self, application: &ApplicationInfo) -> Result<Option<String>> {
        application::selected_text(application)
    }
    fn file_selection_supported(&self, application: &ApplicationInfo) -> bool {
        application.bundle_id == "com.apple.finder"
    }
    fn finder_selected_paths(&self, application: &ApplicationInfo) -> Result<Vec<PathBuf>> {
        finder::selected_paths(application)
    }
    fn browser_current_url_supported(&self, application: &ApplicationInfo) -> bool {
        browser::supports(&application.bundle_id)
    }
    fn browser_current_url(&self, application: &ApplicationInfo) -> Result<Option<String>> {
        browser::current_url(application)
    }
    fn copy_text(&self, text: &str) -> Result<()> {
        clipboard::copy_text(text)
    }
    fn git_available(&self) -> bool {
        crate::git::available()
    }
    fn run_git(&self, repository: &Path, operation: GitOperation) -> Result<GitOutput> {
        crate::git::run(repository, operation)
    }
    fn open_url(&self, value: &str) -> Result<()> {
        shell::open_url(value)
    }
    fn open_file_in_application(&self, application: &Path, target: &Path) -> Result<()> {
        shell::open_file_in_application(application, target)
    }
    fn open_file(&self, path: &Path) -> Result<()> {
        shell::open_file(path)
    }
    fn reveal_file(&self, path: &Path) -> Result<()> {
        shell::reveal_file(path)
    }
    fn open_directory(&self, path: &Path) -> Result<()> {
        shell::open_directory(path)
    }
    fn open_terminal(&self, path: &Path) -> Result<()> {
        shell::open_terminal(path)
    }
    fn accessibility_trusted(&self, prompt: bool) -> bool {
        application::accessibility_trusted(prompt)
    }
    fn discover_menu_actions(&self, app: &ApplicationInfo) -> Result<crate::MenuDiscovery> {
        application::discover_menu_actions(app)
    }
    fn perform_menu_action(
        &self,
        app: &ApplicationInfo,
        locator: &crate::MenuLocator,
    ) -> Result<()> {
        application::perform_menu_action(app, locator)
    }
    fn application_version(&self, bundle_id: &str) -> Option<String> {
        application::application_version(bundle_id)
    }
}

fn c_path(path: &str) -> Result<CString> {
    let value = CString::new(path).with_context(|| format!("path is not valid C: {path}"))?;
    Ok(value)
}
