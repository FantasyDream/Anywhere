mod backdrop;
mod icon;
pub(super) mod login_item;
pub(super) mod preview;
mod tray;

pub(super) use tray::Tray;

use super::{ApplicationReturnResult, PlatformEvent};
use gpui_kit::{AnyWindowHandle, AsyncApp, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ptr::null,
};
use tokio::sync::mpsc;
use windows_sys::{
    w,
    Win32::{
        Foundation::{
            CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM, LRESULT, POINT,
            RECT, WPARAM,
        },
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, MONITORINFO,
            MONITOR_DEFAULTTONEAREST,
        },
        System::Threading::CreateMutexW,
        UI::{
            HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI},
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                GetCursorPos, GetForegroundWindow, GetPropW, GetWindowLongW, GetWindowRect,
                GetWindowThreadProcessId, IsIconic, IsWindowVisible, IsZoomed, SetForegroundWindow,
                SetPropW, SetWindowLongW, SetWindowPos, ShowWindow, ShowWindowAsync, GWL_EXSTYLE,
                HWND_NOTOPMOST, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
                SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, SW_SHOW,
                USER_DEFAULT_SCREEN_DPI, WM_KEYDOWN, WM_NCDESTROY, WM_SYSKEYDOWN, WS_EX_APPWINDOW,
                WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
            },
        },
    },
};

const BACKDROP_PROPERTY: windows_sys::core::PCWSTR = w!("dev.anywhere.window.acrylic");
const WINDOW_PROPERTY: windows_sys::core::PCWSTR = w!("dev.anywhere.launcher.window");

pub struct InstanceGuard(HANDLE);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseHandle(self.0) };
        }
    }
}

