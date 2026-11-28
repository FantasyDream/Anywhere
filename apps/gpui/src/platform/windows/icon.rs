use std::cell::RefCell;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::{
        HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
        WindowsAndMessaging::{
            CreateIconFromResourceEx, DestroyIcon, GetSystemMetrics, SendMessageW, HICON, ICON_BIG,
            ICON_SMALL, LR_DEFAULTCOLOR, SM_CXICON, SM_CXSMICON, WM_SETICON,
        },
    },
};

const APP_ICON_BYTES: &[u8] = include_bytes!("../../../../../resources/icon/Anywhere.ico");
const ICON_RESOURCE_VERSION: u32 = 0x0003_0000;

pub(super) struct OwnedIcon(HICON);

impl OwnedIcon {
    pub(super) fn raw(&self) -> HICON {
        self.0
    }
}

impl Drop for OwnedIcon {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                DestroyIcon(self.0);
            }
        }
    }
}

#[derive(Default)]
struct WindowIcons {
    small: Option<OwnedIcon>,
    big: Option<OwnedIcon>,
}

thread_local! {
    static WINDOW_ICONS: RefCell<Option<WindowIcons>> = const { RefCell::new(None) };
}

pub(super) fn create_tray_icon(hwnd: HWND) -> Option<OwnedIcon> {
    let size = small_icon_metric(hwnd);
    create_for_size(size)
}

pub(super) fn apply_window_icons(hwnd: HWND) {
    let (small, big) = WINDOW_ICONS.with_borrow_mut(|slot| {
        let icons = slot.get_or_insert_with(|| WindowIcons {
            small: create_for_size(small_icon_metric(hwnd)),
            big: create_for_size(big_icon_metric(hwnd)),
        });
        (
            icons.small.as_ref().map(OwnedIcon::raw),
            icons.big.as_ref().map(OwnedIcon::raw),
        )
    });
    if let Some(small) = small {
        unsafe {
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL as WPARAM, small as LPARAM);
        }
    }
    if let Some(big) = big {
        unsafe {
            SendMessageW(hwnd, WM_SETICON, ICON_BIG as WPARAM, big as LPARAM);
        }
    }
}

pub(super) fn create_for_size(size: i32) -> Option<OwnedIcon> {
    let clamped = size.clamp(16, 256);
    let frame = pick_ico_frame(APP_ICON_BYTES, clamped as u32)?;
    let bytes = u32::try_from(frame.len()).ok()?;
    let handle = unsafe {
        CreateIconFromResourceEx(
            frame.as_ptr(),
            bytes,
            1,
            ICON_RESOURCE_VERSION,
            clamped,
            clamped,
            LR_DEFAULTCOLOR,
        )
    };
    if handle.is_null() {
        None
    } else {
        Some(OwnedIcon(handle))
    }
}

fn small_icon_metric(hwnd: HWND) -> i32 {
    metric_for_window(hwnd, SM_CXSMICON, 16)
}

fn big_icon_metric(hwnd: HWND) -> i32 {
    metric_for_window(hwnd, SM_CXICON, 32)
}

fn metric_for_window(hwnd: HWND, metric: i32, fallback: i32) -> i32 {
    let dpi = if hwnd.is_null() {
        0
    } else {
        unsafe { GetDpiForWindow(hwnd) }
    };
    let value = if dpi > 0 {
        unsafe { GetSystemMetricsForDpi(metric, dpi) }
    } else {
        unsafe { GetSystemMetrics(metric) }
    };
    if value > 0 {
        value
    } else {
        fallback
    }
}

fn pick_ico_frame(ico: &[u8], desired_px: u32) -> Option<&[u8]> {
    if ico.len() < 6 {
        return None;
    }
    let reserved = u16::from_le_bytes([ico[0], ico[1]]);
    let image_type = u16::from_le_bytes([ico[2], ico[3]]);
    let count = usize::from(u16::from_le_bytes([ico[4], ico[5]]));
    if reserved != 0 || image_type != 1 || count == 0 {
        return None;
    }
    let mut best_ge: Option<(u32, &[u8])> = None;
    let mut largest: Option<(u32, &[u8])> = None;
    for index in 0..count {
        let entry_offset = 6 + index * 16;
        let entry = ico.get(entry_offset..entry_offset + 16)?;
        let width = match entry[0] {
            0 => 256,
            value => u32::from(value),
        };
        let size = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
        let offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
        let end = offset.checked_add(size)?;
        let payload = ico.get(offset..end)?;
        if width >= desired_px && best_ge.is_none_or(|(best_w, _)| width < best_w) {
            best_ge = Some((width, payload));
        }
        if largest.is_none_or(|(largest_w, _)| width > largest_w) {
            largest = Some((width, payload));
        }
    }
    best_ge.or(largest).map(|(_, payload)| payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_ico_has_expected_frames_and_creates_win32_icons() {
        for size in [16, 20, 24, 32, 48, 64, 128, 256] {
            let frame = pick_ico_frame(APP_ICON_BYTES, size).expect("matching ico frame");
            assert!(frame.starts_with(b"\x89PNG\r\n\x1a\n"));
            let width = u32::from_be_bytes([frame[16], frame[17], frame[18], frame[19]]);
            assert_eq!(width, size);
        }
        let small = create_for_size(16).expect("16px tray icon");
        assert!(!small.raw().is_null());
        let big = create_for_size(32).expect("32px window icon");
        assert!(!big.raw().is_null());
    }
}
