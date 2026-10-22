//! Reads bounded project-window metadata from the originally captured IDEA instance.
use super::native::{launcher_editor_project_window, launcher_free_string};
use crate::{editor_context, ApplicationInfo, ContextReadError};
use anyhow::{Context, Result};
use std::{
    ffi::{CStr, CString},
    path::PathBuf,
};

pub(super) fn application_directory(application: &ApplicationInfo) -> Result<Option<PathBuf>> {
    if !editor_context::supports_window_project(&application.bundle_id) {
        return Ok(None);
    }
    if application.pid <= 0 || application.instance_id.as_deref().is_none_or(str::is_empty) {
        return Err(ContextReadError::ApplicationChanged.into());
    }
    let identity = CString::new(serde_json::to_string(application)?)?;
    // SAFETY: identity outlives the call; returned malloc-owned JSON is copied and freed once.
    let value: serde_json::Value = unsafe {
        let raw = launcher_editor_project_window(identity.as_ptr());
        if raw.is_null() {
            return Err(ContextReadError::Unavailable.into());
        }
        let decoded = serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
        launcher_free_string(raw);
        decoded
    }
    .context("decode editor project window")?;
    match value.get("status").and_then(|v| v.as_str()) {
        Some("ok") => {
            let home = std::env::var_os("HOME").map(PathBuf::from);
            Ok(value
                .get("title")
                .and_then(|v| v.as_str())
                .and_then(|title| editor_context::idea_project_directory(title, home.as_deref())))
        }
        Some("no_window" | "unsupported") => Ok(None),
        Some("untrusted") => Err(ContextReadError::PermissionDenied.into()),
        Some("timeout") => Err(ContextReadError::TimedOut.into()),
        Some("changed") => Err(ContextReadError::ApplicationChanged.into()),
        Some("too_large") => Err(ContextReadError::TooLarge.into()),
        _ => Err(ContextReadError::Unavailable.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_capture_rejects_unbound_identity_and_unsupported_apps() {
        let mut app = ApplicationInfo {
            bundle_id: "com.jetbrains.intellij".into(),
            name: "IDEA".into(),
            pid: 1,
            instance_id: None,
        };
        assert_eq!(
            application_directory(&app)
                .unwrap_err()
                .downcast_ref::<ContextReadError>(),
            Some(&ContextReadError::ApplicationChanged)
        );
        app.bundle_id = "com.microsoft.VSCode".into();
        assert!(application_directory(&app).unwrap().is_none());
    }
}