pub fn acquire_instance() -> Option<InstanceGuard> {
    if std::env::var_os("ANYWHERE_DATA_DIR").is_some()
        || std::env::var_os("ANYWHERE_PREVIEW").is_some()
    {
        return Some(InstanceGuard(std::ptr::null_mut()));
    }
    let handle = unsafe { CreateMutexW(null(), 0, w!(r"Local\dev.anywhere.launcher")) };
    if handle.is_null() {
        tracing::error!(error = %std::io::Error::last_os_error(), "windows_instance_mutex_failed");
        return None;
    }
    let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let guard = InstanceGuard(handle);
    if already_running {
        if !background_launch() {
            for _ in 0..20 {
                if tray::restore_existing() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        return None;
    }
    Some(guard)
}

#[derive(Clone, Copy)]
struct NativeWindow {
    hwnd: HWND,
    handle: AnyWindowHandle,
    initial_placement_pending: bool,
}

impl NativeWindow {
    fn identity(self) -> HWND {
        std::ptr::without_provenance_mut(self.handle.window_id().as_u64() as usize)
    }

    fn is_live(self) -> bool {
        (unsafe { GetPropW(self.hwnd, WINDOW_PROPERTY) }) == self.identity()
    }
}

#[derive(Default)]
struct Ui {
    app: Option<AsyncApp>,
    windows: HashMap<String, NativeWindow>,
    previous: Option<(HWND, u32)>,
    events: Option<mpsc::Sender<PlatformEvent>>,
    tray_available: bool,
    shelf_position: Option<POINT>,
}

thread_local! {
    static UI: RefCell<Ui> = RefCell::default();
    static KEY_REPEAT: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn init(app: &gpui_kit::App) {
    UI.with_borrow_mut(|ui| ui.app = Some(app.to_async()));
}

pub(super) fn bind_window(window: &Window, title: &str) {
    let Some(hwnd) = window_hwnd(window) else {
        return;
    };
    icon::apply_window_icons(hwnd);
    install_key_repeat_tracking(hwnd);
    let is_launcher = title == crate::PRODUCT_NAME;
    if is_launcher {
        apply_launcher_window_style(hwnd);
    }
    let installed = install_backdrop(hwnd, window);
    let visible = unsafe { IsWindowVisible(hwnd) } != 0;
    let native = NativeWindow {
        hwnd,
        handle: window.window_handle(),
        initial_placement_pending: is_launcher && !visible,
    };
    if unsafe { SetPropW(hwnd, WINDOW_PROPERTY, native.identity()) } != 0 {
        UI.with_borrow_mut(|ui| {
            ui.windows.retain(|_, window| window.is_live());
            ui.windows.insert(title.to_owned(), native);
        });
    }
    defer(move |_| {
        if !native.is_live() {
            return;
        }
        if let Some(installed) = installed {
            backdrop::finalize_install(native.hwnd, installed);
        }
        if is_launcher && visible {
            position_launcher_at_pointer(native.hwnd);
        }
    });
}

fn window(title: &str) -> Option<NativeWindow> {
    UI.with_borrow(|ui| ui.windows.get(title).copied())
        .filter(|window| window.is_live())
}

pub(super) fn window_exists(title: &str) -> bool {
    window(title).is_some()
}

pub(super) fn configure_main_window(title: &str) -> bool {
    let Some(window) = window(title) else {
        return false;
    };
    apply_launcher_window_style(window.hwnd);
    true
}

pub(super) fn configure_settings_window(title: &str) -> bool {
    let Some(window) = window(title) else {
        return false;
    };
    apply_settings_window_style(window.hwnd);
    true
}

fn apply_launcher_window_style(hwnd: HWND) {
    set_taskbar_and_topmost_style(hwnd, false, true);
}

fn apply_settings_window_style(hwnd: HWND) {
    set_taskbar_and_topmost_style(hwnd, true, false);
}

fn set_taskbar_and_topmost_style(hwnd: HWND, show_in_taskbar: bool, topmost: bool) {
    let current = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) };
    let mut desired = current;
    if show_in_taskbar {
        desired &= !(WS_EX_TOOLWINDOW as i32);
        desired |= WS_EX_APPWINDOW as i32;
    } else {
        desired &= !(WS_EX_APPWINDOW as i32);
        desired |= WS_EX_TOOLWINDOW as i32;
    }
    if topmost {
        desired |= WS_EX_TOPMOST as i32;
    } else {
        desired &= !(WS_EX_TOPMOST as i32);
    }
    let insert_after = if topmost {
        HWND_TOPMOST
    } else {
        HWND_NOTOPMOST
    };
    let taskbar_changed = (current & (WS_EX_TOOLWINDOW | WS_EX_APPWINDOW) as i32)
        != (desired & (WS_EX_TOOLWINDOW | WS_EX_APPWINDOW) as i32);
    let was_visible = unsafe { IsWindowVisible(hwnd) != 0 };
    if desired != current {
        if taskbar_changed && was_visible {
            unsafe { ShowWindow(hwnd, SW_HIDE) };
        }
        unsafe { SetWindowLongW(hwnd, GWL_EXSTYLE, desired) };
    }
    unsafe {
        SetWindowPos(
            hwnd,
            insert_after,
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
        );
    }
    if taskbar_changed && was_visible {
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            SetForegroundWindow(hwnd);
        }
    }
}

pub(super) fn refresh_window_appearance(title: &str) -> bool {
    let Some(window) = window(title) else {
        return false;
    };
    backdrop::update_appearance(window.hwnd);
    true
}

pub(super) fn configure_tool_window(window: &Window) {
    if let Some(hwnd) = window_hwnd(window) {
        icon::apply_window_icons(hwnd);
        install_key_repeat_tracking(hwnd);
        let installed = install_backdrop(hwnd, window);
        if let Some(installed) = installed {
            let hwnd_bits = hwnd as usize;
            defer(move |_| {
                backdrop::finalize_install(
                    std::ptr::without_provenance_mut(hwnd_bits),
                    installed,
                );
            });
        }
    }
}

