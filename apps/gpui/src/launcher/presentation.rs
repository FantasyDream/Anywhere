use anywhere_protocol::{
    ActionResult, CandidateKind, ContextCaptureStatus, ContextSource, FileObjectKind,
    FileSearchFilter,
};
use gpui_kit::{
    assets::IconName,
    component::input::{InputEditsEvent, InputState, TextareaState},
    AppContext, Entity, SharedString, Window,
};

use crate::{
    host::{ShortcutCatalogAction, UiLanguage},
    l10n::{self, Locale},
    ui::theme,
};

pub(crate) fn locale_for_language(language: UiLanguage) -> Locale {
    language.locale()
}

pub(crate) fn readable_file_path(path: &str) -> &str {
    anywhere_platform::paths::display_path(std::path::Path::new(path))
        .to_str()
        .unwrap_or(path)
}

pub(crate) fn readable_extended_path(value: &str) -> Option<String> {
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        return Some(format!(r"\\{unc}"));
    }
    value.strip_prefix(r"\\?\").map(|path| path.to_owned())
}

pub(crate) fn readable_search_roots(roots: &[String]) -> Vec<String> {
    roots
        .iter()
        .map(|root| {
            let readable = readable_file_path(root);
            readable
                .strip_prefix(r"\\?\UNC\")
                .map(|unc| format!(r"\\{unc}"))
                .unwrap_or_else(|| readable.to_owned())
        })
        .collect()
}

/// Replace the user's home directory with `~` and middle-truncate deep paths so
/// the root anchor and immediate parent directories stay readable.
pub(crate) fn format_path_middle_truncated(path: &str) -> String {
    let trimmed = readable_file_path(path.trim());
    if trimmed.is_empty() {
        return String::new();
    }
    let contracted = std::env::var("HOME")
        .ok()
        .filter(|home| !home.is_empty())
        .and_then(|home| {
            trimmed
                .strip_prefix(&home)
                .filter(|rest| rest.is_empty() || rest.starts_with('/'))
                .map(|rest| format!("~{rest}"))
        })
        .unwrap_or_else(|| trimmed.to_owned());
    let segments: Vec<&str> = contracted
        .split(std::path::is_separator)
        .filter(|part| !part.is_empty())
        .collect();
    if segments.len() <= 2 || segments.len() <= 4 && contracted.chars().count() <= 48 {
        return contracted;
    }
    let leading = if contracted.starts_with('~') {
        if segments.len() >= 2 {
            format!("~/{}", segments[1])
        } else {
            "~".to_owned()
        }
    } else if contracted.starts_with('/') {
        format!("/{}", segments[0])
    } else {
        segments[0].to_owned()
    };
    let tail_count = segments.len().min(2);
    let tail = segments[segments.len() - tail_count..].join(" › ");
    format!("{leading} › … › {tail}")
}

/// Human-friendly byte size formatting (e.g. `512 B`, `14.2 KB`, `2.4 MB`).
pub(crate) fn current_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

pub(crate) fn format_byte_count(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let value = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if value < MB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{:.1} MB", value / MB)
    }
}

/// Human-friendly elapsed duration formatting for Running Tasks.
pub(crate) fn format_duration_ms(
    started_at_ms: i64,
    finished_at_ms: Option<i64>,
    now_ms: i64,
) -> String {
    let end = finished_at_ms.unwrap_or(now_ms);
    let elapsed = (end - started_at_ms).max(0) as u64;
    if elapsed < 1_000 {
        format!("{elapsed} ms")
    } else if elapsed < 60_000 {
        format!("{:.1} s", elapsed as f64 / 1000.0)
    } else {
        let secs = elapsed / 1_000;
        let mins = secs / 60;
        let rem = secs % 60;
        format!("{mins}m {rem}s")
    }
}

