//! clipboard-rs owns format IO. macOS guards Cocoa failures at the platform boundary.
use super::*;
use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
#[cfg(not(target_os = "macos"))]
use clipboard_rs::{ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext, WatcherShutdown};

#[cfg(target_os = "macos")]
fn native_operation<T>(operation: impl FnOnce() -> Result<T>) -> Result<T> {
    // Cocoa can throw when the pasteboard owner changes during a read. Rust's
    // catch_unwind cannot catch foreign exceptions and aborts the whole process.
    objc2::exception::catch(std::panic::AssertUnwindSafe(operation))
        .map_err(|_| ContextReadError::Unavailable)?
}

fn context<T>(operation: impl FnOnce(&ClipboardContext) -> Result<T>) -> Result<T> {
    let run = || {
        let clipboard = ClipboardContext::new().map_err(|_| ContextReadError::Unavailable)?;
        operation(&clipboard)
    };
    #[cfg(target_os = "macos")]
    {
        native_operation(|| objc2::rc::autoreleasepool(|_| run()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        run()
    }
}
#[cfg(target_os = "macos")]
fn allowed() -> bool {
    // SAFETY: a parameter-free query of system access policy, without reading content.
    unsafe { crate::macos::native::launcher_clipboard_history_access_allowed() == 1 }
}
#[cfg(not(target_os = "macos"))]
fn allowed() -> bool {
    true
}
#[cfg(target_os = "macos")]
fn same_generation(generation: i64) -> bool {
    // SAFETY: generation-only query.
    unsafe { crate::macos::native::launcher_clipboard_change_count() == generation }
}
#[cfg(not(target_os = "macos"))]
fn same_generation(_: i64) -> bool {
    true
}

#[cfg(target_os = "macos")]
fn guard_format(name: &str, capacity: usize) -> Result<bool> {
    let name = std::ffi::CString::new(name)?;
    // SAFETY: NUL-terminated format identifier, no output pointers.
    let status = unsafe {
        crate::macos::native::launcher_clipboard_format_guard(name.as_ptr(), capacity as u64)
    };
    match status {
        1 => Ok(true),
        0 => Ok(false),
        3 => Err(ContextReadError::TooLarge.into()),
        4 => Err(ContextReadError::PermissionDenied.into()),
        _ => Err(ContextReadError::Unavailable.into()),
    }
}
#[cfg(not(target_os = "macos"))]
fn guard_format(_: &str, _: usize) -> Result<bool> {
    Ok(true)
}
fn persistent_format(name: &str) -> bool {
    valid_format_name(name)
        && !sensitive_format(name)
        && !matches!(
            name,
            "com.apple.pasteboard.promised-file-url"
                | "com.apple.pasteboard.promised-file-content-type"
                | "com.apple.pasteboard.promised-file-name"
        )
}

pub fn read(generation: i64) -> Result<Option<ClipboardPayload>> {
    #[cfg(target_os = "macos")]
    ensure!(generation >= 0, ContextReadError::Unavailable);
    if !same_generation(generation) {
        return Ok(None);
    }
    ensure!(allowed(), ContextReadError::PermissionDenied);
    let payload = context(|clipboard| collect(clipboard, guard_format, guard_files))?;
    ensure!(allowed(), ContextReadError::PermissionDenied);
    if !same_generation(generation) {
        return Ok(None);
    }
    Ok(payload)
}
#[cfg(target_os = "macos")]
fn guard_files() -> Result<()> {
    // SAFETY: checks item count before the library allocates paths.
    ensure!(
        unsafe { crate::macos::native::launcher_clipboard_item_count() } <= 4096,
        ContextReadError::TooManyItems
    );
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn guard_files() -> Result<()> {
    Ok(())
}
fn collect(
    clipboard: &dyn Clipboard,
    check_format: impl Fn(&str, usize) -> Result<bool>,
    check_files: impl Fn() -> Result<()>,
) -> Result<Option<ClipboardPayload>> {
    let mut names = clipboard
        .available_formats()
        .map_err(|_| ContextReadError::Unavailable)?;
    if names.iter().any(|n| sensitive_format(n)) {
        return Ok(None);
    }
    ensure!(names.len() <= MAX_FORMATS, ContextReadError::TooManyItems);
    names.sort();
    names.dedup();
    let mut payload = ClipboardPayload::default();
    // clipboard-rs reconstructs the complete file list across pasteboard items.
    if clipboard.has(clipboard_rs::ContentFormat::Files) {
        check_files()?;
        let paths = clipboard
            .get_files()
            .map_err(|_| ContextReadError::Unavailable)?;
        payload.formats.push(ClipboardFormat {
            name: FILES.into(),
            data: serde_json::to_vec(&paths)?,
        });
        // Do not replay Finder's private move/cut or promise metadata.
    } else {
        for name in names {
            if !persistent_format(&name) {
                continue;
            }
            let remaining = MAX_PAYLOAD_BYTES.saturating_sub(payload.byte_count());
            if !check_format(&name, remaining)? {
                continue;
            }
            let Ok(data) = clipboard.get_buffer(&name) else {
                continue;
            };
            ensure!(data.len() <= remaining, ContextReadError::TooLarge);
            payload.formats.push(ClipboardFormat { name, data });
        }
        // Store a canonical UTF-8 text representation for searching and portable restore.
        if clipboard.has(clipboard_rs::ContentFormat::Text) {
            let text = clipboard
                .get_text()
                .map_err(|_| ContextReadError::Unavailable)?;
            // AppKit advertises legacy aliases for the same plain text. Keeping
            // those as independent formats makes copying a migrated entry create
            // another history item, even though its text has not changed.
            payload.formats.retain(|f| !plain_text_alias(&f.name));
            payload.formats.push(ClipboardFormat {
                name: TEXT.into(),
                data: text.into_bytes(),
            });
        }
    }
    if payload.formats.is_empty() {
        return Ok(None);
    }
    payload.validate()?;
    Ok(Some(payload))
}

fn plain_text_alias(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "text/plain;charset=utf-8"
            | "text/plain"
            | "utf8_string"
            | "string"
            | "text"
            | "cf_unicodetext"
            | "cf_text"
            | "nsstringpboardtype"
            | "public.utf8-plain-text"
            | "public.utf16-plain-text"
            | "public.utf16-external-plain-text"
            | "corepasteboardflavortype 0x75743136"
            | "corepasteboardflavortype 0x75747874"
            | "corepasteboardflavortype 0x54455854"
    )
}

pub fn write(payload: &ClipboardPayload) -> Result<()> {
    payload.validate()?;
    let files = payload.files()?;
    // Reject partial file restores: the library silently filters missing paths on macOS.
    if !files.is_empty() {
        ensure!(
            files.iter().all(|p| std::path::Path::new(p).exists()),
            "Some copied files no longer exist or cannot be accessed."
        );
        return context(|c| {
            c.set_files(files)
                .map_err(|_| ContextReadError::Unavailable.into())
        });
    }
    let contents = payload
        .formats
        .iter()
        .map(|format| {
            if format.name == TEXT {
                ClipboardContent::Text(String::from_utf8_lossy(&format.data).into_owned())
            } else {
                ClipboardContent::Other(format.name.clone(), format.data.clone())
            }
        })
        .collect();
    context(|c| {
        c.set(contents)
            .map_err(|_| ContextReadError::Unavailable.into())
    })
}
pub fn copy_text(text: &str) -> Result<()> {
    context(|c| {
        c.set_text(text.to_owned())
            .map_err(|_| ContextReadError::Unavailable.into())
    })
}
pub fn request_access() -> Result<()> {
    // Explicit Enable only; a read may present the OS prompt, but is never saved.
    context(|c| {
        let _ = c.get_text();
        Ok(())
    })?;
    ensure!(allowed(), ContextReadError::PermissionDenied);
    Ok(())
}
pub fn read_text(capacity: usize) -> Result<Option<String>> {
    context(|c| {
        if !c.has(clipboard_rs::ContentFormat::Text) {
            return Ok(None);
        }
        // This foreground path preserves the existing system prompt behavior.
        #[cfg(target_os = "macos")]
        {
            // SAFETY: bounded foreground size query, may request clipboard access.
            let status =
                unsafe { crate::macos::native::launcher_clipboard_text_guard(capacity as u64) };
            match status {
                1 => {}
                2 => return Ok(None),
                3 => return Err(ContextReadError::TooLarge.into()),
                4 => return Err(ContextReadError::PermissionDenied.into()),
                _ => return Err(ContextReadError::Unavailable.into()),
            }
        }
        let text = c.get_text().map_err(|_| ContextReadError::Unavailable)?;
        ensure!(text.len() <= capacity, ContextReadError::TooLarge);
        Ok((!text.is_empty()).then_some(text))
    })
}
#[cfg(not(target_os = "macos"))]
struct Handler(Box<dyn Fn() + Send>);
#[cfg(not(target_os = "macos"))]
impl ClipboardHandler for Handler {
    fn on_clipboard_change(&mut self) {
        (self.0)();
    }
}
#[cfg(not(target_os = "macos"))]
struct Watch(Option<WatcherShutdown>);
#[cfg(not(target_os = "macos"))]
impl ClipboardWatch for Watch {}
#[cfg(not(target_os = "macos"))]
impl Drop for Watch {
    fn drop(&mut self) {
        if let Some(shutdown) = self.0.take() {
            shutdown.stop();
        }
    }
}
#[cfg(not(target_os = "macos"))]
pub fn watch(changed: Box<dyn Fn() + Send>) -> Result<Box<dyn ClipboardWatch>> {
    let mut watcher = ClipboardWatcherContext::new().map_err(|_| ContextReadError::Unavailable)?;
    watcher.add_handler(Handler(changed));
    let shutdown = watcher.get_shutdown_channel();
    std::thread::Builder::new()
        .name("clipboard-rs-watch".into())
        .spawn(move || watcher.start_watch())?;
    Ok(Box::new(Watch(Some(shutdown))))
}

#[cfg(target_os = "macos")]
pub fn watch(changed: Box<dyn Fn() + Send>) -> Result<Box<dyn ClipboardWatch>> {
    // clipboard-rs uses the same 500 ms changeCount polling on macOS, but its
    // watcher calls Cocoa outside an exception boundary. Keep format IO in the
    // library and use the guarded metadata-only native query for notifications.
    super::macos_watch::watch(changed)
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
