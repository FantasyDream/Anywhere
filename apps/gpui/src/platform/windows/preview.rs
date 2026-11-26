//! Windows preview adapter integrating with QuickLook for Windows when installed.
use crate::platform::preview::PreviewPoll;
use anyhow::{bail, Result};
use std::{
    fs::OpenOptions,
    io::Write as _,
    path::Path,
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, HANDLE},
    Security::{
        Authorization::ConvertSidToStringSidW, GetTokenInformation, TokenUser, TOKEN_QUERY,
        TOKEN_USER,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

pub struct PreviewHost {
    pipe_path: String,
    active: Option<String>,
}

impl PreviewHost {
    pub fn new() -> Result<Self> {
        let sid = current_user_sid()
            .ok_or_else(|| anyhow::anyhow!("QuickLook for Windows is unavailable"))?;
        let pipe_path = format!(r"\\.\pipe\QuickLook.App.Pipe.{sid}");
        if !Path::new(&pipe_path).exists() {
            bail!("Install and run QuickLook for Windows to preview files with Space");
        }
        Ok(Self {
            pipe_path,
            active: None,
        })
    }

    pub fn show(
        &mut self,
        resource_id: String,
        paths: &[String],
        _owner_title: &str,
    ) -> Result<Option<String>> {
        let Some(first) = paths.first() else {
            bail!("invalid preview selection");
        };
        let command = if self.active.is_some() {
            "QuickLook.App.PipeMessages.Switch"
        } else {
            "QuickLook.App.PipeMessages.Toggle"
        };
        send_pipe_message(&self.pipe_path, &format!("{command}|{first}\n"))?;
        Ok(self.active.replace(resource_id))
    }

    pub fn poll(&mut self) -> PreviewPoll {
        PreviewPoll::Open
    }

    pub fn owns_panel(&self) -> bool {
        self.active.is_some()
    }

    pub fn close(&mut self) -> Option<String> {
        let active = self.active.take()?;
        let _ = send_pipe_message(&self.pipe_path, "QuickLook.App.PipeMessages.Close|\n");
        Some(active)
    }
}

fn send_pipe_message(pipe_path: &str, message: &str) -> Result<()> {
    let mut pipe = OpenOptions::new().write(true).open(pipe_path)?;
    pipe.write_all(message.as_bytes())?;
    pipe.flush()?;
    Ok(())
}

fn current_user_sid() -> Option<String> {
    let mut token: HANDLE = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0
        || token.is_null()
    {
        return None;
    }
    struct TokenGuard(HANDLE);
    impl Drop for TokenGuard {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }
    let _guard = TokenGuard(token);
    let mut needed = 0u32;
    unsafe { GetTokenInformation(token, TokenUser, null_mut(), 0, &mut needed) };
    if needed == 0 || needed > 4096 {
        return None;
    }
    let mut buffer = vec![0u8; needed as usize];
    if unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    } == 0
    {
        return None;
    }
    let user = unsafe { &*(buffer.as_ptr().cast::<TOKEN_USER>()) };
    let mut wide_sid: *mut u16 = null_mut();
    if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut wide_sid) } == 0 || wide_sid.is_null() {
        return None;
    }
    let mut len = 0usize;
    while unsafe { *wide_sid.add(len) } != 0 && len < 256 {
        len += 1;
    }
    let slice = unsafe { std::slice::from_raw_parts(wide_sid, len) };
    let sid = String::from_utf16(slice).ok();
    unsafe { LocalFree(wide_sid.cast()) };
    sid
}
