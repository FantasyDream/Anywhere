//! The text clipboard: one bounded snapshot, and writing a value back.
use super::native::*;
use super::shell::run;
use crate::ContextReadError;
use anyhow::Result;
use std::os::raw::c_char;

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
