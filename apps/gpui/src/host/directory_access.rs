use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    io::Read as _,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum DirectoryAccessError {
    Authorization,
    Unavailable,
}

impl std::fmt::Display for DirectoryAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("search directory access unavailable")
    }
}
impl std::error::Error for DirectoryAccessError {}

pub(super) fn localized_error(error: &anyhow::Error, locale: crate::l10n::Locale) -> String {
    match error.downcast_ref::<DirectoryAccessError>() {
        Some(DirectoryAccessError::Authorization) => {
            crate::l10n::search_directory_authorization_failed(locale)
        }
        Some(DirectoryAccessError::Unavailable) => {
            crate::l10n::search_directory_unavailable(locale)
        }
        None => error.to_string(),
    }
}

const MAX_BOOKMARK_FILE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BookmarkFile {
    version: u32,
    bookmarks: Vec<StoredBookmark>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredBookmark {
    path: String,
    bytes: Vec<u8>,
}

/// Owns the security-scoped leases used by file search. The bookmark file is
/// deliberately small and separate from user preferences so corrupt or stale
/// grants fail closed instead of silently widening the file-search scope.
pub struct DirectoryAccess {
    bookmark_file: PathBuf,
    #[cfg(target_os = "macos")]
    leases: std::sync::Mutex<Vec<SecurityScope>>,
}

#[cfg(target_os = "macos")]
struct SecurityScope(objc2::rc::Retained<objc2_foundation::NSURL>);

#[cfg(target_os = "macos")]
impl Drop for SecurityScope {
    fn drop(&mut self) {
        // SAFETY: Constructed only after a successful start; this guard owns
        // exactly one scope, including during partial reconciliation failure.
        unsafe { self.0.stopAccessingSecurityScopedResource() };
    }
}

impl DirectoryAccess {
    pub fn new(bookmark_file: PathBuf) -> Self {
        Self {
            bookmark_file,
            #[cfg(target_os = "macos")]
            leases: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Saves user-selected directory grants and verifies that the selected
    /// roots can all be restored and opened.
    pub fn authorize(
        &self,
        selected: &[PathBuf],
        roots: &[String],
        saved_roots: &[String],
    ) -> Result<Vec<String>> {
        self.authorize_impl(selected, roots, saved_roots)
            .map_err(|error| {
                tracing::warn!(%error, "search_directory_authorization_failed");
                DirectoryAccessError::Authorization.into()
            })
    }
    fn authorize_impl(
        &self,
        selected: &[PathBuf],
        roots: &[String],
        saved_roots: &[String],
    ) -> Result<Vec<String>> {
        let requested = canonical_roots(roots)?;
        let mut current = self.load().unwrap_or_else(|error| {
            tracing::warn!(%error, "search_directory_bookmarks_reset");
            BookmarkFile {
                version: 1,
                bookmarks: Vec::new(),
            }
        });
        current.keep_roots(&requested, saved_roots);
        for path in selected {
            let path = path
                .canonicalize()
                .with_context(|| format!("search directory is unavailable: {}", path.display()))?;
            anyhow::ensure!(
                path.is_dir(),
                "search directory is not a directory: {}",
                path.display()
            );
            let path_text = path
                .to_str()
                .context("search directory name is not valid Unicode")?
                .to_owned();
            if !requested
                .iter()
                .any(|root| root.to_string_lossy() == path_text)
            {
                continue;
            }
            let bytes = self.bookmark_bytes(&path)?;
            current
                .bookmarks
                .retain(|bookmark| bookmark.path != path_text);
            current.bookmarks.push(StoredBookmark {
                path: path_text,
                bytes,
            });
        }
        // Probe the draft grants with temporary scopes. Only reconciliation
        // of the saved preferences replaces the leases used by live search.
        #[cfg(target_os = "macos")]
        Self::resolve_macos(&current, &requested)?;
        self.save(&current)?;
        Ok(requested
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect())
    }

    /// Starts access for exactly the configured roots. Missing, stale, or
    /// unreadable grants are errors; callers must leave file search disabled.
    pub fn reconcile(&self, roots: &[String]) -> Result<Vec<PathBuf>> {
        let requested = canonical_roots(roots).map_err(|error| {
            tracing::warn!(%error, "search_directory_roots_unavailable");
            DirectoryAccessError::Unavailable
        })?;
        if requested.is_empty() {
            self.clear();
            return Ok(Vec::new());
        }
        #[cfg(target_os = "macos")]
        {
            (|| {
                let resolved = Self::resolve_macos(&self.load()?, &requested)?;
                *self
                    .leases
                    .lock()
                    .map_err(|_| anyhow::anyhow!("search directory access is unavailable"))? =
                    resolved;
                Ok(requested)
            })()
            .map_err(|error: anyhow::Error| {
                tracing::warn!(%error, "search_directory_reconcile_failed");
                DirectoryAccessError::Authorization.into()
            })
        }
        #[cfg(not(target_os = "macos"))]
        {
            for root in &requested {
                anyhow::ensure!(
                    root.is_dir(),
                    "search directory is unavailable: {}",
                    root.display()
                );
            }
            Ok(requested)
        }
    }

    pub fn clear(&self) {
        #[cfg(target_os = "macos")]
        {
            if let Ok(mut leases) = self.leases.lock() {
                leases.clear();
            }
        }
    }

    fn load(&self) -> Result<BookmarkFile> {
        match std::fs::File::open(&self.bookmark_file) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(MAX_BOOKMARK_FILE_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)?;
                anyhow::ensure!(
                    bytes.len() <= MAX_BOOKMARK_FILE_BYTES,
                    "search access file is too large"
                );
                let file: BookmarkFile = serde_json::from_slice(&bytes)
                    .context("search directory authorization is unreadable")?;
                anyhow::ensure!(file.version == 1, "unsupported search access file");
                anyhow::ensure!(
                    file.bookmarks.len() <= 16,
                    "too many search access bookmarks"
                );
                Ok(file)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BookmarkFile {
                version: 1,
                bookmarks: Vec::new(),
            }),
            Err(error) => Err(error).context("search directory authorization is unreadable"),
        }
    }

    fn save(&self, file: &BookmarkFile) -> Result<()> {
        anyhow::ensure!(
            file.bookmarks.len() <= 16,
            "too many search access bookmarks"
        );
        let bytes = serde_json::to_vec_pretty(file)?;
        anyhow::ensure!(
            bytes.len() <= MAX_BOOKMARK_FILE_BYTES,
            "search access file is too large"
        );
        super::atomic_write_private(&self.bookmark_file, &bytes)
    }

    #[cfg(target_os = "macos")]
    fn bookmark_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        use objc2_foundation::{NSString, NSURLBookmarkCreationOptions, NSURL};

        let url = NSURL::fileURLWithPath_isDirectory(
            &NSString::from_str(
                path.to_str()
                    .context("search directory name is not valid Unicode")?,
            ),
            true,
        );
        let data = url
            .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
                NSURLBookmarkCreationOptions::WithSecurityScope
                    | NSURLBookmarkCreationOptions::SecurityScopeAllowOnlyReadAccess,
                None,
                None,
            )
            .map_err(|error| {
                anyhow::anyhow!(
                    "search directory could not be authorized: {}",
                    error.localizedDescription()
                )
            })?;
        Ok(nsdata_bytes(&data))
    }

    #[cfg(target_os = "macos")]
    fn resolve_macos(file: &BookmarkFile, requested: &[PathBuf]) -> Result<Vec<SecurityScope>> {
        use objc2_foundation::{NSData, NSURLBookmarkResolutionOptions, NSURL};

        let mut resolved = Vec::new();
        for root in requested {
            let bookmark = file
                .bookmarks
                .iter()
                .find(|bookmark| root.to_string_lossy() == bookmark.path)
                .with_context(|| {
                    format!("search directory needs authorization: {}", root.display())
                })?;
            // SAFETY: `bytes` is copied into owned NSData and the pointer is
            // valid for that allocation's lifetime.
            let data = unsafe {
                NSData::dataWithBytes_length(bookmark.bytes.as_ptr().cast(), bookmark.bytes.len())
            };
            let mut stale = objc2::runtime::Bool::NO;
            let url = unsafe {
                NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(
                    &data,
                    NSURLBookmarkResolutionOptions::WithSecurityScope
                        | NSURLBookmarkResolutionOptions::WithoutUI
                        | NSURLBookmarkResolutionOptions::WithoutMounting
                        | NSURLBookmarkResolutionOptions::WithoutImplicitStartAccessing,
                    None,
                    &mut stale,
                )
            }
            .map_err(|error| {
                anyhow::anyhow!(
                    "search directory authorization expired: {}",
                    error.localizedDescription()
                )
            })?;
            anyhow::ensure!(
                !stale.as_bool(),
                "search directory authorization is stale; authorize it again"
            );
            let path = url
                .path()
                .map(|path| path.to_string())
                .context("authorized search directory has no path")?;
            let path_text = path.clone();
            let path = PathBuf::from(path).canonicalize().with_context(|| {
                format!("authorized search directory is unavailable: {path_text}")
            })?;
            anyhow::ensure!(
                path == *root,
                "search directory authorization changed; authorize it again"
            );
            // SAFETY: The resolved security-scoped URL is retained by `url`.
            anyhow::ensure!(
                unsafe { url.startAccessingSecurityScopedResource() },
                "search directory access could not be opened: {}",
                root.display()
            );
            resolved.push(SecurityScope(url));
        }
        Ok(resolved)
    }
}

