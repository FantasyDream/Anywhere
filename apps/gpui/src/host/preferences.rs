use crate::l10n;
use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::{io::Read as _, path::Path};

const MAX_PREFERENCES_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiLanguage {
    #[default]
    System,
    En,
    Zh,
}

impl UiLanguage {
    pub fn locale(self) -> l10n::Locale {
        match self {
            Self::En => l10n::Locale::En,
            Self::Zh => l10n::Locale::Zh,
            Self::System => {
                if sys_locale::get_locale().is_some_and(|locale| {
                    locale
                        .split(['-', '_'])
                        .next()
                        .is_some_and(|language| language.eq_ignore_ascii_case("zh"))
                }) {
                    l10n::Locale::Zh
                } else {
                    l10n::Locale::En
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTheme {
    #[default]
    System,
    Light,
    Dark,
}

impl UiTheme {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn label(self, locale: l10n::Locale) -> String {
        match self {
            Self::System => l10n::language_system(locale),
            Self::Light => l10n::theme_light(locale),
            Self::Dark => l10n::theme_dark(locale),
        }
    }

    pub fn appearance(self) -> Option<crate::platform::WindowAppearance> {
        use crate::platform::WindowAppearance;
        match self {
            Self::System => None,
            Self::Light => Some(WindowAppearance::Light),
            Self::Dark => Some(WindowAppearance::Dark),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionShortcutRoute {
    pub action_id: String,
    pub title: String,
    #[serde(default)]
    pub app_bundle_id: Option<String>,
    #[serde(default)]
    pub app_name: Option<String>,
    #[serde(default)]
    pub context_type: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionShortcutBinding {
    pub action_id: String,
    pub title: String,
    pub shortcut: String,
    #[serde(default)]
    pub global: bool,
    #[serde(default)]
    pub routes: Vec<ActionShortcutRoute>,
}

const DEFAULT_CURRENT_APP_SHORTCUT: &str = "control+alt+Space";
const DEFAULT_UNIVERSAL_SHORTCUT: &str = crate::platform::capabilities::DEFAULT_UNIVERSAL_SHORTCUT;
const LEGACY_CURRENT_APP_SHORTCUT: &str = "meta+alt+Space";
const LEGACY_UNIVERSAL_SHORTCUT: &str = "meta+Space";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HostPreferences {
    pub current_app_shortcut: String,
    pub universal_shortcut: String,
    pub launch_at_login: bool,
    pub ui_language: UiLanguage,
    pub ui_theme: UiTheme,
    pub excluded_apps: Vec<String>,
    pub application_search_paths: Vec<String>,
    pub capture_finder_selection: bool,
    pub capture_selected_text: bool,
    pub capture_browser_current_page: bool,
    pub capture_clipboard: bool,
    pub file_search_enabled: bool,
    pub file_search_engine: anywhere_platform::FileSearchEngine,
    pub file_search_include_hidden: bool,
    pub file_search_roots: Vec<String>,
    pub file_search_excluded_names: Vec<String>,
    #[serde(deserialize_with = "deserialize_action_shortcuts")]
    pub action_shortcuts: Vec<ActionShortcutBinding>,
}

impl Default for HostPreferences {
    fn default() -> Self {
        Self {
            current_app_shortcut: DEFAULT_CURRENT_APP_SHORTCUT.to_owned(),
            universal_shortcut: DEFAULT_UNIVERSAL_SHORTCUT.to_owned(),
            launch_at_login: false,
            ui_language: UiLanguage::System,
            ui_theme: UiTheme::System,
            excluded_apps: Vec::new(),
            application_search_paths: Vec::new(),
            capture_finder_selection: crate::platform::capabilities::DESKTOP_CONTEXT_CAPTURE,
            capture_selected_text: crate::platform::capabilities::DESKTOP_CONTEXT_CAPTURE,
            capture_browser_current_page: crate::platform::capabilities::DESKTOP_CONTEXT_CAPTURE,
            capture_clipboard: true,
            file_search_enabled: false,
            file_search_engine: crate::platform::capabilities::default_file_search_engine(),
            file_search_include_hidden: false,
            file_search_roots: Vec::new(),
            file_search_excluded_names: Vec::new(),
            action_shortcuts: Vec::new(),
        }
    }
}

impl HostPreferences {
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::File::open(path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(MAX_PREFERENCES_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)?;
                anyhow::ensure!(
                    bytes.len() <= MAX_PREFERENCES_BYTES,
                    l10n::settings_too_large(UiLanguage::System.locale())
                );
                let mut preferences = serde_json::from_slice::<Self>(&bytes)
                    .with_context(|| format!("read {}", path.display()))?;
                preferences.migrate_legacy_default_shortcuts();
                Ok(preferences)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
        }
    }

    fn migrate_legacy_default_shortcuts(&mut self) {
        let current_is_legacy =
            equivalent_hotkeys(&self.current_app_shortcut, LEGACY_CURRENT_APP_SHORTCUT);
        let universal_is_legacy =
            equivalent_hotkeys(&self.universal_shortcut, LEGACY_UNIVERSAL_SHORTCUT);
        if current_is_legacy && universal_is_legacy {
            self.current_app_shortcut = DEFAULT_CURRENT_APP_SHORTCUT.to_owned();
            self.universal_shortcut = DEFAULT_UNIVERSAL_SHORTCUT.to_owned();
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self)?;
        anyhow::ensure!(
            bytes.len() <= MAX_PREFERENCES_BYTES,
            l10n::settings_too_large(self.ui_language.locale())
        );
        super::atomic_write_private(path, &bytes)
    }

    pub fn validate(&self) -> Result<()> {
        let locale = self.ui_language.locale();
        if !crate::platform::capabilities::APPLICATION_SEARCH_FILE_EXTENSIONS.is_empty() {
            anyhow::ensure!(
                self.application_search_paths.len() <= 64
                    && self.application_search_paths.iter().all(|value| {
                        let path = Path::new(value);
                        path.is_absolute()
                            && value.len()
                                <= anywhere_protocol::policy::MAX_SEARCH_PATH_BYTES as usize
                            && !value.chars().any(char::is_control)
                    }),
                l10n::windows_application_search_path_error(locale)
            );
        }
        anyhow::ensure!(
            self.file_search_roots.len() <= 8
                && self.file_search_excluded_names.len()
                    <= anywhere_protocol::policy::MAX_SEARCH_EXCLUSIONS as usize,
            l10n::search_directory_limits(locale)
        );
        anyhow::ensure!(
            !self.file_search_enabled || !self.file_search_roots.is_empty(),
            l10n::choose_authorized_directories(locale)
        );
        for root in &self.file_search_roots {
            anyhow::ensure!(
                Path::new(root).is_absolute()
                    && root.len() <= anywhere_protocol::policy::MAX_SEARCH_PATH_BYTES as usize
                    && !root.chars().any(char::is_control),
                l10n::absolute_search_directories(locale)
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
                l10n::invalid_search_exclusions(locale)
            );
        }
        let current_app_hotkey = self
            .current_app_shortcut
            .parse::<global_hotkey::hotkey::HotKey>()
            .with_context(|| l10n::invalid_global_shortcut(locale))?;
        let universal_hotkey = self
            .universal_shortcut
            .parse::<global_hotkey::hotkey::HotKey>()
            .with_context(|| l10n::invalid_global_shortcut(locale))?;
        anyhow::ensure!(
            current_app_hotkey != universal_hotkey,
            l10n::distinct_launcher_shortcuts(locale)
        );
        anyhow::ensure!(
            self.action_shortcuts.len() <= anywhere_protocol::policy::MAX_ACTION_SHORTCUTS as usize,
            l10n::too_many_action_shortcuts(locale)
        );
        let mut action_ids = std::collections::HashSet::new();
        let mut action_shortcuts = std::collections::HashSet::new();
        for binding in &self.action_shortcuts {
            anyhow::ensure!(
                !binding.action_id.trim().is_empty()
                    && binding.action_id.len()
                        <= anywhere_protocol::policy::MAX_ACTION_ID_BYTES as usize,
                l10n::invalid_action_shortcut_identity(locale)
            );
            anyhow::ensure!(
                action_ids.insert(binding.action_id.as_str()),
                l10n::distinct_action_shortcuts(locale)
            );
            let hotkey = binding
                .shortcut
                .parse::<global_hotkey::hotkey::HotKey>()
                .with_context(|| l10n::invalid_action_shortcut(locale))?;
            anyhow::ensure!(
                binding.shortcut.rsplit('+').next().is_some_and(|key| {
                    key.len() == 4
                        && key.starts_with("Key")
                        && key.as_bytes()[3].is_ascii_uppercase()
                }) && ["control", "alt", "meta"]
                    .iter()
                    .any(|modifier| { binding.shortcut.split('+').any(|part| part == *modifier) }),
                l10n::action_shortcut_modifiers(locale)
            );
            anyhow::ensure!(
                hotkey != current_app_hotkey && hotkey != universal_hotkey,
                l10n::reserved_launcher_shortcut(locale)
            );
            anyhow::ensure!(
                action_shortcuts.insert(hotkey),
                l10n::distinct_shortcut_keys(locale)
            );
            anyhow::ensure!(
                !reserved_action_shortcut(&binding.shortcut),
                l10n::reserved_editing_shortcut(locale)
            );
            let is_intent = binding.action_id.starts_with("intent:");
            anyhow::ensure!(
                is_intent == !binding.routes.is_empty(),
                l10n::invalid_context_shortcut_identity(locale)
            );
            if is_intent {
                anyhow::ensure!(
                    !binding.title.trim().is_empty()
                        && binding.title.len()
                            <= anywhere_protocol::policy::MAX_ACTION_TITLE_BYTES as usize
                        && binding.routes.len()
                            <= anywhere_protocol::policy::MAX_SHORTCUT_ROUTES as usize,
                    l10n::invalid_context_shortcut(locale)
                );
                let mut conditions = std::collections::HashSet::new();
                for route in &binding.routes {
                    anyhow::ensure!(
                        valid_shortcut_condition(&route.action_id, 512)
                            && route.title.len()
                                <= anywhere_protocol::policy::MAX_ACTION_TITLE_BYTES as usize
                            && route
                                .app_bundle_id
                                .as_deref()
                                .is_none_or(|value| valid_shortcut_condition(value, 256))
                            && route
                                .context_type
                                .as_deref()
                                .is_none_or(|value| valid_shortcut_condition(value, 128)),
                        l10n::invalid_context_shortcut_rule(locale)
                    );
                    anyhow::ensure!(
                        conditions.insert((&route.app_bundle_id, &route.context_type)),
                        l10n::distinct_context_shortcut_conditions(locale)
                    );
                }
            }
        }
        Ok(())
    }

    pub fn file_search_engine(&self) -> anywhere_platform::FileSearchEngine {
        self.file_search_engine
    }
}

fn valid_shortcut_condition(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

pub fn reserved_action_shortcut(shortcut: &str) -> bool {
    use global_hotkey::hotkey::Code;
    let Ok(hotkey) = shortcut.parse::<global_hotkey::hotkey::HotKey>() else {
        return false;
    };
    hotkey.mods == global_hotkey::hotkey::Modifiers::META
        && matches!(
            hotkey.key,
            Code::KeyA
                | Code::KeyC
                | Code::KeyK
                | Code::KeyM
                | Code::KeyQ
                | Code::KeyR
                | Code::KeyW
                | Code::KeyX
                | Code::KeyY
                | Code::KeyZ
        )
}

fn equivalent_hotkeys(left: &str, right: &str) -> bool {
    let (Ok(left), Ok(right)) = (
        left.parse::<global_hotkey::hotkey::HotKey>(),
        right.parse::<global_hotkey::hotkey::HotKey>(),
    ) else {
        return false;
    };
    left.id() == right.id()
}

fn deserialize_action_shortcuts<'de, D>(
    deserializer: D,
) -> Result<Vec<ActionShortcutBinding>, D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<serde_json::Value>::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .filter_map(|value| {
            let object = value.as_object()?;
            let action_id = object.get("actionId")?.as_str()?.to_owned();
            let title = object.get("title")?.as_str()?.to_owned();
            let shortcut = object
                .get("shortcut")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .or_else(|| legacy_action_shortcut(object))?;
            (!action_id.is_empty() && !shortcut.is_empty() && !title.is_empty()).then_some(
                ActionShortcutBinding {
                    routes: shortcut_routes(object),
                    action_id,
                    title,
                    shortcut,
                    global: object
                        .get("global")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                },
            )
        })
        .collect())
}

fn shortcut_routes(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Vec<ActionShortcutRoute> {
    let Some(routes) = object.get("routes").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    routes
        .iter()
        .filter_map(|value| {
            let object = value.as_object()?;
            Some(ActionShortcutRoute {
                action_id: object.get("actionId")?.as_str()?.to_owned(),
                title: object.get("title")?.as_str()?.to_owned(),
                app_bundle_id: object
                    .get("appBundleId")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                app_name: object
                    .get("appName")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                context_type: object
                    .get("contextType")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
            })
        })
        .collect()
}

fn legacy_action_shortcut(object: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    let key_code = object.get("keyCode")?.as_u64()?;
    let modifiers = object.get("modifiers")?.as_u64()?;
    let key = if (4..=29).contains(&key_code) {
        format!("Key{}", char::from(b'A' + (key_code - 4) as u8))
    } else {
        match key_code {
            40 => "Enter".to_owned(),
            41 => "Escape".to_owned(),
            43 => "Tab".to_owned(),
            44 => "Space".to_owned(),
            _ => return None,
        }
    };
    let labels = [
        (modifiers & 1 != 0, "control"),
        (modifiers & 2 != 0, "alt"),
        (modifiers & 4 != 0, "shift"),
        (modifiers & 8 != 0, "meta"),
    ];
    let modifiers = labels
        .into_iter()
        .filter_map(|(enabled, label)| enabled.then_some(label))
        .collect::<Vec<_>>();
    (!modifiers.is_empty()).then(|| format!("{}+{key}", modifiers.join("+")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_preferences_use_authorization_safe_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let preferences = HostPreferences::load(&directory.path().join("settings.json")).unwrap();
        assert_eq!(
            preferences.current_app_shortcut,
            DEFAULT_CURRENT_APP_SHORTCUT
        );
        assert_eq!(preferences.universal_shortcut, DEFAULT_UNIVERSAL_SHORTCUT);
        assert_eq!(preferences.ui_theme, UiTheme::System);
        assert!(!preferences.file_search_enabled);
        assert!(preferences.file_search_roots.is_empty());
        assert!(preferences.validate().is_ok());
    }

    #[test]
    fn theme_defaults_for_legacy_settings_and_round_trips_all_modes() {
        use crate::platform::WindowAppearance;
        let legacy: HostPreferences = serde_json::from_str(r#"{"uiLanguage":"zh"}"#).unwrap();
        assert_eq!(legacy.ui_theme, UiTheme::System);
        for (mode, saved, appearance) in [
            (UiTheme::System, "system", None),
            (UiTheme::Light, "light", Some(WindowAppearance::Light)),
            (UiTheme::Dark, "dark", Some(WindowAppearance::Dark)),
        ] {
            let preferences = HostPreferences {
                ui_theme: mode,
                ..legacy.clone()
            };
            let value = serde_json::to_value(&preferences).unwrap();
            assert_eq!(value["uiTheme"], saved);
            assert_eq!(
                serde_json::from_value::<HostPreferences>(value).unwrap(),
                preferences
            );
            assert_eq!(mode.appearance(), appearance);
        }
    }

    #[test]
    fn legacy_default_shortcuts_migrate_without_overwriting_custom_pairs() {
        let directory = tempfile::tempdir().unwrap();
        let legacy_path = directory.path().join("legacy.json");
        std::fs::write(
            &legacy_path,
            serde_json::json!({
                "currentAppShortcut": LEGACY_CURRENT_APP_SHORTCUT,
                "universalShortcut": LEGACY_UNIVERSAL_SHORTCUT
            })
            .to_string(),
        )
        .unwrap();
        let migrated = HostPreferences::load(&legacy_path).unwrap();
        assert_eq!(migrated.current_app_shortcut, DEFAULT_CURRENT_APP_SHORTCUT);
        assert_eq!(migrated.universal_shortcut, DEFAULT_UNIVERSAL_SHORTCUT);

        let custom_path = directory.path().join("custom.json");
        std::fs::write(
            &custom_path,
            serde_json::json!({
                "currentAppShortcut": "control+shift+Space",
                "universalShortcut": LEGACY_UNIVERSAL_SHORTCUT
            })
            .to_string(),
        )
        .unwrap();
        let custom = HostPreferences::load(&custom_path).unwrap();
        assert_eq!(custom.current_app_shortcut, "control+shift+Space");
        assert_eq!(custom.universal_shortcut, LEGACY_UNIVERSAL_SHORTCUT);
    }

    #[test]
    fn oversized_preferences_are_rejected_without_reading_the_full_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_PREFERENCES_BYTES as u64 + 1).unwrap();
        let error = HostPreferences::load(&path).unwrap_err();
        assert_eq!(
            error.to_string(),
            l10n::settings_too_large(UiLanguage::System.locale())
        );
    }

    #[test]
    fn unsafe_search_scope_is_rejected() {
        let mut preferences = HostPreferences::default();
        preferences.file_search_roots.push("relative".to_owned());
        assert!(preferences.validate().is_err());
        let absolute_scope = if cfg!(windows) {
            r"C:\tmp\..\scope"
        } else {
            "/tmp/../scope"
        };
        preferences.file_search_roots = vec![absolute_scope.to_owned()];
        assert!(preferences.validate().is_ok());
        preferences.file_search_excluded_names = vec!["../cache".to_owned()];
        assert!(preferences.validate().is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_application_search_paths_accept_drive_roots() {
        let preferences = HostPreferences {
            application_search_paths: [r"C:\", r"D:\", r"E:\"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            ..HostPreferences::default()
        };
        assert!(preferences.validate().is_ok());

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        preferences.save(&path).unwrap();
        let loaded = HostPreferences::load(&path).unwrap();
        assert_eq!(
            loaded.application_search_paths,
            preferences.application_search_paths
        );
    }

    #[test]
    fn concurrent_saves_use_independent_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let mut writers = Vec::new();
        for index in 0..8 {
            let path = path.clone();
            writers.push(std::thread::spawn(move || {
                let preferences = HostPreferences {
                    universal_shortcut: format!("meta+control+Key{}", char::from(b'A' + index)),
                    ..HostPreferences::default()
                };
                preferences.save(&path)
            }));
        }
        for writer in writers {
            writer.join().unwrap().unwrap();
        }
        let loaded = HostPreferences::load(&path).unwrap();
        assert!(loaded.validate().is_ok());
        assert!(std::fs::read_dir(directory.path())
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")));
    }

    #[test]
    fn legacy_action_shortcuts_migrate_and_invalid_entries_are_skipped() {
        let preferences: HostPreferences = serde_json::from_value(serde_json::json!({
            "actionShortcuts": [
                {
                    "actionId": "core.url.open",
                    "title": "Open URL",
                    "keyCode": 14,
                    "modifiers": 10,
                    "global": true
                },
                {
                    "actionId": "",
                    "title": "Broken",
                    "keyCode": 14,
                    "modifiers": 8
                }
            ]
        }))
        .unwrap();
        assert_eq!(
            preferences.action_shortcuts,
            vec![ActionShortcutBinding {
                action_id: "core.url.open".to_owned(),
                title: "Open URL".to_owned(),
                shortcut: "alt+meta+KeyK".to_owned(),
                global: true,
                routes: Vec::new(),
            }]
        );
    }

    #[test]
    fn action_shortcuts_reject_duplicate_chords_and_launcher_conflicts() {
        let mut preferences = HostPreferences {
            action_shortcuts: vec![ActionShortcutBinding {
                action_id: "core.url.open".to_owned(),
                title: "Open URL".to_owned(),
                shortcut: "meta+shift+KeyU".to_owned(),
                global: true,
                routes: Vec::new(),
            }],
            ..HostPreferences::default()
        };
        assert!(preferences.validate().is_ok());

        preferences.action_shortcuts[0].shortcut = "alt+KeyU".to_owned();
        assert!(
            preferences.validate().is_ok(),
            "Option is sufficient as an action shortcut modifier"
        );
        preferences.action_shortcuts[0].shortcut = "meta+shift+KeyU".to_owned();

        preferences.action_shortcuts.push(ActionShortcutBinding {
            action_id: "core.files.search".to_owned(),
            title: "Find files".to_owned(),
            shortcut: "meta+shift+KeyU".to_owned(),
            global: true,
            routes: Vec::new(),
        });
        assert!(preferences.validate().is_err());

        preferences.action_shortcuts.truncate(1);
        preferences.action_shortcuts[0].shortcut = preferences.universal_shortcut.clone();
        assert!(preferences.validate().is_err());
    }

    #[test]
    fn context_shortcuts_migrate_routes_and_require_distinct_conditions() {
        let mut preferences: HostPreferences = serde_json::from_value(serde_json::json!({
            "actionShortcuts": [{
                "actionId": "intent:terminal",
                "title": "Open terminal",
                "keyCode": 14,
                "modifiers": 10,
                "global": true,
                "routes": [{
                    "actionId": "developer.terminal.open",
                    "title": "Open terminal",
                    "appBundleId": "com.apple.finder",
                    "appName": "Finder",
                    "contextType": "core.file"
                }]
            }]
        }))
        .unwrap();

        assert_eq!(
            preferences.action_shortcuts[0].routes,
            vec![ActionShortcutRoute {
                action_id: "developer.terminal.open".to_owned(),
                title: "Open terminal".to_owned(),
                app_bundle_id: Some("com.apple.finder".to_owned()),
                app_name: Some("Finder".to_owned()),
                context_type: Some("core.file".to_owned()),
            }]
        );
        assert!(preferences.validate().is_ok());

        let duplicate = preferences.action_shortcuts[0].routes[0].clone();
        preferences.action_shortcuts[0].routes.push(duplicate);
        assert!(preferences.validate().is_err());
    }
    #[test]
    fn validation_uses_the_selected_language() {
        let mut preferences = HostPreferences {
            ui_language: UiLanguage::Zh,
            file_search_enabled: true,
            ..Default::default()
        };
        assert_eq!(
            preferences.validate().unwrap_err().to_string(),
            l10n::choose_authorized_directories(l10n::Locale::Zh)
        );
        preferences.ui_language = UiLanguage::En;
        assert_eq!(
            preferences.validate().unwrap_err().to_string(),
            l10n::choose_authorized_directories(l10n::Locale::En)
        );
    }
}
