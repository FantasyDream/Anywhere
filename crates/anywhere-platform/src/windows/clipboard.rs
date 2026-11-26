mod files;

use anyhow::{ensure, Result};
use std::{
    ptr::null_mut,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{GlobalFree, HANDLE, HGLOBAL},
    System::{
        DataExchange::{
            CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
            IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
        },
        Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE},
    },
};

use crate::{
    clipboard::{ClipboardFormat, ClipboardPayload, FILES, MAX_PAYLOAD_BYTES, TEXT},
    ContextReadError,
};

const UNICODE_TEXT: u32 = 13;
const CF_DIB: u32 = 8;
const BMP_FILE_HEADER_BYTES: usize = 14;
const DIB_HEADER_MIN_BYTES: usize = 40;

pub(super) fn generation() -> Result<u32> {
    let generation = unsafe { GetClipboardSequenceNumber() };
    ensure!(generation != 0, ContextReadError::Unavailable);
    Ok(generation)
}

struct OpenClipboardGuard;

impl Drop for OpenClipboardGuard {
    fn drop(&mut self) {
        unsafe { CloseClipboard() };
    }
}

struct LockedClipboard(HGLOBAL);

impl Drop for LockedClipboard {
    fn drop(&mut self) {
        unsafe { GlobalUnlock(self.0) };
    }
}

pub(super) fn read_text(capacity: usize, expected: Option<u32>) -> Result<Option<String>> {
    read_contents(expected, || read_open_text(capacity))
}

pub(super) fn read_history(expected: u32) -> Result<Option<ClipboardPayload>> {
    read_contents(Some(expected), || {
        if unsafe { IsClipboardFormatAvailable(files::FORMAT) } != 0 {
            return files::read_open().map(Some);
        }
        let mut formats = Vec::new();
        let mut remaining = MAX_PAYLOAD_BYTES;

        if let Some(image) = read_open_image(remaining)? {
            remaining = remaining.saturating_sub(image.data.len());
            formats.push(image);
        }
        if let Some(text) = read_open_text(remaining)? {
            remaining = remaining.saturating_sub(text.len());
            formats.push(ClipboardFormat {
                name: TEXT.into(),
                data: text.into_bytes(),
            });
            for (label, wide_name) in [
                ("HTML Format", windows_sys::w!("HTML Format")),
                ("Rich Text Format", windows_sys::w!("Rich Text Format")),
            ] {
                let id = unsafe { RegisterClipboardFormatW(wide_name) };
                if id != 0 {
                    if let Some(bytes) = read_open_bytes(id, remaining)? {
                        remaining = remaining.saturating_sub(bytes.len());
                        formats.push(ClipboardFormat {
                            name: label.into(),
                            data: bytes,
                        });
                    }
                }
            }
        }
        Ok((!formats.is_empty()).then_some(ClipboardPayload { formats }))
    })
}

pub(super) fn write_payload(payload: &ClipboardPayload) -> Result<()> {
    payload.validate()?;
    if payload.formats.iter().any(|f| f.name == FILES) {
        return crate::clipboard::write(payload);
    }
    let mut pending: Vec<(u32, Vec<u8>)> = Vec::new();
    for format in &payload.formats {
        match format.name.as_str() {
            TEXT => {
                let text = std::str::from_utf8(&format.data)
                    .map_err(|_| ContextReadError::Unavailable)?;
                let mut wide: Vec<u16> = text.encode_utf16().collect();
                wide.push(0);
                let mut bytes = Vec::with_capacity(wide.len() * 2);
                for unit in wide {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
                pending.push((UNICODE_TEXT, bytes));
            }
            "HTML Format" => {
                let id = unsafe { RegisterClipboardFormatW(windows_sys::w!("HTML Format")) };
                if id != 0 {
                    pending.push((id, null_terminated_bytes(&format.data)));
                }
            }
            "Rich Text Format" => {
                let id = unsafe { RegisterClipboardFormatW(windows_sys::w!("Rich Text Format")) };
                if id != 0 {
                    pending.push((id, null_terminated_bytes(&format.data)));
                }
            }
            "PNG" | "image/png" => {
                let id = unsafe { RegisterClipboardFormatW(windows_sys::w!("PNG")) };
                if id != 0 {
                    pending.push((id, format.data.clone()));
                }
            }
            "image/bmp"
                if format.data.len() > BMP_FILE_HEADER_BYTES && &format.data[..2] == b"BM" =>
            {
                pending.push((CF_DIB, format.data[BMP_FILE_HEADER_BYTES..].to_vec()));
            }
            _ => {}
        }
    }
    if pending.is_empty() {
        return Err(ContextReadError::Unsupported.into());
    }
    ensure!(
        unsafe { OpenClipboard(null_mut()) } != 0,
        ContextReadError::Unavailable
    );
    let _clipboard = OpenClipboardGuard;
    ensure!(
        unsafe { EmptyClipboard() } != 0,
        ContextReadError::Unavailable
    );
    for (format_id, bytes) in pending {
        set_open_clipboard_bytes(format_id, &bytes)?;
    }
    Ok(())
}

fn null_terminated_bytes(data: &[u8]) -> Vec<u8> {
    if data.last() == Some(&0) {
        data.to_vec()
    } else {
        let mut out = Vec::with_capacity(data.len() + 1);
        out.extend_from_slice(data);
        out.push(0);
        out
    }
}

fn set_open_clipboard_bytes(format_id: u32, bytes: &[u8]) -> Result<()> {
    let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) };
    ensure!(!handle.is_null(), ContextReadError::Unavailable);
    let ptr = unsafe { GlobalLock(handle) }.cast::<u8>();
    if ptr.is_null() {
        unsafe { GlobalFree(handle) };
        return Err(ContextReadError::Unavailable.into());
    }
    unsafe {
        if !bytes.is_empty() {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        }
        GlobalUnlock(handle);
        if SetClipboardData(format_id, handle as HANDLE).is_null() {
            GlobalFree(handle);
            return Err(ContextReadError::Unavailable.into());
        }
    }
    Ok(())
}

