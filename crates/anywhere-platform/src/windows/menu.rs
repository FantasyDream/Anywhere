//! Standard Win32 menu bars on the exact window captured at activation.
use super::shell::captured_application_window;
use crate::{ApplicationInfo, DiscoveredMenuItem, MenuDiscovery, MenuLocator};
use anyhow::{ensure, Context as _, Result};
use std::{
    collections::HashMap,
    mem::size_of,
    ptr::null_mut,
    time::{Duration, Instant},
};
use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetMenu, GetMenuInfo, GetMenuItemCount, GetMenuItemInfoW, PostMessageW, SendMessageTimeoutW,
    HMENU, MENUINFO, MENUITEMINFOW, MFS_DISABLED, MFT_BITMAP, MFT_OWNERDRAW, MFT_SEPARATOR,
    MIIM_FTYPE, MIIM_ID, MIIM_STATE, MIIM_STRING, MIIM_SUBMENU, MIM_STYLE, MNS_NOTIFYBYPOS,
    SMTO_ABORTIFHUNG, SMTO_BLOCK, SMTO_ERRORONEXIT, WM_COMMAND, WM_INITMENU, WM_INITMENUPOPUP,
};

const MAX_MENU_DEPTH: usize = 8;
const MAX_MENU_ITEMS: usize = 512;
const COMMAND_PREFIX: &str = "win32.command:";
const MENU_INITIALIZATION_TIMEOUT: Duration = Duration::from_millis(350);

pub(super) fn discover(application: &ApplicationInfo) -> Result<MenuDiscovery> {
    let Some(window) = captured_application_window(application) else {
        return Ok(MenuDiscovery::Unavailable);
    };
    if unsafe { IsWindowEnabled(window) } == 0 {
        return Ok(MenuDiscovery::Ready(Vec::new()));
    }
    let root = unsafe { GetMenu(window) };
    initialize_menu_tree(
        application,
        window,
        root,
        Instant::now() + MENU_INITIALIZATION_TIMEOUT,
    )?;
    let items = read_menu(root)?;
    if !same_menu_owner(application, window, root) {
        return Ok(MenuDiscovery::Unavailable);
    }
    Ok(MenuDiscovery::Ready(
        items.into_iter().map(|item| item.discovery).collect(),
    ))
}

pub(super) fn perform(application: &ApplicationInfo, locator: &MenuLocator) -> Result<()> {
    ensure!(
        locator.bundle_id == application.bundle_id,
        "menu application changed"
    );
    let window =
        captured_application_window(application).context("menu window no longer exists")?;
    let root = unsafe { GetMenu(window) };
    let deadline = Instant::now() + MENU_INITIALIZATION_TIMEOUT;
    initialize_menu_tree(application, window, root, deadline)?;
    let mut items = read_menu(root)?;
    let popup_count = find_command(&items, locator)
        .context("menu command changed or is unavailable")?
        .popups
        .len();
    initialize_menu(application, window, root, WM_INITMENU, root, 0, deadline)?;
    for level in 0..popup_count {
        items = read_menu(root)?;
        let popup = find_command(&items, locator)
            .and_then(|item| item.popups.get(level))
            .context("menu command changed or is unavailable")?;
        initialize_menu(
            application,
            window,
            root,
            WM_INITMENUPOPUP,
            popup.menu,
            popup.position as LPARAM,
            deadline,
        )?;
    }
    items = read_menu(root)?;
    let command =
        find_command(&items, locator).context("menu command changed or is unavailable")?;
    ensure!(
        same_menu_owner(application, window, root),
        "menu window changed or is unavailable"
    );
    // WM_COMMAND takes the command ID in the low word with high word and lParam zero for a menu.
    if unsafe { PostMessageW(window, WM_COMMAND, usize::from(command.id), 0) } == 0 {
        return Err(std::io::Error::last_os_error()).context("Windows rejected the menu command");
    }
    Ok(())
}

