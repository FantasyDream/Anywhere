use super::tray_available;
use crate::platform::{StatusItemAction, StatusMenuLabels};
use std::{
    cell::RefCell,
    ptr::{null, null_mut},
};
use tokio::sync::mpsc;
use windows_sys::{
    w,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Shell::{
                Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD,
                NIM_DELETE, NIM_SETVERSION, NIN_SELECT, NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
            },
            WindowsAndMessaging::{
                AllowSetForegroundWindow, AppendMenuW, CreatePopupMenu, CreateWindowExW,
                DefWindowProcW, DestroyMenu, DestroyWindow, FindWindowW, LoadIconW, PostMessageW,
                RegisterClassW, RegisterWindowMessageW, SetForegroundWindow, TrackPopupMenu, HMENU,
                IDI_APPLICATION, MF_STRING, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP, WM_CONTEXTMENU,
                WM_NULL, WNDCLASSW,
            },
        },
    },
};

const NOTIFICATION: u32 = WM_APP + 1;
const RESTORE_EXISTING: u32 = WM_APP + 2;
const CLASS: windows_sys::core::PCWSTR = w!("Anywhere.Tray");
const NORMAL_INSTANCE: windows_sys::core::PCWSTR = w!("Anywhere.Tray.Default");
const KEY_SELECT: u32 = NIN_SELECT | 1;

pub(super) fn restore_existing() -> bool {
    let window = unsafe { FindWindowW(CLASS, NORMAL_INSTANCE) };
    let Some(pid) = super::process_id(window) else {
        return false;
    };
    unsafe {
        AllowSetForegroundWindow(pid);
        PostMessageW(window, RESTORE_EXISTING, 0, 0) != 0
    }
}

struct State {
    taskbar_created: u32,
    actions: mpsc::Sender<StatusItemAction>,
    labels: StatusMenuLabels,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

pub(in crate::platform) struct Tray(HWND);

impl Tray {
    pub(in crate::platform) fn install(
        actions: mpsc::Sender<StatusItemAction>,
        labels: &StatusMenuLabels,
    ) -> Option<Self> {
        let instance = unsafe { GetModuleHandleW(null()) };
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: CLASS,
            ..Default::default()
        };
        unsafe { RegisterClassW(&class) };
        let title = if std::env::var_os("ANYWHERE_DATA_DIR").is_some()
            || std::env::var_os("ANYWHERE_PREVIEW").is_some()
        {
            w!("Anywhere.Tray.Isolated")
        } else {
            NORMAL_INSTANCE
        };
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                CLASS,
                title,
                0,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                instance,
                null(),
            )
        };
        if hwnd.is_null() {
            return None;
        }
        let tray = Self(hwnd);
        let taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        STATE.with_borrow_mut(|state| {
            *state = Some(State {
                taskbar_created,
                actions,
                labels: labels.clone(),
            });
        });
        tray_available(add_icon(hwnd));
        Some(tray)
    }

    pub(in crate::platform) fn update_labels(&self, labels: &StatusMenuLabels) -> bool {
        STATE.with_borrow_mut(|state| {
            let Some(state) = state else { return false };
            state.labels = labels.clone();
            true
        })
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        let data = icon(self.0);
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &data);
            DestroyWindow(self.0);
        }
        STATE.with_borrow_mut(|state| *state = None);
        tray_available(false);
    }
}

fn icon(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    }
}

fn add_icon(hwnd: HWND) -> bool {
    let mut data = icon(hwnd);
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
    data.uCallbackMessage = NOTIFICATION;
    data.hIcon = unsafe { LoadIconW(null_mut(), IDI_APPLICATION) };
    for (unit, character) in data
        .szTip
        .iter_mut()
        .zip(crate::PRODUCT_NAME.encode_utf16())
    {
        *unit = character;
    }
    if unsafe { Shell_NotifyIconW(NIM_ADD, &data) } == 0 {
        return false;
    }
    data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    unsafe { Shell_NotifyIconW(NIM_SETVERSION, &data) != 0 }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let Some((taskbar_created, actions)) = STATE.with_borrow(|state| {
        state
            .as_ref()
            .map(|state| (state.taskbar_created, state.actions.clone()))
    }) else {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    };
    if message != 0 && message == taskbar_created {
        tray_available(add_icon(hwnd));
        return 0;
    }
    if message == RESTORE_EXISTING {
        let _ = actions.try_send(StatusItemAction::ShowUniversal);
        return 0;
    }
    if message == NOTIFICATION {
        super::remember_destination();
        let event = lparam as u32 & 0xffff;
        let action = match event {
            NIN_SELECT | KEY_SELECT => Some(StatusItemAction::ShowUniversal),
            WM_CONTEXTMENU => {
                let x = i32::from(wparam as i16);
                let y = i32::from((wparam >> 16) as i16);
                let action = menu_action(hwnd, x, y);
                super::return_to_previous_application();
                action
            }
            _ => None,
        };
        if let Some(action) = action {
            let _ = actions.try_send(action);
        }
        return 0;
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

struct Menu(HMENU);

impl Drop for Menu {
    fn drop(&mut self) {
        unsafe { DestroyMenu(self.0) };
    }
}

fn menu_action(hwnd: HWND, x: i32, y: i32) -> Option<StatusItemAction> {
    let labels = STATE.with_borrow(|state| state.as_ref().map(|state| state.labels.clone()))?;
    let entries = [
        (labels.universal, StatusItemAction::ShowUniversal),
        (labels.current_app, StatusItemAction::ShowCurrentApp),
        (labels.running_tasks, StatusItemAction::OpenTasks),
        (labels.settings, StatusItemAction::OpenSettings),
        (labels.quit, StatusItemAction::Quit),
    ];
    let menu = unsafe { CreatePopupMenu() };
    if menu.is_null() {
        return None;
    }
    let menu = Menu(menu);
    for (index, (label, _)) in entries.iter().enumerate() {
        let label: Vec<_> = label.replace('&', "&&").encode_utf16().chain([0]).collect();
        unsafe { AppendMenuW(menu.0, MF_STRING, index + 1, label.as_ptr()) };
    }
    unsafe { SetForegroundWindow(hwnd) };
    let selected = unsafe {
        TrackPopupMenu(
            menu.0,
            TPM_RIGHTBUTTON | TPM_RETURNCMD,
            x,
            y,
            0,
            hwnd,
            null(),
        )
    };
    unsafe { PostMessageW(hwnd, WM_NULL, 0, 0) };
    entries
        .get(usize::try_from(selected).ok()?.checked_sub(1)?)
        .map(|(_, action)| *action)
}
