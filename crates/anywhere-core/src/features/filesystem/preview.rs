//! Bounded, encoding-aware plain-text previews. No shell, directory traversal, or external viewers.
use crate::core::context::ResolvedContext;
use anyhow::Result;
use std::{fs::File, io::Read, time::UNIX_EPOCH};

mod text;

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
    // Legacy guessing is limited to names intended for text / source. Unknown
    // binary files keep the old UTF-8-or-no-preview behavior. BOMs are explicit.
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let allow_legacy =
        super::file_types::accepts(anywhere_protocol::FileSearchFilter::Code, name, false)
            || matches!(
                extension.as_str(),
                "txt" | "md" | "csv" | "tsv" | "log" | "rtf" | "svg"
            );
    if let Some(decoded) = text::decode(&bytes, truncated, allow_legacy) {
        let mut notices = Vec::new();
        if decoded.truncated {
            notices.push("Showing the first 64 KiB".to_owned());
        }
        if decoded.guessed {
            notices.push(format!("Encoding guessed as {}", decoded.encoding));
        } else if decoded.encoding != "UTF-8" {
            notices.push(format!("Decoded as {}", decoded.encoding));
        }
        preview.notice = (!notices.is_empty()).then(|| notices.join(" · "));
        preview.text = Some(decoded.text);
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
    #[test]
    fn file_preview_decodes_boms_and_reports_guessed_legacy_encoding() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap().join("settings.xml");
        let context = ResolvedContext::new(
            FILE,
            ContextValue::File(path.clone()),
            ContextSource::UserInput,
            "test",
        );
        let source = "项目设置文件，包含中文路径以及服务器连接配置。\n".repeat(12);
        let (bytes, _, errors) = encoding_rs::GBK.encode(&source);
        assert!(!errors);
        std::fs::write(&path, &bytes).unwrap();
        let result = read(&context).unwrap();
        assert_eq!(result.text.as_deref(), Some(source.as_str()));
        assert_eq!(result.notice.as_deref(), Some("Encoding guessed as GBK"));
        let other = dir.path().canonicalize().unwrap().join("unknown.bin");
        std::fs::write(&other, &bytes).unwrap();
        let unknown = ResolvedContext::new(
            FILE,
            ContextValue::File(other),
            ContextSource::UserInput,
            "test",
        );
        assert!(read(&unknown).unwrap().text.is_none());
        let mut utf16 = vec![0xff, 0xfe];
        for unit in source.encode_utf16() {
            utf16.extend(unit.to_le_bytes());
        }
        std::fs::write(&path, utf16).unwrap();
        let result = read(&context).unwrap();
        assert_eq!(result.text.as_deref(), Some(source.as_str()));
        assert_eq!(result.notice.as_deref(), Some("Decoded as UTF-16LE"));
        std::fs::write(&path, b"\xef\xbb\xbfUTF-8 BOM").unwrap();
        assert_eq!(read(&context).unwrap().text.as_deref(), Some("UTF-8 BOM"));
    }
}
