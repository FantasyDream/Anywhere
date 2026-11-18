//! Finder context read through Automation. An empty selection is distinct from a failed read.
use super::native::launcher_application_identity_status;
use crate::{ApplicationInfo, ContextReadError};
use anyhow::{bail, Context, Result};
use std::{
    collections::HashSet,
    ffi::CString,
    path::{Component, Path, PathBuf},
    time::Duration,
};

const MAX_SELECTED_PATHS: usize = 32;
const MAX_SCRIPT_OUTPUT_BYTES: u32 = 256 * 1024;
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
    let output =
        super::automation::read(script, application, SCRIPT_TIMEOUT, MAX_SCRIPT_OUTPUT_BYTES)?;
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
/// The native sender binds Apple Events to the captured PID and validates its
/// kernel start identity before dispatch and before publishing a reply.
pub(super) fn selected_paths(application: &ApplicationInfo) -> Result<Vec<PathBuf>> {
    if application.bundle_id != "com.apple.finder" {
        return Ok(Vec::new());
    }
    verify_finder_instance(application, "before_selection")?;
    let output = super::automation::read(
        include_str!("../../native/finder_selection.applescript"),
        application,
        SCRIPT_TIMEOUT,
        MAX_SCRIPT_OUTPUT_BYTES,
    )?;
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
