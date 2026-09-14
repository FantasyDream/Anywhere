//! OS capabilities behind a platform-neutral trait.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplicationInfo {
    pub bundle_id: String,
    pub name: String,
}

pub trait Platform: Send + Sync {
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>>;
    fn application_directory(
        &self,
        application: &ApplicationInfo,
    ) -> Result<Option<std::path::PathBuf>>;
    fn open_url(&self, url: &str) -> Result<()>;
    fn open_file(&self, path: &Path) -> Result<()>;
    fn reveal_file(&self, path: &Path) -> Result<()>;
    fn open_directory(&self, path: &Path) -> Result<()>;
    fn open_terminal(&self, path: &Path) -> Result<()>;
    fn copy_text(&self, text: &str) -> Result<()>;
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
