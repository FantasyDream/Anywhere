//! Applications: identity, localized names, launch and icon.
use super::c_path;
use super::native::*;
use crate::{ApplicationInfo, BundleApplication, SelectedTextError};
use anyhow::{bail, Context, Result};
use std::{
    ffi::CStr,
    path::{Path, PathBuf},
};

pub(super) fn frontmost_application() -> Result<Option<ApplicationInfo>> {
    // SAFETY: Native side returns a malloc-owned NUL-terminated string, freed exactly once.
    let result = unsafe {
        let raw = launcher_frontmost_application();
        if raw.is_null() {
            return Ok(None);
        }
        let result = serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
        launcher_free_string(raw);
        result
    };
    result.map(Some).context("decode frontmost application")
}

pub(super) fn installed_applications(roots: &[PathBuf]) -> crate::InstalledApplications {
    let languages = preferred_languages();
    let mut applications = Vec::new();
    let mut skipped = 0usize;
    for path in crate::catalog::discover(roots, &|path| {
        crate::Platform::is_application_entry(&super::MacOsPlatform, path)
    }) {
        match read_bundle(&path, &languages) {
            Ok(Some(application)) => applications.push(application),
            Ok(None) => skipped += 1,
            Err(error) => {
                skipped += 1;
                tracing::debug!(bundle = %path.display(), error = %error, "app_index_skip");
            }
        }
    }
    if skipped > 0 {
        tracing::info!(skipped, "app_index_incomplete");
    }
    crate::InstalledApplications {
        applications,
        skipped,
    }
}

pub(super) fn bundle_application(bundle: &Path) -> Result<Option<BundleApplication>> {
    read_bundle(bundle, &preferred_languages())
}