/// Human-friendly relative timestamp for Clipboard items.
pub(crate) fn format_relative_time(timestamp_ms: i64, now_ms: i64, locale: Locale) -> String {
    if timestamp_ms <= 0 {
        return String::new();
    }
    let delta_secs = ((now_ms - timestamp_ms) / 1000).max(0);
    match locale {
        Locale::Zh => {
            if delta_secs < 60 {
                "刚刚".to_owned()
            } else if delta_secs < 3600 {
                format!("{} 分钟前", delta_secs / 60)
            } else if delta_secs < 86_400 {
                format!("{} 小时前", delta_secs / 3600)
            } else if delta_secs < 172_800 {
                "昨天".to_owned()
            } else {
                format!("{} 天前", delta_secs / 86_400)
            }
        }
        Locale::En => {
            if delta_secs < 60 {
                "Just now".to_owned()
            } else if delta_secs < 3600 {
                format!("{}m ago", delta_secs / 60)
            } else if delta_secs < 86_400 {
                format!("{}h ago", delta_secs / 3600)
            } else if delta_secs < 172_800 {
                "Yesterday".to_owned()
            } else {
                format!("{}d ago", delta_secs / 86_400)
            }
        }
    }
}

/// Parse `#RGB`, `#RRGGBB`, `#RRGGBBAA`, or `rgb(r, g, b)` literals into an `Hsla` swatch.
pub(crate) fn detect_color_swatch(text: &str) -> Option<gpui_kit::Hsla> {
    let trimmed = text.trim();
    if let Some(hex) = trimmed.strip_prefix('#') {
        let rgba_u32 = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                u32::from_be_bytes([r, g, b, 0xff])
            }
            6 => {
                let rgb = u32::from_str_radix(hex, 16).ok()?;
                (rgb << 8) | 0xff
            }
            8 => u32::from_str_radix(hex, 16).ok()?,
            _ => return None,
        };
        return Some(gpui_kit::rgba(rgba_u32).into());
    }
    let inner = trimmed
        .strip_prefix("rgb(")
        .or_else(|| trimmed.strip_prefix("RGB("))?
        .strip_suffix(')')?;
    let parts: Vec<u8> = inner
        .split(',')
        .map(|part| part.trim().parse::<u8>().ok())
        .collect::<Option<Vec<_>>>()?;
    if parts.len() == 3 {
        let rgba_u32 = u32::from_be_bytes([parts[0], parts[1], parts[2], 0xff]);
        return Some(gpui_kit::rgba(rgba_u32).into());
    }
    None
}

pub(crate) fn lines(value: &str) -> Vec<String> {
    unique_trimmed(value.split('\n'))
}

pub(crate) fn normalized_shortcut(
    key: &str,
    command: bool,
    control: bool,
    alt: bool,
    shift: bool,
) -> Option<String> {
    if !command && !control && !alt {
        return None;
    }
    let key = match key {
        "space" => "Space".to_owned(),
        "enter" | "return" => "Enter".to_owned(),
        "escape" => "Escape".to_owned(),
        "tab" => "Tab".to_owned(),
        "backspace" => "Backspace".to_owned(),
        "delete" => "Delete".to_owned(),
        "up" | "arrowup" => "ArrowUp".to_owned(),
        "down" | "arrowdown" => "ArrowDown".to_owned(),
        "left" | "arrowleft" => "ArrowLeft".to_owned(),
        "right" | "arrowright" => "ArrowRight".to_owned(),
        value if value.len() == 1 && value.as_bytes()[0].is_ascii_alphabetic() => {
            format!("Key{}", value.to_ascii_uppercase())
        }
        value if value.len() == 1 && value.as_bytes()[0].is_ascii_digit() => {
            format!("Digit{value}")
        }
        value
            if value.len() >= 2
                && value.len() <= 3
                && value.as_bytes()[0].eq_ignore_ascii_case(&b'f')
                && value[1..].bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            format!("F{}", &value[1..])
        }
        _ => return None,
    };
    let parsed = [
        (control, "control"),
        (alt, "alt"),
        (shift, "shift"),
        (command, "meta"),
    ]
    .into_iter()
    .filter_map(|(enabled, label)| enabled.then_some(label))
    .collect::<Vec<_>>();
    Some(format!("{}+{key}", parsed.join("+")))
}