fn install_backdrop(hwnd: HWND, window: &Window) -> Option<backdrop::InstalledBackdrop> {
    let installed = backdrop::install(hwnd, window)?;
    // Window properties are scoped to the HWND and released on destruction.
    unsafe { SetPropW(hwnd, BACKDROP_PROPERTY, hwnd) };
    Some(installed)
}

fn install_key_repeat_tracking(hwnd: HWND) {
    if unsafe { SetWindowSubclass(hwnd, Some(track_key_repeat), 0, 0) } == 0 {
        tracing::warn!("windows_key_repeat_tracking_failed");
    }
}

unsafe extern "system" fn track_key_repeat(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    subclass: usize,
    _: usize,
) -> LRESULT {
    if message == WM_NCDESTROY {
        unsafe { RemoveWindowSubclass(hwnd, Some(track_key_repeat), subclass) };
    }
    let repeated = matches!(message, WM_KEYDOWN | WM_SYSKEYDOWN) && lparam & (1 << 30) != 0;
    let previous = KEY_REPEAT.replace(repeated);
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    KEY_REPEAT.set(previous);
    result
}

pub(super) fn current_key_event_is_repeat() -> bool {
    KEY_REPEAT.get()
}

#[cfg(test)]
pub(super) fn set_key_event_is_repeat_for_test(repeated: bool) {
    KEY_REPEAT.set(repeated);
}

pub(super) fn has_window_backdrop(window: &Window) -> bool {
    window_hwnd(window).is_some_and(backdrop_installed)
}

pub(super) fn launcher_has_backdrop() -> Option<bool> {
    window(crate::PRODUCT_NAME).map(|window| backdrop_installed(window.hwnd))
}

fn backdrop_installed(hwnd: HWND) -> bool {
    (unsafe { GetPropW(hwnd, BACKDROP_PROPERTY) }) == hwnd
}

fn window_hwnd(window: &Window) -> Option<HWND> {
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as HWND),
        _ => None,
    }
}

pub(super) fn is_window_maximized(window: &Window) -> bool {
    window_hwnd(window).is_some_and(|hwnd| unsafe { IsZoomed(hwnd) != 0 })
}

pub(super) fn toggle_window_size(window: &Window) {
    if let Some(hwnd) = window_hwnd(window) {
        unsafe {
            let command = if IsZoomed(hwnd) == 0 {
                SW_MAXIMIZE
            } else {
                SW_RESTORE
            };
            ShowWindowAsync(hwnd, command);
        }
    }
}

pub(super) fn install_observers() -> mpsc::Receiver<PlatformEvent> {
    let (sender, receiver) = mpsc::channel(64);
    UI.with_borrow_mut(|ui| ui.events = Some(sender));
    receiver
}

pub(crate) fn report_activation(active: bool) {
    UI.with_borrow(|ui| {
        if let Some(sender) = &ui.events {
            let event = if active {
                PlatformEvent::LauncherBecameKey
            } else {
                PlatformEvent::LauncherLostKey
            };
            let _ = sender.try_send(event);
        }
    });
}

pub(super) fn notify_accessibility_display_changed() {
    crate::platform::prepare_window_material();
    UI.with_borrow(|ui| {
        if let Some(sender) = &ui.events {
            let _ = sender.try_send(PlatformEvent::AccessibilityDisplayChanged);
        }
    });
}

pub(super) fn configure_webview_builder<'a>(
    builder: wry::WebViewBuilder<'a>,
) -> wry::WebViewBuilder<'a> {
    use wry::WebViewBuilderExtWindows as _;
    builder.with_https_scheme(true)
}

pub(super) fn web_document_origin(canonical_origin: &str) -> String {
    if let Some(host) = canonical_origin.strip_prefix("anywhere-plugin://") {
        format!("https://anywhere-plugin.{host}")
    } else {
        canonical_origin.to_owned()
    }
}

