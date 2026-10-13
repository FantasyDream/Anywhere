//! Finder context read through Automation. An empty selection is distinct from a failed read.
use super::native::launcher_application_identity_status;
use crate::{ApplicationInfo, ContextReadError};
use anyhow::{bail, Context, Result};
use std::{
    collections::HashSet,
    ffi::CString,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const MAX_SELECTED_PATHS: usize = 32;
const MAX_SCRIPT_OUTPUT_BYTES: usize = 256 * 1024;
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn application_directory(application: &ApplicationInfo) -> Result<Option<PathBuf>> {
    if application.bundle_id != "com.apple.finder" {
        return Ok(None);
    }
    // The diagnostic CLI can request an unbound directory read with pid=0.
    // Activation always supplies a captured instance, checked on both sides.
    let captured = application.pid != 0 || application.instance_id.is_some();
    if captured {
        verify_finder_instance(application, "before_directory")?;
    }
    let script = include_str!("../../native/finder.applescript");
    let output = run_finder_script(script)?;
    if captured {
        verify_finder_instance(application, "after_directory")?;
    }
    // osascript appends one newline; do not trim valid spaces from a filesystem name.
    let path = output.strip_suffix('\n').unwrap_or(&output);
    if path.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(path);
    if !path.is_dir() {
        return Err(ContextReadError::Unavailable.into());
    }
    Ok(Some(
        path.canonicalize()
            .map_err(|_| ContextReadError::Unavailable)?,
    ))
}

/// Read the selection associated with the Finder instance captured at activation.
///
/// Finder Automation addresses an app by bundle ID. Checking the captured PID's
/// kernel start identity on both sides of the call prevents publishing a result
/// after Finder restarts while the Apple event is in flight.
pub(super) fn selected_paths(application: &ApplicationInfo) -> Result<Vec<PathBuf>> {
    if application.bundle_id != "com.apple.finder" {
        return Ok(Vec::new());
    }
    verify_finder_instance(application, "before_selection")?;
    let output = run_finder_script(include_str!("../../native/finder_selection.applescript"))?;
    verify_finder_instance(application, "after_selection")?;
    parse_selected_paths(&output)
}

fn verify_finder_instance(application: &ApplicationInfo, stage: &'static str) -> Result<()> {
    let expected = application
        .instance_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or(ContextReadError::Unavailable)?;
    if application.pid <= 0 {
        return Err(ContextReadError::Unavailable.into());
    }
    let bundle_id = CString::new(application.bundle_id.as_str())
        .context("captured Finder bundle ID is invalid")?;
    let instance_id = CString::new(expected).context("captured Finder instance ID is invalid")?;
    // SAFETY: both C strings outlive the synchronous native identity check.
    let status = unsafe {
        launcher_application_identity_status(
            application.pid,
            bundle_id.as_ptr(),
            instance_id.as_ptr(),
        )
    };
    if status != 1 {
        let reason = match status {
            0 => "invalid_identity",
            2 => "missing_process",
            3 => "terminated",
            4 => "bundle_mismatch",
            5 => "process_start_mismatch",
            _ => "unknown",
        };
        tracing::info!(
            pid = application.pid,
            stage,
            reason,
            "finder_identity_rejected"
        );
        return Err(ContextReadError::ApplicationChanged.into());
    }
    Ok(())
}

fn run_finder_script(script: &str) -> Result<String> {
    let mut child = Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .env("LC_ALL", "en_US.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("start Finder context capture")?;

    // Drain both pipes while waiting: a selection of 32 long paths can exceed
    // a pipe's capacity, otherwise the child could block until our timeout.
    let stdout = child
        .stdout
        .take()
        .context("Finder response pipe missing")?;
    let stderr = child.stderr.take().context("Finder error pipe missing")?;
    let output_reader = thread::spawn(move || read_limited(stdout, MAX_SCRIPT_OUTPUT_BYTES));
    let error_reader = thread::spawn(move || read_limited(stderr, 16 * 1024));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().context("wait for Finder selection")? {
            break Some(status);
        }
        if started.elapsed() >= SCRIPT_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        thread::sleep(Duration::from_millis(5));
    };
    let output = output_reader
        .join()
        .map_err(|_| anyhow::anyhow!("Finder response reader failed"))??;
    let error = error_reader
        .join()
        .map_err(|_| anyhow::anyhow!("Finder error reader failed"))??;
    let Some(status) = status else {
        return Err(ContextReadError::TimedOut.into());
    };
    if !status.success() {
        // osascript may include a selected filename in its diagnostic. Only
        // return a category; never copy that diagnostic into logs or UI.
        let detail = String::from_utf8_lossy(&error);
        if detail.contains("(-1743)") || detail.contains("not authorized") {
            return Err(ContextReadError::PermissionDenied.into());
        }
        if detail.contains("(17201)") {
            return Err(ContextReadError::TooManyItems.into());
        }
        bail!("Finder selection is unavailable (osascript failed)");
    }
    String::from_utf8(output).context("Finder selection response is not UTF-8")
}

fn read_limited(mut reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut overflow = false;
    let mut chunk = [0u8; 4096];
    loop {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        if bytes.len().saturating_add(count) <= limit {
            bytes.extend_from_slice(&chunk[..count]);
        } else {
            overflow = true;
        }
    }
    if overflow {
        return Err(io::Error::other("Finder selection response exceeded limit"));
    }
    Ok(bytes)
}

fn parse_selected_paths(output: &str) -> Result<Vec<PathBuf>> {
    // osascript adds one newline to its returned text. Do not trim more: a
    // filename may itself end in whitespace or a newline.
    let payload = output
        .strip_suffix('\n')
        .context("incomplete Finder selection response")?;
    let mut parts = payload.split('\0');
    let count = parts
        .next()
        .context("missing Finder selection count")?
        .parse::<usize>()
        .context("invalid Finder selection count")?;
    if count > MAX_SELECTED_PATHS {
        bail!("Finder selected too many items");
    }
    let raw_paths: Vec<_> = parts.collect();
    if raw_paths.len() != count {
        bail!("Finder selection response count mismatch");
    }
    let mut seen = HashSet::new();
    let mut paths = Vec::with_capacity(count);
    for raw in raw_paths {
        let path = Path::new(raw);
        if raw.is_empty()
            || !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
        {
            bail!("Finder selection response contains an invalid path");
        }
        let path = path.to_path_buf();
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_finder_directory_rejects_missing_identity_before_automation() {
        let application = ApplicationInfo {
            bundle_id: "com.apple.finder".into(),
            name: "Finder".into(),
            pid: -1,
            instance_id: None,
        };
        assert_eq!(
            application_directory(&application)
                .unwrap_err()
                .downcast_ref::<ContextReadError>(),
            Some(&ContextReadError::Unavailable)
        );
        assert_eq!(
            selected_paths(&application)
                .unwrap_err()
                .downcast_ref::<ContextReadError>(),
            Some(&ContextReadError::Unavailable)
        );
    }

    #[test]
    fn parses_empty_selection() {
        assert_eq!(parse_selected_paths("0\n").unwrap(), Vec::<PathBuf>::new());
    }

    #[test]
    fn preserves_unicode_delimiters_and_trailing_newline() {
        let output = "3\0/Users/用户/项目, one\0/Users/用户/line\nname\0/Users/用户/trailing \n\n";
        assert_eq!(
            parse_selected_paths(output).unwrap(),
            vec![
                PathBuf::from("/Users/用户/项目, one"),
                PathBuf::from("/Users/用户/line\nname"),
                PathBuf::from("/Users/用户/trailing \n"),
            ]
        );
    }

    #[test]
    fn duplicate_paths_are_collapsed_in_first_seen_order() {
        assert_eq!(
            parse_selected_paths("3\0/a\0/b\0/a\n").unwrap(),
            vec![PathBuf::from("/a"), PathBuf::from("/b")]
        );
    }

    #[test]
    fn rejects_truncated_or_untrusted_responses() {
        for response in [
            "0",
            "2\0/a\n",
            "1\0relative\n",
            "1\0/../secret\n",
            "1\0\n",
            "33\n",
        ] {
            assert!(parse_selected_paths(response).is_err(), "{response:?}");
        }
    }

    #[test]
    fn non_finder_never_uses_automation() {
        let application = ApplicationInfo {
            bundle_id: "com.apple.TextEdit".to_owned(),
            name: "TextEdit".to_owned(),
            pid: 0,
            instance_id: None,
        };
        assert!(selected_paths(&application).unwrap().is_empty());
    }
}