fn read_bundle(bundle: &Path, languages: &[String]) -> Result<Option<BundleApplication>> {
    let path = c_path(&bundle.to_string_lossy())?;
    // SAFETY: path outlives the call; the native side returns a malloc-owned JSON string.
    let decoded = unsafe {
        let raw = launcher_bundle_application(path.as_ptr());
        if raw.is_null() {
            return Ok(None);
        }
        let decoded: std::result::Result<serde_json::Value, _> =
            serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
        launcher_free_string(raw);
        decoded
    };
    let value = decoded.context("decode bundle metadata")?;
    let string = |key: &str| -> Option<String> {
        value
            .get(key)
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let (Some(bundle_id), Some(mut name)) = (string("bundle_id"), string("name")) else {
        return Ok(None);
    };
    let mut aliases = value
        .get("aliases")
        .and_then(|value| value.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str())
                .filter(|entry| !entry.is_empty() && *entry != name)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    aliases.extend(
        super::aliases::well_known_aliases(&bundle_id)
            .iter()
            .map(|alias| (*alias).to_owned()),
    );
    if !languages.is_empty() {
        if let Ok(names) = bundle_localized_names(bundle, languages) {
            if let Some(preferred) = names
                .first()
                .map(String::as_str)
                .filter(|entry| !entry.is_empty())
            {
                if !preferred.eq_ignore_ascii_case(&name) {
                    aliases.insert(0, name);
                    name = preferred.to_owned();
                }
            }
            for localized in names {
                if localized != name && !aliases.iter().any(|alias| alias == &localized) {
                    aliases.push(localized);
                }
            }
        }
    }
    aliases.retain(|alias| !alias.eq_ignore_ascii_case(&name));
    Ok(Some(BundleApplication {
        aliases,
        bundle_id,
        name,
        path: string("path"),
        icon: None,
    }))
}

pub(super) fn preferred_languages() -> Vec<String> {
    // SAFETY: returns a malloc-owned JSON string, freed exactly once here.
    unsafe {
        let raw = launcher_preferred_languages();
        if raw.is_null() {
            return Vec::new();
        }
        let decoded: std::result::Result<Vec<String>, _> =
            serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
        launcher_free_string(raw);
        decoded.unwrap_or_default()
    }
}

pub(super) fn bundle_localized_names(bundle: &Path, languages: &[String]) -> Result<Vec<String>> {
    if languages.is_empty() {
        return Ok(Vec::new());
    }
    let path = c_path(&bundle.to_string_lossy())?;
    let languages = c_path(&languages.join(","))?;
    // SAFETY: both inputs are NUL-terminated and outlive the call; the returned
    // string is malloc-owned and freed exactly once.
    let decoded = unsafe {
        let raw = launcher_bundle_localized_names(path.as_ptr(), languages.as_ptr());
        if raw.is_null() {
            return Ok(Vec::new());
        }
        let decoded: std::result::Result<Vec<String>, _> =
            serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
        launcher_free_string(raw);
        decoded
    };
    Ok(decoded.unwrap_or_default())
}

pub(super) fn launch_application(path: &str) -> Result<bool> {
    if path.is_empty() {
        bail!("application path is empty");
    }
    let path = c_path(path)?;
    // SAFETY: null-terminated path stays alive for the synchronous native call.
    let launched = unsafe { launcher_launch_application(path.as_ptr()) };
    Ok(launched == 1)
}

pub(super) fn accessibility_trusted(prompt: bool) -> bool {
    // SAFETY: no pointers; the native side only reads a flag.
    unsafe { launcher_accessibility_trusted(if prompt { 1 } else { 0 }) == 1 }
}

pub(super) fn selected_text(application: &ApplicationInfo) -> Result<Option<String>> {
    if application.pid <= 0 || application.instance_id.is_none() {
        return Err(SelectedTextError::ApplicationChanged.into());
    }
    let identity = c_path(&serde_json::to_string(application)?)?;
    // SAFETY: the NUL-terminated identity outlives the call; native returns an
    // owned JSON string and the matching free runs exactly once.
    let decoded = unsafe {
        let raw = launcher_selected_text(identity.as_ptr());
        if raw.is_null() {
            bail!("selected text inspection returned no status");
        }
        let decoded = serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
        launcher_free_string(raw);
        decoded
    };
    decode_selected_text(decoded.context("decode selected text status")?)
}

fn decode_selected_text(value: serde_json::Value) -> Result<Option<String>> {
    match value.get("status").and_then(|status| status.as_str()) {
        Some("ok") => {
            if value.get("truncated").and_then(|flag| flag.as_bool()) == Some(true) {
                return Err(SelectedTextError::TooLarge.into());
            }
            let text = value
                .get("text")
                .and_then(|text| text.as_str())
                .context("selected text status has no text")?;
            // The native shim already caps the string. Guard the Rust boundary too
            // in case a future native implementation forgets that contract.
            if text.len() > 16 * 1024 {
                return Err(SelectedTextError::TooLarge.into());
            }
            Ok((!text.is_empty()).then(|| text.to_owned()))
        }
        Some("no_selection") => Ok(None),
        Some("unsupported") => Err(SelectedTextError::Unsupported.into()),
        Some("untrusted") => Err(SelectedTextError::PermissionDenied.into()),
        Some("application_changed") => Err(SelectedTextError::ApplicationChanged.into()),
        Some("timeout") => Err(SelectedTextError::TimedOut.into()),
        Some("too_large") => Err(SelectedTextError::TooLarge.into()),
        Some("unavailable") => Err(SelectedTextError::Unavailable.into()),
        _ => bail!("unknown selected text status"),
    }
}

pub(super) fn application_version(bundle_id: &str) -> Option<String> {
    let Ok(path) = super::c_path(bundle_id) else {
        return None;
    };
    // SAFETY: NUL-terminated input outlives the call; the returned string is malloc-owned.
    unsafe {
        let raw = launcher_application_version(path.as_ptr());
        if raw.is_null() {
            return None;
        }
        let value = CStr::from_ptr(raw).to_string_lossy().into_owned();
        launcher_free_string(raw);
        (!value.is_empty()).then_some(value)
    }
}

pub(super) fn discover_menu_actions(application: &ApplicationInfo) -> Result<crate::MenuDiscovery> {
    if application.pid <= 0 {
        return Ok(crate::MenuDiscovery::Unavailable);
    }
    let identity = c_path(&serde_json::to_string(application)?)?;
    // SAFETY: native returns a malloc-owned JSON object, freed once.
    let decoded = unsafe {
        let raw = launcher_discover_menu_actions(identity.as_ptr());
        if raw.is_null() {
            return Ok(crate::MenuDiscovery::Unavailable);
        }
        let decoded = serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
        launcher_free_string(raw);
        decoded
    };
    decode_menu_discovery(decoded.context("decode menu actions")?)
}

fn decode_menu_discovery(value: serde_json::Value) -> Result<crate::MenuDiscovery> {
    if value.is_array() {
        return Ok(crate::MenuDiscovery::Ready(serde_json::from_value(value)?));
    }
    match value.get("status").and_then(|status| status.as_str()) {
        Some("ok") => {
            let items = value
                .get("items")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            Ok(crate::MenuDiscovery::Ready(serde_json::from_value(items)?))
        }
        Some("untrusted") => Ok(crate::MenuDiscovery::Untrusted),
        Some("timeout") => {
            tracing::warn!(
                ax_error = ?value.get("ax_error").and_then(|v| v.as_i64()),
                operation = ?value.get("operation").and_then(|v| v.as_str()),
                deadline_reached = ?value.get("deadline_reached").and_then(|v| v.as_bool()),
                "menu_discovery_incomplete"
            );
            Ok(crate::MenuDiscovery::TimedOut)
        }
        Some("unavailable") => Ok(crate::MenuDiscovery::Unavailable),
        _ => anyhow::bail!("unknown menu discovery status"),
    }
}

pub(super) fn perform_menu_action(
    application: &ApplicationInfo,
    locator: &crate::MenuLocator,
) -> Result<()> {
    let identity = super::c_path(&serde_json::to_string(application)?)?;
    let json = serde_json::to_string(locator).context("encode menu locator")?;
    let json = super::c_path(&json)?;
    // SAFETY: both CStrings outlive the synchronous native call.
    let status = unsafe { launcher_perform_menu_action(identity.as_ptr(), json.as_ptr()) };
    match status {
        0 => Ok(()),
        1 => bail!("application instance changed or is not running"),
        2 => bail!("menu bar is not available"),
        3 => bail!("menu item was not found"),
        4 => bail!("菜单项当前不可用"),
        5 => bail!("menu item could not be performed"),
        6 => bail!("menu locator is invalid"),
        7 => bail!("accessibility permission is not granted"),
        8 => bail!("could not activate the captured application"),
        9 => bail!("menu inspection could not finish; try again"),
        other => bail!("menu action failed ({other})"),
    }
}

pub(super) fn application_icon(path: &str, size: u32) -> Result<Option<Vec<u8>>> {
    let path = c_path(path)?;
    let mut length = 0u32;
    // SAFETY: the native side allocates the buffer and reports its exact length.
    let buffer = unsafe { launcher_application_icon(path.as_ptr(), size, &mut length) };
    if buffer.is_null() || length == 0 {
        return Ok(None);
    }
    // SAFETY: `buffer` is a malloc-owned allocation of `length` bytes, freed once.
    let bytes = unsafe { std::slice::from_raw_parts(buffer, length as usize).to_vec() };
    unsafe { launcher_free_bytes(buffer) };
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::macos::MacOsPlatform;
    use crate::Platform;

    #[test]
    fn incomplete_discovery_never_decodes_partial_items_as_ready() {
        let partial = serde_json::json!({"status": "timeout", "ax_error": -25204,
            "operation": "AXIdentifier", "deadline_reached": false,
            "items": [{"title": "unsafe partial item"}]});
        assert_eq!(
            decode_menu_discovery(partial).unwrap(),
            crate::MenuDiscovery::TimedOut
        );
        assert_eq!(
            decode_menu_discovery(serde_json::json!({"status": "timeout"})).unwrap(),
            crate::MenuDiscovery::TimedOut
        );
        assert_eq!(
            decode_menu_discovery(serde_json::json!({"status": "ok", "items": []})).unwrap(),
            crate::MenuDiscovery::Ready(vec![])
        );
    }

    #[test]
    fn selected_text_statuses_remain_distinct_and_bounded() {
        use serde_json::json;

        assert_eq!(
            decode_selected_text(json!({"status":"no_selection"})).unwrap(),
            None
        );
        assert_eq!(
            decode_selected_text(json!({"status":"ok", "text":"hello"})).unwrap(),
            Some("hello".into())
        );
        for (status, expected) in [
            ("unsupported", SelectedTextError::Unsupported),
            ("untrusted", SelectedTextError::PermissionDenied),
            ("application_changed", SelectedTextError::ApplicationChanged),
            ("timeout", SelectedTextError::TimedOut),
            ("too_large", SelectedTextError::TooLarge),
            ("unavailable", SelectedTextError::Unavailable),
        ] {
            let error = decode_selected_text(json!({"status":status})).unwrap_err();
            assert_eq!(error.downcast_ref::<SelectedTextError>(), Some(&expected));
        }
        assert!(
            decode_selected_text(json!({"status":"ok", "text":"x".repeat(16 * 1024 + 1)})).is_err()
        );
        assert_eq!(
            decode_selected_text(json!({"status":"ok", "text":"truncated", "truncated":true}))
                .unwrap_err()
                .downcast_ref::<SelectedTextError>(),
            Some(&SelectedTextError::TooLarge)
        );
    }

    #[test]
    fn selected_text_requires_a_frozen_process_identity() {
        let application = ApplicationInfo {
            bundle_id: "test.editor".into(),
            name: "Editor".into(),
            pid: 0,
            instance_id: None,
        };
        let error = selected_text(&application).unwrap_err();
        assert_eq!(
            error.downcast_ref::<SelectedTextError>(),
            Some(&SelectedTextError::ApplicationChanged)
        );
    }

    #[test]
    fn bundle_metadata_is_reported_for_a_real_system_application() {
        let platform = MacOsPlatform;
        let finder = Path::new("/System/Library/CoreServices/Finder.app");
        if finder.is_dir() {
            let bundle = platform.bundle_application(finder).unwrap().unwrap();
            assert_eq!(bundle.bundle_id, "com.apple.finder");
            assert!(!bundle.name.is_empty());
            assert!(
                !bundle.name.to_lowercase().ends_with(".app"),
                "the title is the system display name, not the file name: {}",
                bundle.name
            );
        }
        let lark = Path::new("/Applications/Lark.app");
        if lark.exists()
            && preferred_languages()
                .iter()
                .any(|language| language.to_ascii_lowercase().starts_with("zh"))
        {
            let bundle = platform.bundle_application(lark).unwrap().unwrap();
            assert_eq!(
                bundle.name, "飞书",
                "preferred-language title, not the English file name; aliases={:?}",
                bundle.aliases
            );
        }
        // A plain directory is not an application.
        let plain = tempfile::tempdir().unwrap();
        assert!(platform.bundle_application(plain.path()).unwrap().is_none());
    }

    #[test]
    fn frontmost_application_carries_kernel_process_instance() {
        let platform = MacOsPlatform;
        let Some(app) = platform.frontmost_application().unwrap() else {
            return;
        };
        assert!(
            app.instance_id
                .as_deref()
                .is_some_and(|id| id.starts_with("proc:")),
            "pid reuse checks require kernel start time: {app:?}"
        );
        if platform.accessibility_trusted(false) {
            let mut rejected = app.clone();
            rejected.instance_id = None;
            assert!(
                matches!(
                    platform.discover_menu_actions(&rejected).unwrap(),
                    crate::MenuDiscovery::Unavailable | crate::MenuDiscovery::TimedOut
                ),
                "a missing instance_id must not discover menus"
            );
        }
    }

    #[test]
    fn menu_discovery_is_empty_when_untrusted() {
        let platform = MacOsPlatform;
        let trusted = platform.accessibility_trusted(false);
        let Some(app) = platform.frontmost_application().unwrap() else {
            return;
        };
        assert!(app.pid > 0);
        let discovery = platform.discover_menu_actions(&app).unwrap();
        if trusted {
            assert!(
                discovery
                    .items()
                    .iter()
                    .all(|item| !item.title.is_empty() && !item.menu_path.is_empty()),
                "menu leaves must have titles"
            );
        } else {
            assert!(
                matches!(discovery, crate::MenuDiscovery::Untrusted) || discovery.is_empty(),
                "untrusted accessibility must not yield menu actions"
            );
        }
    }
}
