//! Narrow OS capability ports composed by [`Platform`].
//!
//! Business services depend on the capability they use. The composition root
//! still injects one adapter through [`Platform`].
use super::{
    catalog, clipboard, file_search, git, ApplicationInfo, BundleApplication, ClipboardSnapshot,
    ContextReadError, FileSearchEngine, FileSearchHit, FileSearchOptions, GitOperation, GitOutput,
    InstalledApplications, MenuDiscovery, MenuLocator,
};
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

/// Discovery and metadata needed to build the installed-application catalog.
pub trait ApplicationCatalog: Send + Sync {
    /// Ordered application catalog roots for the default directory walk.
    ///
    /// Empty means this host has nothing to walk. Ignored by a platform that
    /// overrides [`Self::installed_applications`].
    fn application_roots(&self) -> Vec<PathBuf> {
        Vec::new()
    }

    /// The adapter decides which entries the shared directory walker treats as applications.
    fn is_application_entry(&self, _path: &Path) -> bool {
        false
    }

    /// Installed applications this host can launch.
    ///
    /// This is the swap point for the catalog. The default walks
    /// [`Self::application_roots`] and identifies each entry with
    /// [`Self::bundle_application`]. A later indexer replaces this method only.
    fn installed_applications(&self) -> InstalledApplications {
        catalog::from_roots(self)
    }

    /// Metadata for one application entry (bundle, executable, or shortcut).
    fn bundle_application(&self, _bundle: &Path) -> Result<Option<BundleApplication>> {
        Ok(None)
    }
}

/// Running-application identity, launch, icon, and version capabilities.
pub trait ApplicationHost: Send + Sync {
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>>;

    /// Activate a running instance, otherwise launch the canonical bundle path.
    fn launch_application(&self, _path: &str) -> Result<bool> {
        bail!("launching applications is not supported on this platform")
    }

    /// Small PNG for an application icon, lazily requested for visible rows only.
    fn application_icon(&self, _path: &str, _size: u32) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    /// Short version string for cache invalidation. `None` when unknown.
    fn application_version(&self, _bundle_id: &str) -> Option<String> {
        None
    }
}

/// One-shot reads made for a captured application.
pub trait ContextReader: Send + Sync {
    /// Current directory represented by the captured application instance.
    fn application_directory(&self, application: &ApplicationInfo) -> Result<Option<PathBuf>>;

    /// Selected text in the captured app's focused control.
    fn selected_text(&self, _application: &ApplicationInfo) -> Result<Option<String>> {
        Ok(None)
    }

    /// Whether this captured application can supply a file selection.
    fn file_selection_supported(&self, _application: &ApplicationInfo) -> bool {
        false
    }

    /// Selected files/directories, independent of the file manager's identity.
    fn selected_file_paths(&self, application: &ApplicationInfo) -> Result<Vec<PathBuf>> {
        self.finder_selected_paths(application)
    }

    /// Legacy adapter hook retained for callers; new adapters implement
    /// [`Self::selected_file_paths`].
    fn finder_selected_paths(&self, _application: &ApplicationInfo) -> Result<Vec<PathBuf>> {
        Ok(Vec::new())
    }

    /// Whether this platform has a current-page adapter for the captured app.
    fn browser_current_url_supported(&self, _application: &ApplicationInfo) -> bool {
        false
    }

    /// URL of the captured browser's current page, when an adapter exists.
    fn browser_current_url(&self, _application: &ApplicationInfo) -> Result<Option<String>> {
        Ok(None)
    }
}

/// Clipboard observation, history persistence, and writes.
pub trait ClipboardHost: Send + Sync {
    fn copy_text(&self, text: &str) -> Result<()>;

    /// Observe generation changes only, without reading or retaining clipboard text.
    fn start_clipboard_tracking(&self) {}

    /// History polling reads only this generation until it changes.
    fn clipboard_generation(&self) -> Result<i64> {
        Err(ContextReadError::Unsupported.into())
    }

    /// Explicit user request only. May show the OS permission prompt; never records text.
    fn request_clipboard_history_access(&self) -> Result<()> {
        Err(ContextReadError::Unsupported.into())
    }

    /// Read all persistent representations of one generation; never prompt.
    fn clipboard_history_payload(
        &self,
        _generation: i64,
    ) -> Result<Option<clipboard::ClipboardPayload>> {
        Err(ContextReadError::Unsupported.into())
    }

