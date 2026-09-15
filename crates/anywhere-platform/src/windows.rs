use super::*;
use anyhow::{bail, Context};
use std::{os::windows::ffi::OsStrExt, path::PathBuf, sync::Mutex};

extern "C" {
    fn launcher_windows_foreground(executable: *mut u16, capacity: u32, window: *mut isize) -> i32;
    fn launcher_windows_directory(window: isize, output: *mut u16, capacity: u32) -> i32;
    fn launcher_windows_open(value: *const u16, kind: i32) -> i32;
    fn launcher_windows_copy(value: *const u16, length: u32) -> i32;
}

#[derive(Default)]
pub struct WindowsPlatform {
    captured: Mutex<Option<(ApplicationInfo, isize)>>,
}

fn status(operation: &str, result: i32) -> Result<bool> {
    if result < 0 {
        tracing::error!(operation, hresult = result, "platform_api_failure");
        bail!("{operation} failed (HRESULT 0x{:08X})", result as u32);
    }
    Ok(result == 0)
}
fn wide(value: &std::ffi::OsStr) -> Result<Vec<u16>> {
    let mut result: Vec<u16> = value.encode_wide().collect();
    if result.contains(&0) {
        bail!("embedded NUL is not supported");
    }
    result.push(0);
    Ok(result)
}
fn shell_path(path: &Path) -> Result<Vec<u16>> {
    let path = path.canonicalize()?;
    let raw = path.as_os_str().to_string_lossy();
    // Shell applications expect ordinary DOS/UNC paths, not Rust's extended prefix.
    let display = raw
        .strip_prefix(r"\\?\UNC\")
        .map(|s| format!(r"\\{s}"))
        .unwrap_or_else(|| raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_owned());
    wide(std::ffi::OsStr::new(&display))
}
fn open(path: &Path, directory: bool, kind: i32) -> Result<()> {
    if if directory {
        !path.is_dir()
    } else {
        !path.is_file()
    } {
        bail!(
            "{} no longer exists: {}",
            if directory { "directory" } else { "file" },
            path.display()
        );
    }
    let path = shell_path(path)?;
    // SAFETY: null-terminated UTF-16 remains alive for the synchronous native call.
    status("open", unsafe {
        launcher_windows_open(path.as_ptr(), kind)
    })?;
    Ok(())
}
impl Platform for WindowsPlatform {
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        let mut snapshot = self
            .captured
            .lock()
            .map_err(|_| anyhow::anyhow!("capture lock poisoned"))?;
        *snapshot = None;
        let mut executable = vec![0; 32768];
        let mut window = 0;
        // SAFETY: output buffers have the supplied capacity; native code retains no pointers.
        if !status("frontmost_application", unsafe {
            launcher_windows_foreground(
                executable.as_mut_ptr(),
                executable.len() as u32,
                &mut window,
            )
        })? {
            return Ok(None);
        }
        let length = executable
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(executable.len());
        let executable = String::from_utf16(&executable[..length])?;
        let app = ApplicationInfo {
            name: Path::new(&executable)
                .file_stem()
                .context("application name missing")?
                .to_string_lossy()
                .into(),
            bundle_id: executable.to_lowercase(),
        };
        *snapshot = Some((app.clone(), window));
        Ok(Some(app))
    }
    fn application_directory(&self, application: &ApplicationInfo) -> Result<Option<PathBuf>> {
        if !application.name.eq_ignore_ascii_case("explorer") {
            return Ok(None);
        }
        let snapshot = self
            .captured
            .lock()
            .map_err(|_| anyhow::anyhow!("capture lock poisoned"))?;
        let Some((_, window)) = snapshot.as_ref().filter(|(app, _)| app == application) else {
            return Ok(None);
        };
        let mut path = vec![0; 32768];
        // SAFETY: native code bounds-checks the output and validates the captured HWND.
        if !status("current_directory", unsafe {
            launcher_windows_directory(*window, path.as_mut_ptr(), path.len() as u32)
        })? {
            return Ok(None);
        }
        let length = path.iter().position(|&c| c == 0).unwrap_or(path.len());
        Ok(Some(PathBuf::from(String::from_utf16(&path[..length])?)))
    }
    fn open_url(&self, value: &str) -> Result<()> {
        let url = url::Url::parse(value).context("invalid URL")?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            bail!("only absolute http/https URLs are permitted");
        }
        let value = wide(std::ffi::OsStr::new(url.as_str()))?;
        // SAFETY: validated null-terminated UTF-16, synchronous call.
        status("open_url", unsafe {
            launcher_windows_open(value.as_ptr(), 0)
        })?;
        Ok(())
    }
    fn open_file(&self, path: &Path) -> Result<()> {
        open(path, false, 0)
    }
    fn reveal_file(&self, path: &Path) -> Result<()> {
        open(path, false, 1)
    }
    fn open_directory(&self, path: &Path) -> Result<()> {
        open(path, true, 0)
    }
    fn open_terminal(&self, path: &Path) -> Result<()> {
        open(path, true, 2)
    }
    fn copy_text(&self, text: &str) -> Result<()> {
        let text = wide(std::ffi::OsStr::new(text))?;
        // SAFETY: length excludes the NUL terminator, but the allocation includes it.
        status("copy_text", unsafe {
            launcher_windows_copy(text.as_ptr(), (text.len() - 1) as u32)
        })?;
        Ok(())
    }

    // V0.2 capabilities are not implemented on Windows yet; they degrade to the
    // explicitly unsupported trait defaults so V0.1 behaviour is unchanged and no
    // application-launch or Git candidate can appear on this platform.
    fn bundle_application(&self, _bundle: &Path) -> Result<Option<BundleApplication>> {
        Ok(None)
    }
    fn application_icon(&self, _path: &str, _size: u32) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }
    fn clipboard_text(&self) -> Result<Option<String>> {
        Ok(None)
    }
    fn git_available(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_urls_and_missing_paths_before_windows_side_effects() {
        let platform = WindowsPlatform::default();
        for url in ["javascript:alert(1)", "file:///C:/Windows", "https://"] {
            assert!(platform.open_url(url).is_err());
        }
        let missing = std::env::temp_dir()
            .join(format!("launcher-missing-{}", std::process::id()))
            .join("file.txt");
        assert!(platform.open_file(&missing).is_err());
        assert!(platform.reveal_file(&missing).is_err());
        assert!(platform.open_directory(&missing).is_err());
        assert!(platform.open_terminal(&missing).is_err());
        assert!(platform.copy_text("invalid\0text").is_err());
    }
}
