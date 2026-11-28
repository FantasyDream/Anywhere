//! Parse the native application-open inputs before they reach the shared review flow.
use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

/// A package handoff accepts an absolute local file path and one extension.
pub fn is_local_package(path: &Path) -> bool {
    #[cfg(windows)]
    if !matches!(
        path.components().next(),
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(),
            std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_))
    ) {
        return false;
    }
    path.is_absolute()
        && path.components().all(|part| part != Component::ParentDir)
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("awplugin"))
}

/// Only one command-line argument can be a document to open.
pub fn launch_package(arguments: impl IntoIterator<Item = OsString>) -> Option<PathBuf> {
    let mut arguments = arguments.into_iter();
    let path = PathBuf::from(arguments.next()?);
    (arguments.next().is_none() && is_local_package(&path)).then_some(path)
}

/// Convert the single file URL from the AppKit open-documents event.
pub fn opened_file_url(urls: &[String]) -> Option<PathBuf> {
    let [value] = urls else {
        return None;
    };
    let url = url::Url::parse(value).ok()?;
    if url.scheme() != "file"
        || url.host_str().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let path = url.to_file_path().ok()?;
    is_local_package(&path).then_some(path)
}