pub(super) fn default_product_paths(
    executable: &std::path::Path,
) -> anyhow::Result<(std::path::PathBuf, std::path::PathBuf)> {
    use anyhow::Context as _;
    let data_directory = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            anywhere_platform::paths::home_directory().map(|path| path.join(r"AppData\Local"))
        })
        .context("Windows local application data directory is unavailable")?
        .join("Anywhere");
    let bundled_plugin_directory = executable.with_file_name("plugins");
    Ok((data_directory, bundled_plugin_directory))
}

fn process_id(hwnd: HWND) -> Option<u32> {
    let mut pid = 0;
    let thread = unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    (thread != 0 && pid != 0).then_some(pid)
}

pub(super) fn remember_destination() -> bool {
    let hwnd = unsafe { GetForegroundWindow() };
    if let Some(pid) = process_id(hwnd).filter(|pid| *pid != std::process::id()) {
        UI.with_borrow_mut(|ui| ui.previous = Some((hwnd, pid)));
    }
    UI.with_borrow(|ui| ui.previous.is_some())
}

pub(super) fn return_to_previous_application() -> ApplicationReturnResult {
    let Some((hwnd, _)) = UI
        .with_borrow(|ui| ui.previous)
        .filter(|(hwnd, pid)| process_id(*hwnd) == Some(*pid))
    else {
        return ApplicationReturnResult::Unavailable;
    };
    if unsafe { IsIconic(hwnd) } != 0 {
        unsafe { ShowWindowAsync(hwnd, SW_RESTORE) };
    }
    if unsafe { SetForegroundWindow(hwnd) } != 0 {
        ApplicationReturnResult::Returned
    } else {
        ApplicationReturnResult::Failed
    }
}

pub(super) fn activate(title: &str) -> bool {
    let Some(window) = window(title) else {
        return false;
    };
    let is_launcher = title == crate::PRODUCT_NAME;
    if is_launcher {
        remember_destination();
    }
    defer(move |app| {
        if !window.is_live() {
            return;
        }
        let minimized = unsafe { IsIconic(window.hwnd) } != 0;
        if is_launcher && !minimized {
            position_launcher_at_pointer(window.hwnd);
        }
        if window.initial_placement_pending {
            UI.with_borrow_mut(|ui| {
                if let Some(bound) = ui.windows.get_mut(crate::PRODUCT_NAME) {
                    bound.initial_placement_pending = false;
                }
            });
            let _ = window
                .handle
                .update(app, |_, window, _| window.activate_window());
            return;
        }
        let command = if minimized { SW_RESTORE } else { SW_SHOW };
        unsafe { ShowWindow(window.hwnd, command) };
        if is_launcher && minimized {
            position_launcher_at_pointer(window.hwnd);
        }
        if unsafe { SetForegroundWindow(window.hwnd) } == 0 {
            let _ = window
                .handle
                .update(app, |_, window, _| window.activate_window());
        }
    })
}

fn position_launcher_at_pointer(hwnd: HWND) {
    let Some((work, width, height, _)) = window_frame_on_pointer_monitor(hwnd) else {
        return;
    };
    let x = work.left + ((work.right - work.left - width) / 2).max(0);
    let top_gap =
        (f64::from(work.bottom - work.top) * super::LAUNCHER_TOP_INSET_RATIO).round() as i32;
    let y = work.top + top_gap.min((work.bottom - work.top - height).max(0));
    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            x,
            y,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
        )
    };
}

pub(super) fn hide_main_window() {
    let Some(window) = window(crate::PRODUCT_NAME) else {
        return;
    };
    let command = if UI.with_borrow(|ui| ui.tray_available) {
        SW_HIDE
    } else {
        SW_MINIMIZE
    };
    unsafe { ShowWindowAsync(window.hwnd, command) };
}

pub(crate) fn background_launch() -> bool {
    std::env::args_os()
        .skip(1)
        .any(|argument| argument == "--background")
}

pub(super) fn should_dismiss() -> bool {
    let foreground = unsafe { GetForegroundWindow() };
    window(crate::PRODUCT_NAME).is_some_and(|window| {
        (unsafe { IsWindowVisible(window.hwnd) != 0 })
            && process_id(foreground).is_some_and(|pid| pid != std::process::id())
    })
}

