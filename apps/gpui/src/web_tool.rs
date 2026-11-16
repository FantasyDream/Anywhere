//! Host-owned document policy and JS transport for the GPUI Web tool surface.
//!
//! This module validates presentation requests. Rust Core remains authoritative
//! for package approval, document lifetime, resource digests and broker grants.

use anyhow::{Context, Result};
use std::{borrow::Cow, path::Path};
use url::Url;
use wry::http::{
    header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS},
    Response,
};

const BOOTSTRAP: &str = include_str!("../../../resources/web-bootstrap.js");
const WEB_TOKENS_CSS: &str = include_str!("../../../packages/plugin-sdk/styles/tokens.css");
const WEB_CONTROLS_CSS: &str = include_str!("../../../packages/plugin-sdk/styles/controls.css");
const HOST_ESCAPE_FRAME: &str = r#"{"method":"host.escape"}"#;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebDocumentPolicy {
    origin: String,
    entry: String,
}

impl WebDocumentPolicy {
    pub fn new(origin: impl Into<String>, entry: impl Into<String>) -> Result<Self> {
        let origin = origin.into();
        let entry = entry.into();
        let url = Url::parse(&origin).context("invalid Web document origin")?;
        let host = url.host_str().unwrap_or_default();
        anyhow::ensure!(
            url.scheme() == "anywhere-plugin"
                && url.username().is_empty()
                && url.password().is_none()
                && url.port().is_none()
                && url.path().is_empty()
                && url.query().is_none()
                && url.fragment().is_none()
                && host.len() == 77
                && host.ends_with(".plugin.local")
                && host[..64].bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid Web document origin"
        );
        validate_entry(&entry)?;
        Ok(Self { origin, entry })
    }

    pub fn url(&self) -> String {
        format!("{}/{}", self.origin, self.entry)
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn entry(&self) -> &str {
        &self.entry
    }

    pub fn accepts_navigation(&self, candidate: &str) -> bool {
        candidate == self.url()
    }

    pub fn accepts_source(&self, candidate: &str) -> bool {
        let Ok(url) = Url::parse(candidate) else {
            return false;
        };
        let Ok(origin) = Url::parse(&self.origin) else {
            return false;
        };
        url.scheme() == origin.scheme()
            && url.host_str() == origin.host_str()
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
    }

    pub fn resource_path(&self, value: &str) -> Option<String> {
        let lowered = value.to_ascii_lowercase();
        if ["%2e", "%2f", "%5c"]
            .iter()
            .any(|escaped| lowered.contains(escaped))
        {
            return None;
        }
        let path = value.strip_prefix(&format!("{}/", self.origin))?;
        if path.contains(['?', '#', '\\', '\0']) {
            return None;
        }
        if !valid_relative_path(path) {
            return None;
        }
        self.content_type(path).map(|_| path.to_owned())
    }

    pub fn content_type(&self, path: &str) -> Option<&'static str> {
        if path.ends_with(".html") {
            return (path == self.entry).then_some("text/html; charset=utf-8");
        }
        let extension = Path::new(path)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        match extension.as_str() {
            "js" | "mjs" => Some("text/javascript; charset=utf-8"),
            "css" => Some("text/css; charset=utf-8"),
            "json" => Some("application/json; charset=utf-8"),
            "png" => Some("image/png"),
            "jpg" | "jpeg" => Some("image/jpeg"),
            "webp" => Some("image/webp"),
            "gif" => Some("image/gif"),
            "ico" => Some("image/vnd.microsoft.icon"),
            "svg" => Some("image/svg+xml"),
            "woff2" => Some("font/woff2"),
            "woff" => Some("font/woff"),
            "ttf" => Some("font/ttf"),
            "otf" => Some("font/otf"),
            "txt" => Some("text/plain; charset=utf-8"),
            _ => None,
        }
    }

    pub fn protect_html(&self, bytes: Vec<u8>) -> Vec<u8> {
        let body = String::from_utf8_lossy(&bytes);
        format!(
            "<!doctype html><html><head><meta charset=\"utf-8\">\
             <meta http-equiv=\"Content-Security-Policy\" content=\"{}\">\
             <style data-anywhere-theme>{}</style></head>{body}</html>",
            content_security_policy(&self.origin),
            web_theme_css()
        )
        .into_bytes()
    }

    pub fn response(&self, path: &str, bytes: Vec<u8>) -> Response<Cow<'static, [u8]>> {
        let Some(content_type) = self.content_type(path) else {
            return plain_response(404, "Not found");
        };
        let body = if content_type.starts_with("text/html") {
            self.protect_html(bytes)
        } else {
            bytes
        };
        let csp = content_security_policy(&self.origin);
        Response::builder()
            .status(200)
            .header(CONTENT_TYPE, content_type)
            .header(CONTENT_SECURITY_POLICY, csp)
            .header(X_CONTENT_TYPE_OPTIONS, "nosniff")
            .header(CACHE_CONTROL, "no-store")
            .header("Access-Control-Allow-Origin", self.origin.as_str())
            .body(Cow::Owned(body))
            .unwrap_or_else(|_| plain_response(500, "Host response failed"))
    }
}

pub fn bootstrap_script(language: &str) -> String {
    format!("{}\n{BOOTSTRAP}", locale_script(language))
}

pub(crate) fn is_host_escape_frame(frame: &str) -> bool {
    frame == HOST_ESCAPE_FRAME
}

pub fn locale_script(language: &str) -> String {
    let language = if language == "zh" { "zh" } else { "en" };
    format!(
        "(() => {{ const next = {language:?}; const changed = \
         window.__anywhereLanguage !== next; window.__anywhereLanguage = next; \
         if (document.documentElement) document.documentElement.lang = next; \
         if (changed) window.dispatchEvent(new CustomEvent(\
         'anywhere:language', {{detail:next}})); }})();"
    )
}