impl BookmarkFile {
    fn keep_roots(&mut self, draft: &[PathBuf], saved: &[String]) {
        let saved = saved
            .iter()
            .map(|root| {
                let path = PathBuf::from(root);
                path.canonicalize().unwrap_or(path)
            })
            .collect::<Vec<_>>();
        self.bookmarks.retain(|bookmark| {
            let path = Path::new(&bookmark.path);
            draft.iter().chain(&saved).any(|root| root == path)
        });
    }
}

fn canonical_roots(roots: &[String]) -> Result<Vec<PathBuf>> {
    let mut canonical = Vec::new();
    for root in roots {
        let root = PathBuf::from(root)
            .canonicalize()
            .with_context(|| format!("search directory is unavailable: {root}"))?;
        anyhow::ensure!(
            root.is_dir(),
            "search directory is not a directory: {}",
            root.display()
        );
        if !canonical.contains(&root) {
            canonical.push(root);
        }
    }
    anyhow::ensure!(canonical.len() <= 8, "use at most 8 search directories");
    Ok(canonical)
}

#[cfg(target_os = "macos")]
fn nsdata_bytes(data: &objc2_foundation::NSData) -> Vec<u8> {
    use std::ptr::NonNull;

    let length = data.length();
    let mut bytes = vec![0; length];
    if let Some(pointer) = NonNull::new(bytes.as_mut_ptr().cast()) {
        // SAFETY: `bytes` is writable for exactly `length` bytes.
        unsafe { data.getBytes_length(pointer, length) };
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_directory_grants_preserves_saved_roots() {
        let temporary = tempfile::tempdir().unwrap();
        let saved = temporary.path().join("saved-offline").display().to_string();
        let abandoned = temporary.path().join("abandoned").display().to_string();
        let access = DirectoryAccess::new(temporary.path().join("bookmarks.json"));
        access
            .save(&BookmarkFile {
                version: 1,
                bookmarks: vec![
                    StoredBookmark {
                        path: saved.clone(),
                        bytes: vec![1],
                    },
                    StoredBookmark {
                        path: abandoned,
                        bytes: vec![2],
                    },
                ],
            })
            .unwrap();

        assert!(access
            .authorize(&[], &[], std::slice::from_ref(&saved))
            .unwrap()
            .is_empty());
        let bookmarks = access.load().unwrap().bookmarks;
        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks[0].path, saved);
        assert_eq!(bookmarks[0].bytes, [1]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn failed_draft_validation_keeps_the_previous_bookmark_file() {
        let temporary = tempfile::tempdir().unwrap();
        let requested = temporary.path().join("no-bookmark");
        std::fs::create_dir(&requested).unwrap();
        let path = temporary.path().join("bookmarks.json");
        let access = DirectoryAccess::new(path.clone());
        access
            .save(&BookmarkFile {
                version: 1,
                bookmarks: vec![StoredBookmark {
                    path: "/saved-offline".into(),
                    bytes: vec![1],
                }],
            })
            .unwrap();
        let previous = std::fs::read(&path).unwrap();
        assert!(access
            .authorize(
                &[],
                &[requested.display().to_string()],
                &["/saved-offline".into()]
            )
            .is_err());
        assert_eq!(std::fs::read(path).unwrap(), previous);
    }

    #[test]
    fn roots_are_canonicalized_and_deduplicated() {
        let temporary = tempfile::tempdir().unwrap();
        let nested = temporary.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        let roots = vec![
            nested.to_string_lossy().into_owned(),
            nested.to_string_lossy().into_owned(),
        ];
        let canonical = canonical_roots(&roots).unwrap();
        assert_eq!(canonical.len(), 1);
        assert_eq!(canonical[0], nested.canonicalize().unwrap());
    }

    #[test]
    fn missing_and_non_directory_roots_fail_closed() {
        let temporary = tempfile::tempdir().unwrap();
        let file = temporary.path().join("file");
        std::fs::write(&file, b"not a directory").unwrap();
        assert!(
            canonical_roots(&[temporary.path().join("missing").display().to_string()]).is_err()
        );
        assert!(canonical_roots(&[file.display().to_string()]).is_err());
    }

    #[test]
    fn bookmark_storage_is_private_and_bounded() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("access.json");
        let access = DirectoryAccess::new(path.clone());
        access
            .save(&BookmarkFile {
                version: 1,
                bookmarks: Vec::new(),
            })
            .unwrap();
        assert!(path.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn oversized_bookmark_files_are_rejected_before_full_allocation() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("access.json");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_BOOKMARK_FILE_BYTES as u64 + 1).unwrap();
        let error = DirectoryAccess::new(path).load().unwrap_err();
        assert!(error.to_string().contains("too large"));
    }

    #[test]
    fn concurrent_bookmark_saves_use_independent_temporary_files() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("access.json");
        let writers = (0..8)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || {
                    DirectoryAccess::new(path).save(&BookmarkFile {
                        version: 1,
                        bookmarks: Vec::new(),
                    })
                })
            })
            .collect::<Vec<_>>();
        for writer in writers {
            writer.join().unwrap().unwrap();
        }
        assert!(path.exists());
        assert!(std::fs::read_dir(temporary.path())
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")));
    }
}
