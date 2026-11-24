mod tray;

pub(super) use tray::Tray;

use super::{ApplicationReturnResult, PlatformEvent};
use gpui_kit::{AnyWindowHandle, AsyncApp, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{cell::RefCell, collections::HashMap, ptr::null};
use tokio::sync::mpsc;
use windows_sys::{
    w,
    Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND},
        System::Threading::CreateMutexW,
        UI::WindowsAndMessaging::{
            GetForegroundWindow, GetPropW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
            SetForegroundWindow, SetPropW, SetWindowPos, ShowWindow, ShowWindowAsync, HWND_TOPMOST,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_MINIMIZE, SW_RESTORE, SW_SHOW,
        },
    },
};

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
}

thread_local! {
    static UI: RefCell<Ui> = RefCell::default();
}

pub(crate) fn init(app: &gpui_kit::App) {
    UI.with_borrow_mut(|ui| ui.app = Some(app.to_async()));
}

pub(super) fn bind_window(window: &Window, title: &str) {
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let hwnd = handle.hwnd.get() as HWND;
    let native = NativeWindow {
        hwnd,
        handle: window.window_handle(),
        initial_placement_pending: title == crate::PRODUCT_NAME
            && unsafe { IsWindowVisible(hwnd) } == 0,
    };
    if unsafe { SetPropW(hwnd, WINDOW_PROPERTY, native.identity()) } != 0 {
        UI.with_borrow_mut(|ui| {
            ui.windows.retain(|_, window| window.is_live());
            ui.windows.insert(title.to_owned(), native);
        });
    }
}

fn window(title: &str) -> Option<NativeWindow> {
    UI.with_borrow(|ui| ui.windows.get(title).copied())
        .filter(|window| window.is_live())
}

pub(super) fn window_exists(title: &str) -> bool {
    window(title).is_some()
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
    if title == crate::PRODUCT_NAME {
        remember_destination();
    }
    defer(move |app| {
        if !window.is_live() {
            return;
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
        let command = if unsafe { IsIconic(window.hwnd) } != 0 {
            SW_RESTORE
        } else {
            SW_SHOW
        };
        unsafe { ShowWindow(window.hwnd, command) };
        if unsafe { SetForegroundWindow(window.hwnd) } == 0 {
            let _ = window
                .handle
                .update(app, |_, window, _| window.activate_window());
        }
    })
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
            unsafe {
                SetWindowPos(
                    window.hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                )
            };
        }
    })
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
