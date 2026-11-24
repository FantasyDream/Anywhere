mod atomic_write;
mod directory_access;
use atomic_write::atomic_write_private;
mod dto;
mod hotkeys;
mod login_item;
mod native_window;
mod preferences;
mod runtime;
mod runtime_support;

use anyhow::{Context, Result};
#[cfg(any(not(windows), test))]
use std::path::Path;
use std::path::{Component, PathBuf};

use anywhere_core::application::frontend_runtime::RuntimePaths;
pub use anywhere_core::application::frontend_runtime::WebPackageOperation;

pub use dto::{
    ActivationInput, AppHistoryItem, AppHistoryObjectAction, AppHistorySnapshot, AppHistorySource,
    ClipboardItem, ClipboardSnapshot, ExecutionDetail, ExecutionRow, PreviewFile,
    ShortcutCandidate, ShortcutCatalogAction, WebDocument, WebPackage, WebPackageReview,
};
pub use hotkeys::{registered_preferences, HotKeyAction, HotKeyController};
pub use login_item::LoginItemState;
pub(crate) use native_window::defer_native_window_update;
pub use preferences::{
    reserved_action_shortcut, ActionShortcutBinding, ActionShortcutRoute, HostPreferences,
    UiLanguage, UiTheme,
};
pub use runtime::{LiveEvent, LiveLauncher};

/// Stable product paths owned by the GPUI application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductPaths {
    pub data_directory: PathBuf,
    pub bundled_plugin_directory: PathBuf,
    pub explicit_data_directory: bool,
}

impl ProductPaths {
    pub fn discover() -> Result<Self> {
        let data_override = path_override("ANYWHERE_DATA_DIR")?;
        let plugin_override = path_override("ANYWHERE_PLUGIN_DIR")?;
        let executable = std::env::current_exe()?;
        #[cfg(windows)]
        {
            let explicit_data_directory = data_override.is_some();
            let data_directory = match data_override {
                Some(path) => path,
                None => std::env::var_os("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .or_else(|| {
                        anywhere_platform::paths::home_directory()
                            .map(|path| path.join(r"AppData\Local"))
                    })
                    .context("Windows local application data directory is unavailable")?
                    .join("Anywhere"),
            };
            let bundled_plugin_directory =
                plugin_override.unwrap_or_else(|| executable.with_file_name("plugins"));
            Ok(Self {
                data_directory,
                bundled_plugin_directory,
                explicit_data_directory,
            })
        }
        #[cfg(not(windows))]
        {
            let home = std::env::var_os("HOME").map(PathBuf::from);
            Self::resolve(home.as_deref(), &executable, data_override, plugin_override)
        }
    }

    #[cfg(not(windows))]
    fn resolve(
        home: Option<&Path>,
        executable: &Path,
        data_override: Option<PathBuf>,
        plugin_override: Option<PathBuf>,
    ) -> Result<Self> {
        let explicit_data_directory = data_override.is_some();
        let data_directory = match data_override {
            Some(path) => path,
            None => {
                let home = home.context("HOME is unavailable")?;
                home.join("Library")
                    .join("Application Support")
                    .join("Anywhere")
            }
        };
        let bundled_plugin_directory = plugin_override.unwrap_or_else(|| {
            executable
                .parent()
                .and_then(|contents| contents.parent())
                .map(|bundle| bundle.join("Resources").join("plugins"))
                .unwrap_or_else(|| PathBuf::from("plugins"))
        });
        Ok(Self {
            data_directory,
            bundled_plugin_directory,
            explicit_data_directory,
        })
    }

    pub fn runtime_paths(&self) -> RuntimePaths {
        RuntimePaths::new(
            self.data_directory.clone(),
            self.bundled_plugin_directory.clone(),
        )
    }
}

fn path_override(name: &str) -> Result<Option<PathBuf>> {
    let Some(value) = std::env::var_os(name) else {
        return Ok(None);
    };
    validate_override_path(PathBuf::from(value), name).map(Some)
}

fn validate_override_path(path: PathBuf, name: &str) -> Result<PathBuf> {
    anyhow::ensure!(path.is_absolute(), "{name} must be an absolute directory");
    anyhow::ensure!(
        path.parent().is_some() && !path.components().any(|c| c == Component::ParentDir),
        "{name} must not be the filesystem root or contain '..'"
    );
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(windows))]
    fn data_and_plugin_overrides_are_independent() {
        let executable = Path::new("/Applications/Anywhere.app/Contents/MacOS/Anywhere");
        let data = PathBuf::from("/tmp/gpui-data");
        let plugins = PathBuf::from("/tmp/gpui-plugins");

        let data_only = ProductPaths::resolve(
            Some(Path::new("/Users/example")),
            executable,
            Some(data.clone()),
            None,
        )
        .unwrap();
        assert_eq!(data_only.data_directory, data);
        assert_eq!(
            data_only.bundled_plugin_directory,
            Path::new("/Applications/Anywhere.app/Contents/Resources/plugins")
        );

        let plugin_only = ProductPaths::resolve(
            Some(Path::new("/Users/example")),
            executable,
            None,
            Some(plugins.clone()),
        )
        .unwrap();
        assert_eq!(
            plugin_only.data_directory,
            Path::new("/Users/example/Library/Application Support/Anywhere")
        );
        assert_eq!(plugin_only.bundled_plugin_directory, plugins);
    }

    #[test]
    fn explicit_directories_reject_unsafe_paths() {
        #[cfg(not(windows))]
        let (invalid, valid) = (["relative/path", "/", "/tmp/../data"], "/tmp/gpui-data");
        #[cfg(windows)]
        let (invalid, valid) = (
            [r"relative\path", r"C:\", r"C:\tmp\..\data"],
            r"C:\tmp\gpui-data",
        );
        for path in invalid {
            assert!(validate_override_path(PathBuf::from(path), "TEST_PATH").is_err());
        }
        assert_eq!(
            validate_override_path(PathBuf::from(valid), "TEST_PATH").unwrap(),
            Path::new(valid)
        );
    }
}
