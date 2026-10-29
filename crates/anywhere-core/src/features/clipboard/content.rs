//! Pure derivation of search metadata and bounded, non-executing previews.
use super::history::*;
use anywhere_platform::clipboard::ClipboardPayload;
use std::borrow::Cow;

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
    // Plain text is already UTF-8 inside the payload. Borrow it while deriving
    // metadata instead of retaining another full body beside the search buffer.
    let searchable = if !files.is_empty() {
        Cow::Owned(files.join("\n"))
    } else {
        payload.plain_text().map(Cow::Borrowed).unwrap_or_else(|| {
            Cow::Owned(
                payload
                    .formats
                    .iter()
                    .filter(|f| rich_format(&f.name))
                    .filter_map(|f| std::str::from_utf8(&f.data).ok())
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
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
    let mut search = format!(
        "{searchable}\n{}\n{kind}",
        payload
            .formats
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    );
    // Most paths / JSON / source text are ASCII. Normalize the owned index in
    // place; Unicode keeps str's context-sensitive rules (for example final Σ).
    if search.is_ascii() {
        search.make_ascii_lowercase();
    } else {
        search = search.to_lowercase();
    }
    (kind.into(), summary, search)
}
#[cfg(test)]
mod tests {
    use super::*;
    use anywhere_platform::clipboard::ClipboardFormat;
    #[test]
    fn borrowed_plain_text_preserves_unicode_search_and_content_past_summary() {
        let mut payload = ClipboardPayload::text("ΟΣ İHELLO\n中文 TailNeedle");
        payload.formats.push(ClipboardFormat {
            name: "Application/X-Example".into(),
            data: vec![],
        });
        let (kind, summary, search) = metadata(&payload);
        assert_eq!(kind, "text");
        assert_eq!(summary, "ΟΣ İHELLO 中文 TailNeedle");
        assert_eq!(search, "ος i\u{307}hello\n中文 tailneedle\ntext/plain;charset=utf-8 application/x-example\ntext");
        let payload = ClipboardPayload::text(&format!("{}TailNeedle", "A".repeat(8192)));
        let (_, summary, search) = metadata(&payload);
        assert_eq!(summary.chars().count(), SUMMARY_CHARACTERS);
        assert!(search.contains("tailneedle\ntext/plain;charset=utf-8\ntext"));
    }
}