/// Populate dynamic menus before reading them. Applications such as 7-Zip add
/// child commands only while Windows sends `WM_INITMENU` and
/// `WM_INITMENUPOPUP`. The same sequence is required again before execution so
/// that the locator is checked against the current enabled command set.
fn initialize_menu_tree(
    application: &ApplicationInfo,
    window: HWND,
    root: HMENU,
    deadline: Instant,
) -> Result<()> {
    if root.is_null() {
        return Ok(());
    }
    initialize_menu(application, window, root, WM_INITMENU, root, 0, deadline)?;

    let mut pending = vec![(root, 0_usize)];
    let mut remaining = MAX_MENU_ITEMS;
    while let Some((menu, depth)) = pending.pop() {
        ensure!(depth < MAX_MENU_DEPTH, "Windows menu exceeds its depth limit");
        let count = unsafe { GetMenuItemCount(menu) };
        let count = usize::try_from(count).context("could not count Windows menu items")?;
        remaining = remaining
            .checked_sub(count)
            .context("Windows menu exceeds its item limit")?;
        for position in 0..count as u32 {
            let mut info = MENUITEMINFOW {
                cbSize: size_of::<MENUITEMINFOW>() as u32,
                fMask: MIIM_SUBMENU,
                ..Default::default()
            };
            ensure!(
                unsafe { GetMenuItemInfoW(menu, position, 1, &mut info) } != 0,
                "could not read Windows menu item"
            );
            if info.hSubMenu.is_null() {
                continue;
            }
            initialize_menu(
                application,
                window,
                root,
                WM_INITMENUPOPUP,
                info.hSubMenu,
                position as LPARAM,
                deadline,
            )?;
            pending.push((info.hSubMenu, depth + 1));
        }
    }
    Ok(())
}

fn same_menu_owner(application: &ApplicationInfo, window: HWND, root: HMENU) -> bool {
    captured_application_window(application) == Some(window)
        && unsafe { GetMenu(window) } == root
        && unsafe { IsWindowEnabled(window) } != 0
}

fn initialize_menu(
    application: &ApplicationInfo,
    window: HWND,
    root: HMENU,
    message: u32,
    menu: HMENU,
    position: LPARAM,
    deadline: Instant,
) -> Result<()> {
    ensure!(
        same_menu_owner(application, window, root),
        "menu window changed or is unavailable"
    );
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .as_millis() as u32;
    ensure!(remaining > 0, "Windows menu initialization timed out");
    let result = unsafe {
        SendMessageTimeoutW(
            window,
            message,
            menu.addr(),
            position,
            SMTO_ABORTIFHUNG | SMTO_BLOCK | SMTO_ERRORONEXIT,
            remaining,
            null_mut(),
        )
    };
    ensure!(
        result != 0,
        "Windows menu initialization failed or timed out"
    );
    Ok(())
}

fn find_command<'a>(items: &'a [MenuEntry], locator: &MenuLocator) -> Option<&'a MenuEntry> {
    items.iter().find(|item| {
        item.discovery.enabled
            && item.discovery.menu_path == locator.titles
            && item.discovery.identifiers == locator.identifiers
    })
}

#[derive(Clone, Copy)]
struct Popup {
    menu: HMENU,
    position: u32,
}

struct MenuEntry {
    id: u16,
    discovery: DiscoveredMenuItem,
    popups: Vec<Popup>,
}

struct MenuScan {
    remaining: usize,
    counts: HashMap<u16, usize>,
    commands: Vec<MenuEntry>,
}

fn read_menu(root: HMENU) -> Result<Vec<MenuEntry>> {
    if root.is_null() {
        return Ok(Vec::new());
    }
    let mut scan = MenuScan {
        remaining: MAX_MENU_ITEMS,
        counts: HashMap::new(),
        commands: Vec::new(),
    };
    scan.visit(root, &mut Vec::new(), &mut Vec::new(), true)?;
    Ok(scan
        .commands
        .into_iter()
        .filter(|item| scan.counts.get(&item.id) == Some(&1))
        .collect())
}

