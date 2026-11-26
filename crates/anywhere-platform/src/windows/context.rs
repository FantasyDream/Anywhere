//! Windows desktop context capture: Explorer directory/selection, UIA selected text, browser URL.
use super::shell::Apartment;
use crate::{ApplicationInfo, ContextReadError};
use anyhow::Result;
use std::{
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
};
use windows::{
    core::{w, Interface, PCWSTR, PWSTR},
    Win32::{
        Foundation::HWND,
        System::{
            Com::{CoCreateInstance, CoTaskMemFree, IServiceProvider, CLSCTX_ALL, CLSCTX_INPROC_SERVER},
            Variant::VARIANT,
        },
        UI::{
            Accessibility::{
                CUIAutomation, IUIAutomation, IUIAutomationTextPattern, IUIAutomationValuePattern,
                TreeScope_Descendants, UIA_ControlTypePropertyId, UIA_EditControlTypeId,
                UIA_TextPatternId, UIA_ValuePatternId,
            },
            Shell::{
                IFolderView, IPersistFolder2, IShellBrowser, IShellItem, IShellItemArray,
                IShellWindows, SHCreateItemFromIDList, ShellWindows, SID_STopLevelBrowser,
                SIGDN_FILESYSPATH, SVGIO_SELECTION,
            },
            WindowsAndMessaging::{
                FindWindowExW, GetAncestor, GetForegroundWindow, GetWindowThreadProcessId,
                GA_ROOT,
            },
        },
    },
};

const MAX_SELECTED_FILES: usize = 64;
const MAX_SELECTED_TEXT_CHARS: i32 = 16 * 1024;
const MAX_BROWSER_EDIT_CONTROLS: i32 = 24;

const SUPPORTED_BROWSERS: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "brave.exe",
    "vivaldi.exe",
    "arc.exe",
    "opera.exe",
    "firefox.exe",
    "zen.exe",
];

fn exe_file_name(application: &ApplicationInfo) -> Option<&str> {
    let raw = application
        .bundle_id
        .strip_prefix("windows.exe:")
        .unwrap_or(&application.bundle_id);
    Path::new(raw).file_name()?.to_str()
}

pub(super) fn is_explorer(application: &ApplicationInfo) -> bool {
    exe_file_name(application)
        .is_some_and(|name| name.eq_ignore_ascii_case("explorer.exe"))
}

pub(super) fn browser_supported(application: &ApplicationInfo) -> bool {
    exe_file_name(application).is_some_and(|name| {
        SUPPORTED_BROWSERS
            .iter()
            .any(|candidate| name.eq_ignore_ascii_case(candidate))
    })
}

fn foreground_hwnd_for_pid(expected_pid: i32) -> Option<HWND> {
    if expected_pid <= 0 {
        return None;
    }
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return None;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    (pid == expected_pid as u32).then_some(hwnd)
}

fn take_pwstr(value: PWSTR) -> Option<OsString> {
    if value.is_null() {
        return None;
    }
    let string = OsString::from_wide(unsafe { value.as_wide() });
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    Some(string)
}

fn active_explorer_folder_view(hwnd: HWND) -> Option<IFolderView> {
    let active_tab = unsafe { FindWindowExW(Some(hwnd), None, w!("ShellTabWindowClass"), PCWSTR::null()) }
        .ok()
        .filter(|child| !child.0.is_null());
    let shell_windows: IShellWindows =
        unsafe { CoCreateInstance(&ShellWindows, None, CLSCTX_ALL) }.ok()?;
    let count = unsafe { shell_windows.Count() }.ok()?;
    let mut fallback_view = None;
    for index in 0..count.clamp(0, 64) {
        let Ok(dispatch) = (unsafe { shell_windows.Item(&VARIANT::from(index)) }) else {
            continue;
        };
        let Ok(provider): windows::core::Result<IServiceProvider> = dispatch.cast() else {
            continue;
        };
        let Ok(browser): windows::core::Result<IShellBrowser> =
            (unsafe { provider.QueryService(&SID_STopLevelBrowser) })
        else {
            continue;
        };
        let Ok(browser_hwnd) = (unsafe { browser.GetWindow() }) else {
            continue;
        };
        let root = unsafe { GetAncestor(browser_hwnd, GA_ROOT) };
        let exact_match = active_tab.is_some_and(|tab| browser_hwnd == tab) || browser_hwnd == hwnd;
        let same_window = root == hwnd || browser_hwnd == hwnd;
        if !exact_match && !same_window {
            continue;
        }
        let Ok(view) = (unsafe { browser.QueryActiveShellView() }) else {
            continue;
        };
        let Ok(folder_view): windows::core::Result<IFolderView> = view.cast() else {
            continue;
        };
        if exact_match {
            return Some(folder_view);
        }
        if fallback_view.is_none() {
            fallback_view = Some(folder_view);
        }
    }
    fallback_view
}

