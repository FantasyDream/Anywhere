use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiLanguage {
    #[default]
    System,
    En,
    Zh,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HostPreferences {
    pub current_app_shortcut: String,
    pub universal_shortcut: String,
    pub launch_at_login: bool,
    pub ui_language: UiLanguage,
    pub excluded_apps: Vec<String>,
    pub capture_finder_selection: bool,
    pub capture_selected_text: bool,
    pub capture_browser_current_page: bool,
    pub capture_clipboard: bool,
    pub file_search_enabled: bool,
    pub file_search_engine: String,
    pub file_search_include_hidden: bool,
    pub file_search_roots: Vec<String>,
    pub file_search_excluded_names: Vec<String>,
}

impl Default for HostPreferences {
    fn default() -> Self {
        Self {
            current_app_shortcut: "meta+alt+Space".to_owned(),
            universal_shortcut: "meta+Space".to_owned(),
            launch_at_login: false,
            ui_language: UiLanguage::System,
            excluded_apps: Vec::new(),
            capture_finder_selection: true,
            capture_selected_text: true,
            capture_browser_current_page: true,
            capture_clipboard: true,
            file_search_enabled: false,
            file_search_engine: "system_index".to_owned(),
            file_search_include_hidden: false,
            file_search_roots: Vec::new(),
            file_search_excluded_names: Vec::new(),
        }
    }
}

impl HostPreferences {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice::<Self>(&bytes)
                .with_context(|| format!("read {}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let parent = path.parent().context("settings path has no parent")?;
        std::fs::create_dir_all(parent)?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&temporary, path)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            matches!(
                self.file_search_engine.as_str(),
                "system_index" | "systemIndex" | "ripgrep" | "combined"
            ),
            "Unsupported file search engine"
        );
        anyhow::ensure!(
            self.file_search_roots.len() <= 8
                && self.file_search_excluded_names.len()
                    <= anywhere_protocol::policy::MAX_SEARCH_EXCLUSIONS as usize,
            "Use at most 8 directories and 32 exclusions"
        );
        anyhow::ensure!(
            !self.file_search_enabled || !self.file_search_roots.is_empty(),
            "Choose and authorize at least one search directory"
        );
        for root in &self.file_search_roots {
            anyhow::ensure!(
                Path::new(root).is_absolute()
                    && root.len() <= anywhere_protocol::policy::MAX_SEARCH_PATH_BYTES as usize
                    && !root.chars().any(char::is_control),
                "Search directories must be absolute paths"
            );
        }
        for name in &self.file_search_excluded_names {
            anyhow::ensure!(
                !name.trim().is_empty()
                    && name.len() <= anywhere_protocol::policy::MAX_SEARCH_EXCLUSION_BYTES as usize
                    && !name.contains(['/', '\\'])
                    && !name.chars().any(char::is_control)
                    && name != "."
                    && name != "..",
                "Exclusions must be directory names"
            );
        }
        for shortcut in [&self.current_app_shortcut, &self.universal_shortcut] {
            shortcut
                .parse::<global_hotkey::hotkey::HotKey>()
                .with_context(|| format!("invalid global shortcut: {shortcut}"))?;
        }
        anyhow::ensure!(
            self.current_app_shortcut != self.universal_shortcut,
            "Current App and Universal Search shortcuts must differ"
        );
        Ok(())
    }

    pub fn file_search_engine(&self) -> anywhere_platform::FileSearchEngine {
        match self.file_search_engine.as_str() {
            "ripgrep" => anywhere_platform::FileSearchEngine::Ripgrep,
            "combined" => anywhere_platform::FileSearchEngine::Combined,
            _ => anywhere_platform::FileSearchEngine::SystemIndex,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_preferences_use_authorization_safe_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let preferences = HostPreferences::load(&directory.path().join("settings.json")).unwrap();
        assert!(!preferences.file_search_enabled);
        assert!(preferences.file_search_roots.is_empty());
        assert!(preferences.validate().is_ok());
    }

    #[test]
    fn unsafe_search_scope_is_rejected() {
        let mut preferences = HostPreferences::default();
        preferences.file_search_roots.push("relative".to_owned());
        assert!(preferences.validate().is_err());
        preferences.file_search_roots = vec!["/tmp/../scope".to_owned()];
        assert!(preferences.validate().is_ok());
        preferences.file_search_excluded_names = vec!["../cache".to_owned()];
        assert!(preferences.validate().is_err());
    }
}