pub(crate) fn reserved_action_shortcut(shortcut: &str) -> bool {
    crate::host::reserved_action_shortcut(shortcut)
}

pub(crate) fn excluded_app_ids(value: &str) -> Vec<String> {
    unique_trimmed(value.split(['\n', ',']))
}

pub(crate) fn unique_trimmed<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        let value = value.trim();
        if !value.is_empty() && !result.iter().any(|existing| existing == value) {
            result.push(value.to_owned());
        }
    }
    result
}

pub(crate) fn set_input_value<C: AppContext>(
    input: &Entity<InputState>,
    value: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut C,
) {
    let value = value.into();
    input.update(cx, |input, cx| input.set_value(value, window, cx));
}

pub(crate) fn set_textarea_value<C: AppContext>(
    input: &Entity<TextareaState>,
    value: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut C,
) {
    let value = value.into();
    input.update(cx, |input, cx| input.set_value(value, window, cx));
}

pub(crate) fn file_filter_id(filter: FileSearchFilter) -> &'static str {
    match filter {
        FileSearchFilter::All => "all",
        FileSearchFilter::Files => "files",
        FileSearchFilter::Folders => "folders",
        FileSearchFilter::Documents => "documents",
        FileSearchFilter::Pdf => "pdf",
        FileSearchFilter::Images => "images",
        FileSearchFilter::Audio => "audio",
        FileSearchFilter::Video => "video",
        FileSearchFilter::Archives => "archives",
        FileSearchFilter::Code => "code",
    }
}

#[derive(Clone, Copy)]
pub(crate) struct RowIconAppearance {
    pub(crate) glyph: IconName,
    pub(crate) tint: fn() -> gpui_kit::Hsla,
    pub(crate) mark: Option<IconName>,
    pub(crate) stamp: Option<&'static str>,
}

fn plain_icon(glyph: IconName, tint: fn() -> gpui_kit::Hsla) -> RowIconAppearance {
    RowIconAppearance {
        glyph,
        tint,
        mark: None,
        stamp: None,
    }
}

fn marked_icon(glyph: IconName, tint: fn() -> gpui_kit::Hsla, mark: IconName) -> RowIconAppearance {
    RowIconAppearance {
        glyph,
        tint,
        mark: Some(mark),
        stamp: None,
    }
}

fn stamped_icon(
    glyph: IconName,
    tint: fn() -> gpui_kit::Hsla,
    stamp: &'static str,
) -> RowIconAppearance {
    RowIconAppearance {
        glyph,
        tint,
        mark: None,
        stamp: Some(stamp),
    }
}

pub(crate) fn file_icon_appearance(name: &str, kind: Option<FileObjectKind>) -> RowIconAppearance {
    if kind == Some(FileObjectKind::Directory) {
        return plain_icon(IconName::Folder, theme::icon_blue);
    }

    use super::file_icons::{kind as icon_kind, FileIconKind::*};
    match icon_kind(name) {
        Some(Pdf) => stamped_icon(IconName::FileText, theme::icon_coral, "PDF"),
        Some(Document) => stamped_icon(IconName::FileText, theme::icon_blue, "DOC"),
        Some(Text) => stamped_icon(IconName::Type, theme::icon_neutral, "TXT"),
        Some(Spreadsheet) => plain_icon(IconName::FileSpreadsheet, theme::icon_green),
        Some(Presentation) => plain_icon(IconName::Presentation, theme::icon_amber),
        Some(Image) => plain_icon(IconName::Image, theme::icon_blue),
        Some(Audio) => plain_icon(IconName::Music, theme::icon_pink),
        Some(Video) => plain_icon(IconName::FileVideoCamera, theme::icon_purple),
        Some(Archive) => plain_icon(IconName::FileArchive, theme::icon_amber),
        Some(Package) => plain_icon(IconName::Package, theme::icon_neutral),
        Some(Book) => plain_icon(IconName::BookOpen, theme::icon_green),
        Some(Font) => plain_icon(IconName::Type, theme::icon_pink),
        Some(Config) => plain_icon(IconName::FileCog, theme::icon_amber),
        Some(Code) => plain_icon(IconName::FileCode, theme::icon_purple),
        None => plain_icon(IconName::File, theme::icon_neutral),
    }
}