fn read_open_bytes(format_id: u32, capacity: usize) -> Result<Option<Vec<u8>>> {
    if unsafe { IsClipboardFormatAvailable(format_id) } == 0 {
        return Ok(None);
    }
    let handle = unsafe { GetClipboardData(format_id) };
    if handle.is_null() {
        return Ok(None);
    }
    let size = unsafe { GlobalSize(handle) };
    if size == 0 {
        return Ok(None);
    }
    ensure!(size <= capacity, ContextReadError::TooLarge);
    let pointer = unsafe { GlobalLock(handle) }.cast::<u8>();
    if pointer.is_null() {
        return Ok(None);
    }
    let _locked = LockedClipboard(handle);
    let slice = unsafe { std::slice::from_raw_parts(pointer, size) };
    let trimmed_len = slice.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    Ok((trimmed_len > 0).then(|| slice[..trimmed_len].to_vec()))
}

fn read_open_image(capacity: usize) -> Result<Option<ClipboardFormat>> {
    let png_id = unsafe { RegisterClipboardFormatW(windows_sys::w!("PNG")) };
    if png_id != 0 {
        if let Some(bytes) = read_open_raw(png_id, capacity)? {
            return Ok(Some(ClipboardFormat {
                name: "PNG".into(),
                data: bytes,
            }));
        }
    }
    if let Some(dib) = read_open_raw(CF_DIB, capacity.saturating_sub(BMP_FILE_HEADER_BYTES))? {
        if let Some(bmp) = wrap_dib_as_bmp(&dib) {
            return Ok(Some(ClipboardFormat {
                name: "image/bmp".into(),
                data: bmp,
            }));
        }
    }
    Ok(None)
}

fn read_open_raw(format_id: u32, capacity: usize) -> Result<Option<Vec<u8>>> {
    if unsafe { IsClipboardFormatAvailable(format_id) } == 0 {
        return Ok(None);
    }
    let handle = unsafe { GetClipboardData(format_id) };
    if handle.is_null() {
        return Ok(None);
    }
    let size = unsafe { GlobalSize(handle) };
    if size == 0 {
        return Ok(None);
    }
    ensure!(size <= capacity, ContextReadError::TooLarge);
    let pointer = unsafe { GlobalLock(handle) }.cast::<u8>();
    if pointer.is_null() {
        return Ok(None);
    }
    let _locked = LockedClipboard(handle);
    Ok(Some(unsafe { std::slice::from_raw_parts(pointer, size) }.to_vec()))
}

fn wrap_dib_as_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    if dib.len() < DIB_HEADER_MIN_BYTES {
        return None;
    }
    let header_size = u32::from_le_bytes(dib[0..4].try_into().ok()?) as usize;
    if header_size < DIB_HEADER_MIN_BYTES || header_size > dib.len() {
        return None;
    }
    let bpp = u16::from_le_bytes(dib[14..16].try_into().ok()?);
    let compression = u32::from_le_bytes(dib[16..20].try_into().ok()?);
    let colors_used = u32::from_le_bytes(dib[32..36].try_into().ok()?) as usize;
    let palette_bytes = if colors_used > 0 {
        colors_used.checked_mul(4)?
    } else if bpp <= 8 {
        (1usize << bpp).checked_mul(4)?
    } else if compression == 3 && header_size == DIB_HEADER_MIN_BYTES {
        12
    } else {
        0
    };
    let pixel_offset = u32::try_from(BMP_FILE_HEADER_BYTES + header_size + palette_bytes).ok()?;
    let file_size = u32::try_from(BMP_FILE_HEADER_BYTES + dib.len()).ok()?;
    let mut bmp = Vec::with_capacity(file_size as usize);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&pixel_offset.to_le_bytes());
    bmp.extend_from_slice(dib);
    Some(bmp)
}

