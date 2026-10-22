//! OS path encoding and private, leased workspaces for complete file searches.
#[cfg(windows)]
use anyhow::bail;
use anyhow::Result;
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::Once,
};
static CLEANUP: Once = Once::new();

/// Normalize lexical traversal and public system path aliases without probing
/// the user-selected target. Canonical target validation still follows this.
pub fn lexical_scope_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                result.pop();
            }
            _ => result.push(part.as_os_str()),
        }
    }
    #[cfg(target_os = "macos")]
    for alias in ["/var", "/tmp", "/etc"] {
        if let Ok(tail) = result.strip_prefix(alias) {
            if let Ok(target) = std::fs::read_link(alias) {
                let target = if target.is_absolute() {
                    target
                } else {
                    Path::new(alias)
                        .parent()
                        .expect("system alias has a parent")
                        .join(target)
                };
                return target.join(tail);
            }
        }
    }
    result
}

pub struct SearchWorkspace {
    lease: Option<File>,
    directory: Option<tempfile::TempDir>,
}
impl SearchWorkspace {
    pub fn new() -> Result<Self> {
        CLEANUP.call_once(clean_stale_workspaces);
        let directory = tempfile::Builder::new()
            .prefix("anywhere-file-search-")
            .tempdir()?;
        let lease = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(directory.path().join("lease"))?;
        lease.try_lock()?;
        Ok(Self {
            lease: Some(lease),
            directory: Some(directory),
        })
    }
    pub fn path(&self) -> &Path {
        self.directory.as_ref().expect("live workspace").path()
    }
}
impl Drop for SearchWorkspace {
    fn drop(&mut self) {
        // Windows cannot delete a directory containing an open lease file.
        self.lease.take();
        self.directory.take();
    }
}
fn clean_stale_workspaces() {
    clean_stale_workspaces_in(&std::env::temp_dir());
}
fn clean_stale_workspaces_in(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with("anywhere-file-search-")
            || !entry.file_type().is_ok_and(|kind| kind.is_dir())
        {
            continue;
        }
        let Ok(lease) = OpenOptions::new()
            .read(true)
            .write(true)
            .open(entry.path().join("lease"))
        else {
            continue;
        };
        if lease.try_lock().is_ok() {
            drop(lease);
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}
#[cfg(unix)]
pub fn encode_path(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}
#[cfg(windows)]
pub fn encode_path(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}
#[cfg(unix)]
pub fn decode_path(bytes: Vec<u8>) -> Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Ok(std::ffi::OsString::from_vec(bytes).into())
}
#[cfg(windows)]
pub fn decode_path(bytes: Vec<u8>) -> Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    if bytes.len() % 2 != 0 {
        bail!("invalid path record");
    }
    Ok(std::ffi::OsString::from_wide(
        &bytes
            .chunks_exact(2)
            .map(|v| u16::from_le_bytes([v[0], v[1]]))
            .collect::<Vec<_>>(),
    )
    .into())
}

pub fn is_hidden(path: &Path, name: &std::ffi::OsStr) -> Result<bool> {
    if name.to_string_lossy().starts_with('.') {
        return Ok(true);
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        Ok(std::fs::symlink_metadata(path)?.st_flags() & libc::UF_HIDDEN != 0)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        Ok(std::fs::symlink_metadata(path)?.file_attributes() & 2 != 0)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = path;
        Ok(false)
    }
}

/// Stable native path identity; display conversion must not merge distinct targets.
pub fn path_fingerprint(path: &Path) -> u64 {
    encode_path(path)
        .into_iter()
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_removes_only_abandoned_leased_workspaces() {
        let root = tempfile::tempdir().unwrap();
        let active = root.path().join("anywhere-file-search-active");
        let abandoned = root.path().join("anywhere-file-search-abandoned");
        let initializing = root.path().join("anywhere-file-search-initializing");
        let unrelated = root.path().join("other-search");
        for directory in [&active, &abandoned, &initializing, &unrelated] {
            std::fs::create_dir(directory).unwrap();
            std::fs::write(directory.join("matches"), b"query data").unwrap();
        }
        let active_lease = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(active.join("lease"))
            .unwrap();
        active_lease.try_lock().unwrap();
        File::create(abandoned.join("lease")).unwrap();
        File::create(unrelated.join("lease")).unwrap();

        clean_stale_workspaces_in(root.path());
        assert!(!abandoned.exists());
        assert!(active.join("matches").exists(), "live lease must survive");
        assert!(
            initializing.exists(),
            "creation without a lease is not abandoned"
        );
        assert!(
            unrelated.exists(),
            "other temporary directories must survive"
        );

        // Parallel process tests may briefly inherit this descriptor during fork.
        // Release the fixture's lock explicitly rather than waiting for every
        // inherited descriptor to close at exec.
        active_lease.unlock().unwrap();
        drop(active_lease);
        clean_stale_workspaces_in(root.path());
        assert!(!active.exists(), "released lease can be reclaimed");
    }

    #[test]
    fn dropping_workspace_releases_lease_and_removes_query_files() {
        let workspace = SearchWorkspace::new().unwrap();
        let directory = workspace.path().to_owned();
        std::fs::write(directory.join("matches"), b"query data").unwrap();
        assert!(directory.join("lease").is_file());
        drop(workspace);
        assert!(!directory.exists());
    }
}
