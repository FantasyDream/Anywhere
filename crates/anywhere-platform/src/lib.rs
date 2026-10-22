//! OS capabilities behind a platform-neutral trait.
//!
//! Every capability has an explicitly unsupported default so test doubles compile
//! without implementing unused methods; macOS implements the ones the product uses.
pub mod clipboard;
pub mod paths;
pub mod private_storage;
pub mod search_exclusions;
pub mod search_storage;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "macos", test))]
pub(crate) mod editor_context;
pub mod file_search;
pub mod process;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSearchHit {
    pub path: PathBuf,
    pub is_directory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationInfo {
    pub bundle_id: String,
    pub name: String,
    #[serde(default)]
    pub pid: i32,
    /// Process launch identity; prevents accepting a reused PID.
    #[serde(default)]
    pub instance_id: Option<String>,
}

/// How to find a menu item again without holding an Accessibility element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuLocator {
    pub bundle_id: String,
    pub titles: Vec<String>,
    #[serde(default)]
    pub identifiers: Vec<Option<String>>,
}

/// One executable menu-bar leaf, captured without retaining AX elements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredMenuItem {
    pub title: String,
    pub menu_path: Vec<String>,
    #[serde(default)]
    pub identifiers: Vec<Option<String>>,
    #[serde(default)]
    pub shortcut: Option<String>,
    pub enabled: bool,
}

/// Outcome of reading another app's menu bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuDiscovery {
    Ready(Vec<DiscoveredMenuItem>),
    Untrusted,
    TimedOut,
    Unavailable,
}

/// Safe, typed capture failures shared by application and clipboard sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextReadError {
    Unsupported,
    PermissionDenied,
    ApplicationChanged,
    TimedOut,
    TooLarge,
    TooManyItems,
    Unavailable,
}

/// Kept for callers of the selected-text capability; all sources share typed errors.
pub type SelectedTextError = ContextReadError;

impl std::fmt::Display for ContextReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Unsupported => "source is not supported by this app",
            Self::PermissionDenied => "context access permission is not granted",
            Self::ApplicationChanged => "captured application identity is no longer valid",
            Self::TimedOut => "context capture timed out",
            Self::TooLarge => "context content exceeds the capture limit",
            Self::TooManyItems => "selection exceeds the capture item limit",
            Self::Unavailable => "context source is unavailable",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ContextReadError {}

impl MenuDiscovery {
    pub fn items(&self) -> &[DiscoveredMenuItem] {
        match self {
            Self::Ready(items) => items,
            _ => &[],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items().is_empty()
    }
}

/// Metadata read from one installed application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleApplication {
    pub bundle_id: String,
    pub name: String,
    /// Bundle-supplied display names by language. Never used as execution identity.
    pub localized_names: std::collections::BTreeMap<String, String>,
    /// Further names this application answers to, beyond `name`.
    ///
    /// Searchable but not shown. A platform whose display name and lookup name disagree
    /// puts the second one here instead of choosing between a title the user recognizes
    /// and a name they will type.
    pub aliases: Vec<String>,
    /// Path to launch and to read the icon from. `None` keeps the discovered entry.
    pub path: Option<String>,
    /// Opaque icon reference; resolved lazily through `application_icon`.
    pub icon: Option<String>,
}

/// Read-only Git operations. Never a general command channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitOperation {
    /// `git status --porcelain=v1`
    Status,
    /// `git log --oneline -n 100`
    Log,
    /// `git diff --no-color` (unstaged working tree changes only)
    Diff,
    StagedDiff,
    Branches,
    BranchName,
    CommitHash,
    ShortCommitHash,
    RemoteUrl,
}

/// Directory walk used by the default `installed_applications` implementation.
mod catalog;
/// Shared bounded draining of a child process's pipes.
pub(crate) mod drain;
/// Shared Git argv and formatting.
pub mod git;

