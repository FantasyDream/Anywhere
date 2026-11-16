use crate::l10n::{self, Locale};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureSetting {
    pub key: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettingsSnapshot {
    pub clipboard_recording: bool,
    pub features: Vec<FeatureSetting>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsCategory {
    General,
    Shortcuts,
    Context,
    Search,
    Clipboard,
    ApplicationHistory,
    Plugins,
}

impl SettingsCategory {
    pub const ALL: [Self; 7] = [
        Self::General,
        Self::Shortcuts,
        Self::Context,
        Self::Search,
        Self::Clipboard,
        Self::ApplicationHistory,
        Self::Plugins,
    ];

    pub fn label(self, locale: Locale) -> String {
        match self {
            Self::General => l10n::general(locale),
            Self::Shortcuts => l10n::shortcuts(locale),
            Self::Context => l10n::context(locale),
            Self::Search => l10n::search(locale),
            Self::Clipboard => l10n::clipboard(locale),
            Self::ApplicationHistory => l10n::app_history(locale),
            Self::Plugins => l10n::features(locale),
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Shortcuts => "shortcuts",
            Self::Context => "context",
            Self::Search => "search",
            Self::Clipboard => "clipboard",
            Self::ApplicationHistory => "app_history",
            Self::Plugins => "plugins",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SettingsModel {
    pub selected: SettingsCategory,
    pub clipboard_recording: bool,
    pub features: Vec<FeatureSetting>,
    pub saving: bool,
    pub error: Option<String>,
}

impl Default for SettingsModel {
    fn default() -> Self {
        Self {
            selected: SettingsCategory::Shortcuts,
            clipboard_recording: false,
            features: Vec::new(),
            saving: false,
            error: None,
        }
    }
}

impl SettingsModel {
    pub fn from_snapshot(snapshot: SettingsSnapshot) -> Self {
        Self {
            clipboard_recording: snapshot.clipboard_recording,
            features: snapshot.features,
            ..Self::default()
        }
    }

    pub fn apply_snapshot(&mut self, snapshot: SettingsSnapshot) {
        self.clipboard_recording = snapshot.clipboard_recording;
        self.features = snapshot.features;
        self.saving = false;
        self.error = None;
    }

    pub fn set_feature_enabled(&mut self, key: &str, enabled: bool) {
        if let Some(feature) = self.features.iter_mut().find(|feature| feature.key == key) {
            feature.enabled = enabled;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_expose_the_original_seven_stable_categories() {
        assert_eq!(SettingsCategory::ALL.len(), 7);
        assert_eq!(
            SettingsCategory::ALL
                .into_iter()
                .map(SettingsCategory::id)
                .collect::<Vec<_>>(),
            [
                "general",
                "shortcuts",
                "context",
                "search",
                "clipboard",
                "app_history",
                "plugins",
            ]
        );
        assert_eq!(
            SettingsModel::default().selected,
            SettingsCategory::Shortcuts
        );
    }
}
