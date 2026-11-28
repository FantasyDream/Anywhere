//! Same-user document handoff for the portable Windows executable.
use crate::plugin_open::is_local_package;
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        io::{AsRawHandle, FromRawHandle},
    },
    path::{Path, PathBuf},
    ptr::null_mut,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use windows_sys::Win32::{
    Foundation::{GetLastError, LocalFree, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE},
    Security::{
        Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1},
        SECURITY_ATTRIBUTES,
    },
    Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND},
    System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PeekNamedPipe,
        PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE,
    },
};

const MAX_BYTES: usize = 64 * 1024;

fn pipe_name(sid: &str) -> String {
    format!(r"\\.\pipe\dev.anywhere.launcher.handoff.{sid}")
}

pub(super) fn send(path: &Path) -> bool {
    let Some(sid) = super::preview::current_user_sid() else {
        return false;
    };
    if !is_local_package(path) {
        return false;
    }
    let message: Vec<u8> = path
        .as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .take(MAX_BYTES + 2)
        .collect();
    if message.len() > MAX_BYTES {
        return false;
    }
    let pipe = pipe_name(&sid);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match OpenOptions::new().write(true).open(&pipe) {
            Ok(mut file) => {
                super::tray::allow_existing_foreground();
                return file
                    .write(&message)
                    .is_ok_and(|count| count == message.len());
            }
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            Err(_) => return false,
        }
    }
}

pub(super) fn listen(sender: mpsc::Sender<PathBuf>) {
    let Some(sid) = super::preview::current_user_sid() else {
        return;
    };
    let pipe = match create_pipe(&sid) {
        Ok(pipe) => pipe,
        Err(error) => {
            tracing::warn!(%error, "plugin_handoff_pipe_unavailable");
            return;
        }
    };
    let _ = std::thread::Builder::new()
        .name("anywhere-plugin-handoff".into())
        .spawn(move || {
            let mut pipe = pipe;
            while !sender.is_closed() {
                let handle = pipe.as_raw_handle();
                // SAFETY: pipe owns this synchronous handle. A client may have connected
                // between CreateNamedPipe and this call.
                let connected = unsafe {
                    ConnectNamedPipe(handle, null_mut()) != 0
                        || GetLastError() == ERROR_PIPE_CONNECTED
                };
                if connected {
                    if let Some(path) = read_path(&mut pipe) {
                        if sender.try_send(path).is_err() {
                            tracing::warn!("plugin_handoff_queue_full");
                        }
                    }
                }
                // SAFETY: the listening instance remains owned by this thread throughout.
                unsafe { DisconnectNamedPipe(handle) };
            }
        });
}

fn create_pipe(sid: &str) -> std::io::Result<File> {
    let name: Vec<_> = pipe_name(sid).encode_utf16().chain([0]).collect();
    let descriptor: Vec<_> = format!("D:P(A;;GA;;;{sid})")
        .encode_utf16()
        .chain([0])
        .collect();
    let mut security = null_mut();
    // SAFETY: both NUL-terminated strings and the output pointer live through the native calls.
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor.as_ptr(),
            SDDL_REVISION_1,
            &mut security,
            null_mut(),
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: security,
        bInheritHandle: 0,
    };
    // SAFETY: the initialized security descriptor and the pipe name outlive creation.
    let handle = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            0,
            MAX_BYTES as u32,
            0,
            &attributes,
        )
    };
    let result = if handle == INVALID_HANDLE_VALUE {
        Err(std::io::Error::last_os_error())
    } else {
        // SAFETY: successful creation returned a newly owned HANDLE.
        Ok(unsafe { File::from_raw_handle(handle) })
    };
    // SAFETY: the converter allocated this descriptor for a matching LocalFree.
    unsafe { LocalFree(security) };
    result
}

fn read_path(pipe: &mut File) -> Option<PathBuf> {
    let deadline = Instant::now() + Duration::from_millis(250);
    let mut bytes = 0u32;
    loop {
        // SAFETY: pipe owns the current connection, and bytes is a writable DWORD.
        let ok = unsafe {
            PeekNamedPipe(
                pipe.as_raw_handle(),
                null_mut(),
                0,
                null_mut(),
                null_mut(),
                &mut bytes,
            )
        };
        if ok == 0 || Instant::now() >= deadline {
            return None;
        }
        if bytes != 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let count = usize::try_from(bytes).ok()?;
    if count > MAX_BYTES || count % 2 != 0 {
        return None;
    }
    let mut message = vec![0u8; count];
    if pipe.read(&mut message).ok()? != count {
        return None;
    }
    let path: Vec<u16> = message
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&unit| u16::from_le_bytes(unit))
        .collect();
    if path.contains(&0) {
        return None;
    }
    let path = PathBuf::from(OsString::from_wide(&path));
    is_local_package(&path).then_some(path)
}
