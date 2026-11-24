use anyhow::{ensure, Result};
use std::{
    ptr::null_mut,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::HGLOBAL,
    System::{
        DataExchange::{
            CloseClipboard, GetClipboardData, GetClipboardSequenceNumber,
            IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
        },
        Memory::{GlobalLock, GlobalSize, GlobalUnlock},
    },
};

use crate::ContextReadError;

const UNICODE_TEXT: u32 = 13;

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
        || unsafe { IsClipboardFormatAvailable(UNICODE_TEXT) } == 0
        || !allows_history(include_history)
    {
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
    Ok((!text.is_empty() && expected_matches(expected)?).then_some(text))
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
