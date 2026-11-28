//! Windows desktop context capture: Explorer directory/selection, UIA selected text, browser URL.
use super::shell::{captured_application_window, Apartment};
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
            Com::{
                CoCreateInstance, CoTaskMemFree, IServiceProvider, CLSCTX_ALL, CLSCTX_INPROC_SERVER,
            },
            Variant::VARIANT,
        },
        UI::{
            Accessibility::{
                CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
                IUIAutomationValuePattern, TreeScope_Descendants, UIA_ControlTypePropertyId,
                UIA_DocumentControlTypeId, UIA_EditControlTypeId, UIA_TextPatternId,
                UIA_ValuePatternId,
            },
            Shell::{
                IFolderView, IPersistFolder2, IShellBrowser, IShellItem, IShellItemArray,
                IShellWindows, SHCreateItemFromIDList, SID_STopLevelBrowser, ShellWindows,
                SIGDN_FILESYSPATH, SVGIO_SELECTION,
            },
            WindowsAndMessaging::{
                FindWindowExW, GetAncestor, GetForegroundWindow, GetWindowThreadProcessId,
                IsWindowVisible, GA_ROOT,
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
    exe_file_name(application).is_some_and(|name| name.eq_ignore_ascii_case("explorer.exe"))
}

pub(super) fn browser_supported(application: &ApplicationInfo) -> bool {
    exe_file_name(application).is_some_and(|name| {
        SUPPORTED_BROWSERS
            .iter()
            .any(|candidate| name.eq_ignore_ascii_case(candidate))
    })
}

fn validate_foreground_window(application: &ApplicationInfo, foreground: HWND) -> Result<HWND> {
    if !foreground.0.is_null() && captured_application_window(application) == Some(foreground.0) {
        return Ok(foreground);
    }
    Err(ContextReadError::ApplicationChanged.into())
}

fn foreground_window(application: &ApplicationInfo) -> Result<HWND> {
    let window = captured_application_window(application)
        .map(HWND)
        .ok_or_else(|| anyhow::anyhow!(ContextReadError::ApplicationChanged))?;
    let foreground = unsafe { GetForegroundWindow() };
    let mut foreground_pid = 0;
    if !foreground.0.is_null() {
        unsafe { GetWindowThreadProcessId(foreground, Some(&mut foreground_pid)) };
    }
    if foreground == window {
        return validate_foreground_window(application, foreground);
    }
    if foreground_pid == std::process::id() {
        return Ok(window);
    }
    Err(ContextReadError::ApplicationChanged.into())
}

fn verify_explorer_view(application: &ApplicationInfo, window: HWND, expected: HWND) -> Result<()> {
    if foreground_window(application)? == window && explorer_view_window(window) == Some(expected) {
        return Ok(());
    }
    Err(ContextReadError::ApplicationChanged.into())
}

fn take_pwstr(value: PWSTR) -> Option<OsString> {
    if value.is_null() {
        return None;
    }
    let string = OsString::from_wide(unsafe { value.as_wide() });
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    Some(string)
}

fn explorer_view_window(window: HWND) -> Option<HWND> {
    let mut previous = None;
    let mut visible = None;
    for _ in 0..64 {
        match unsafe {
            FindWindowExW(
                Some(window),
                previous,
                w!("ShellTabWindowClass"),
                PCWSTR::null(),
            )
        } {
            Ok(child) if !child.0.is_null() => {
                if unsafe { IsWindowVisible(child) }.as_bool() && visible.replace(child).is_some() {
                    // During a tab transition the Shell can expose two visible views.
                    return None;
                }
                previous = Some(child);
            }
            _ if previous.is_none() => return Some(window),
            _ => return visible,
        }
    }
    None
}

fn explorer_folder_view(target: HWND) -> Option<IFolderView> {
    let shell_windows: IShellWindows =
        unsafe { CoCreateInstance(&ShellWindows, None, CLSCTX_ALL) }.ok()?;
    let count = unsafe { shell_windows.Count() }.ok()?;
    if !(0..=64).contains(&count) {
        return None;
    }
    let mut matched = None;
    for index in 0..count {
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
        if unsafe { browser.GetWindow() }.ok() != Some(target) {
            continue;
        }
        let Ok(view) = (unsafe { browser.QueryActiveShellView() }) else {
            continue;
        };
        let Ok(folder_view): windows::core::Result<IFolderView> = view.cast() else {
            continue;
        };
        if matched.replace(folder_view).is_some() {
            return None;
        }
    }
    matched
}

pub(super) fn application_directory(application: &ApplicationInfo) -> Result<Option<PathBuf>> {
    if !is_explorer(application) {
        return Ok(None);
    }
    let hwnd = foreground_window(application)?;
    let Some(target) = explorer_view_window(hwnd) else {
        return Ok(None);
    };
    let Ok(_apartment) = Apartment::open() else {
        return Ok(None);
    };
    let Some(folder_view) = explorer_folder_view(target) else {
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
    verify_explorer_view(application, hwnd, target)?;
    Ok((path.is_absolute() && path.is_dir()).then_some(path))
}

pub(super) fn selected_file_paths(application: &ApplicationInfo) -> Result<Vec<PathBuf>> {
    if !is_explorer(application) {
        return Ok(Vec::new());
    }
    let hwnd = foreground_window(application)?;
    let Some(target) = explorer_view_window(hwnd) else {
        return Ok(Vec::new());
    };
    let Ok(_apartment) = Apartment::open() else {
        return Ok(Vec::new());
    };
    let Some(folder_view) = explorer_folder_view(target) else {
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
    verify_explorer_view(application, hwnd, target)?;
    Ok(paths)
}

pub(super) fn selected_text(application: &ApplicationInfo) -> Result<Option<String>> {
    let hwnd = foreground_window(application)?;
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
    if !element_belongs_to_window(&automation, &focused, hwnd) {
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
    foreground_window(application)?;
    let same_selection_control = unsafe {
        automation
            .GetFocusedElement()
            .and_then(|current| automation.CompareElements(&focused, &current))
    };
    if !same_selection_control.is_ok_and(|equal| equal.as_bool()) {
        return Err(ContextReadError::ApplicationChanged.into());
    }
    Ok((!collected.trim().is_empty()).then_some(collected))
}

fn element_belongs_to_window(
    automation: &IUIAutomation,
    element: &IUIAutomationElement,
    window: HWND,
) -> bool {
    let Ok(walker) = (unsafe { automation.RawViewWalker() }) else {
        return false;
    };
    let mut element = element.clone();
    for _ in 0..32 {
        if let Ok(owner) = unsafe { element.CurrentNativeWindowHandle() } {
            if !owner.0.is_null() {
                return (unsafe { GetAncestor(owner, GA_ROOT) }) == window;
            }
        }
        let Ok(parent) = (unsafe { walker.GetParentElement(&element) }) else {
            break;
        };
        element = parent;
    }
    false
}

pub(super) fn browser_current_url(application: &ApplicationInfo) -> Result<Option<String>> {
    if !browser_supported(application) {
        return Ok(None);
    }
    let hwnd = foreground_window(application)?;
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
    let mut address = None;
    for index in 0..count.min(MAX_BROWSER_EDIT_CONTROLS) {
        let Ok(element) = (unsafe { edits.GetElement(index) }) else {
            continue;
        };
        if !is_browser_address_control(&automation, &element, hwnd) {
            continue;
        }
        if address.replace(element).is_some() {
            return Ok(None);
        }
    }
    let Some(address) = address else {
        return Ok(None);
    };
    // An active edit can hold an unsubmitted URL or search text.
    if unsafe { address.CurrentHasKeyboardFocus() }.map_or(true, |focused| focused.as_bool()) {
        return Ok(None);
    }
    let Ok(value_pattern): windows::core::Result<IUIAutomationValuePattern> =
        (unsafe { address.GetCurrentPatternAs(UIA_ValuePatternId) })
    else {
        return Ok(None);
    };
    let Ok(raw) = (unsafe { value_pattern.CurrentValue() }) else {
        return Ok(None);
    };
    foreground_window(application)?;
    Ok(normalize_browser_url(&raw.to_string()))
}

fn address_control_identity(
    automation_id: &str,
    class: &str,
    accelerator: &str,
    name: &str,
) -> bool {
    matches!(automation_id, "addressEditBox" | "urlbar-input")
        || class == "OmniboxViewViews"
        || accelerator.eq_ignore_ascii_case("Ctrl+L")
        || matches!(
            name,
            "Address and search bar" | "地址和搜索栏" | "地址列和搜尋列"
        )
}

fn is_browser_address_control(
    automation: &IUIAutomation,
    element: &IUIAutomationElement,
    window: HWND,
) -> bool {
    let offscreen = unsafe { element.CurrentIsOffscreen() }.map_or(true, |value| value.as_bool());
    if offscreen {
        return false;
    }
    let identity = unsafe {
        address_control_identity(
            &element
                .CurrentAutomationId()
                .map(|value| value.to_string())
                .unwrap_or_default(),
            &element
                .CurrentClassName()
                .map(|value| value.to_string())
                .unwrap_or_default(),
            &element
                .CurrentAcceleratorKey()
                .map(|value| value.to_string())
                .unwrap_or_default(),
            &element
                .CurrentName()
                .map(|value| value.to_string())
                .unwrap_or_default(),
        )
    };
    if !identity {
        return false;
    }
    let Ok(walker) = (unsafe { automation.RawViewWalker() }) else {
        return false;
    };
    let mut ancestor = element.clone();
    for _ in 0..32 {
        let Ok(control_type) = (unsafe { ancestor.CurrentControlType() }) else {
            break;
        };
        if control_type == UIA_DocumentControlTypeId {
            break;
        }
        if unsafe { ancestor.CurrentNativeWindowHandle() }.ok() == Some(window) {
            return true;
        }
        let Ok(parent) = (unsafe { walker.GetParentElement(&ancestor) }) else {
            break;
        };
        ancestor = parent;
    }
    false
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
        || (host_only.contains('.') && !host_only.starts_with('.') && !host_only.ends_with('.'));
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
    fn browser_address_control_identity_does_not_accept_a_generic_edit() {
        assert!(address_control_identity("urlbar-input", "", "", ""));
        assert!(address_control_identity("", "OmniboxViewViews", "", ""));
        assert!(address_control_identity("", "", "Ctrl+L", ""));
        assert!(!address_control_identity(
            "website",
            "Edit",
            "",
            "Website address"
        ));
        assert!(!address_control_identity(
            "",
            "Edit",
            "Ctrl+F",
            "Find in page"
        ));
    }

    #[test]
    fn another_window_from_the_same_process_cannot_supply_context() {
        use super::super::shell::window_application;
        use std::ptr::{null, null_mut};
        use windows_sys::{
            w,
            Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WS_OVERLAPPEDWINDOW},
        };
        struct Window(HWND);
        impl Drop for Window {
            fn drop(&mut self) {
                unsafe { DestroyWindow(self.0 .0) };
            }
        }
        let make_window = || {
            Window(HWND(unsafe {
                CreateWindowExW(
                    0,
                    w!("STATIC"),
                    w!("Context fixture"),
                    WS_OVERLAPPEDWINDOW,
                    0,
                    0,
                    100,
                    100,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    null(),
                )
            }))
        };
        let first = make_window();
        let second = make_window();
        let captured = window_application(first.0 .0).unwrap();
        assert_eq!(captured.pid, window_application(second.0 .0).unwrap().pid);
        assert_eq!(
            validate_foreground_window(&captured, first.0).unwrap(),
            first.0
        );
        let error = validate_foreground_window(&captured, second.0).unwrap_err();
        assert!(matches!(
            error.downcast_ref(),
            Some(ContextReadError::ApplicationChanged)
        ));
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
