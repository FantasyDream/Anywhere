use crate::ApplicationInfo;
use anyhow::{ensure, Result};
use std::{
    ffi::{OsStr, OsString},
    os::windows::ffi::{OsStrExt, OsStringExt},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, FILETIME, HANDLE, HWND},
    System::{
        Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED},
        SystemInformation::GetSystemDirectoryW,
        Threading::{
            GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, CREATE_NEW_CONSOLE,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
    UI::{
        Shell::{
            Common::ITEMIDLIST, SHOpenFolderAndSelectItems, SHParseDisplayName, ShellExecuteW,
        },
        WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId, SW_SHOWNORMAL},
    },
};

pub(super) fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|value| value.eq_ignore_ascii_case(extension))
}

struct Process(HANDLE);

impl Drop for Process {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

pub(super) fn frontmost_application() -> Option<ApplicationInfo> {
    let window = unsafe { GetForegroundWindow() };
    let application = window_application(window)?;
    (application.pid as u32 != std::process::id() && unsafe { GetForegroundWindow() } == window)
        .then_some(application)
}

/// Resolve exactly the window captured at activation, even while Anywhere is foreground.
pub(super) fn captured_application_window(application: &ApplicationInfo) -> Option<HWND> {
    let (_, raw_window) = application.instance_id.as_deref()?.rsplit_once(':')?;
    let window = std::ptr::without_provenance_mut(raw_window.parse::<usize>().ok()?);
    (window_application(window)?.eq(application)).then_some(window)
}

pub(super) fn window_application(window: HWND) -> Option<ApplicationInfo> {
    if window.is_null() {
        return None;
    }
    let mut pid = 0;
    let thread = unsafe { GetWindowThreadProcessId(window, &mut pid) };
    if thread == 0 || pid == 0 {
        return None;
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }
    let process = Process(process);
    let mut path = vec![0; 32_768];
    let mut length = u32::try_from(path.len()).ok()?;
    if unsafe { QueryFullProcessImageNameW(process.0, 0, path.as_mut_ptr(), &mut length) } == 0 {
        return None;
    }
    let path = PathBuf::from(OsString::from_wide(path.get(..length as usize)?));
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    if unsafe { GetProcessTimes(process.0, &mut created, &mut exited, &mut kernel, &mut user) } == 0
    {
        return None;
    }
    let mut current_pid = 0;
    if unsafe { GetWindowThreadProcessId(window, &mut current_pid) } != thread || current_pid != pid
    {
        return None;
    }
    let created = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    Some(ApplicationInfo {
        bundle_id: format!("windows.exe:{}", path.to_str()?.to_lowercase()),
        name: path.file_stem()?.to_str()?.to_owned(),
        pid: i32::try_from(pid).ok()?,
        instance_id: Some(format!("{pid}:{created}:{}", window.addr())),
    })
}

pub(super) fn wide(value: &OsStr) -> Result<Vec<u16>> {
    let mut units: Vec<_> = value.encode_wide().collect();
    ensure!(!units.contains(&0), "shell target contains an embedded NUL");
    units.push(0);
    Ok(units)
}

pub(super) struct Apartment;

impl Apartment {
    pub(super) fn open() -> Result<Self> {
        let status = unsafe { CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) };
        ensure!(
            status >= 0,
            "could not initialize the Windows Shell (HRESULT {status:#x})"
        );
        Ok(Self)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

pub(super) fn open(target: &OsStr) -> Result<()> {
    let target = wide(target)?;
    let _apartment = Apartment::open()?;
    let status = unsafe {
        ShellExecuteW(
            null_mut(),
            null(),
            target.as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    ensure!(
        status > 32,
        "Windows could not open the target (ShellExecute {status})"
    );
    Ok(())
}

struct ShellItem(*mut ITEMIDLIST);

impl Drop for ShellItem {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(self.0.cast()) };
    }
}

pub(super) fn reveal(path: &Path) -> Result<()> {
    ensure!(path.is_absolute() && path.exists(), "item no longer exists");
    let path = wide(dunce::simplified(path).as_os_str())?;
    let _apartment = Apartment::open()?;
    let mut item = null_mut();
    let status = unsafe { SHParseDisplayName(path.as_ptr(), null_mut(), &mut item, 0, null_mut()) };
    ensure!(
        status >= 0 && !item.is_null(),
        "Windows could not resolve the target (HRESULT {status:#x})"
    );
    let item = ShellItem(item);
    let status = unsafe { SHOpenFolderAndSelectItems(item.0, 0, null(), 0) };
    ensure!(
        status >= 0,
        "Windows Explorer could not select the item (HRESULT {status:#x})"
    );
    Ok(())
}

pub(super) fn open_terminal(path: &Path) -> Result<()> {
    ensure!(
        path.is_absolute() && path.is_dir(),
        "directory no longer exists"
    );
    let mut system = vec![0_u16; 32_768];
    let length = unsafe { GetSystemDirectoryW(system.as_mut_ptr(), system.len() as u32) } as usize;
    ensure!(
        length > 0 && length < system.len(),
        "Windows system directory is unavailable"
    );
    let command = PathBuf::from(OsString::from_wide(&system[..length])).join("cmd.exe");
    Command::new(command)
        .arg("/D")
        .current_dir(dunce::simplified(path))
        .creation_flags(CREATE_NEW_CONSOLE)
        .spawn()?;
    Ok(())
}

pub(super) fn open_file_in_application(application: &Path, target: &Path) -> Result<()> {
    ensure!(
        target.is_absolute() && target.exists(),
        "history target no longer exists"
    );
    let executable = super::applications::resolve_launch_executable(application)?;
    Command::new(&executable)
        .arg(dunce::simplified(target))
        .spawn()?;
    Ok(())
}

pub(super) fn open_url_in_application(application: &Path, url_value: &str) -> Result<()> {
    let parsed = url::Url::parse(url_value)?;
    ensure!(
        matches!(parsed.scheme(), "http" | "https" | "obsidian"),
        "unsupported application URL scheme"
    );
    if parsed.scheme() == "obsidian" {
        return open(parsed.as_str().as_ref());
    }
    let executable = super::applications::resolve_launch_executable(application)?;
    Command::new(&executable).arg(parsed.as_str()).spawn()?;
    Ok(())
}
