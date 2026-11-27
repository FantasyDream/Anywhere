use crate::l10n::{self, Locale};
use anywhere_platform::FileSearchEngine;

pub(crate) const DESKTOP_CONTEXT_CAPTURE: bool = cfg!(any(target_os = "macos", windows));
pub(crate) const WEB_TOOLS: bool = cfg!(any(target_os = "macos", windows));
pub(crate) const FILE_SEARCH_PRIVACY_SETTINGS: bool = cfg!(target_os = "macos");

pub(crate) const DEFAULT_UNIVERSAL_SHORTCUT: &str = if cfg!(windows) {
    "control+alt+K"
} else {
    "alt+Space"
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WindowControlsStyle {
    /// macOS draws native AppKit traffic lights in the top-left inset.
    NativeTrafficLights,
    /// Windows draws client caption buttons (Minimize, Maximize/Restore, Close) in the top-right.
    TrailingCaptionButtons,
}

pub(crate) const WINDOW_CONTROLS: WindowControlsStyle = if cfg!(target_os = "macos") {
    WindowControlsStyle::NativeTrafficLights
} else {
    WindowControlsStyle::TrailingCaptionButtons
};

pub(crate) fn auxiliary_window_traffic_light_position() -> Option<gpui_kit::Point<gpui_kit::Pixels>>
{
    const TRAFFIC_LIGHT_X: f32 = 14.0;
    const TRAFFIC_LIGHT_Y: f32 = 16.0;
    match WINDOW_CONTROLS {
        WindowControlsStyle::NativeTrafficLights => Some(gpui_kit::point(
            gpui_kit::px(TRAFFIC_LIGHT_X),
            gpui_kit::px(TRAFFIC_LIGHT_Y),
        )),
        WindowControlsStyle::TrailingCaptionButtons => None,
    }
}

pub(crate) const APPLICATION_SEARCH_FILE_EXTENSIONS: &[&str] = if cfg!(windows) {
    &["lnk", "url", "exe"]
} else {
    &[]
};

pub(crate) fn file_search_engines() -> &'static [FileSearchEngine] {
    anywhere_platform::supported_file_search_engines()
}

pub(crate) fn default_file_search_engine() -> FileSearchEngine {
    anywhere_platform::default_file_search_engine()
}

pub(crate) fn file_search_engine_label(engine: FileSearchEngine, locale: Locale) -> String {
    match engine {
        FileSearchEngine::SystemIndex if cfg!(windows) => l10n::search_engine_windows_index(locale),
        FileSearchEngine::SystemIndex => l10n::search_engine_system_index(locale),
        FileSearchEngine::Everything => l10n::search_engine_everything(locale),
        FileSearchEngine::Ripgrep => l10n::search_engine_ripgrep(locale),
        FileSearchEngine::Combined => l10n::search_engine_combined(locale),
        FileSearchEngine::DirectScan => l10n::search_engine_direct_scan(locale),
    }
}

pub(crate) const FILE_SEARCH_PATH_EXAMPLES: &str = if cfg!(windows) {
    r"C:\Users\me\Documents, D:\Projects"
} else {
    "/Users/me/Documents, /Users/me/Projects"
};

pub(crate) fn file_search_access_description(locale: Locale) -> String {
    if cfg!(windows) {
        l10n::windows_file_search_access_message(locale)
    } else {
        l10n::file_search_access_message(locale)
    }
}

pub(crate) fn file_selection_label(locale: Locale) -> String {
    if cfg!(windows) {
        l10n::explorer_selection(locale)
    } else {
        l10n::finder_selection(locale)
    }
}

pub(crate) fn context_sources_description(locale: Locale) -> String {
    if cfg!(windows) {
        l10n::windows_context_description(locale)
    } else {
        l10n::context_description(locale)
    }
}
