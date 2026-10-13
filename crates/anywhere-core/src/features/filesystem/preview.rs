//! Bounded, plain-text previews. No shell, directory traversal, or external viewers.
use crate::core::context::ResolvedContext;
use anyhow::Result;
use std::{fs::File, io::Read, time::UNIX_EPOCH};

pub const MAX_PREVIEW_BYTES: usize = 64 * 1024;
pub struct FilePreview {
    pub path: String,
    pub is_directory: bool,
    pub size_bytes: u64,
    pub modified_ms: Option<i64>,
    pub text: Option<String>,
    pub notice: Option<String>,
}

pub fn read(context: &ResolvedContext) -> Result<FilePreview> {
    super::search::validate_target(context)?;
    let path = context
        .value
        .path()
        .ok_or_else(|| anyhow::anyhow!("file target missing"))?;
    let metadata = path.metadata()?;
    let mut preview = FilePreview {
        path: path.to_string_lossy().into_owned(),
        is_directory: metadata.is_dir(),
        size_bytes: metadata.len(),
        modified_ms: metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .and_then(|d| i64::try_from(d.as_millis()).ok()),
        text: None,
        notice: None,
    };
    if metadata.is_dir() {
        return Ok(preview);
    }
    let file = File::open(path)?;
    // Validate the opened handle as well; never stream devices or pipes as text.
    if !file.metadata()?.is_file() {
        anyhow::bail!("preview target changed");
    }
    let mut bytes = Vec::new();
    file.take((MAX_PREVIEW_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    let truncated = bytes.len() > MAX_PREVIEW_BYTES;
    bytes.truncate(MAX_PREVIEW_BYTES);
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => Some(text),
        Err(error) if truncated && error.error_len().is_none() => {
            std::str::from_utf8(&bytes[..error.valid_up_to()]).ok()
        }
        Err(_) => None,
    };
    if let Some(text) = text.filter(|text| {
        !text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    }) {
        preview.text = Some(text.into());
        if truncated {
            preview.notice = Some("Showing the first 64 KiB".into());
        }
    } else {
        preview.notice = Some("Text preview is unavailable for this file type".into());
    }
    Ok(preview)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::{ContextValue, ResolvedContext};
    use anywhere_protocol::{ContextSource, FILE};
    #[test]
    fn preview_is_bounded_and_does_not_decode_binary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap().join("preview.txt");
        let context = ResolvedContext::new(
            FILE,
            ContextValue::File(path.clone()),
            ContextSource::UserInput,
            "test",
        );
        std::fs::write(&path, "界".repeat(MAX_PREVIEW_BYTES)).unwrap();
        let p = read(&context).unwrap();
        assert!(p.text.unwrap().len() <= MAX_PREVIEW_BYTES);
        assert_eq!(p.notice.as_deref(), Some("Showing the first 64 KiB"));
        std::fs::write(&path, [0, 1, 2, 255]).unwrap();
        assert!(read(&context).unwrap().text.is_none());
        std::fs::remove_file(&path).unwrap();
        assert!(read(&context).is_err());
    }
}
