//! macOS platform capabilities.
//!
//! Each capability is its own module, and the `Platform` impl below is only a delegation
//! layer: it holds no logic, so a change to how Git is run or how an icon is read touches one
//! file and cannot reach the others.
mod application;
mod clipboard;
mod finder;
mod git;
mod native;
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

impl Platform for MacOsPlatform {
    fn application_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Applications"));
        }
        roots.push(PathBuf::from("/Applications"));
        roots.push(PathBuf::from("/System/Applications"));
        roots
    }

    fn installed_applications(&self) -> InstalledApplications {
        application::installed_applications(&self.application_roots())
    }

    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        application::frontmost_application()
    }
    fn application_directory(&self, application: &ApplicationInfo) -> Result<Option<PathBuf>> {
        finder::application_directory(application)
    }
    fn bundle_application(&self, bundle: &Path) -> Result<Option<BundleApplication>> {
        application::bundle_application(bundle)
    }
    fn preferred_languages(&self) -> Vec<String> {
        application::preferred_languages()
    }
    fn bundle_localized_names(&self, bundle: &Path, languages: &[String]) -> Result<Vec<String>> {
        application::bundle_localized_names(bundle, languages)
    }
    fn launch_application(&self, path: &str) -> Result<bool> {
        application::launch_application(path)
    }
    fn application_icon(&self, path: &str, size: u32) -> Result<Option<Vec<u8>>> {
        application::application_icon(path, size)
    }
    fn clipboard_text(&self) -> Result<Option<String>> {
        clipboard::clipboard_text()
    }
    fn copy_text(&self, text: &str) -> Result<()> {
        clipboard::copy_text(text)
    }
    fn git_available(&self) -> bool {
        git::git_available()
    }
    fn run_git(&self, repository: &Path, operation: GitOperation) -> Result<GitOutput> {
        git::run_git(repository, operation)
    }
    fn open_url(&self, value: &str) -> Result<()> {
        shell::open_url(value)
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
