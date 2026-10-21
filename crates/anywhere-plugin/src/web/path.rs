use anyhow::{ensure, Result};
use unicode_casefold::UnicodeCaseFold;
use unicode_normalization::UnicodeNormalization;

/// One portable spelling. Never repair or percent-decode an archive entry name.
pub(super) fn validate(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty() && path.len() <= 1024,
        "invalid asset path length"
    );
    ensure!(path.split('/').count() <= 16, "asset path is too deep");
    for part in path.split('/') {
        ensure!(
            !part.is_empty() && part != "." && part != ".." && part.len() <= 255,
            "invalid asset path component"
        );
        ensure!(
            !part.ends_with(['.', ' '])
                && !part
                    .chars()
                    .any(|c| c.is_control() || "\\:<>\"|?*%#".contains(c)),
            "non-portable asset path"
        );
        let stem = part.split('.').next().unwrap_or_default().to_uppercase();
        let reserved = matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
        ensure!(!reserved, "reserved asset path");
    }
    Ok(())
}

pub(super) fn key(path: &str) -> String {
    path.nfc().case_fold().nfc().collect()
}

pub(super) fn allowed_asset(path: &str) -> bool {
    if matches!(path, "LICENSE" | "NOTICE") {
        return true;
    }
    let extension = path.rsplit('.').next().unwrap_or_default();
    matches!(
        extension,
        "html"
            | "htm"
            | "js"
            | "mjs"
            | "css"
            | "json"
            | "txt"
            | "md"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "ico"
            | "svg"
            | "woff"
            | "woff2"
            | "ttf"
            | "otf"
            | "map"
    )
}