pub(super) fn application_directory(application: &ApplicationInfo) -> Result<Option<PathBuf>> {
    if !is_explorer(application) {
        return Ok(None);
    }
    let Some(hwnd) = foreground_hwnd_for_pid(application.pid) else {
        return Ok(None);
    };
    let Ok(_apartment) = Apartment::open() else {
        return Ok(None);
    };
    let Some(folder_view) = active_explorer_folder_view(hwnd) else {
        return Ok(None);
    };
    let Ok(persist): windows::core::Result<IPersistFolder2> = (unsafe { folder_view.GetFolder() })
    else {
        return Ok(None);
    };
    let Ok(pidl) = (unsafe { persist.GetCurFolder() }) else {
        return Ok(None);
    };
    if pidl.is_null() {
        return Ok(None);
    }
    let item: windows::core::Result<IShellItem> = unsafe { SHCreateItemFromIDList(pidl) };
    unsafe { CoTaskMemFree(Some(pidl.cast())) };
    let Ok(item) = item else {
        return Ok(None);
    };
    let Ok(raw) = (unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }) else {
        return Ok(None);
    };
    let Some(path) = take_pwstr(raw).map(PathBuf::from) else {
        return Ok(None);
    };
    Ok((path.is_absolute() && path.is_dir()).then_some(path))
}

pub(super) fn selected_file_paths(application: &ApplicationInfo) -> Result<Vec<PathBuf>> {
    if !is_explorer(application) {
        return Ok(Vec::new());
    }
    let Some(hwnd) = foreground_hwnd_for_pid(application.pid) else {
        return Ok(Vec::new());
    };
    let Ok(_apartment) = Apartment::open() else {
        return Ok(Vec::new());
    };
    let Some(folder_view) = active_explorer_folder_view(hwnd) else {
        return Ok(Vec::new());
    };
    let Ok(items): windows::core::Result<IShellItemArray> =
        (unsafe { folder_view.Items(SVGIO_SELECTION) })
    else {
        return Ok(Vec::new());
    };
    let count = unsafe { items.GetCount() }.unwrap_or(0) as usize;
    if count > MAX_SELECTED_FILES {
        return Err(ContextReadError::TooManyItems.into());
    }
    let mut paths = Vec::with_capacity(count);
    for index in 0..count {
        let Ok(item) = (unsafe { items.GetItemAt(index as u32) }) else {
            continue;
        };
        let Ok(raw) = (unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }) else {
            continue;
        };
        if let Some(path) = take_pwstr(raw).map(PathBuf::from) {
            if path.is_absolute() && path.exists() {
                paths.push(path);
            }
        }
    }
    Ok(paths)
}

pub(super) fn selected_text(application: &ApplicationInfo) -> Result<Option<String>> {
    if application.pid <= 0 {
        return Ok(None);
    }
    if foreground_hwnd_for_pid(application.pid).is_none() {
        return Ok(None);
    }
    let Ok(_apartment) = Apartment::open() else {
        return Ok(None);
    };
    let Ok(automation): windows::core::Result<IUIAutomation> =
        (unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) })
    else {
        return Ok(None);
    };
    let Ok(focused) = (unsafe { automation.GetFocusedElement() }) else {
        return Ok(None);
    };
    let Ok(pid) = (unsafe { focused.CurrentProcessId() }) else {
        return Ok(None);
    };
    if pid != application.pid {
        return Ok(None);
    }
    let Ok(pattern): windows::core::Result<IUIAutomationTextPattern> =
        (unsafe { focused.GetCurrentPatternAs(UIA_TextPatternId) })
    else {
        return Ok(None);
    };
    let Ok(ranges) = (unsafe { pattern.GetSelection() }) else {
        return Ok(None);
    };
    let count = unsafe { ranges.Length() }.unwrap_or(0);
    if count <= 0 {
        return Ok(None);
    }
    let mut collected = String::new();
    for index in 0..count.min(8) {
        let Ok(range) = (unsafe { ranges.GetElement(index) }) else {
            continue;
        };
        let Ok(bstr) = (unsafe { range.GetText(MAX_SELECTED_TEXT_CHARS) }) else {
            continue;
        };
        let part = bstr.to_string();
        if part.is_empty() {
            continue;
        }
        if !collected.is_empty() {
            collected.push('\n');
        }
        collected.push_str(&part);
        if collected.len() > MAX_SELECTED_TEXT_CHARS as usize {
            return Err(ContextReadError::TooLarge.into());
        }
    }
    if foreground_hwnd_for_pid(application.pid).is_none() {
        return Ok(None);
    }
    Ok((!collected.trim().is_empty()).then_some(collected))
}

