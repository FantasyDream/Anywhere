//! Applications: identity, localized names, launch and icon.
use super::c_path;
use super::native::*;
use crate::{ApplicationInfo, BundleApplication};
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

pub(super) fn installed_applications(roots: &[PathBuf]) -> Vec<BundleApplication> {
    let languages = preferred_languages();
    let mut applications = Vec::new();
    let mut unreadable = 0usize;
    for path in crate::catalog::discover(roots) {
        match read_bundle(&path, &languages) {
            Ok(Some(application)) => applications.push(application),
            Ok(None) => unreadable += 1,
            Err(error) => {
                unreadable += 1;
                tracing::debug!(bundle = %path.display(), error = %error, "app_index_skip");
            }
        }
    }
    if unreadable > 0 {
        tracing::info!(unreadable, "app_index_incomplete");
    }
    applications
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
}
