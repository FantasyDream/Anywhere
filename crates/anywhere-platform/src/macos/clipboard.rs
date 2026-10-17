//! The text clipboard: one bounded snapshot, and writing a value back.
use super::native::*;
use anyhow::Result;

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct Observation {
    count: i64,
    checked: Instant,
    copied: Option<Instant>,
}
impl Observation {
    fn observe(&mut self, count: i64, now: Instant) {
        if self.count != count {
            // Use the beginning of the observation interval. This expires up to
            // 500ms early instead of presenting content copied more than 15s ago.
            self.copied = Some(self.checked);
            self.count = count;
        }
        self.checked = now;
    }
}
static OBSERVATION: OnceLock<Mutex<Observation>> = OnceLock::new();
pub(super) fn change_count() -> i64 {
    // SAFETY: the function returns a pasteboard generation and touches no buffers.
    unsafe { launcher_clipboard_change_count() }
}

static TRACKER: OnceLock<Mutex<Option<Box<dyn crate::clipboard::ClipboardWatch>>>> =
    OnceLock::new();
pub(super) fn start_tracking() {
    OBSERVATION.get_or_init(|| {
        Mutex::new(Observation {
            count: change_count(),
            checked: Instant::now(),
            copied: None,
        })
    });
    TRACKER.get_or_init(|| {
        let watch = crate::clipboard::watch(Box::new(|| {
            if let Some(observation) = OBSERVATION.get() {
                if let Ok(mut held) = observation.lock() {
                    let now = Instant::now();
                    held.checked = now - Duration::from_millis(500);
                    held.observe(change_count(), now);
                }
            }
        }))
        .ok();
        Mutex::new(watch)
    });
}
pub(super) fn snapshot() -> Result<crate::ClipboardSnapshot> {
    start_tracking();
    let count = change_count();
    let text = clipboard_text()?;
    let after = change_count();
    let age_ms = OBSERVATION.get().and_then(|observation| {
        let mut held = observation.lock().ok()?;
        let now = Instant::now();
        held.checked = now - Duration::from_millis(500);
        held.observe(after, now);
        (count == after)
            .then(|| {
                held.copied
                    .map(|time| time.elapsed().as_millis().min(u64::MAX as u128) as u64)
            })
            .flatten()
    });
    Ok(crate::ClipboardSnapshot { text, age_ms })
}

/// Clipboard snapshots are bounded so a huge clipboard cannot stall activation.
pub const MAX_CLIPBOARD_BYTES: usize = 16 * 1024;

pub(super) fn clipboard_text() -> Result<Option<String>> {
    crate::clipboard::read_text(MAX_CLIPBOARD_BYTES)
}
pub(super) fn copy_text(text: &str) -> Result<()> {
    crate::clipboard::copy_text(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_clipboard_has_unknown_age_and_new_generations_do_not_refresh_on_read() {
        let start = Instant::now();
        let mut held = Observation {
            count: 10,
            checked: start,
            copied: None,
        };
        held.observe(10, start + Duration::from_secs(60));
        assert!(held.copied.is_none());
        held.observe(11, start + Duration::from_millis(60_250));
        assert_eq!(held.copied, Some(start + Duration::from_secs(60)));
        held.observe(11, start + Duration::from_secs(80));
        assert_eq!(held.copied, Some(start + Duration::from_secs(60)));
        held.observe(12, start + Duration::from_millis(80_250));
        assert_eq!(held.copied, Some(start + Duration::from_secs(80)));
    }
}