pub(super) fn browser_current_url(application: &ApplicationInfo) -> Result<Option<String>> {
    if !browser_supported(application) {
        return Ok(None);
    }
    let Some(hwnd) = foreground_hwnd_for_pid(application.pid) else {
        return Ok(None);
    };
    let Ok(_apartment) = Apartment::open() else {
        return Ok(None);
    };
    let Ok(automation): windows::core::Result<IUIAutomation> =
        (unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) })
    else {
        return Ok(None);
    };
    let Ok(window) = (unsafe { automation.ElementFromHandle(hwnd) }) else {
        return Ok(None);
    };
    let Ok(condition) = (unsafe {
        automation.CreatePropertyCondition(
            UIA_ControlTypePropertyId,
            &VARIANT::from(UIA_EditControlTypeId.0),
        )
    }) else {
        return Ok(None);
    };
    let Ok(edits) = (unsafe { window.FindAll(TreeScope_Descendants, &condition) }) else {
        return Ok(None);
    };
    let count = unsafe { edits.Length() }.unwrap_or(0);
    for index in 0..count.min(MAX_BROWSER_EDIT_CONTROLS) {
        let Ok(element) = (unsafe { edits.GetElement(index) }) else {
            continue;
        };
        let Ok(value_pattern): windows::core::Result<IUIAutomationValuePattern> =
            (unsafe { element.GetCurrentPatternAs(UIA_ValuePatternId) })
        else {
            continue;
        };
        let Ok(raw) = (unsafe { value_pattern.CurrentValue() }) else {
            continue;
        };
        let text = raw.to_string();
        if let Some(url) = normalize_browser_url(&text) {
            return Ok(Some(url));
        }
    }
    Ok(None)
}

fn normalize_browser_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 4096 || trimmed.chars().any(char::is_whitespace) {
        return None;
    }
    if trimmed.contains("://") {
        let parsed = url::Url::parse(trimmed).ok()?;
        return (matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some())
            .then(|| parsed.to_string());
    }
    if trimmed.starts_with('/') {
        return None;
    }
    let host_part = trimmed.split('/').next().unwrap_or_default();
    let (host_only, port) = match host_part.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (host_part, None),
    };
    if let Some(port) = port {
        if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    let plausible_host = host_only.eq_ignore_ascii_case("localhost")
        || (host_only.contains('.')
            && !host_only.starts_with('.')
            && !host_only.ends_with('.'));
    if !plausible_host {
        return None;
    }
    let candidate = format!("https://{trimmed}");
    let parsed = url::Url::parse(&candidate).ok()?;
    (parsed.scheme() == "https" && parsed.host_str().is_some()).then(|| parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_windows_explorer_and_supported_browsers() {
        let app = |bundle_id: &str| ApplicationInfo {
            bundle_id: bundle_id.into(),
            name: "test".into(),
            pid: 1,
            instance_id: None,
        };
        assert!(is_explorer(&app(r"windows.exe:c:\windows\explorer.exe")));
        assert!(!is_explorer(&app(r"windows.exe:c:\windows\notepad.exe")));
        assert!(browser_supported(&app(
            r"windows.exe:c:\program files\microsoft\edge\application\msedge.exe"
        )));
        assert!(browser_supported(&app(
            r"windows.exe:c:\program files\google\chrome\application\chrome.exe"
        )));
        assert!(!browser_supported(&app(
            r"windows.exe:c:\windows\explorer.exe"
        )));
    }

    #[test]
    fn normalizes_and_validates_browser_address_bar_values() {
        assert_eq!(
            normalize_browser_url("https://example.com/docs?q=1"),
            Some("https://example.com/docs?q=1".into())
        );
        assert_eq!(
            normalize_browser_url("github.com/openai/codex"),
            Some("https://github.com/openai/codex".into())
        );
        assert_eq!(
            normalize_browser_url("localhost:3000/test"),
            Some("https://localhost:3000/test".into())
        );
        assert_eq!(normalize_browser_url("chrome://settings"), None);
        assert_eq!(normalize_browser_url("search query with spaces"), None);
        assert_eq!(normalize_browser_url("justaword"), None);
    }
}
