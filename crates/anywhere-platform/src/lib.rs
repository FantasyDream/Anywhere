//! OS capabilities behind a platform-neutral trait.
//!
//! V0.2 adds application indexing, application activation, clipboard reading and
//! Git execution. Every new capability has an explicitly unsupported default so a
//! platform that does not implement it degrades instead of failing to compile.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationInfo {
    pub bundle_id: String,
    pub name: String,
}

/// Metadata read from one installed application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleApplication {
    pub bundle_id: String,
    pub name: String,
    /// Further names this application answers to, beyond `name`.
    ///
    /// Searchable but not shown. A platform whose display name and lookup name disagree —
    /// a shortcut's description versus its file name — puts the second one here instead of
    /// choosing between a title the user recognizes and a name they will type.
    pub aliases: Vec<String>,
    /// Path to launch and to read the icon from. `None` keeps the discovered entry.
    ///
    /// This is the *entry*, not the program: a Windows shortcut carries the arguments and
    /// working directory, so launching its target directly silently drops them.
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

/// Shared bounded draining of a child process's pipes.
pub(crate) mod drain;
/// Shared Git argv and formatting, so both desktop platforms run the same command.
pub mod git;
/// Native shim declarations for the Windows platform.
#[cfg(target_os = "windows")]
mod windows_native;

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

pub trait Platform: Send + Sync {
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

    /// Whether the read-only Git operations can run at all.
    fn git_available(&self) -> bool {
        false
    }

    /// Run one allowlisted read-only Git operation inside `repository`.
    fn run_git(&self, _repository: &Path, _operation: GitOperation) -> Result<GitOutput> {
        bail!("git is not supported on this platform")
    }
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacOsPlatform;
#[cfg(target_os = "macos")]
pub use macos::MacOsPlatform as NativePlatform;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsPlatform;
#[cfg(target_os = "windows")]
pub use windows::WindowsPlatform as NativePlatform;

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
        assert!(platform
            .bundle_application(Path::new("/x.app"))
            .unwrap()
            .is_none());
        assert!(platform.application_icon("/x.app", 64).unwrap().is_none());
        assert!(platform.clipboard_text().unwrap().is_none());
        assert!(!platform.git_available());
        assert!(platform.launch_application("/x.app").is_err());
        assert!(platform
            .run_git(Path::new("/tmp"), GitOperation::Status)
            .is_err());
    }

    #[test]
    fn git_operations_expose_only_status_log_and_diff() {
        assert_eq!(
            GitOperation::ALL.map(GitOperation::as_str),
            ["status", "log", "diff"]
        );
    }
}