pub fn reply_script(frame: &str) -> String {
    let bytes = frame.as_bytes();
    let values = serde_json::to_string(bytes).expect("byte array serialization");
    format!("window.anywhereNative.onmessage({{data:new Uint8Array({values}).buffer}});")
}

pub fn state_script(frame: &str) -> String {
    let frame = serde_json::to_string(frame).expect("JSON string serialization");
    format!("window.dispatchEvent(new CustomEvent('anywhere:state',{{detail:{frame}}}));")
}

pub fn visibility_script(visible: bool) -> String {
    format!("window.dispatchEvent(new CustomEvent('anywhere:visibility',{{detail:{visible}}}));")
}

pub fn canvas_script(transparent: bool) -> String {
    let value = if transparent {
        "transparent"
    } else {
        "var(--aw-fallback-surface)"
    };
    format!("document.documentElement.style.setProperty('--aw-web-canvas','{value}','important');")
}

pub fn plain_response(status: u16, message: &str) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(CACHE_CONTROL, "no-store")
        .body(Cow::Owned(message.as_bytes().to_vec()))
        .expect("static response")
}

fn validate_entry(entry: &str) -> Result<()> {
    anyhow::ensure!(valid_relative_path(entry), "invalid Web document entry");
    Ok(())
}

fn valid_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|segment| !matches!(segment, "" | "." | ".."))
}

fn web_theme_css() -> String {
    format!(
        "{WEB_TOKENS_CSS}\n{WEB_CONTROLS_CSS}\n\
         html:root, html:root > body, #app, #root, #__next {{\
         background: var(--aw-web-canvas, var(--aw-fallback-surface)) !important;\
         color: var(--aw-text) !important;\
         }}\nhtml:root {{ color-scheme: normal !important; }}"
    )
}

fn content_security_policy(origin: &str) -> String {
    format!(
        "default-src 'none'; script-src {origin}; \
         style-src {origin} 'unsafe-inline'; img-src {origin} data:; \
         font-src {origin}; media-src 'none'; connect-src 'none'; \
         frame-src 'none'; child-src 'none'; worker-src 'none'; \
         object-src 'none'; base-uri 'none'; form-action 'none'; \
         script-src-attr 'none'; upgrade-insecure-requests"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: &str =
        "anywhere-plugin://0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.plugin.local";

    #[test]
    fn only_the_bound_origin_entry_and_runtime_assets_are_addressable() {
        let policy = WebDocumentPolicy::new(ORIGIN, "assets/index.html").unwrap();
        assert_eq!(
            policy
                .resource_path(&format!("{ORIGIN}/assets/index.html"))
                .as_deref(),
            Some("assets/index.html")
        );
        assert_eq!(
            policy
                .resource_path(&format!("{ORIGIN}/assets/app.js"))
                .as_deref(),
            Some("assets/app.js")
        );
        for rejected in [
            format!("{ORIGIN}/assets/%2e%2e/app.js"),
            format!("{ORIGIN}/assets%2fapp.js"),
            format!("{ORIGIN}/../app.js"),
            format!("{ORIGIN}/assets/app.exe"),
            format!("{ORIGIN}/assets/app.js?x=1"),
            format!("{ORIGIN}/assets/app.js#fragment"),
            "anywhere-plugin://other.plugin.local/assets/app.js".to_owned(),
            "https://example.com/assets/app.js".to_owned(),
        ] {
            assert_eq!(policy.resource_path(&rejected), None, "{rejected}");
        }
        assert!(policy.accepts_navigation(&format!("{ORIGIN}/assets/index.html")));
        assert!(!policy.accepts_navigation("https://example.com/"));
    }

    #[test]
    fn html_policy_is_prepended_and_markup_is_preserved() {
        let policy = WebDocumentPolicy::new(ORIGIN, "index.html").unwrap();
        let protected = String::from_utf8(
            policy.protect_html(b"<body><script src=\"/app.js\"></script></body>".to_vec()),
        )
        .unwrap();
        assert!(protected.starts_with("<!doctype html><html><head>"));
        assert!(protected.contains("Content-Security-Policy"));
        assert!(protected.contains("frame-src 'none'"));
        assert!(protected.contains("<body><script src=\"/app.js\"></script></body>"));
    }

    #[test]
    fn reply_and_state_scripts_use_binary_transport_and_structured_details() {
        assert_eq!(
            reply_script("{\"x\":\"中\"}"),
            "window.anywhereNative.onmessage({data:new Uint8Array([123,34,120,34,58,34,228,184,173,34,125]).buffer});"
        );
        let script = state_script("{\"event\":\"state.changed\"}");
        assert!(script.contains("\"{\\\"event\\\":\\\"state.changed\\\"}\""));
        assert!(script.contains("anywhere:state"));
    }

    #[test]
    fn only_the_exact_host_escape_frame_bypasses_the_broker() {
        assert!(is_host_escape_frame(r#"{"method":"host.escape"}"#));
        for rejected in [
            r#"{ "method": "host.escape" }"#,
            r#"{"method":"host.escape","requestId":"1"}"#,
            r#"{"method":"host.close"}"#,
        ] {
            assert!(!is_host_escape_frame(rejected), "{rejected}");
        }
        let bootstrap = bootstrap_script("en");
        assert!(bootstrap.contains("event.key !== 'Escape'"));
        assert!(bootstrap.contains("event.isComposing"));
        assert!(bootstrap.contains(r#"{"method":"host.escape"}"#));
    }
}
