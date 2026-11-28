use crate::{
    everything_wire, FileIndexResults, FileSearchBackend, FileSearchEngine, FileSearchHit,
    FileSearchQuery,
};
use anyhow::{bail, ensure, Context, Result};
use std::{
    borrow::Cow,
    cell::RefCell,
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::{Component, Path, PathBuf, Prefix},
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicU32, Ordering},
        OnceLock,
    },
    time::{Duration, Instant},
};
use windows_sys::{
    w,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::{DataExchange::COPYDATASTRUCT, LibraryLoader::GetModuleHandleW},
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, FindWindowW,
            MsgWaitForMultipleObjectsEx, PeekMessageW, RegisterClassW, SendMessageTimeoutW,
            HWND_MESSAGE, MSG, MWMO_INPUTAVAILABLE, PM_REMOVE, QS_SENDMESSAGE, SMTO_ABORTIFHUNG,
            SMTO_ERRORONEXIT, WM_COPYDATA, WM_USER, WNDCLASSW,
        },
    },
};

const QUERY2W: usize = 18;
const IS_DB_LOADED: usize = 401;
const MAX_RESULTS: u32 = 4096;
const MAX_REPLY_BYTES: usize = 16 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_millis(1500);
const CLASS: windows_sys::core::PCWSTR = w!("Anywhere.EverythingReply");
static NEXT_REPLY: AtomicU32 = AtomicU32::new(1);
static CLASS_REGISTERED: OnceLock<bool> = OnceLock::new();

pub(super) struct EverythingBackend;

impl FileSearchBackend for EverythingBackend {
    fn engine(&self) -> FileSearchEngine {
        FileSearchEngine::Everything
    }

    fn is_available(&self) -> bool {
        is_running()
    }

    fn search(
        &self,
        query: &FileSearchQuery<'_>,
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<FileIndexResults> {
        search(query.text, query.roots, cancelled)
    }
}

pub(super) fn is_running() -> bool {
    let server = unsafe { FindWindowW(w!("EVERYTHING_TASKBAR_NOTIFICATION"), null()) };
    !server.is_null()
}

struct PendingReply {
    id: u32,
    data: Option<Result<Vec<u8>>>,
}

thread_local! {
    static PENDING: RefCell<Option<PendingReply>> = const { RefCell::new(None) };
}

struct ReplyWindow(HWND);

impl ReplyWindow {
    fn open(id: u32) -> Result<Self> {
        let module = unsafe { GetModuleHandleW(null()) };
        ensure!(
            *CLASS_REGISTERED.get_or_init(|| {
                let class = WNDCLASSW {
                    lpfnWndProc: Some(receive),
                    hInstance: module,
                    lpszClassName: CLASS,
                    ..Default::default()
                };
                (unsafe { RegisterClassW(&class) }) != 0
            }),
            "could not register the Everything IPC reply window"
        );
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                CLASS,
                null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                null_mut(),
                module,
                null(),
            )
        };
        ensure!(
            !hwnd.is_null(),
            "could not open the Everything IPC reply window"
        );
        PENDING.with_borrow_mut(|pending| *pending = Some(PendingReply { id, data: None }));
        Ok(Self(hwnd))
    }

    fn wait(&self, deadline: Instant, cancelled: &(dyn Fn() -> bool + Sync)) -> Result<Vec<u8>> {
        loop {
            ensure!(!cancelled(), "search cancelled");
            let mut message = MSG::default();
            if unsafe { PeekMessageW(&mut message, self.0, 0, 0, PM_REMOVE) } != 0 {
                unsafe { DispatchMessageW(&message) };
            }
            if let Some(reply) = PENDING.with_borrow_mut(|pending| pending.as_mut()?.data.take()) {
                return reply;
            }
            let remaining = remaining_ms(deadline)?;
            unsafe {
                MsgWaitForMultipleObjectsEx(
                    0,
                    null(),
                    remaining.min(10),
                    QS_SENDMESSAGE,
                    MWMO_INPUTAVAILABLE,
                )
            };
        }
    }
}

impl Drop for ReplyWindow {
    fn drop(&mut self) {
        PENDING.with_borrow_mut(|pending| *pending = None);
        unsafe { DestroyWindow(self.0) };
    }
}

unsafe extern "system" fn receive(
    window: HWND,
    message: u32,
    sender: WPARAM,
    parameter: LPARAM,
) -> LRESULT {
    if message != WM_COPYDATA || parameter == 0 {
        return unsafe { DefWindowProcW(window, message, sender, parameter) };
    }
    let envelope = unsafe { &*(parameter as *const COPYDATASTRUCT) };
    PENDING.with_borrow_mut(|pending| {
        let Some(pending) = pending.as_mut() else {
            return 0;
        };
        if envelope.dwData != pending.id as usize || pending.data.is_some() {
            return 0;
        }
        let length = envelope.cbData as usize;
        if envelope.lpData.is_null() || length == 0 || length > MAX_REPLY_BYTES {
            pending.data = Some(Err(anyhow::anyhow!(
                "Everything IPC reply exceeds the size limit or is empty"
            )));
            return 0;
        }
        let bytes = unsafe { std::slice::from_raw_parts(envelope.lpData.cast::<u8>(), length) };
        pending.data = Some(Ok(bytes.to_vec()));
        1
    })
}

fn remaining_ms(deadline: Instant) -> Result<u32> {
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .as_millis();
    if remaining == 0 {
        bail!("Everything search timed out");
    }
    Ok(remaining as u32)
}