pub(crate) fn action_icon_appearance(key: &str, kind: CandidateKind) -> RowIconAppearance {
    match key.to_ascii_lowercase().as_str() {
        "terminal" => plain_icon(IconName::SquareTerminal, theme::icon_green),
        "folder" => plain_icon(IconName::Folder, theme::icon_blue),
        "folder-open" => plain_icon(IconName::FolderOpen, theme::icon_blue),
        "file" => plain_icon(IconName::File, theme::icon_neutral),
        "file-open" => marked_icon(IconName::File, theme::icon_blue, IconName::ExternalLink),
        "file-reveal" => marked_icon(IconName::Folder, theme::icon_blue, IconName::Search),
        "copy" => plain_icon(IconName::Copy, theme::icon_neutral),
        "copy-path" => marked_icon(IconName::Folder, theme::icon_blue, IconName::Copy),
        "copy-selection" => stamped_icon(IconName::Files, theme::icon_purple, "{}"),
        "copy-url" => marked_icon(IconName::Link, theme::icon_blue, IconName::Copy),
        "globe" => plain_icon(IconName::Globe, theme::icon_blue),
        "code" => plain_icon(IconName::Code, theme::icon_purple),
        "git" | "git-branches" => plain_icon(IconName::GitBranch, theme::icon_coral),
        "git-status" => plain_icon(IconName::ListChecks, theme::icon_coral),
        "git-log" => plain_icon(IconName::Clock3, theme::icon_coral),
        "git-diff" => plain_icon(IconName::Diff, theme::icon_coral),
        "git-staged" => plain_icon(IconName::ListChecks, theme::icon_green),
        "copy-branch" => marked_icon(IconName::GitBranch, theme::icon_coral, IconName::Copy),
        "copy-commit" => marked_icon(
            IconName::GitCommitHorizontal,
            theme::icon_coral,
            IconName::Copy,
        ),
        "copy-short-commit" => stamped_icon(IconName::GitCommitHorizontal, theme::icon_coral, "…"),
        "search" => plain_icon(IconName::Search, theme::icon_neutral),
        "file-search" => plain_icon(IconName::FileSearch, theme::icon_blue),
        "clipboard" => plain_icon(IconName::Clipboard, theme::icon_neutral),
        "clipboard-history" => {
            marked_icon(IconName::Clipboard, theme::icon_purple, IconName::Clock3)
        }
        "clipboard-shelf" => plain_icon(IconName::ClipboardList, theme::icon_amber),
        "menu" => plain_icon(IconName::Menu, theme::icon_neutral),
        "app" => plain_icon(IconName::AppWindow, theme::icon_blue),
        "developer-check" => plain_icon(IconName::FileCheck, theme::icon_green),
        "developer-test" => plain_icon(IconName::FlaskConical, theme::icon_green),
        "developer-build" => plain_icon(IconName::Hammer, theme::icon_amber),
        "developer-package" => plain_icon(IconName::Package, theme::icon_amber),
        "developer-run" => plain_icon(IconName::Play, theme::icon_green),
        "developer-script" => plain_icon(IconName::SquareTerminal, theme::icon_purple),
        _ if kind == CandidateKind::Application => {
            plain_icon(IconName::AppWindow, theme::icon_blue)
        }
        _ => plain_icon(IconName::Command, theme::secondary),
    }
}

