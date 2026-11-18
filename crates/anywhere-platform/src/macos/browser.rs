//! One current-tab URL snapshot from the captured foreground browser. Core
//! freezes its identity before the launcher takes focus; this adapter never
//! activates or searches other browsers.
use super::native::launcher_application_identity_matches;
use crate::{ApplicationInfo, ContextReadError};
use anyhow::Result;
use std::{ffi::CString, time::Duration};

const SAFARI_BUNDLE_ID: &str = "com.apple.Safari";
const CHROME_BUNDLE_ID: &str = "com.google.Chrome";
const EDGE_BUNDLE_ID: &str = "com.microsoft.edgemac";
const MAX_URL_BYTES: usize = 8 * 1024;
// A first Automation prompt can outlive the ordinary fast-path read. The caller
// runs this off the first-snapshot path; it still needs a finite process deadline.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(15);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Browser {
    Safari,
    Chrome,
    Edge,
}

impl Browser {
    fn for_bundle_id(bundle_id: &str) -> Option<Self> {
        match bundle_id {
            SAFARI_BUNDLE_ID => Some(Self::Safari),
            CHROME_BUNDLE_ID => Some(Self::Chrome),
            EDGE_BUNDLE_ID => Some(Self::Edge),
            _ => None,
        }
    }

    fn script(self) -> &'static str {
        match self {
            Self::Safari => include_str!("../../native/safari_url.applescript"),
            Self::Chrome => include_str!("../../native/chrome_url.applescript"),
            Self::Edge => include_str!("../../native/edge_url.applescript"),
        }
    }
}

pub(super) fn supports(bundle_id: &str) -> bool {
    Browser::for_bundle_id(bundle_id).is_some()
}

pub(super) fn current_url(application: &ApplicationInfo) -> Result<Option<String>> {
    let Some(browser) = Browser::for_bundle_id(&application.bundle_id) else {
        return Ok(None);
    };
    if application.pid <= 0 || application.instance_id.as_deref().is_none_or(str::is_empty) {
        return Err(ContextReadError::Unavailable.into());
    }
    if !same_running_instance(application) {
        return Err(ContextReadError::ApplicationChanged.into());
    }
    let result = super::automation::read(
        browser.script(),
        application,
        CAPTURE_TIMEOUT,
        MAX_URL_BYTES as u32,
    );
    // The sender addresses this PID. A quit or PID reuse during the event still
    // invalidates its reply, regardless of which bundle happens to be running.
    if !same_running_instance(application) {
        return Err(ContextReadError::ApplicationChanged.into());
    }
    let response = result?;
    Ok(validate_url(&response))
}

fn same_running_instance(application: &ApplicationInfo) -> bool {
    let Some(instance) = application
        .instance_id
        .as_deref()
        .filter(|id| !id.is_empty())
    else {
        return false;
    };
    if application.pid <= 0 || !supports(&application.bundle_id) {
        return false;
    }
    let Ok(bundle) = CString::new(application.bundle_id.as_str()) else {
        return false;
    };
    let Ok(instance) = CString::new(instance) else {
        return false;
    };
    // SAFETY: both strings remain alive for this synchronous FFI call. The native
    // side only checks the running application's bundle, PID, and process start.
    unsafe {
        launcher_application_identity_matches(application.pid, bundle.as_ptr(), instance.as_ptr())
            == 1
    }
}

fn validate_url(output: &str) -> Option<String> {
    let output = output.strip_suffix('\n').unwrap_or(output);
    let output = output.strip_suffix('\r').unwrap_or(output);
    if output.is_empty() || output == "missing value" {
        return None;
    }
    if output.len() > MAX_URL_BYTES
        || output
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return None;
    }
    let Ok(url) = url::Url::parse(output) else {
        return None;
    };
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    Some(output.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_urls_are_context_and_exact_query_is_preserved() {
        let raw = "https://example.org/search?q=a%20b&token=private\n";
        assert_eq!(validate_url(raw).as_deref(), Some(&raw[..raw.len() - 1]));
        for raw in [
            "file:///tmp/a",
            "javascript:alert(1)",
            "about:blank",
            "chrome://settings",
            "edge://settings",
            "not a URL",
        ] {
            assert!(validate_url(raw).is_none(), "{raw}");
        }
        assert!(validate_url("https://example.org/a\nb").is_none());
        assert!(validate_url(&format!(
            "https://example.org/{}",
            "x".repeat(MAX_URL_BYTES)
        ))
        .is_none());
    }

    #[test]
    fn unsupported_or_unidentified_app_never_requests_automation() {
        let app = ApplicationInfo {
            bundle_id: "org.mozilla.firefox".into(),
            name: "Firefox".into(),
            pid: 1,
            instance_id: Some("fake".into()),
        };
        assert!(current_url(&app).unwrap().is_none());
        for bundle_id in [SAFARI_BUNDLE_ID, CHROME_BUNDLE_ID, EDGE_BUNDLE_ID] {
            assert!(supports(bundle_id));
            let unidentified = ApplicationInfo {
                bundle_id: bundle_id.into(),
                instance_id: None,
                ..app.clone()
            };
            assert_eq!(
                current_url(&unidentified)
                    .unwrap_err()
                    .downcast_ref::<ContextReadError>(),
                Some(&ContextReadError::Unavailable)
            );
        }
        assert!(!supports(&app.bundle_id));
    }
}