fn send(
    window: HWND,
    message: u32,
    sender: WPARAM,
    parameter: LPARAM,
    deadline: Instant,
) -> Result<usize> {
    let mut status = 0;
    let timeout = remaining_ms(deadline)?;
    let sent = unsafe {
        SendMessageTimeoutW(
            window,
            message,
            sender,
            parameter,
            SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT,
            timeout,
            &mut status,
        )
    };
    ensure!(sent != 0, "Everything IPC is unavailable or timed out");
    Ok(status)
}

pub(super) fn search(
    query: &str,
    roots: &[PathBuf],
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<FileIndexResults> {
    if roots.is_empty() || cancelled() {
        return Ok(FileIndexResults::default());
    }
    let roots: Vec<_> = roots
        .iter()
        .map(|root| (root.as_path(), index_path(root)))
        .collect();
    let search_roots = roots
        .iter()
        .map(|(_, indexed)| {
            indexed
                .to_str()
                .context("Everything search root is not Unicode")
        })
        .collect::<Result<Vec<_>>>()?;
    let pattern = everything_wire::pattern(query, &search_roots)?;
    let server = unsafe { FindWindowW(w!("EVERYTHING_TASKBAR_NOTIFICATION"), null()) };
    ensure!(
        !server.is_null(),
        "Start Everything 1.4.1 or later to search filenames"
    );
    let deadline = Instant::now() + TIMEOUT;
    ensure!(
        send(server, WM_USER, IS_DB_LOADED, 0, deadline)? != 0,
        "Everything is still loading its index"
    );
    ensure!(!cancelled(), "search cancelled");
    let id = NEXT_REPLY.fetch_add(1, Ordering::Relaxed);
    let window = ReplyWindow::open(id)?;
    let mut request = everything_wire::request(window.0 as usize as u32, id, &pattern, MAX_RESULTS);
    let envelope = COPYDATASTRUCT {
        dwData: QUERY2W,
        cbData: request.len() as u32,
        lpData: request.as_mut_ptr().cast(),
    };
    ensure!(
        send(
            server,
            WM_COPYDATA,
            window.0 as WPARAM,
            &raw const envelope as LPARAM,
            deadline
        )? != 0,
        "Everything rejected the file search request"
    );
    let reply = everything_wire::parse_reply(&window.wait(deadline, cancelled)?, MAX_RESULTS)?;
    let mut hits = Vec::with_capacity(reply.paths.len());
    for (path, is_directory) in reply.paths {
        if cancelled() {
            bail!("search cancelled");
        }
        let path = PathBuf::from(OsString::from_wide(&path));
        if let Some(path) = scoped_path(&path, &roots) {
            hits.push(FileSearchHit { path, is_directory });
        }
    }
    Ok(FileIndexResults {
        hits,
        truncated: reply.truncated,
    })
}

fn index_path(path: &Path) -> Cow<'_, Path> {
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return Cow::Borrowed(path);
    };
    match prefix.kind() {
        Prefix::VerbatimDisk(_) => path
            .to_str()
            .and_then(|value| value.strip_prefix(r"\\?\"))
            .map_or(Cow::Borrowed(path), |value| Cow::Borrowed(Path::new(value))),
        Prefix::VerbatimUNC(_, _) => path
            .to_str()
            .and_then(|value| value.strip_prefix(r"\\?\UNC\"))
            .map_or(Cow::Borrowed(path), |value| {
                Cow::Owned(PathBuf::from(format!(r"\\{value}")))
            }),
        _ => Cow::Borrowed(path),
    }
}

fn scoped_path(path: &Path, roots: &[(&Path, Cow<'_, Path>)]) -> Option<PathBuf> {
    let path = index_path(path);
    roots.iter().find_map(|(root, indexed)| {
        let mut remaining = path.components();
        for expected in indexed.components() {
            let actual = remaining.next()?;
            if !actual
                .as_os_str()
                .eq_ignore_ascii_case(expected.as_os_str())
            {
                return None;
            }
        }
        remaining
            .clone()
            .all(|part| matches!(part, Component::Normal(_)))
            .then(|| root.join(remaining.as_path()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_paths_keep_authorized_namespace_and_component_boundaries() {
        for (root, child, sibling) in [
            (r"\\?\C:\Data", r"c:\data\报告.txt", r"C:\Database\报告.txt"),
            (
                r"\\?\UNC\host\share\Data",
                r"\\host\share\Data\报告.txt",
                r"\\host\share\Database\报告.txt",
            ),
        ] {
            let root = Path::new(root);
            let roots = [(root, index_path(root))];
            assert_eq!(
                scoped_path(Path::new(child), &roots),
                Some(root.join("报告.txt"))
            );
            assert_eq!(scoped_path(Path::new(sibling), &roots), None);
        }
        let root = Path::new(r"\\?\C:\Data");
        assert_eq!(
            scoped_path(
                Path::new(r"C:\Data\..\outside.txt"),
                &[(root, index_path(root))]
            ),
            None
        );
    }

    #[test]
    fn everything_reply_window_ignores_stale_reply_ids() {
        let deadline = Instant::now() + TIMEOUT;
        let receiver = ReplyWindow::open(5).unwrap();
        let send_reply = |id, value: u8| {
            let mut body = [value];
            let message = COPYDATASTRUCT {
                dwData: id,
                cbData: 1,
                lpData: body.as_mut_ptr().cast(),
            };
            send(
                receiver.0,
                WM_COPYDATA,
                0,
                &raw const message as LPARAM,
                deadline,
            )
            .unwrap()
        };
        assert_eq!(send_reply(4, 99_u8), 0);
        assert_eq!(send_reply(5, 10_u8), 1);
        assert_eq!(receiver.wait(deadline, &|| false).unwrap(), [10]);
    }
}
