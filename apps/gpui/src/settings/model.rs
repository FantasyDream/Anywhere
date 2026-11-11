use crate::l10n::Locale;

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
    Language,
}

impl SettingsCategory {
    pub const ALL: [Self; 8] = [
        Self::General,
        Self::Shortcuts,
        Self::Context,
        Self::Search,
        Self::Clipboard,
        Self::ApplicationHistory,
        Self::Plugins,
        Self::Language,
    ];

    pub fn label(self, locale: Locale) -> &'static str {
        match (self, locale) {
            (Self::General, Locale::En) => "General",
            (Self::General, Locale::Zh) => "通用",
            (Self::Shortcuts, Locale::En) => "Shortcuts",
            (Self::Shortcuts, Locale::Zh) => "快捷键",
            (Self::Context, Locale::En) => "Context",
            (Self::Context, Locale::Zh) => "上下文",
            (Self::Search, Locale::En) => "Search",
            (Self::Search, Locale::Zh) => "搜索",
            (Self::Clipboard, Locale::En) => "Clipboard",
            (Self::Clipboard, Locale::Zh) => "剪贴板",
            (Self::ApplicationHistory, Locale::En) => "App History",
            (Self::ApplicationHistory, Locale::Zh) => "应用历史",
            (Self::Plugins, Locale::En) => "Features",
            (Self::Plugins, Locale::Zh) => "功能",
            (Self::Language, Locale::En) => "Language",
            (Self::Language, Locale::Zh) => "语言",
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
            selected: SettingsCategory::General,
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