pub(crate) fn context_capture_status_label(
    status: ContextCaptureStatus,
    source: ContextSource,
    locale: Locale,
) -> String {
    match status {
        ContextCaptureStatus::Captured => l10n::capture_ready(locale),
        ContextCaptureStatus::Empty => match source {
            ContextSource::FinderSelection => l10n::no_items_selected(locale),
            ContextSource::SelectedText => l10n::no_text_selected(locale),
            ContextSource::BrowserCurrentPage => l10n::no_page_available(locale),
            ContextSource::Clipboard => l10n::no_text_copied(locale),
            _ => l10n::capture_not_read(locale),
        },
        ContextCaptureStatus::PermissionDenied => l10n::permission_needed(locale),
        ContextCaptureStatus::Unsupported => l10n::source_unsupported(locale),
        ContextCaptureStatus::ApplicationChanged => l10n::capture_app_changed(locale),
        ContextCaptureStatus::TimedOut => l10n::timed_out(locale),
        ContextCaptureStatus::TooLarge => l10n::content_too_long(locale),
        ContextCaptureStatus::TooManyItems => l10n::too_many_items(locale),
        ContextCaptureStatus::Unavailable => l10n::source_unavailable(locale),
        ContextCaptureStatus::Failed => l10n::read_failed(locale),
    }
}

pub(crate) fn context_capture_status_detail(
    status: ContextCaptureStatus,
    source: ContextSource,
    locale: Locale,
) -> String {
    match status {
        ContextCaptureStatus::Captured => l10n::last_context_capture(locale),
        ContextCaptureStatus::PermissionDenied => match source {
            ContextSource::FinderSelection if cfg!(target_os = "macos") => {
                l10n::finder_permission_help(locale)
            }
            ContextSource::SelectedText => l10n::selected_text_permission_help(locale),
            ContextSource::BrowserCurrentPage => l10n::browser_permission_help(locale),
            ContextSource::Clipboard => l10n::clipboard_permission_help(locale),
            _ => l10n::permission_needed(locale),
        },
        ContextCaptureStatus::ApplicationChanged => l10n::app_changed_help(locale),
        ContextCaptureStatus::TooManyItems if cfg!(windows) => {
            l10n::windows_too_many_items_help(locale)
        }
        ContextCaptureStatus::TooManyItems => l10n::too_many_items_help(locale),
        _ => String::new(),
    }
}

pub(crate) fn shortcut_action_title(action: &ShortcutCatalogAction, locale: Locale) -> String {
    let language = match locale {
        Locale::En => "en",
        Locale::Zh => "zh",
    };
    action
        .localized_titles
        .get(language)
        .cloned()
        .unwrap_or_else(|| action.title.clone())
}

pub(crate) fn web_permission_label(permission: &str, locale: Locale) -> String {
    if let Some(types) = permission.strip_prefix("context.read:") {
        return l10n::permission_context_read(locale, types);
    }
    if let Some(origins) = permission.strip_prefix("url.open:") {
        return l10n::permission_open_url(locale, origins);
    }
    match permission {
        "current-app" => l10n::permission_current_app(locale),
        "clipboard.read-text" => l10n::permission_read_clipboard(locale),
        "clipboard.write-text" => l10n::permission_write_clipboard(locale),
        "local-storage" => l10n::permission_local_storage(locale),
        "input.read" => l10n::permission_input_read(locale),
        "input.write" => l10n::permission_input_write(locale),
        "frontmost-app" => l10n::permission_frontmost_app(locale),
        "resources.read" => l10n::permission_read_selection(locale),
        "preview.open" => l10n::permission_preview_selection(locale),
        _ => permission.to_owned(),
    }
}

pub(crate) fn action_result_message(result: &ActionResult, locale: Locale) -> String {
    if let Some(error) = &result.error {
        return l10n::action_failed_with(locale, error);
    }
    if let Some(warning) = &result.warning {
        return l10n::action_completed_with_warning(locale, warning);
    }
    if matches!(
        result.view,
        Some(anywhere_protocol::HostView::WebPlugin { .. })
    ) {
        return l10n::opening_web_tool(locale);
    }
    if let Some(output) = result
        .output_text
        .as_deref()
        .map(str::trim)
        .filter(|output| !output.is_empty())
    {
        return output.chars().take(240).collect();
    }
    l10n::action_completed(locale)
}

