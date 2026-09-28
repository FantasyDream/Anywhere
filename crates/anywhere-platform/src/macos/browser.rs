//! One Safari current-tab URL snapshot. The foreground app is frozen by Core before
//! the launcher takes focus; this adapter never activates or searches other browsers.
use super::native::launcher_application_identity_matches;
use crate::ApplicationInfo;
use anyhow::{anyhow, Result};
use std::{
    ffi::CString,
    io::Read,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

const SAFARI_BUNDLE_ID: &str = "com.apple.Safari";
const MAX_URL_BYTES: usize = 8 * 1024;
const MAX_ERROR_BYTES: usize = 4 * 1024;
// A first Automation prompt can outlive the ordinary fast-path read. The caller
// runs this off the first-snapshot path; it still needs a finite process deadline.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(15);
const PIPE_TIMEOUT: Duration = Duration::from_secs(1);
const SCRIPT: &str = include_str!("../../native/safari_url.applescript");

pub(super) fn current_url(application: &ApplicationInfo) -> Result<Option<String>> {
    if application.bundle_id != SAFARI_BUNDLE_ID || !same_running_instance(application) {
        return Ok(None);
    }
    let result = read_current_url();
    // `application id` resolves a bundle, not the PID captured at activation. A quit,
    // relaunch, or PID reuse during the Apple Event must discard the reply.
    if !same_running_instance(application) {
        return Ok(None);
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
    if application.pid <= 0 || application.bundle_id != SAFARI_BUNDLE_ID {
        return false;
    }
    let Ok(bundle) = CString::new(SAFARI_BUNDLE_ID) else {
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

struct BoundedOutput {
    bytes: Vec<u8>,
    exceeded: bool,
}

fn drain_to_limit(mut pipe: impl Read, limit: usize) -> std::io::Result<BoundedOutput> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 1024];
    let mut exceeded = false;
    loop {
        let count = pipe.read(&mut buffer)?;
        if count == 0 {
            return Ok(BoundedOutput { bytes, exceeded });
        }
        let room = limit.saturating_sub(bytes.len());
        bytes.extend_from_slice(&buffer[..count.min(room)]);
        exceeded |= count > room;
    }
}

fn spawn_reader(
    pipe: impl Read + Send + 'static,
    limit: usize,
) -> Receiver<std::io::Result<BoundedOutput>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(drain_to_limit(pipe, limit));
    });
    rx
}

fn receive(reader: Receiver<std::io::Result<BoundedOutput>>) -> Result<BoundedOutput> {
    reader
        .recv_timeout(PIPE_TIMEOUT)
        .map_err(|_| anyhow!("Safari URL capture output did not close"))?
        .map_err(|_| anyhow!("Safari URL capture output could not be read"))
}

fn read_current_url() -> Result<String> {
    let mut child = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(SCRIPT)
        .env("LC_ALL", "en_US.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| anyhow!("Safari URL capture could not start"))?;
    let stdout = spawn_reader(
        child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Safari URL capture has no output"))?,
        MAX_URL_BYTES + 2, // osascript adds a line ending
    );
    let stderr = spawn_reader(
        child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Safari URL capture has no error pipe"))?,
        MAX_ERROR_BYTES,
    );
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|_| anyhow!("Safari URL capture could not be checked"))?
        {
            break status;
        }
        if started.elapsed() >= CAPTURE_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(anyhow!("Safari URL capture timed out"));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let output = receive(stdout)?;
    let errors = receive(stderr)?;
    if !status.success() {
        if errors.bytes.windows(5).any(|window| window == b"-1743") {
            return Err(anyhow!("Safari Automation permission denied"));
        }
        return Err(anyhow!("Safari URL capture failed"));
    }
    if output.exceeded {
        return Err(anyhow!("Safari URL exceeds the capture size limit"));
    }
    String::from_utf8(output.bytes).map_err(|_| anyhow!("Safari URL is not UTF-8"))
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
    fn bounded_pipe_read_keeps_only_the_limit() {
        let output = drain_to_limit(&b"123456"[..], 4).unwrap();
        assert_eq!(output.bytes, b"1234");
        assert!(output.exceeded);
        let output = drain_to_limit(&b"https://example.org"[..], MAX_URL_BYTES).unwrap();
        assert!(!output.exceeded);
    }

    #[test]
    fn unsupported_or_unidentified_app_never_requests_automation() {
        let app = ApplicationInfo {
            bundle_id: "com.google.Chrome".into(),
            name: "Chrome".into(),
            pid: 1,
            instance_id: Some("fake".into()),
        };
        assert!(current_url(&app).unwrap().is_none());
        let safari = ApplicationInfo {
            bundle_id: SAFARI_BUNDLE_ID.into(),
            instance_id: None,
            ..app
        };
        assert!(current_url(&safari).unwrap().is_none());
    }
}