impl MenuScan {
    fn visit(
        &mut self,
        menu: HMENU,
        parents: &mut Vec<String>,
        popups: &mut Vec<Popup>,
        enabled: bool,
    ) -> Result<()> {
        ensure!(
            parents.len() < MAX_MENU_DEPTH,
            "Windows menu exceeds its depth limit"
        );
        let mut style = MENUINFO {
            cbSize: size_of::<MENUINFO>() as u32,
            fMask: MIM_STYLE,
            ..Default::default()
        };
        ensure!(
            unsafe { GetMenuInfo(menu, &mut style) } != 0,
            "could not read Windows menu style"
        );
        let dispatchable = style.dwStyle & MNS_NOTIFYBYPOS == 0;
        let count = unsafe { GetMenuItemCount(menu) };
        let count = usize::try_from(count).context("could not count Windows menu items")?;
        self.remaining = self
            .remaining
            .checked_sub(count)
            .context("Windows menu exceeds its item limit")?;
        for position in 0..count as u32 {
            let mut label = [0_u16; 256];
            let mut info = MENUITEMINFOW {
                cbSize: size_of::<MENUITEMINFOW>() as u32,
                fMask: MIIM_FTYPE | MIIM_ID | MIIM_STATE | MIIM_SUBMENU | MIIM_STRING,
                dwTypeData: label.as_mut_ptr(),
                cch: label.len() as u32,
                ..Default::default()
            };
            ensure!(
                unsafe { GetMenuItemInfoW(menu, position, 1, &mut info) } != 0,
                "could not read Windows menu item"
            );
            if info.fType & MFT_SEPARATOR != 0 {
                continue;
            }
            let (title, shortcut) = if info.fType & (MFT_BITMAP | MFT_OWNERDRAW) == 0
                && (info.cch as usize) < label.len() - 1
            {
                menu_label(&label[..info.cch as usize])
            } else {
                (String::new(), None)
            };
            let enabled = enabled && info.fState & MFS_DISABLED == 0;
            if !info.hSubMenu.is_null() {
                parents.push(title);
                popups.push(Popup {
                    menu: info.hSubMenu,
                    position,
                });
                self.visit(info.hSubMenu, parents, popups, enabled)?;
                popups.pop();
                parents.pop();
            } else if dispatchable {
                self.add_leaf(info.wID, parents, popups, title, shortcut, enabled);
            }
        }
        Ok(())
    }

    fn add_leaf(
        &mut self,
        id: u32,
        parents: &[String],
        popups: &[Popup],
        title: String,
        shortcut: Option<String>,
        enabled: bool,
    ) {
        let Ok(id) = u16::try_from(id) else {
            return;
        };
        if id == 0 {
            return;
        }
        *self.counts.entry(id).or_default() += 1;
        if title.is_empty() || parents.iter().any(String::is_empty) {
            return;
        }
        let mut menu_path = parents.to_vec();
        menu_path.push(title.clone());
        let mut identifiers = vec![None; parents.len()];
        identifiers.push(Some(format!("{COMMAND_PREFIX}{id}")));
        self.commands.push(MenuEntry {
            id,
            discovery: DiscoveredMenuItem {
                title,
                menu_path,
                identifiers,
                shortcut,
                enabled,
            },
            popups: popups.to_vec(),
        });
    }
}