pub(crate) fn plain_space_insertion(
    event: &InputEditsEvent,
    previous: &str,
) -> Option<(String, usize)> {
    let (_, after) = event.edits().last()?;
    let after = after.to_string();
    if after.len() != previous.len().checked_add(1)? {
        return None;
    }
    let cursor = after
        .bytes()
        .zip(previous.bytes())
        .position(|(after, previous)| after != previous)
        .unwrap_or(previous.len());
    if after.as_bytes().get(cursor) != Some(&b' ')
        || after.as_bytes().get(cursor + 1..) != previous.as_bytes().get(cursor..)
    {
        return None;
    }
    Some((previous.to_owned(), cursor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_path_projection_leaves_ordinary_copy_untouched() {
        assert_eq!(
            readable_extended_path(r"\\?\C:\Users\me\project\package.json").as_deref(),
            Some(r"C:\Users\me\project\package.json")
        );
        assert_eq!(
            readable_extended_path(r"\\?\UNC\server\share\report.txt").as_deref(),
            Some(r"\\server\share\report.txt")
        );
        assert_eq!(readable_extended_path("Run in this project"), None);
        assert_eq!(readable_extended_path(r"C:\Users\me\project"), None);
    }

    #[test]
    fn middle_truncation_keeps_prefix_and_tail_segments() {
        let short = "/Users/me/notes.txt";
        assert_eq!(format_path_middle_truncated(short), short);
        let deep = "/Users/alice/Documents/Projects/Anywhere/crates/anywhere-core/src/lib.rs";
        let truncated = format_path_middle_truncated(deep);
        assert!(truncated.contains('…'));
        assert!(truncated.ends_with("src › lib.rs"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_file_paths_hide_redundant_verbatim_prefixes_and_truncate_by_component() {
        let short = r"\\?\C:\Test\报告";
        assert_eq!(format_path_middle_truncated(short), r"C:\Test\报告");
        let deep = r"\\?\C:\Test\Documents\Projects\Anywhere\crates\anywhere-core\src";
        assert_eq!(
            format_path_middle_truncated(deep),
            "C: › … › anywhere-core › src"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_file_capture_label_and_limit_refer_to_explorer() {
        assert_eq!(
            crate::platform::capabilities::file_selection_label(Locale::En),
            "File Explorer selection"
        );
        assert_eq!(
            context_capture_status_detail(
                ContextCaptureStatus::TooManyItems,
                ContextSource::FinderSelection,
                Locale::En,
            ),
            "Select up to 64 items in File Explorer, then reopen Anywhere.",
        );
    }

    #[test]
    fn formatting_helpers_handle_bytes_durations_and_relative_time() {
        assert_eq!(format_byte_count(512), "512 B");
        assert_eq!(format_byte_count(2048), "2.0 KB");
        assert_eq!(format_duration_ms(1_000, Some(2_500), 5_000), "1.5 s");
        assert_eq!(format_duration_ms(0, Some(125_000), 200_000), "2m 5s");
        assert_eq!(format_relative_time(10_000, 25_000, Locale::En), "Just now");
        assert_eq!(format_relative_time(10_000, 25_000, Locale::Zh), "刚刚");
        assert_eq!(format_relative_time(1_000, 181_000, Locale::En), "3m ago");
    }

    #[test]
    fn color_swatch_parses_hex_and_rgb_literals() {
        assert!(detect_color_swatch("#fff").is_some());
        assert!(detect_color_swatch("#0a84ff").is_some());
        assert!(detect_color_swatch("#0a84ff80").is_some());
        assert!(detect_color_swatch("rgb(10, 132, 255)").is_some());
        assert!(detect_color_swatch("not a color").is_none());
    }
}
