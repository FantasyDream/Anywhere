//! OS capabilities behind a platform-neutral trait.
//!
//! Every capability has an explicitly unsupported default so test doubles compile
//! without implementing unused methods; macOS implements the ones the product uses.
pub mod clipboard;
pub mod paths;
pub mod private_storage;
pub mod search_exclusions;
pub mod search_storage;
mod traits;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
pub use traits::{
    ApplicationCatalog, ApplicationHost, ClipboardHost, ContextReader, FileSearchHost, GitHost,
    MenuHost, Platform, ResourceOpener,
};
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
        anyhow::bail!("This build has no native system adapter. Shared Core remains available with an injected Platform.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Compile-time guard: the unsupported defaults must exist and stay inactive.
    #[test]
    fn unsupported_defaults_degrade_without_side_effects() {
        struct Minimal;
        impl ApplicationCatalog for Minimal {}
        impl ApplicationHost for Minimal {
            fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
                Ok(None)
            }
        }
        impl ContextReader for Minimal {
            fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<PathBuf>> {
                Ok(None)
            }
        }
        impl ClipboardHost for Minimal {
            fn copy_text(&self, _: &str) -> Result<()> {
                Ok(())
            }
        }
        impl FileSearchHost for Minimal {}
        impl ResourceOpener for Minimal {
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
        }
        impl MenuHost for Minimal {}
        impl GitHost for Minimal {}
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
    fn narrow_capabilities_keep_clipboard_context_and_catalog_defaults() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Forwarding {
            root: PathBuf,
            copied: AtomicUsize,
        }

        impl ApplicationCatalog for Forwarding {
            fn application_roots(&self) -> Vec<PathBuf> {
                vec![self.root.clone()]
            }

            fn is_application_entry(&self, path: &Path) -> bool {
                path.extension().is_some_and(|extension| extension == "app")
            }

            fn bundle_application(&self, path: &Path) -> Result<Option<BundleApplication>> {
                Ok(Some(BundleApplication {
                    bundle_id: "test.forwarding".into(),
                    name: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    localized_names: Default::default(),
                    aliases: Vec::new(),
                    path: Some(path.to_string_lossy().into_owned()),
                    icon: None,
                }))
            }
        }

        impl ClipboardHost for Forwarding {
            fn copy_text(&self, _: &str) -> Result<()> {
                self.copied.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            fn clipboard_text(&self) -> Result<Option<String>> {
                Ok(Some("forwarded".into()))
            }
        }

        impl ContextReader for Forwarding {
            fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<PathBuf>> {
                Ok(Some(PathBuf::from("/captured-directory")))
            }

            fn finder_selected_paths(&self, _: &ApplicationInfo) -> Result<Vec<PathBuf>> {
                Ok(vec![PathBuf::from("/selected")])
            }
        }

        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("Applications");
        std::fs::create_dir_all(root.join("Alpha.app/Contents")).unwrap();
        let forwarding = Forwarding {
            root,
            copied: AtomicUsize::new(0),
        };
        let clipboard = &forwarding as &dyn ClipboardHost;
        assert_eq!(
            clipboard.clipboard_snapshot().unwrap().text.as_deref(),
            Some("forwarded")
        );
        let payload = clipboard::ClipboardPayload::text("copied");
        clipboard.write_clipboard(&payload).unwrap();
        assert_eq!(forwarding.copied.load(Ordering::SeqCst), 1);

        let context = &forwarding as &dyn ContextReader;
        let application = ApplicationInfo {
            bundle_id: "x".into(),
            name: "x".into(),
            pid: 0,
            instance_id: None,
        };
        assert_eq!(
            context.selected_file_paths(&application).unwrap(),
            [PathBuf::from("/selected")]
        );
        assert_eq!(
            context.application_directory(&application).unwrap(),
            Some(PathBuf::from("/captured-directory"))
        );

        let catalog = &forwarding as &dyn ApplicationCatalog;
        assert_eq!(catalog.installed_applications().applications.len(), 1);
    }

    #[test]
    fn configured_search_scope_is_rejected_by_the_narrow_port() {
        struct IndexOnly;

        impl FileSearchHost for IndexOnly {
            fn search_files(
                &self,
                _: &str,
                _: usize,
                _: &(dyn Fn() -> bool + Sync),
            ) -> Result<Vec<FileSearchHit>> {
                Ok(vec![FileSearchHit {
                    path: PathBuf::from("/indexed"),
                    is_directory: false,
                }])
            }
        }

        let search = IndexOnly;
        let search = &search as &dyn FileSearchHost;
        let cancelled = || false;
        assert_eq!(
            search
                .search_files_with_options("query", 1, &FileSearchOptions::default(), &cancelled)
                .unwrap()[0]
                .path,
            PathBuf::from("/indexed")
        );
        let configured = FileSearchOptions {
            restrict_to_roots: true,
            ..Default::default()
        };
        assert!(search
            .search_files_with_options("query", 1, &configured, &cancelled)
            .is_err());
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