pub(super) fn configure_shelf(title: &str) -> bool {
    let Some(window) = window(title) else {
        return false;
    };
    defer(move |_| {
        if window.is_live() {
            let origin = shelf_origin(window.hwnd);
            let (x, y, flags) = origin.map_or((0, 0, SWP_NOMOVE), |point| (point.x, point.y, 0));
            unsafe {
                SetWindowPos(
                    window.hwnd,
                    HWND_TOPMOST,
                    x,
                    y,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOSIZE | flags,
                )
            };
        }
    })
}

fn shelf_origin(hwnd: HWND) -> Option<POINT> {
    let (work, width, height, scale) = window_frame_on_pointer_monitor(hwnd)?;
    let (right_aligned, bottom_inset) = super::bottom_trailing_origin(
        0.0,
        0.0,
        f64::from(work.right - work.left) / scale,
        f64::from(work.bottom - work.top) / scale,
        f64::from(width) / scale,
        f64::from(height) / scale,
    );
    let origin = UI.with_borrow(|ui| ui.shelf_position).unwrap_or(POINT {
        x: work.left + (right_aligned * scale).round() as i32,
        y: work.bottom - height - (bottom_inset * scale).round() as i32,
    });
    Some(POINT {
        x: origin
            .x
            .clamp(work.left, (work.right - width).max(work.left)),
        y: origin
            .y
            .clamp(work.top, (work.bottom - height).max(work.top)),
    })
}

fn window_frame_on_pointer_monitor(hwnd: HWND) -> Option<(RECT, i32, i32, f64)> {
    let mut frame = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut frame) } == 0 {
        return None;
    }
    let mut pointer = POINT::default();
    let monitor = unsafe {
        if GetCursorPos(&mut pointer) != 0 {
            MonitorFromPoint(pointer, MONITOR_DEFAULTTONEAREST)
        } else {
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)
        }
    };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return None;
    }
    let current_dpi = unsafe { GetDpiForWindow(hwnd) }.max(1);
    let (mut dpi, mut dpi_y) = (current_dpi, current_dpi);
    if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y) } < 0 {
        dpi = current_dpi;
    }
    let scale = f64::from(dpi) / f64::from(USER_DEFAULT_SCREEN_DPI);
    let resize_scale = f64::from(dpi) / f64::from(current_dpi);
    let width = (f64::from(frame.right - frame.left) * resize_scale).round() as i32;
    let height = (f64::from(frame.bottom - frame.top) * resize_scale).round() as i32;
    Some((info.rcWork, width, height, scale))
}

pub(super) fn remember_shelf_position() {
    let Some(window) = window(crate::CLIPBOARD_SHELF_TITLE) else {
        return;
    };
    let mut frame = RECT::default();
    if unsafe {
        IsIconic(window.hwnd) == 0
            && IsZoomed(window.hwnd) == 0
            && GetWindowRect(window.hwnd, &mut frame) != 0
    } {
        UI.with_borrow_mut(|ui| {
            ui.shelf_position = Some(POINT {
                x: frame.left,
                y: frame.top,
            })
        });
    }
}

pub(super) fn shelf_is_active() -> bool {
    window(crate::CLIPBOARD_SHELF_TITLE)
        .is_some_and(|window| unsafe { GetForegroundWindow() == window.hwnd })
}

fn defer(operation: impl FnOnce(&mut AsyncApp) + 'static) -> bool {
    let Some(mut app) = UI.with_borrow(|ui| ui.app.clone()) else {
        return false;
    };
    app.foreground_executor()
        .clone()
        .spawn(async move { operation(&mut app) })
        .detach();
    true
}

fn tray_available(available: bool) {
    UI.with_borrow_mut(|ui| ui.tray_available = available);
}

pub(crate) fn has_tray_icon() -> bool {
    UI.with_borrow(|ui| ui.tray_available)
}
