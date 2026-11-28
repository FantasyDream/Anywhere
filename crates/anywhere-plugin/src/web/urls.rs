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
    ensure!(url.host_str().is_some(), "URL has no host");
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
