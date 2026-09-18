//! The text clipboard: one bounded snapshot, and writing a value back.
use super::native::*;
use super::shell::run;
use anyhow::{bail, Context, Result};
use std::os::raw::c_char;

/// Clipboard snapshots are bounded so a huge clipboard cannot stall activation.
pub const MAX_CLIPBOARD_BYTES: usize = 1024 * 1024;

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
    match status {
        1 => {
            let length = (length as usize).min(output.len());
            output.truncate(length);
            let text = String::from_utf8(output).context("clipboard is not valid UTF-8")?;
            if text.is_empty() {
                Ok(None)
            } else {
                Ok(Some(text))
            }
        }
        // No text, or the value is larger than the bounded snapshot.
        2 | 3 => Ok(None),
        other => bail!("clipboard read failed (status {other})"),
    }
}

pub(super) fn copy_text(text: &str) -> Result<()> {
    run("/usr/bin/pbcopy", &[], Some(text)).map(|_| ())
}
