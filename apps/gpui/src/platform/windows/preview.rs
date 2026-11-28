//! Windows preview adapter integrating with QuickLook for Windows when installed.
use crate::platform::preview::PreviewPoll;
use anyhow::{anyhow, ensure, Context as _, Result};
use std::{
    cell::RefCell,
    fs::{File, OpenOptions},
    io::Write as _,
    mem::size_of,
    os::windows::io::AsRawHandle as _,
    path::Path,
    ptr::null_mut,
    time::{Duration, Instant},
};
use windows_sys::core::BOOL;
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, ERROR_PIPE_BUSY, HANDLE, HWND, LPARAM},
    Security::{
        Authorization::ConvertSidToStringSidW, GetTokenInformation, TokenUser, TOKEN_QUERY,
        TOKEN_USER,
    },
    System::{
        Pipes::{GetNamedPipeServerProcessId, WaitNamedPipeW},
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
    UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowTextW, IsWindowVisible,
        PostMessageW, WM_CLOSE,
    },
};

const OPEN_TIMEOUT: Duration = Duration::from_secs(3);
const PIPE_CONNECT_WAIT_MS: u32 = 50;

pub struct PreviewHost {
    pipe_path: String,
    active: Option<String>,
}

struct PreviewPanel {
    resource_id: String,
    pid: u32,
    visible_before_request: Vec<HWND>,
    deadline: Instant,
    window: Option<HWND>,
    owner: Option<super::NativeWindow>,
    file_name: String,
}

thread_local! {
    // QuickLook has one unpinned viewer per process. The Shelf and Launcher
    // can each own a lease, but only the latest request can control the viewer.
    static PANEL: RefCell<Option<PreviewPanel>> = const { RefCell::new(None) };
}

impl PreviewHost {
    pub fn new() -> Result<Self> {
        let sid = current_user_sid().context(WindowsPreviewError::NotRunning)?;
        Ok(Self {
            pipe_path: format!(r"\\.\pipe\QuickLook.App.Pipe.{sid}"),
            active: None,
        })
    }

    pub fn show(
        &mut self,
        resource_id: String,
        paths: &[String],
        owner_title: &str,
    ) -> Result<Option<String>> {
        let first = paths.first().context("invalid preview selection")?;
        let path = anywhere_platform::paths::display_path(Path::new(first));
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let path = path
            .to_str()
            .context("preview path must be valid Unicode")?;
        ensure!(!path.contains(['|', '\r', '\n']), "invalid preview path");
        let (pipe, pid) = open_pipe(&self.pipe_path).context(WindowsPreviewError::NotRunning)?;

        let mut excluded = viewer_windows(pid);
        let reusable = PANEL.with_borrow(|active| {
            active
                .as_ref()
                .filter(|previous| previous.pid == pid && previous.window.is_some())
                .and_then(PreviewPanel::observe_window)
                .filter(|window| excluded.contains(window))
        });
        if let Some(window) = reusable {
            excluded.retain(|candidate| *candidate != window);
        } else if !excluded.is_empty() {
            // Invoke has no ownership token. It could otherwise replace an
            // existing QuickLook window opened outside Anywhere.
            return Err(WindowsPreviewError::ExistingWindow.into());
        }
        pipe.send(&format!("QuickLook.App.PipeMessages.Invoke|{path}\n"))?;
        PANEL.with_borrow_mut(|active| {
            *active = Some(PreviewPanel {
                resource_id: resource_id.clone(),
                pid,
                visible_before_request: excluded,
                deadline: Instant::now() + OPEN_TIMEOUT,
                window: None,
                owner: super::window(owner_title),
                file_name: file_name.to_owned(),
            });
        });
        Ok(self.active.replace(resource_id))
    }

    pub fn poll(&mut self) -> PreviewPoll {
        let Some(resource_id) = self.active.as_ref() else {
            return PreviewPoll::Open;
        };
        let closed = PANEL.with_borrow_mut(|active| {
            let Some(panel) = active.as_mut().filter(|p| &p.resource_id == resource_id) else {
                return Some(None);
            };
            if let Some(window) = panel.observe_window() {
                panel.window = Some(window);
                return None;
            }
            if panel.window.is_none() && Instant::now() < panel.deadline {
                return None;
            }
            Some(active.take())
        });
        let Some(closed) = closed else {
            return PreviewPoll::Open;
        };
        let application_active = closed.is_some_and(|panel| {
            if panel.window.is_none() {
                panel.cancel_opening(&self.pipe_path);
            }
            panel.owner.is_some_and(|owner| {
                owner.is_live() && unsafe { GetForegroundWindow() } == owner.hwnd
            })
        });
        PreviewPoll::Closed {
            resource_id: resource_id.clone(),
            application_active,
        }
    }