    fn write_clipboard(&self, payload: &clipboard::ClipboardPayload) -> Result<()> {
        if payload.formats.len() == 1 {
            if let Some(text) = payload.plain_text() {
                return self.copy_text(text);
            }
        }
        Err(ContextReadError::Unsupported.into())
    }

    fn watch_clipboard(
        &self,
        _changed: Box<dyn Fn() + Send>,
    ) -> Result<Box<dyn clipboard::ClipboardWatch>> {
        Err(ContextReadError::Unsupported.into())
    }

    fn clipboard_snapshot(&self) -> Result<ClipboardSnapshot> {
        Ok(ClipboardSnapshot {
            text: self.clipboard_text()?,
            age_ms: None,
        })
    }

    /// One plain-text clipboard snapshot. `Ok(None)` covers non-text and empty.
    fn clipboard_text(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

/// System-index and traversal-backed file search.
pub trait FileSearchHost: Send + Sync {
    fn file_search_supported(&self) -> bool {
        false
    }

    /// Read the system index; callers own cancellation and result limits.
    fn search_files(
        &self,
        _query: &str,
        _limit: usize,
        _cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<FileSearchHit>> {
        bail!("file search is not supported on this platform")
    }

    fn search_files_with_options(
        &self,
        query: &str,
        limit: usize,
        options: &FileSearchOptions,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<FileSearchHit>> {
        if options.engine == FileSearchEngine::Ripgrep {
            return file_search::ripgrep(
                query,
                limit,
                options,
                cancelled,
                std::time::Duration::from_secs(2),
            );
        }
        if options != &FileSearchOptions::default() {
            bail!("configured file search is unsupported");
        }
        self.search_files(query, limit, cancelled)
    }
}

/// Explicit local and URL opening, including application-targeted handoff.
pub trait ResourceOpener: Send + Sync {
    fn open_url(&self, url: &str) -> Result<()>;
    fn open_file(&self, path: &Path) -> Result<()>;
    fn reveal_file(&self, path: &Path) -> Result<()>;
    fn open_directory(&self, path: &Path) -> Result<()>;
    fn open_terminal(&self, path: &Path) -> Result<()>;

    /// Open a local history object in an explicitly selected installed application.
    fn open_file_in_application(&self, _application: &Path, _target: &Path) -> Result<()> {
        bail!("opening an object in a specific application is unsupported")
    }

    /// Deliver a feature-validated deep link to an explicit application.
    fn open_url_in_application(&self, _application: &Path, _url: &str) -> Result<()> {
        bail!("opening a URL in a specific application is unsupported")
    }
}

/// Accessibility and menu-bar discovery and activation.
pub trait MenuHost: Send + Sync {
    /// Whether this process may read other apps' accessibility trees.
    fn accessibility_trusted(&self, _prompt: bool) -> bool {
        false
    }

    /// Menu-bar leaves of `application`.
    fn discover_menu_actions(&self, _application: &ApplicationInfo) -> Result<MenuDiscovery> {
        Ok(MenuDiscovery::Ready(Vec::new()))
    }

    /// Activate `application`, relocate the item, and press it. Never panics.
    fn perform_menu_action(
        &self,
        _application: &ApplicationInfo,
        _locator: &MenuLocator,
    ) -> Result<()> {
        bail!("menu actions are not supported on this platform")
    }
}

/// Read-only Git operations.
pub trait GitHost: Send + Sync {
    /// Whether the read-only Git operations can run at all.
    fn git_available(&self) -> bool {
        git::available()
    }

    /// Run one allowlisted read-only Git operation inside `repository`.
    fn run_git(&self, repository: &Path, operation: GitOperation) -> Result<GitOutput> {
        git::run(repository, operation)
    }
}

/// Complete platform adapter used by application-level composition.
pub trait Platform:
    ApplicationCatalog
    + ApplicationHost
    + ContextReader
    + ClipboardHost
    + FileSearchHost
    + ResourceOpener
    + MenuHost
    + GitHost
    + Send
    + Sync
{
}

impl<T> Platform for T where
    T: ApplicationCatalog
        + ApplicationHost
        + ContextReader
        + ClipboardHost
        + FileSearchHost
        + ResourceOpener
        + MenuHost
        + GitHost
        + Send
        + Sync
        + ?Sized
{
}