impl GitOperation {
    pub const ALL: [GitOperation; 9] = [
        Self::Status,
        Self::Log,
        Self::Diff,
        Self::StagedDiff,
        Self::Branches,
        Self::BranchName,
        Self::CommitHash,
        Self::ShortCommitHash,
        Self::RemoteUrl,
    ];

    pub fn returns_value(self) -> bool {
        matches!(
            self,
            Self::BranchName | Self::CommitHash | Self::ShortCommitHash | Self::RemoteUrl
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Log => "log",
            Self::Diff => "diff",
            Self::StagedDiff => "staged-diff",
            Self::Branches => "branches",
            Self::BranchName => "branch-name",
            Self::CommitHash => "commit-hash",
            Self::ShortCommitHash => "short-commit-hash",
            Self::RemoteUrl => "remote-url",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOutput {
    /// Combined report shown in the result preview.
    pub text: String,
    /// True when output exceeded the cap and was cut short.
    pub truncated: bool,
}

/// What a catalog walk found. `skipped` counts bundles that were seen and not indexed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstalledApplications {
    pub applications: Vec<BundleApplication>,
    pub skipped: usize,
}

impl InstalledApplications {
    pub fn is_empty(&self) -> bool {
        self.applications.is_empty() && self.skipped == 0
    }
}

/// A single clipboard read paired with the age of that exact pasteboard generation.
#[derive(Debug, Default)]
pub struct ClipboardSnapshot {
    pub text: Option<String>,
    pub age_ms: Option<u64>,
}

/// Platform-neutral search scope. Empty roots mean the user home directory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileSearchOptions {
    /// Host-authorized scope: explicit path queries cannot escape these roots.
    pub restrict_to_roots: bool,
    pub roots: Vec<PathBuf>,
    pub excluded_directory_names: Vec<String>,
    pub engine: FileSearchEngine,
    pub include_hidden: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FileSearchEngine {
    #[default]
    SystemIndex,
    Ripgrep,
    Combined,
}

pub trait Platform: Send + Sync {
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
        // Adapters must not silently ignore a configured search scope.
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
    /// This is the swap point for the catalog. The default walks [`Self::application_roots`]
    /// and identifies each entry with [`Self::bundle_application`]. A later indexer
    /// (Launch Services, a native database) replaces this method only;
    /// Core keeps matching, ranking and refresh.
    ///
    /// `name` is the title shown to the user and must be the system-language display
    /// name, not the on-disk file name. Extra lookup names go in `aliases`.
    fn installed_applications(&self) -> InstalledApplications {
        catalog::from_roots(self)
    }

    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>>;
    fn application_directory(&self, application: &ApplicationInfo) -> Result<Option<PathBuf>>;
    fn open_url(&self, url: &str) -> Result<()>;
    fn open_file(&self, path: &Path) -> Result<()>;
    fn reveal_file(&self, path: &Path) -> Result<()>;
    fn open_directory(&self, path: &Path) -> Result<()>;
    fn open_terminal(&self, path: &Path) -> Result<()>;
    fn copy_text(&self, text: &str) -> Result<()>;

    // ---- V0.2 capabilities; unsupported platforms keep their V0.1 behaviour ----

    /// Metadata for one application entry (bundle, executable, or shortcut). `Ok(None)` means "not an application".
    fn bundle_application(&self, _bundle: &Path) -> Result<Option<BundleApplication>> {
        Ok(None)
    }

    /// Activate a running instance, otherwise launch the canonical bundle path.
    /// Returns whether an application was launched or activated.
    fn launch_application(&self, _path: &str) -> Result<bool> {
        bail!("launching applications is not supported on this platform")
    }

    /// Small PNG for an application icon, lazily requested for visible rows only.
    fn application_icon(&self, _path: &str, _size: u32) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

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

    /// Selected text in the captured app's focused control. Never activates
    /// another app or reads the clipboard. Empty selection is `Ok(None)`;
    /// unsupported controls and permission failures return typed errors.
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

    /// Legacy adapter hook retained for callers; new adapters implement selected_file_paths.
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

    /// Whether the read-only Git operations can run at all.
    fn git_available(&self) -> bool {
        git::available()
    }

    /// Run one allowlisted read-only Git operation inside `repository`.
    fn run_git(&self, repository: &Path, operation: GitOperation) -> Result<GitOutput> {
        git::run(repository, operation)
    }

    // ---- V0.3 menu actions; unsupported platforms stay empty ----

    /// Whether this process may read other apps' accessibility trees.
    ///
    /// `prompt` asks the OS to show the system permission dialog. Universal launcher
    /// passes false so a search does not nag.
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

    /// Short version string for cache invalidation. `None` when unknown.
    fn application_version(&self, _bundle_id: &str) -> Option<String> {
        None
    }
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacOsPlatform;
#[cfg(target_os = "macos")]
pub use macos::MacOsPlatform as NativePlatform;

/// Composition root selects the OS adapter; shared bridge code does not name an OS.
pub fn native_platform() -> Result<std::sync::Arc<dyn Platform>> {
    #[cfg(target_os = "macos")]
    {
        Ok(std::sync::Arc::new(MacOsPlatform))
    }
    #[cfg(not(target_os = "macos"))]
    {
        bail!("This build has no native system adapter. Shared Core remains available with an injected Platform.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time guard: the unsupported defaults must exist and stay inactive.
    #[test]
    fn unsupported_defaults_degrade_without_side_effects() {
        struct Minimal;
        impl Platform for Minimal {
            fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
                Ok(None)
            }
            fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<PathBuf>> {
                Ok(None)
            }
            fn open_url(&self, _: &str) -> Result<()> {
                Ok(())
            }
            fn open_file(&self, _: &Path) -> Result<()> {
                Ok(())
            }
            fn reveal_file(&self, _: &Path) -> Result<()> {
                Ok(())
            }
            fn open_directory(&self, _: &Path) -> Result<()> {
                Ok(())
            }
            fn open_terminal(&self, _: &Path) -> Result<()> {
                Ok(())
            }
            fn copy_text(&self, _: &str) -> Result<()> {
                Ok(())
            }
        }
        let platform = Minimal;
        assert!(platform.installed_applications().is_empty());
        assert!(platform
            .bundle_application(Path::new("/x.app"))
            .unwrap()
            .is_none());
        assert!(platform.application_icon("/x.app", 64).unwrap().is_none());
        assert!(platform.clipboard_text().unwrap().is_none());
        let app = ApplicationInfo {
            bundle_id: "x".into(),
            name: "x".into(),
            pid: 0,
            instance_id: None,
        };
        assert!(platform.selected_text(&app).unwrap().is_none());
        assert!(platform.finder_selected_paths(&app).unwrap().is_empty());
        assert!(platform.browser_current_url(&app).unwrap().is_none());
        assert_eq!(platform.git_available(), git::available());
        assert!(platform.launch_application("/x.app").is_err());
        assert!(platform
            .run_git(Path::new("/tmp"), GitOperation::Status)
            .is_err());
        assert!(!platform.accessibility_trusted(false));
        assert!(platform
            .discover_menu_actions(&ApplicationInfo {
                bundle_id: "x".into(),
                name: "x".into(),
                pid: 0,
                instance_id: None,
            })
            .unwrap()
            .is_empty());
        assert!(platform
            .perform_menu_action(
                &ApplicationInfo {
                    bundle_id: "x".into(),
                    name: "x".into(),
                    pid: 0,
                    instance_id: None,
                },
                &MenuLocator {
                    bundle_id: "x".into(),
                    titles: vec!["File".into()],
                    identifiers: vec![None],
                },
            )
            .is_err());
        assert!(platform.application_version("x").is_none());
    }

    #[test]
    fn git_operations_are_an_explicit_read_only_allowlist() {
        assert_eq!(
            GitOperation::ALL.map(GitOperation::as_str),
            [
                "status",
                "log",
                "diff",
                "staged-diff",
                "branches",
                "branch-name",
                "commit-hash",
                "short-commit-hash",
                "remote-url"
            ]
        );
    }
}
