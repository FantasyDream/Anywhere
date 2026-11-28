//! Per-invocation authority for a single file whose identity came from the OS selection.
use super::{broker::error, rpc_state::RequestControl};
use anywhere_platform::selected_file::SelectedFileIdentity;
use anywhere_protocol::{
    web_bridge::{WebErrorCode, WebResourceStat, WebResult, MAX_TEXT_BYTES},
    web_plugin::WebResourceKind,
};
use std::{
    fs::{File, Metadata},
    io::Read,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::UNIX_EPOCH,
};

const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;
pub(super) const MAX_PREVIEW_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct SelectedResource {
    pub(super) handle: String,
    pub(super) name: String,
    pub(super) path: PathBuf,
    pub(super) identity: SelectedFileIdentity,
    pub(super) access: Arc<Mutex<()>>,
}

impl SelectedResource {
    fn open(&self) -> WebResult<File> {
        self.identity
            .open(&self.path)
            .map_err(|_| error(WebErrorCode::Expired))
    }

    pub(super) fn stat(&self, control: &RequestControl) -> WebResult<WebResourceStat> {
        control.check()?;
        let _access = self
            .access
            .try_lock()
            .map_err(|_| error(WebErrorCode::Busy))?;
        let metadata = metadata(&self.open()?)?;
        control.check()?;
        Ok(WebResourceStat {
            handle: self.handle.clone(),
            name: self.name.clone(),
            kind: WebResourceKind::File,
            bytes: Some(metadata.len()).filter(|&size| size <= MAX_SAFE_INTEGER),
            modified_ms: metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .and_then(|duration| u64::try_from(duration.as_millis()).ok())
                .filter(|&time| time <= MAX_SAFE_INTEGER),
        })
    }

    pub(super) fn text(&self, limit: usize, control: &RequestControl) -> WebResult<String> {
        if !(1..=MAX_TEXT_BYTES).contains(&limit) {
            return Err(error(WebErrorCode::InvalidRequest));
        }
        let bytes = self.bytes(limit, control)?;
        if bytes.contains(&0) {
            return Err(error(WebErrorCode::InvalidRequest));
        }
        String::from_utf8(bytes).map_err(|_| error(WebErrorCode::InvalidRequest))
    }

    pub(super) fn bytes(&self, limit: usize, control: &RequestControl) -> WebResult<Vec<u8>> {
        control.check()?;
        let _access = self
            .access
            .try_lock()
            .map_err(|_| error(WebErrorCode::Busy))?;
        let file = self.open()?;
        let before = metadata(&file)?;
        if before.len() > limit as u64 {
            return Err(error(WebErrorCode::TooLarge));
        }
        let mut read = (&file).take((limit + 1) as u64);
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        loop {
            control.check()?;
            let count = read
                .read(&mut chunk)
                .map_err(|_| error(WebErrorCode::Expired))?;
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..count]);
            if bytes.len() > limit {
                return Err(error(WebErrorCode::TooLarge));
            }
        }
        let after = metadata(&file)?;
        if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
            return Err(error(WebErrorCode::Expired));
        }
        // The held FD cannot switch objects. The path must still name the selected
        // file when publishing a result, including after a concurrent rename.
        self.open()?;
        control.check()?;
        Ok(bytes)
    }
}

fn metadata(file: &File) -> WebResult<Metadata> {
    file.metadata().map_err(|_| error(WebErrorCode::Expired))
}