    pub fn owns_panel(&self) -> bool {
        PANEL.with_borrow(|active| {
            active
                .as_ref()
                .is_some_and(|panel| self.active.as_ref() == Some(&panel.resource_id))
        })
    }

    pub fn close(&mut self) -> Option<String> {
        let resource_id = self.active.take()?;
        let panel = PANEL.with_borrow_mut(|active| {
            if active
                .as_ref()
                .is_some_and(|panel| panel.resource_id == resource_id)
            {
                active.take()
            } else {
                None
            }
        });
        if let Some(panel) = panel {
            if let Some(window) = panel.observe_window() {
                // Target just this observed viewer, even if QuickLook has other windows.
                unsafe { PostMessageW(window, WM_CLOSE, 0, 0) };
            } else if panel.window.is_none() {
                panel.cancel_opening(&self.pipe_path);
            }
        }
        Some(resource_id)
    }
}

struct QuickLookPipe {
    file: File,
    sent_line: bool,
}

impl QuickLookPipe {
    fn send(mut self, line: &str) -> std::io::Result<()> {
        self.sent_line = true;
        self.file.write_all(line.as_bytes())
    }
}

impl Drop for QuickLookPipe {
    fn drop(&mut self) {
        // QuickLook expects ReadLine to return a string for every connection.
        // An empty line is ignored before it can cancel a queued request.
        if !self.sent_line {
            let _ = self.file.write_all(b"\n");
        }
    }
}

fn open_pipe(path: &str) -> Result<(QuickLookPipe, u32)> {
    let options = &mut OpenOptions::new();
    options.write(true);
    let pipe = match options.open(path) {
        Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
            // A busy single-instance pipe is usually disconnecting a previous command.
            let wide_path: Vec<_> = path.encode_utf16().chain(std::iter::once(0)).collect();
            if unsafe { WaitNamedPipeW(wide_path.as_ptr(), PIPE_CONNECT_WAIT_MS) } == 0 {
                return Err(error.into());
            }
            options.open(path)?
        }
        result => result?,
    };
    let pipe = QuickLookPipe {
        file: pipe,
        sent_line: false,
    };
    let mut pid = 0;
    if unsafe { GetNamedPipeServerProcessId(pipe.file.as_raw_handle(), &mut pid) } == 0 || pid == 0
    {
        return Err(anyhow!("could not identify the QuickLook pipe server"));
    }
    Ok((pipe, pid))
}

impl PreviewPanel {
    fn observe_window(&self) -> Option<HWND> {
        if let Some(window) = self.window {
            return (is_viewer_window(window, self.pid)
                && is_preview_caption(window, &self.file_name))
            .then_some(window);
        }
        self.new_viewer_in(
            viewer_windows(self.pid)
                .into_iter()
                .filter(|window| is_preview_caption(*window, &self.file_name)),
        )
    }

    fn new_viewer_in(&self, windows: impl IntoIterator<Item = HWND>) -> Option<HWND> {
        let mut candidates = windows
            .into_iter()
            .filter(|window| !self.visible_before_request.contains(window));
        let window = candidates.next()?;
        candidates.next().is_none().then_some(window)
    }

    fn cancel_opening(&self, pipe_path: &str) {
        // The third-party protocol only has a global Close. Send it while
        // the server has no other visible window and no pre-existing viewer.
        if !self.visible_before_request.is_empty() || !viewer_windows(self.pid).is_empty() {
            return;
        }
        if let Ok((pipe, pid)) = open_pipe(pipe_path) {
            if pid == self.pid {
                let _ = pipe.send("QuickLook.App.PipeMessages.Close|\n");
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum WindowsPreviewError {
    NotRunning,
    ExistingWindow,
}

impl std::fmt::Display for WindowsPreviewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::NotRunning => "could not open the QuickLook for Windows pipe",
            Self::ExistingWindow => "another QuickLook window is still open",
        })
    }
}

impl std::error::Error for WindowsPreviewError {}

fn is_preview_caption(window: HWND, file_name: &str) -> bool {
    let mut caption = [0u16; 512];
    let length = unsafe { GetWindowTextW(window, caption.as_mut_ptr(), caption.len() as i32) };
    let caption = String::from_utf16_lossy(&caption[..length.max(0) as usize]);
    preview_caption_matches(&caption, file_name)
}

fn preview_caption_matches(caption: &str, file_name: &str) -> bool {
    if caption == "QuickLook" {
        return true;
    }
    if file_name.is_empty() {
        return false;
    }
    if caption == file_name {
        return true;
    }

    let Some(title) = caption.strip_prefix("QuickLook - ") else {
        return false;
    };
    if title == file_name {
        return true;
    }

    // QuickLook 4.5 uses both `QuickLook - <name>` and
    // `QuickLook - 1 / 1: <name>` while the viewer is loading.
    title
        .strip_suffix(file_name)
        .map(str::trim_end)
        .is_some_and(|prefix| !prefix.is_empty() && prefix.ends_with(':'))
}

