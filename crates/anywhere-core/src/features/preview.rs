//! Platform-neutral, on-demand resources. Native viewers own rendering, never file authority.
use anyhow::{bail, ensure, Context, Result};
use anywhere_platform::clipboard::ClipboardPayload;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct PreviewResource {
    pub id: String,
    pub paths: Vec<String>,
    pub title: String,
}
struct Lease {
    _temporary: Option<tempfile::TempDir>,
    pending_until: Option<Instant>,
}
#[derive(Default)]
pub struct PreviewStore {
    root: Option<PathBuf>,
    initialized: bool,
    leases: BTreeMap<String, Lease>,
}
impl PreviewStore {
    pub fn configure(&mut self, root: PathBuf) {
        self.root = Some(root);
        self.initialized = false;
    }
    pub fn file(&mut self, path: PathBuf) -> Result<PreviewResource> {
        self.insert(vec![path], String::new(), None)
    }
    pub fn clipboard(
        &mut self,
        payload: &ClipboardPayload,
        title: String,
    ) -> Result<PreviewResource> {
        let files = payload.files().unwrap_or_default();
        if !files.is_empty() {
            return self.insert(files.into_iter().map(PathBuf::from).collect(), title, None);
        }
        // Native renderers decode the original bounded image, not a Rust thumbnail.
        let image = payload.formats.iter().find_map(|f| {
            let extension = match f.name.as_str() {
                "public.png" | "PNG" | "image/png" => "png",
                "public.tiff" | "image/tiff" => "tiff",
                "public.jpeg" | "JFIF" | "image/jpeg" => "jpg",
                "public.jpeg-2000" | "image/jp2" => "jp2",
                "com.compuserve.gif" | "image/gif" => "gif",
                "org.webmproject.webp" | "image/webp" => "webp",
                "image/bmp" => "bmp",
                _ => return None,
            };
            Some((extension, f.data.as_slice()))
        });
        let rich = payload.formats.iter().find(|f| {
            matches!(
                f.name.as_str(),
                "public.rtf" | "text/rtf" | "Rich Text Format"
            )
        });
        let (extension, bytes) = if let Some(image) = image {
            image
        } else if let Some(text) = payload.plain_text() {
            ("txt", text.as_bytes())
        } else if let Some(rich) = rich {
            ("rtf", rich.data.as_slice())
        } else if let Some(html) = payload
            .formats
            .iter()
            .find(|f| matches!(f.name.as_str(), "public.html" | "text/html" | "HTML Format"))
        {
            // Display source as inert text; never render clipboard HTML with network access.
            std::str::from_utf8(&html.data).context("unsupported text encoding")?;
            ("txt", html.data.as_slice())
        } else {
            bail!("this clipboard format cannot be previewed")
        };
        ensure!(
            bytes.len() <= super::clipboard::history::MAX_ENTRY_BYTES,
            "preview is too large"
        );
        let directory = self.temporary()?;
        let path = directory.path().join(format!("Clipboard.{extension}"));
        std::fs::write(&path, bytes)?;
        self.insert(vec![path], title, Some(directory))
    }
    fn temporary(&mut self) -> Result<tempfile::TempDir> {
        if let Some(root) = &self.root {
            std::fs::create_dir_all(root)?;
            if !self.initialized {
                // This dedicated app directory contains only resources from prior runs.
                for entry in std::fs::read_dir(root)? {
                    let entry = entry?;
                    if entry.file_name().to_string_lossy().starts_with("preview-")
                        && entry.file_type()?.is_dir()
                    {
                        let _ = std::fs::remove_dir_all(entry.path());
                    }
                }
                self.initialized = true;
            }
            return Ok(tempfile::Builder::new()
                .prefix("preview-")
                .tempdir_in(root)?);
        }
        Ok(tempfile::Builder::new()
            .prefix("anywhere-preview-")
            .tempdir()?)
    }
    fn insert(
        &mut self,
        paths: Vec<PathBuf>,
        title: String,
        temporary: Option<tempfile::TempDir>,
    ) -> Result<PreviewResource> {
        self.leases.retain(|_, lease| {
            lease
                .pending_until
                .is_none_or(|until| until > Instant::now())
        });
        ensure!(self.leases.len() < 4, "too many active previews");
        ensure!(
            !paths.is_empty() && paths.len() <= 256,
            "invalid preview selection"
        );
        let paths = paths
            .into_iter()
            .map(|path| {
                ensure!(path.is_absolute(), "preview requires a local absolute path");
                let path = path.canonicalize().context("preview file is unavailable")?;
                let metadata = path.metadata()?;
                ensure!(
                    metadata.is_file() || metadata.is_dir(),
                    "unsupported preview resource"
                );
                path.into_os_string()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("unsupported file name"))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut token = [0u8; 16];
        getrandom::fill(&mut token).map_err(|_| anyhow::anyhow!("preview token unavailable"))?;
        let id = token
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let resource = PreviewResource {
            id: id.clone(),
            paths,
            title,
        };
        self.leases.insert(
            id,
            Lease {
                _temporary: temporary,
                pending_until: Some(Instant::now() + Duration::from_secs(5)),
            },
        );
        Ok(resource)
    }
    pub fn retain(&mut self, id: &str) -> Result<()> {
        let lease = self
            .leases
            .get_mut(id)
            .context("preview resource expired")?;
        ensure!(
            lease
                .pending_until
                .is_none_or(|until| until > Instant::now()),
            "preview resource expired"
        );
        lease.pending_until = None;
        Ok(())
    }
    pub fn release(&mut self, id: &str) {
        self.leases.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_is_complete_private_and_lives_only_for_its_lease() {
        let root = tempfile::tempdir().unwrap();
        let stale = root.path().join("preview-stale");
        std::fs::create_dir(&stale).unwrap();
        std::fs::write(stale.join("old.txt"), "private").unwrap();
        let mut store = PreviewStore::default();
        store.configure(root.path().into());
        let text = "中文".repeat(40000);
        let resource = store
            .clipboard(&ClipboardPayload::text(&text), "Text".into())
            .unwrap();
        assert!(!stale.exists());
        assert_eq!(std::fs::read_to_string(&resource.paths[0]).unwrap(), text);
        store.release(&resource.id);
        assert!(!std::path::Path::new(&resource.paths[0]).exists());
        assert!(store.retain(&resource.id).is_err());
    }
    #[test]
    fn unclaimed_resources_expire_but_displayed_leases_survive() {
        let root = tempfile::tempdir().unwrap();
        let mut store = PreviewStore::default();
        store.configure(root.path().into());
        let abandoned = store
            .clipboard(&ClipboardPayload::text("orphan"), "".into())
            .unwrap();
        let displayed = store
            .clipboard(&ClipboardPayload::text("shown"), "".into())
            .unwrap();
        store.retain(&displayed.id).unwrap();
        store.leases.get_mut(&abandoned.id).unwrap().pending_until =
            Some(Instant::now() - Duration::from_secs(1));
        let next = store
            .clipboard(&ClipboardPayload::text("next"), "".into())
            .unwrap();
        assert!(!std::path::Path::new(&abandoned.paths[0]).exists());
        assert!(std::path::Path::new(&displayed.paths[0]).exists());
        assert!(store.retain(&abandoned.id).is_err());
        store.release(&displayed.id);
        store.release(&next.id);
    }
    #[test]
    fn image_bytes_are_not_decoded_and_unsupported_formats_do_not_create_a_lease() {
        use anywhere_platform::clipboard::ClipboardFormat;
        let mut store = PreviewStore::default();
        let payload = ClipboardPayload {
            formats: vec![ClipboardFormat {
                name: "public.png".into(),
                data: vec![137, 80, 78, 71],
            }],
        };
        let resource = store.clipboard(&payload, "image".into()).unwrap();
        assert_eq!(
            std::fs::read(&resource.paths[0]).unwrap(),
            payload.formats[0].data
        );
        store.release(&resource.id);
        let payload = ClipboardPayload {
            formats: vec![ClipboardFormat {
                name: "application/custom".into(),
                data: vec![0, 255],
            }],
        };
        assert!(store.clipboard(&payload, "".into()).is_err());
        assert!(store.leases.is_empty());
    }
    #[test]
    fn original_file_is_not_copied_or_deleted_and_invalid_targets_fail() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("test.txt");
        std::fs::write(&path, "test").unwrap();
        let mut store = PreviewStore::default();
        let resource = store.file(path.clone()).unwrap();
        store.release(&resource.id);
        assert!(path.exists());
        assert!(store.file("relative.txt".into()).is_err());
        assert!(store.file(root.path().join("missing")).is_err());
    }
}
