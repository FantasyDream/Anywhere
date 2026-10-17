//! Pure derivation of search metadata and bounded, non-executing previews.
use super::history::*;
use anywhere_platform::clipboard::{ClipboardPayload, FILES};
use std::io::Cursor;

pub fn image_format(name: &str) -> bool {
    matches!(
        name,
        "public.png"
            | "public.tiff"
            | "public.jpeg"
            | "public.jpeg-2000"
            | "com.compuserve.gif"
            | "org.webmproject.webp"
            | "PNG"
            | "JFIF"
    ) || name.starts_with("image/")
}
pub fn rich_format(name: &str) -> bool {
    matches!(
        name,
        "public.html"
            | "public.rtf"
            | "text/html"
            | "text/rtf"
            | "HTML Format"
            | "Rich Text Format"
    )
}
pub fn metadata(payload: &ClipboardPayload) -> (String, String, String) {
    let files = payload.files().unwrap_or_default();
    let kind = if !files.is_empty() {
        "files"
    } else if payload.formats.iter().any(|f| image_format(&f.name)) {
        "image"
    } else if payload.formats.iter().any(|f| rich_format(&f.name)) {
        "rich_text"
    } else if payload.plain_text().is_some() {
        "text"
    } else {
        "binary"
    };
    let searchable = if !files.is_empty() {
        files.join("\n")
    } else {
        payload.plain_text().map(str::to_owned).unwrap_or_else(|| {
            payload
                .formats
                .iter()
                .filter(|f| rich_format(&f.name))
                .filter_map(|f| std::str::from_utf8(&f.data).ok())
                .collect::<Vec<_>>()
                .join("\n")
        })
    };
    let title = if !files.is_empty() {
        let name = std::path::Path::new(&files[0])
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&files[0]);
        if files.len() == 1 {
            name.to_owned()
        } else {
            format!("{name} + {} files", files.len() - 1)
        }
    } else if !searchable.trim().is_empty() {
        searchable.chars().take(SUMMARY_CHARACTERS).collect()
    } else if kind == "image" {
        "Image".into()
    } else {
        format!("{} data", payload.formats[0].name)
    };
    let summary = title
        .chars()
        .take(SUMMARY_CHARACTERS)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let search = format!(
        "{searchable}\n{}\n{kind}",
        payload
            .formats
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    )
    .to_lowercase();
    (kind.into(), summary, search)
}
pub fn preview(item: HistoryItem, payload: ClipboardPayload) -> HistoryDetail {
    // Bound before copying: a 16 MiB body needs only a preview-sized temporary.
    let rich = payload.formats.iter().find(|f| rich_format(&f.name));
    let source = payload
        .plain_text()
        .map(std::borrow::Cow::Borrowed)
        .unwrap_or_else(|| {
            rich.map(|f| String::from_utf8_lossy(&f.data[..f.data.len().min(MAX_PREVIEW_BYTES)]))
                .unwrap_or_default()
        });
    let mut end = source.len().min(MAX_PREVIEW_BYTES);
    while !source.is_char_boundary(end) {
        end -= 1;
    }
    let mut text = source[..end].to_owned();
    if source.len() > end
        || (payload.plain_text().is_none()
            && rich.is_some_and(|f| f.data.len() > MAX_PREVIEW_BYTES))
    {
        text.push_str("\n… Preview truncated");
    }
    let image_png = payload
        .formats
        .iter()
        .filter(|f| image_format(&f.name))
        .find_map(|f| thumbnail(&f.data))
        .unwrap_or_default();
    let files = payload.files().unwrap_or_default();
    let formats = payload
        .formats
        .iter()
        .map(|f| {
            if f.name == FILES {
                "File list".into()
            } else {
                f.name.clone()
            }
        })
        .collect();
    HistoryDetail {
        item,
        text,
        image_png,
        files,
        formats,
    }
}
fn thumbnail(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?.thumbnail(512, 512);
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, image::ImageFormat::Png).ok()?;
    Some(output.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anywhere_platform::clipboard::ClipboardFormat;
    fn item() -> HistoryItem {
        HistoryItem {
            id: 1,
            summary: "test".into(),
            kind: "text".into(),
            copied_at_ms: 0,
            byte_count: 0,
        }
    }
    #[test]
    fn previews_bound_unicode_text_lossy_rich_text_images_and_file_references() {
        let text = "中".repeat(MAX_PREVIEW_BYTES);
        let detail = preview(item(), ClipboardPayload::text(&text));
        assert!(detail.text.len() <= MAX_PREVIEW_BYTES + 64);
        assert!(detail.text.ends_with("Preview truncated"));
        assert!(!detail.text.contains('�'));
        let detail = preview(
            item(),
            ClipboardPayload {
                formats: vec![ClipboardFormat {
                    name: "public.html".into(),
                    data: vec![255; MAX_PREVIEW_BYTES * 2],
                }],
            },
        );
        assert!(detail.text.len() <= MAX_PREVIEW_BYTES + 64);
        assert!(detail.text.ends_with("Preview truncated"));
        let mut image = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(1024, 768)
            .write_to(&mut image, image::ImageFormat::Png)
            .unwrap();
        let detail = preview(
            item(),
            ClipboardPayload {
                formats: vec![ClipboardFormat {
                    name: "public.png".into(),
                    data: image.into_inner(),
                }],
            },
        );
        let thumbnail = image::load_from_memory(&detail.image_png).unwrap();
        assert!(thumbnail.width() <= 512 && thumbnail.height() <= 512);
        let paths = vec!["/missing/中文.png", "/missing/report.pdf"];
        let detail = preview(
            item(),
            ClipboardPayload {
                formats: vec![ClipboardFormat {
                    name: FILES.into(),
                    data: serde_json::to_vec(&paths).unwrap(),
                }],
            },
        );
        assert_eq!(detail.files, paths);
        assert!(detail.text.is_empty() && detail.image_png.is_empty());
    }
}
