//! The directory the ambient application is showing, read through Finder Automation.
use super::shell::{require_directory, run};
use crate::ApplicationInfo;
use anyhow::Result;
use std::{ffi::OsStr, path::PathBuf};

pub(super) fn application_directory(application: &ApplicationInfo) -> Result<Option<PathBuf>> {
    if application.bundle_id != "com.apple.finder" {
        return Ok(None);
    }
    let script = include_str!("../../native/finder.applescript");
    let output = run(
        "/usr/bin/osascript",
        &[OsStr::new("-e"), OsStr::new(script)],
        None,
    )?;
    // osascript appends one newline; do not trim valid spaces from a filesystem name.
    let path = output.strip_suffix('\n').unwrap_or(&output);
    if path.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(path);
    require_directory(&path)?;
    Ok(Some(path.canonicalize()?))
}
