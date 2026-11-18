//! Serialized, cached read-only OSA scripts. The native sender addresses the
//! captured PID and checks its kernel start identity before and after dispatch.
use super::native::{launcher_run_compiled_script, NativeBytes};
use crate::{ApplicationInfo, ContextReadError};
use anyhow::{Context, Result};
use std::{ffi::CString, time::Duration};

pub(super) fn read(
    script: &'static str,
    application: &ApplicationInfo,
    timeout: Duration,
    capacity: u32,
) -> Result<String> {
    let source = CString::new(script)?;
    let bundle = CString::new(application.bundle_id.as_str())?;
    let instance = CString::new(application.instance_id.as_deref().unwrap_or_default())?;
    let mut length = 0;
    let mut status = 0;
    // SAFETY: all inputs live through the synchronous call. A successful result
    // is a native allocation of exactly `length` bytes, released with its owner.
    let pointer = unsafe {
        launcher_run_compiled_script(
            source.as_ptr(),
            application.pid,
            bundle.as_ptr(),
            instance.as_ptr(),
            timeout.as_millis().min(u128::from(u32::MAX)) as u32,
            capacity,
            &mut length,
            &mut status,
        )
    };
    // SAFETY: a non-null reply is a native allocation of exactly `length` bytes.
    let Some(bytes) = (unsafe { NativeBytes::from_raw(pointer, length) }) else {
        return Err(match status {
            2 => ContextReadError::TimedOut,
            3 => ContextReadError::TooManyItems,
            4 => ContextReadError::PermissionDenied,
            5 => ContextReadError::ApplicationChanged,
            6 => ContextReadError::TooLarge,
            _ => ContextReadError::Unavailable,
        }
        .into());
    };
    let reply =
        std::str::from_utf8(bytes.as_slice()).context("Automation response is not UTF-8")?;
    // Keep the existing parsers' single transport newline, separate from valid
    // trailing whitespace in a selected filename or returned URL.
    let mut text = String::with_capacity(reply.len() + 1);
    text.push_str(reply);
    text.push('\n');
    Ok(text)
}
