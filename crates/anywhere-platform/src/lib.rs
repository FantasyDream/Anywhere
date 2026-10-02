//! OS capabilities behind a platform-neutral trait.
//!
//! Every capability has an explicitly unsupported default so test doubles compile
//! without implementing unused methods; macOS implements the ones the product uses.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
pub mod process;

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
            Self::ApplicationChanged => "captured application is no longer running",
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

/// The only Git operations V0.2 exposes. Never a general command channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitOperation {
    /// `git status --porcelain=v1 --branch`
    Status,
    /// `git log --oneline -n 100`
    Log,
    /// `git diff --no-color` (unstaged working tree changes only)
    Diff,
}

/// Directory walk used by the default `installed_applications` implementation.
mod catalog;
/// Shared bounded draining of a child process's pipes.
pub(crate) mod drain;
/// Shared Git argv and formatting.
pub mod git;

impl GitOperation {
    pub const ALL: [GitOperation; 3] = [Self::Status, Self::Log, Self::Diff];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Log => "log",
            Self::Diff => "diff",
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

pub trait Platform: Send + Sync {
    /// Ordered application catalog roots for the default directory walk.
    ///
    /// Empty means this host has nothing to walk. Ignored by a platform that
    /// overrides [`Self::installed_applications`].
    fn application_roots(&self) -> Vec<PathBuf> {
        Vec::new()
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

    /// Metadata for one `.app` bundle. `Ok(None)` means "not an application".
    fn bundle_application(&self, _bundle: &Path) -> Result<Option<BundleApplication>> {
        Ok(None)
    }

    /// The user's preferred languages, most preferred first.
    fn preferred_languages(&self) -> Vec<String> {
        Vec::new()
    }

    /// The bundle's display name in each of `languages`, most preferred first.
    ///
    /// Localized names live in `<lang>.lproj/InfoPlist.strings` and are not part of
    /// `Info.plist`, so a search index that only reads the latter cannot be searched
    /// in a localized language. Empty when the bundle ships no such localization.
    fn bundle_localized_names(&self, _bundle: &Path, _languages: &[String]) -> Result<Vec<String>> {
        Ok(Vec::new())
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

    /// Files and directories selected in the captured Finder instance.
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
        false
    }

    /// Run one allowlisted read-only Git operation inside `repository`.
    fn run_git(&self, _repository: &Path, _operation: GitOperation) -> Result<GitOutput> {
        bail!("git is not supported on this platform")
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
        assert!(!platform.git_available());
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
    fn git_operations_expose_only_status_log_and_diff() {
        assert_eq!(
            GitOperation::ALL.map(GitOperation::as_str),
            ["status", "log", "diff"]
        );
    }
}