fn menu_label(text: &[u16]) -> (String, Option<String>) {
    let text = String::from_utf16_lossy(text);
    let (label, shortcut) = text.split_once('\t').unwrap_or((&text, ""));
    let mut mnemonic = false;
    let title: String = label
        .trim()
        .chars()
        .filter(|character| {
            if *character == '&' {
                mnemonic = !mnemonic;
                !mnemonic
            } else {
                mnemonic = false;
                true
            }
        })
        .collect();
    let shortcut = shortcut.trim();
    (title, (!shortcut.is_empty()).then(|| shortcut.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows::shell::window_application;
    use std::ptr::{null, null_mut};
    use windows_sys::{
        w,
        Win32::{
            Foundation::HWND,
            UI::WindowsAndMessaging::{
                AppendMenuW, CreateMenu, CreatePopupMenu, CreateWindowExW, DestroyMenu,
                DestroyWindow, EnableMenuItem, PeekMessageW, SetMenu, MF_BYCOMMAND, MF_GRAYED,
                MF_POPUP, MF_STRING, MSG, PM_REMOVE, WS_OVERLAPPEDWINDOW,
            },
        },
    };

    struct Menu(HMENU);
    impl Drop for Menu {
        fn drop(&mut self) {
            unsafe { DestroyMenu(self.0) };
        }
    }

    #[test]
    fn native_menu_snapshot_omits_ambiguous_ids_and_refuses_disabled_or_changed_targets() {
        let root = Menu(unsafe { CreateMenu() });
        let file = unsafe { CreatePopupMenu() };
        assert_ne!(
            unsafe { AppendMenuW(root.0, MF_POPUP, file.addr(), w!("&File")) },
            0
        );
        for (id, label) in [
            (42, w!("Save && E&xit\tCtrl+S")),
            (43, w!("One")),
            (43, w!("Two")),
        ] {
            assert_ne!(unsafe { AppendMenuW(file, MF_STRING, id, label) }, 0);
        }

        let items = read_menu(root.0).unwrap();
        assert_eq!(items.len(), 1);
        let item = &items[0].discovery;
        assert_eq!(item.menu_path, ["File", "Save & Exit"]);
        assert_eq!(item.shortcut.as_deref(), Some("Ctrl+S"));
        let mut locator = MenuLocator {
            bundle_id: "test".into(),
            titles: item.menu_path.clone(),
            identifiers: item.identifiers.clone(),
        };
        assert_eq!(
            find_command(&items, &locator).map(|entry| entry.id),
            Some(42)
        );
        locator.titles[1] = "Other".into();
        assert!(find_command(&items, &locator).is_none());
        locator.titles = item.menu_path.clone();
        unsafe { EnableMenuItem(file, 42, MF_BYCOMMAND | MF_GRAYED) };
        assert!(find_command(&read_menu(root.0).unwrap(), &locator).is_none());
    }

    struct Window(HWND);

    impl Drop for Window {
        fn drop(&mut self) {
            unsafe { DestroyWindow(self.0) };
        }
    }

    #[test]
    fn menu_dispatch_posts_only_the_confirmed_window_command() {
        let window = Window(unsafe {
            CreateWindowExW(
                0,
                w!("STATIC"),
                w!("Anywhere menu fixture"),
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
        });
        assert!(!window.0.is_null());
        let root = unsafe { CreateMenu() };
        assert_ne!(
            unsafe { AppendMenuW(root, MF_STRING, 42, w!("Test action")) },
            0
        );
        // The window owns and destroys the attached menu.
        assert_ne!(unsafe { SetMenu(window.0, root) }, 0);
        let app = window_application(window.0).unwrap();
        let MenuDiscovery::Ready(items) = discover(&app).unwrap() else {
            panic!("fixture menu could not be read");
        };
        let item = &items[0];
        let locator = MenuLocator {
            bundle_id: app.bundle_id.clone(),
            titles: item.menu_path.clone(),
            identifiers: item.identifiers.clone(),
        };
        let mut msg = MSG::default();
        perform(&app, &locator).unwrap();
        assert_ne!(
            unsafe { PeekMessageW(&mut msg, window.0, WM_COMMAND, WM_COMMAND, PM_REMOVE) },
            0
        );
        assert_eq!((msg.wParam, msg.lParam), (42, 0));

        unsafe { EnableMenuItem(root, 42, MF_BYCOMMAND | MF_GRAYED) };
        assert!(perform(&app, &locator).is_err());
        assert_eq!(
            unsafe { PeekMessageW(&mut msg, window.0, WM_COMMAND, WM_COMMAND, PM_REMOVE) },
            0
        );
    }
    #[test]
    fn popup_initialization_can_disable_a_previously_discovered_command() {
        use windows_sys::Win32::{
            Foundation::{LRESULT, WPARAM},
            UI::{
                Input::KeyboardAndMouse::EnableWindow,
                WindowsAndMessaging::{DefWindowProcW, SetWindowLongPtrW, GWLP_WNDPROC},
            },
        };
        unsafe extern "system" fn update_menu(
            window: HWND,
            message: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            if message == WM_INITMENUPOPUP {
                unsafe {
                    EnableMenuItem(
                        std::ptr::without_provenance_mut(wparam),
                        42,
                        MF_BYCOMMAND | MF_GRAYED,
                    )
                };
                return 0;
            }
            unsafe { DefWindowProcW(window, message, wparam, lparam) }
        }

        let window = Window(unsafe {
            CreateWindowExW(
                0,
                w!("STATIC"),
                w!("Anywhere dynamic menu fixture"),
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
        });
        assert!(!window.0.is_null());
        let root = unsafe { CreateMenu() };
        let popup = unsafe { CreatePopupMenu() };
        assert_ne!(
            unsafe { AppendMenuW(popup, MF_STRING, 42, w!("Test dynamic action")) },
            0
        );
        assert_ne!(
            unsafe { AppendMenuW(root, MF_POPUP, popup.addr(), w!("File")) },
            0
        );
        assert_ne!(unsafe { SetMenu(window.0, root) }, 0);

        let app = window_application(window.0).unwrap();
        let MenuDiscovery::Ready(items) = discover(&app).unwrap() else {
            panic!("fixture menu could not be read");
        };
        assert!(items[0].enabled);
        let locator = MenuLocator {
            bundle_id: app.bundle_id.clone(),
            titles: items[0].menu_path.clone(),
            identifiers: items[0].identifiers.clone(),
        };
        unsafe { SetWindowLongPtrW(window.0, GWLP_WNDPROC, update_menu as *const () as isize) };
        assert!(perform(&app, &locator).is_err());
        assert!(find_command(&read_menu(root).unwrap(), &locator).is_none());
        let mut message = MSG::default();
        assert_eq!(
            unsafe { PeekMessageW(&mut message, window.0, WM_COMMAND, WM_COMMAND, PM_REMOVE) },
            0
        );

        unsafe { EnableWindow(window.0, 0) };
        assert_eq!(discover(&app).unwrap(), MenuDiscovery::Ready(Vec::new()));
    }

    #[test]
    fn discovery_initializes_dynamic_popup_menus_before_reading_them() {
        use windows_sys::Win32::{
            Foundation::LRESULT,
            UI::WindowsAndMessaging::{
                DefWindowProcW, SetWindowLongPtrW, GWLP_WNDPROC, WM_INITMENUPOPUP,
            },
        };

        unsafe extern "system" fn add_menu_item(
            window: HWND,
            message: u32,
            wparam: windows_sys::Win32::Foundation::WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            if message == WM_INITMENUPOPUP {
                let popup = std::ptr::without_provenance_mut(wparam);
                if unsafe { GetMenuItemCount(popup) } == 0 {
                    unsafe { AppendMenuW(popup, MF_STRING, 77, w!("Dynamic action")) };
                }
                return 0;
            }
            unsafe { DefWindowProcW(window, message, wparam, lparam) }
        }

        let window = Window(unsafe {
            CreateWindowExW(
                0,
                w!("STATIC"),
                w!("Anywhere dynamic discovery fixture"),
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
        });
        assert!(!window.0.is_null());
        let root = unsafe { CreateMenu() };
        let popup = unsafe { CreatePopupMenu() };
        assert_ne!(
            unsafe { AppendMenuW(root, MF_POPUP, popup.addr(), w!("Help")) },
            0
        );
        assert_ne!(unsafe { SetMenu(window.0, root) }, 0);
        unsafe {
            SetWindowLongPtrW(window.0, GWLP_WNDPROC, add_menu_item as *const () as isize);
        }

        let app = window_application(window.0).unwrap();
        let MenuDiscovery::Ready(items) = discover(&app).unwrap() else {
            panic!("fixture menu was not readable");
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].menu_path, ["Help", "Dynamic action"]);
        assert_eq!(items[0].identifiers, [None, Some("win32.command:77".into())]);
    }
}
