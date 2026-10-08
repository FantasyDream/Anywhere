//! The text clipboard: one bounded snapshot, and writing a value back.
use super::native::*;
use super::shell::run;
use crate::ContextReadError;
use anyhow::Result;
use std::os::raw::c_char;

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
            // 250ms early instead of presenting content copied more than 15s ago.
            self.copied = Some(self.checked);
            self.count = count;
        }
        self.checked = now;
    }
}
static OBSERVATION: OnceLock<Mutex<Observation>> = OnceLock::new();
fn change_count() -> i64 {
    // SAFETY: the function returns a pasteboard generation and touches no buffers.
    unsafe { launcher_clipboard_change_count() }
}
pub(super) fn start_tracking() {
    OBSERVATION.get_or_init(|| {
        let observation = Mutex::new(Observation {
            count: change_count(),
            checked: Instant::now(),
            copied: None,
        });
        std::thread::spawn(|| loop {
            std::thread::sleep(Duration::from_millis(250));
            if let Some(observation) = OBSERVATION.get() {
                if let Ok(mut held) = observation.lock() {
                    held.observe(change_count(), Instant::now());
                }
            }
        });
        observation
    });
}
pub(super) fn snapshot() -> Result<crate::ClipboardSnapshot> {
    start_tracking();
    let count = change_count();
    let text = clipboard_text()?;
    let after = change_count();
    let age_ms = OBSERVATION.get().and_then(|observation| {
        let mut held = observation.lock().ok()?;
        held.observe(after, Instant::now());
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
    let mut output = vec![0u8; MAX_CLIPBOARD_BYTES];
    let mut length = 0u32;
    // SAFETY: output has the supplied capacity; native code writes at most that much.
    let status = unsafe {
        launcher_clipboard_text(
            output.as_mut_ptr() as *mut c_char,
            output.len() as u32,
            &mut length,
        )
    };
    decode_clipboard(status, output, length as usize)
}

fn decode_clipboard(status: i32, mut output: Vec<u8>, length: usize) -> Result<Option<String>> {
    match status {
        1 => {
            if length > output.len() {
                return Err(ContextReadError::TooLarge.into());
            }
            output.truncate(length);
            let text = String::from_utf8(output).map_err(|_| ContextReadError::Unavailable)?;
            if text.is_empty() {
                Ok(None)
            } else {
                Ok(Some(text))
            }
        }
        2 => Ok(None),
        3 => Err(ContextReadError::TooLarge.into()),
        4 => Err(ContextReadError::PermissionDenied.into()),
        _ => Err(ContextReadError::Unavailable.into()),
    }
}

pub(super) fn copy_text(text: &str) -> Result<()> {
    run("/usr/bin/pbcopy", &[], Some(text)).map(|_| ())
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
    #[test]
    fn clipboard_status_does_not_conflate_denial_size_and_empty_content() {
        assert_eq!(
            decode_clipboard(1, b"hello".to_vec(), 5).unwrap(),
            Some("hello".into())
        );
        assert_eq!(decode_clipboard(2, vec![], 0).unwrap(), None);
        for (status, expected) in [
            (3, ContextReadError::TooLarge),
            (4, ContextReadError::PermissionDenied),
            (0, ContextReadError::Unavailable),
        ] {
            assert_eq!(
                decode_clipboard(status, vec![], 0)
                    .unwrap_err()
                    .downcast_ref::<ContextReadError>(),
                Some(&expected)
            );
        }
        assert_eq!(
            decode_clipboard(1, vec![0xff], 1)
                .unwrap_err()
                .downcast_ref::<ContextReadError>(),
            Some(&ContextReadError::Unavailable)
        );
        assert_eq!(
            decode_clipboard(1, vec![0], 2)
                .unwrap_err()
                .downcast_ref::<ContextReadError>(),
            Some(&ContextReadError::TooLarge)
        );
    }
}