fn read_contents<T>(
    expected: Option<u32>,
    read: impl FnOnce() -> Result<Option<T>>,
) -> Result<Option<T>> {
    let excluded = unsafe {
        RegisterClipboardFormatW(windows_sys::w!(
            "ExcludeClipboardContentFromMonitorProcessing"
        ))
    };
    let include_history =
        unsafe { RegisterClipboardFormatW(windows_sys::w!("CanIncludeInClipboardHistory")) };
    ensure!(
        excluded != 0 && include_history != 0,
        ContextReadError::Unavailable
    );
    ensure!(
        unsafe { OpenClipboard(null_mut()) } != 0,
        ContextReadError::Unavailable
    );
    let _clipboard = OpenClipboardGuard;
    if !expected_matches(expected)?
        || unsafe { IsClipboardFormatAvailable(excluded) } != 0
        || !allows_history(include_history)
    {
        return Ok(None);
    }
    let value = read()?;
    Ok(if expected_matches(expected)? {
        value
    } else {
        None
    })
}

fn read_open_text(capacity: usize) -> Result<Option<String>> {
    if unsafe { IsClipboardFormatAvailable(UNICODE_TEXT) } == 0 {
        return Ok(None);
    }
    let handle = unsafe { GetClipboardData(UNICODE_TEXT) };
    ensure!(!handle.is_null(), ContextReadError::Unavailable);
    let length = unsafe { GlobalSize(handle) } / size_of::<u16>();
    ensure!(length > 0, ContextReadError::Unavailable);
    let pointer = unsafe { GlobalLock(handle) }.cast::<u16>();
    ensure!(!pointer.is_null(), ContextReadError::Unavailable);
    let _locked = LockedClipboard(handle);
    let units = unsafe { std::slice::from_raw_parts(pointer, length.min(capacity + 1)) };
    let end = units
        .iter()
        .position(|unit| *unit == 0)
        .ok_or(ContextReadError::TooLarge)?;
    let text = String::from_utf16(&units[..end]).map_err(|_| ContextReadError::Unavailable)?;
    ensure!(text.len() <= capacity, ContextReadError::TooLarge);
    Ok((!text.is_empty()).then_some(text))
}

fn allows_history(format: u32) -> bool {
    if unsafe { IsClipboardFormatAvailable(format) } == 0 {
        return true;
    }
    let handle = unsafe { GetClipboardData(format) };
    if handle.is_null() || unsafe { GlobalSize(handle) } < size_of::<u32>() {
        return false;
    }
    let pointer = unsafe { GlobalLock(handle) }.cast::<u32>();
    if pointer.is_null() {
        return false;
    }
    let _locked = LockedClipboard(handle);
    unsafe { pointer.read_unaligned() == 1 }
}

fn expected_matches(expected: Option<u32>) -> Result<bool> {
    match expected {
        Some(expected) => Ok(generation()? == expected),
        None => Ok(true),
    }
}

struct Observation {
    generation: Option<u32>,
    checked: Instant,
    copied: Option<Instant>,
}

pub(super) struct Tracker(Mutex<Observation>);

impl Tracker {
    pub(super) fn start() -> Arc<Self> {
        let tracker = Arc::new(Self(Mutex::new(Observation {
            generation: generation().ok(),
            checked: Instant::now(),
            copied: None,
        })));
        let weak = Arc::downgrade(&tracker);
        if let Err(error) = std::thread::Builder::new()
            .name("windows-clipboard-generation".into())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_millis(250));
                let Some(tracker) = weak.upgrade() else { break };
                tracker.observe();
            })
        {
            tracing::warn!(%error, "clipboard_generation_tracker_unavailable");
        }
        tracker
    }

    fn observe(&self) {
        if let Ok(mut observation) = self.0.lock() {
            let checked = Instant::now();
            let current = generation().ok();
            match (observation.generation, current) {
                (Some(previous), Some(current)) if previous != current => {
                    observation.copied = Some(observation.checked);
                }
                (None, _) | (_, None) => observation.copied = None,
                _ => {}
            }
            observation.generation = current;
            observation.checked = checked;
        }
    }

    pub(super) fn age_ms(&self, generation: u32) -> Option<u64> {
        self.observe();
        let observation = self.0.lock().ok()?;
        (observation.generation == Some(generation))
            .then(|| observation.copied?.elapsed().as_millis().try_into().ok())
            .flatten()
    }
}
