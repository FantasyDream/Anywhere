//! Shared URL parsing for installation grants and the runtime broker.
use anyhow::{ensure, Result};
use std::collections::BTreeSet;
use url::Url;

/// Bound before parsing or displaying a destination.
pub const MAX_URL_BYTES: usize = 8 * 1024;

fn parse(value: &str) -> Result<Url> {
    ensure!(
        value.len() <= MAX_URL_BYTES
            && !value
                .chars()
                .any(|ch| ch.is_control() || ch.is_whitespace() || ch == '\\'),
        "invalid URL characters or length"
    );
    let (scheme, rest) = value.split_once("://").unwrap_or_default();
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    ensure!(
        (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
            && !authority.is_empty()
            && !authority.contains(['@', '*']),
        "expected an HTTP(S) URL without credentials"
    );
    let url = Url::parse(value)?;
    ensure!(
        url.as_str().len() <= MAX_URL_BYTES
            && matches!(url.scheme(), "http" | "https")
            && url.username().is_empty()
            && url.password().is_none()
            && url.host_str().is_some(),
        "invalid canonical HTTP(S) URL"
    );
    Ok(url)
}

fn origin(value: &str) -> Result<String> {
    let (_, rest) = value.split_once("://").unwrap_or_default();
    ensure!(
        !rest.contains(['/', '?', '#', '*']),
        "URL grant must declare an exact origin"
    );
    Ok(parse(value)?.origin().ascii_serialization())
}

pub(super) fn validate_origins(values: &[String]) -> Result<()> {
    if values == ["*"] {
        return Ok(());
    }
    ensure!(
        (1..=16).contains(&values.len()),
        "expected 1..16 URL origins"
    );
    let mut seen = BTreeSet::new();
    for value in values {
        ensure!(seen.insert(origin(value)?), "duplicate URL origin");
    }
    Ok(())
}

/// Return the canonical destination and whether a grant covers its origin.
///
/// # Errors
/// Returns an error for a malformed destination, including credentials.
pub fn granted_url(value: &str, origins: &[String]) -> Result<(Url, bool)> {
    let url = parse(value)?;
    let canonical = url.origin().ascii_serialization();
    let allowed = origins == ["*"]
        || origins
            .iter()
            .any(|entry| origin(entry).is_ok_and(|entry| entry == canonical));
    Ok((url, allowed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_exact_origins_and_star_exclusively() {
        assert!(validate_origins(&["*".into()]).is_ok());
        assert!(validate_origins(&["https://issues.example.com".into()]).is_ok());
        assert!(validate_origins(&["http://[::1]:8080".into()]).is_ok());
        for bad in [
            vec![],
            vec!["*".into(), "https://example.com".into()],
            vec!["https://example.com/".into()],
            vec!["https://example.com/path".into()],
            vec!["https://example.com?q=1".into()],
            vec!["https://example.com#frag".into()],
            vec!["https://user@example.com".into()],
            vec!["https://*.example.com".into()],
            vec!["ftp://example.com".into()],
            vec![
                "https://example.com".into(),
                "https://EXAMPLE.COM:443".into(),
            ],
            (0..17)
                .map(|i| format!("https://s{i}.example.com"))
                .collect(),
        ] {
            assert!(validate_origins(&bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn enforces_canonical_length_and_normalizes_origins() {
        let (url, allowed) = granted_url(
            "HTTPS://EXAMPLE.COM:443/issues/1?q=ok",
            &["https://example.com".into()],
        )
        .unwrap();
        assert!(allowed);
        assert_eq!(url.as_str(), "https://example.com/issues/1?q=ok");

        let (idn, allowed) = granted_url(
            "https://例子.测试/path",
            &["https://xn--fsqu00a.xn--0zwm56d".into()],
        )
        .unwrap();
        assert!(allowed);
        assert_eq!(
            idn.origin().ascii_serialization(),
            "https://xn--fsqu00a.xn--0zwm56d"
        );

        // 3000 Chinese chars fit raw (9000 -> no, 1500 chars = 4500 raw bytes) but expand 3x to 13500 bytes canonical.
        let unicode_url = format!("https://example.com/?q={}", "中".repeat(1500));
        assert!(unicode_url.len() <= MAX_URL_BYTES);
        assert!(granted_url(&unicode_url, &["*".into()]).is_err());

        for bad in [
            "javascript:alert(1)",
            "https://user:pass@example.com/",
            "https://example.com/a b",
            "https://example.com/a\\b",
            "https://example.com/\n",
        ] {
            assert!(granted_url(bad, &["*".into()]).is_err(), "accepted {bad}");
        }
    }
}
