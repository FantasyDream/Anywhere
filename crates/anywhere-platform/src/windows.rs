mod clipboard;
mod everything;
mod shell;

use crate::{
    ApplicationCatalog, ApplicationHost, ApplicationInfo, BundleApplication, ClipboardHost,
    ClipboardSnapshot, ContextReader, FileIndexResults, FileSearchHost, FileSearchIndex, GitHost,
    MenuHost, ResourceOpener,
};
use anyhow::{ensure, Result};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

#[derive(Default)]
pub(super) struct WindowsPlatform {
    clipboard_tracker: OnceLock<Arc<clipboard::Tracker>>,
}

impl ApplicationCatalog for WindowsPlatform {
    fn application_roots(&self) -> Vec<PathBuf> {
        ["APPDATA", "ProgramData"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .map(|path| path.join(r"Microsoft\Windows\Start Menu\Programs"))
            .collect()
    }

    fn is_application_entry(&self, path: &Path) -> bool {
        shell::has_extension(path, "lnk") && path.is_file()
    }

    fn bundle_application(&self, path: &Path) -> Result<Option<BundleApplication>> {
        if !self.is_application_entry(path) {
            return Ok(None);
        }
        let Some((name, path)) = path
            .file_stem()
            .and_then(|name| name.to_str())
            .zip(path.to_str())
        else {
            return Ok(None);
        };
        Ok(Some(BundleApplication {
            bundle_id: format!("windows.shortcut:{}", path.to_lowercase()),
            name: name.to_owned(),
            localized_names: Default::default(),
            aliases: Vec::new(),
            path: Some(path.to_owned()),
            icon: None,
        }))
    }
}

impl ApplicationHost for WindowsPlatform {
    fn frontmost_application(&self) -> Result<Option<ApplicationInfo>> {
        Ok(shell::frontmost_application())
    }

    fn launch_application(&self, path: &str) -> Result<bool> {
        let path = Path::new(path);
        ensure!(
            path.is_absolute() && path.is_file() && shell::has_extension(path, "lnk"),
            "Start Menu shortcut is unavailable"
        );
        shell::open(path.as_os_str())?;
        Ok(true)
    }
}

impl ContextReader for WindowsPlatform {
    fn application_directory(&self, _: &ApplicationInfo) -> Result<Option<PathBuf>> {
        Ok(None)
    }
}

impl ClipboardHost for WindowsPlatform {
    fn copy_text(&self, text: &str) -> Result<()> {
        ensure!(
            !text.contains('\0'),
            "clipboard text contains an embedded NUL"
        );
        crate::clipboard::copy_text(text)
    }

    fn start_clipboard_tracking(&self) {
        self.clipboard_tracker
            .get_or_init(clipboard::Tracker::start);
    }

    fn clipboard_generation(&self) -> Result<i64> {
        Ok(i64::from(clipboard::generation()?))
    }

    fn request_clipboard_history_access(&self) -> Result<()> {
        clipboard::generation().map(|_| ())
    }

    fn clipboard_history_payload(
        &self,
        generation: i64,
    ) -> Result<Option<crate::clipboard::ClipboardPayload>> {
        let generation = u32::try_from(generation)?;
        Ok(
            clipboard::read_text(crate::clipboard::MAX_PAYLOAD_BYTES, Some(generation))?
                .map(|text| crate::clipboard::ClipboardPayload::text(&text)),
        )
    }

    fn watch_clipboard(
        &self,
        changed: Box<dyn Fn() + Send>,
    ) -> Result<Box<dyn crate::clipboard::ClipboardWatch>> {
        crate::clipboard::watch(changed)
    }

    fn clipboard_text(&self) -> Result<Option<String>> {
        clipboard::read_text(16 * 1024, None)
    }

    fn clipboard_snapshot(&self) -> Result<ClipboardSnapshot> {
        let generation = clipboard::generation()?;
        let text = clipboard::read_text(16 * 1024, Some(generation))?;
        let age_ms = self
            .clipboard_tracker
            .get()
            .and_then(|tracker| tracker.age_ms(generation));
        Ok(ClipboardSnapshot { text, age_ms })
    }
}

impl FileSearchHost for WindowsPlatform {
    fn file_search_supported(&self) -> bool {
        true
    }

    fn file_search_index(&self) -> Option<&dyn FileSearchIndex> {
        Some(self)
    }
}

impl FileSearchIndex for WindowsPlatform {
    fn search_index(
        &self,
        query: &str,
        roots: &[PathBuf],
        cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<FileIndexResults> {
        everything::search(query, roots, cancelled)
    }
}

impl ResourceOpener for WindowsPlatform {
    fn open_url(&self, value: &str) -> Result<()> {
        let url = url::Url::parse(value)?;
        ensure!(
            matches!(url.scheme(), "http" | "https") && url.host_str().is_some(),
            "only absolute http/https URLs are permitted"
        );
        shell::open(url.as_str().as_ref())
    }

    fn open_file(&self, path: &Path) -> Result<()> {
        ensure!(
            path.is_absolute() && path.is_file(),
            "file no longer exists"
        );
        shell::open(dunce::simplified(path).as_os_str())
    }

    fn reveal_file(&self, path: &Path) -> Result<()> {
        shell::reveal(path)
    }

    fn open_directory(&self, path: &Path) -> Result<()> {
        ensure!(
            path.is_absolute() && path.is_dir(),
            "directory no longer exists"
        );
        shell::open(dunce::simplified(path).as_os_str())
    }

    fn open_terminal(&self, path: &Path) -> Result<()> {
        shell::open_terminal(path)
    }
}

impl MenuHost for WindowsPlatform {}
impl GitHost for WindowsPlatform {}