pub(super) fn owns_foreground(window: HWND) -> bool {
    PANEL.with_borrow(|panel| panel.as_ref().and_then(PreviewPanel::observe_window) == Some(window))
}

fn viewer_windows(pid: u32) -> Vec<HWND> {
    unsafe extern "system" fn collect(hwnd: HWND, data: LPARAM) -> BOOL {
        let (pid, windows) = unsafe { &mut *(data as *mut (u32, Vec<HWND>)) };
        if is_viewer_window(hwnd, *pid) {
            windows.push(hwnd);
        }
        1
    }
    let mut data = (pid, Vec::new());
    unsafe { EnumWindows(Some(collect), std::ptr::from_mut(&mut data) as LPARAM) };
    data.1
}

fn is_viewer_window(window: HWND, pid: u32) -> bool {
    if super::process_id(window) != Some(pid) || unsafe { IsWindowVisible(window) } == 0 {
        return false;
    }
    // QuickLook's viewer uses a top-level WPF window. The process also
    // owns native utility windows that should not hold a preview lease.
    let mut class = [0u16; 128];
    let length = unsafe { GetClassNameW(window, class.as_mut_ptr(), class.len() as i32) };
    let prefix = b"HwndWrapper[";
    length.max(0) as usize >= prefix.len()
        && class
            .iter()
            .zip(prefix)
            .all(|(&window_unit, &expected)| window_unit == u16::from(expected))
}

pub(super) fn current_user_sid() -> Option<String> {
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
    let mut buffer = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
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
    while len < 256 && unsafe { *wide_sid.add(len) } != 0 {
        len += 1;
    }
    let slice = unsafe { std::slice::from_raw_parts(wide_sid, len) };
    let sid = String::from_utf16(slice).ok();
    unsafe { LocalFree(wide_sid.cast()) };
    sid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quicklook_caption_forms_match_only_the_expected_file() {
        assert!(preview_caption_matches("QuickLook", ""));
        assert!(preview_caption_matches(
            "QuickLook - 报告 2026.txt",
            "报告 2026.txt"
        ));
        assert!(preview_caption_matches(
            "QuickLook - 1 / 1: 小文件.pdf",
            "小文件.pdf"
        ));
        assert!(!preview_caption_matches(
            "QuickLook - 另一个文件.pdf",
            "小文件.pdf"
        ));
        assert!(!preview_caption_matches(
            "QuickLook - 1 / 1: 另一个文件.pdf",
            "小文件.pdf"
        ));
        assert!(!preview_caption_matches("另一个文件.pdf", "小文件.pdf"));
    }

    #[test]
    fn preview_closed_or_replaced_releases_only_its_own_resource() {
        let mut previous = PreviewHost {
            pipe_path: String::new(),
            active: Some("old".into()),
        };
        let mut current = PreviewHost {
            pipe_path: String::new(),
            active: Some("current".into()),
        };
        PANEL.with_borrow_mut(|active| {
            *active = Some(PreviewPanel {
                resource_id: "current".into(),
                pid: std::process::id(),
                visible_before_request: viewer_windows(std::process::id()),
                deadline: Instant::now(),
                window: None,
                owner: None,
                file_name: String::new(),
            });
        });

        assert!(
            matches!(previous.poll(), PreviewPoll::Closed { resource_id, .. } if resource_id == "old")
        );
        assert_eq!(previous.close().as_deref(), Some("old"));
        assert!(current.owns_panel());
        assert!(
            matches!(current.poll(), PreviewPoll::Closed { resource_id, .. } if resource_id == "current")
        );
        assert_eq!(current.close().as_deref(), Some("current"));
        assert!(!current.owns_panel());
    }
    #[test]
    fn visible_windows_from_another_request_are_never_adopted_as_new_viewers() {
        let unrelated = std::ptr::without_provenance_mut(1);
        let viewer = std::ptr::without_provenance_mut(2);
        let duplicate = std::ptr::without_provenance_mut(3);
        let panel = PreviewPanel {
            resource_id: "current".into(),
            pid: 1,
            visible_before_request: vec![unrelated],
            deadline: Instant::now(),
            window: None,
            owner: None,
            file_name: String::new(),
        };
        assert_eq!(panel.new_viewer_in([unrelated]), None);
        assert_eq!(panel.new_viewer_in([unrelated, viewer]), Some(viewer));
        assert_eq!(panel.new_viewer_in([unrelated, viewer, duplicate]), None);
    }
}
