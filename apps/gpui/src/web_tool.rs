//! Host-owned document policy and JS transport for the GPUI Web tool surface.
//!
//! This module validates presentation requests. Rust Core remains authoritative
//! for package approval, document lifetime, resource digests and broker grants.

use anyhow::{Context, Result};
use std::{borrow::Cow, path::Path};
use url::Url;

use crate::ui::{
    design_tokens::{colors::FixedColors, Palette},
    theme,
};
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

    pub fn runtime_origin(&self) -> String {
        crate::platform::web_document_origin(&self.origin)
    }

    pub fn entry(&self) -> &str {
        &self.entry
    }

    pub fn accepts_navigation(&self, candidate: &str) -> bool {
        candidate == self.url() || candidate == format!("{}/{}", self.runtime_origin(), self.entry)
    }

    pub fn accepts_source(&self, candidate: &str) -> bool {
        let Ok(url) = Url::parse(candidate) else {
            return false;
        };
        if !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
            return false;
        }
        let runtime_origin = self.runtime_origin();
        let matches = [self.origin.as_str(), runtime_origin.as_str()]
            .into_iter()
            .filter_map(|candidate_origin| Url::parse(candidate_origin).ok())
            .any(|origin| url.scheme() == origin.scheme() && url.host_str() == origin.host_str());
        matches
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

    pub fn protect_html(&self, bytes: &[u8]) -> Vec<u8> {
        let body = String::from_utf8_lossy(bytes);
        let active_origin = self.runtime_origin();
        let injection = format!(
            "<meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"{}\">\
             <style data-anywhere-theme>{}</style>",
            content_security_policy(&active_origin),
            web_theme_css()
        );
        inject_document_head(&body, &injection).into_bytes()
    }

    pub fn response(&self, path: &str, bytes: Vec<u8>) -> Response<Cow<'static, [u8]>> {
        let Some(content_type) = self.content_type(path) else {
            return plain_response(404, "Not found");
        };
        let body = if content_type.starts_with("text/html") {
            self.protect_html(&bytes)
        } else {
            bytes
        };
        let active_origin = self.runtime_origin();
        let csp = content_security_policy(&active_origin);
        Response::builder()
            .status(200)
            .header(CONTENT_TYPE, content_type)
            .header(CONTENT_SECURITY_POLICY, csp)
            .header(X_CONTENT_TYPE_OPTIONS, "nosniff")
            .header(CACHE_CONTROL, "no-store")
            .header("Access-Control-Allow-Origin", active_origin.as_str())
            .body(Cow::Owned(body))
            .unwrap_or_else(|_| plain_response(500, "Host response failed"))
    }
}

fn inject_document_head(body: &str, injection: &str) -> String {
    let body = body.strip_prefix('\u{feff}').unwrap_or(body);
    let mut offset = 0;
    let mut html_end = None;
    let mut doctype_end = None;
    while offset < body.len() {
        let rest = &body[offset..];
        let trimmed = rest.trim_start();
        offset += rest.len() - trimmed.len();
        if trimmed.starts_with("<!--") {
            let Some(end) = trimmed.find("-->") else {
                break;
            };
            offset += end + 3;
            continue;
        }
        if !trimmed.starts_with('<') {
            break;
        }
        let mut quote = None;
        let end = trimmed.char_indices().skip(1).find_map(|(index, ch)| {
            if let Some(current) = quote {
                if current == ch {
                    quote = None;
                }
            } else if ch == '\'' || ch == '"' {
                quote = Some(ch);
            } else if ch == '>' {
                return Some(index);
            }
            None
        });
        let Some(end) = end else { break };
        let tag = &trimmed[..end + 1];
        let name = tag[1..]
            .split(|c: char| c.is_ascii_whitespace() || c == '>')
            .next()
            .unwrap_or_default();
        if name.eq_ignore_ascii_case("head") {
            let index = offset + end + 1;
            return format!("{}{}{}", &body[..index], injection, &body[index..]);
        }
        if name.eq_ignore_ascii_case("html") {
            html_end = Some(offset + end + 1);
        } else if name.eq_ignore_ascii_case("!doctype") {
            doctype_end = Some(offset + end + 1);
        } else {
            break;
        }
        offset += end + 1;
    }
    if let Some(index) = html_end {
        format!(
            "{}<head>{injection}</head>{}",
            &body[..index],
            &body[index..]
        )
    } else if let Some(index) = doctype_end {
        format!(
            "{}<html><head>{injection}</head>{}</html>",
            &body[..index],
            &body[index..]
        )
    } else {
        format!("<!doctype html><html><head>{injection}</head>{body}</html>")
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
    let frame = serde_json::to_string(frame).expect("JSON string serialization");
    format!("window.anywhereNative.onmessage({{data:{frame}}});")
}

pub fn state_script(frame: &str) -> String {
    let frame = serde_json::to_string(frame).expect("JSON string serialization");
    format!("window.dispatchEvent(new CustomEvent('anywhere:state',{{detail:{frame}}}));")
}

pub fn visibility_script(visible: bool) -> String {
    format!("window.dispatchEvent(new CustomEvent('anywhere:visibility',{{detail:{visible}}}));")
}

fn canvas(transparent: bool) -> &'static str {
    if transparent {
        "transparent"
    } else {
        "var(--aw-web-fallback-surface, var(--aw-fallback-surface))"
    }
}

pub fn canvas_script(transparent: bool) -> String {
    let value = canvas(transparent);
    format!("document.documentElement.style.setProperty('--aw-web-canvas','{value}','important');")
}

fn css_color(argb: u32) -> String {
    format!("#{:06x}{:02x}", argb & 0x00ff_ffff, argb >> 24)
}

fn web_appearance(
    palette: &Palette,
    fixed: &FixedColors,
    light_surface: bool,
) -> impl Iterator<Item = (&'static str, String)> {
    let scheme = if light_surface { "light" } else { "dark" };
    let fallback = if light_surface {
        "var(--aw-light-fallback-surface)"
    } else {
        "var(--aw-fallback-surface)"
    };
    palette
        .web_variables()
        .into_iter()
        .chain(fixed.web_contrast_variables())
        .map(|(name, argb)| (name, css_color(argb)))
        .chain([
            ("--aw-web-fallback-surface", fallback.to_owned()),
            (
                "--aw-select-caret",
                format!("var(--aw-select-caret-on-{scheme})"),
            ),
            ("color-scheme", scheme.to_owned()),
        ])
}

pub fn web_theme_script(palette: &Palette, fixed: &FixedColors, light_surface: bool) -> String {
    let assignments: String = web_appearance(palette, fixed, light_surface)
        .map(|(name, value)| format!("root.style.setProperty('{name}','{value}','important');"))
        .collect();
    format!("(() => {{ const root=document.documentElement; {assignments} }})();")
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
    let appearance: String =
        web_appearance(theme::palette(), theme::fixed(), theme::surface_is_light())
            .map(|(name, value)| format!("{name}:{value};"))
            .collect();
    let canvas = canvas(!crate::platform::uses_opaque_fallback());
    format!(
        "{WEB_TOKENS_CSS}\n{WEB_CONTROLS_CSS}\n\
         html:root {{ {appearance} --aw-web-canvas:{canvas}; }}\n\
         html:root, html:root > body, #app, #root, #__next {{\
         background: var(--aw-web-canvas, var(--aw-fallback-surface)) !important;\
         color: var(--aw-text) !important;\
         }}"
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
    fn head_injection_preserves_quoted_tags_and_doctype_fragments() {
        let text = inject_document_head("<!doctype html><body>中文 > result</body>", "INJECT");
        assert_eq!(text.matches("<!doctype html>").count(), 1);
        assert!(text.starts_with("<!doctype html><html><head>INJECT</head>"));
        assert_eq!(
            inject_document_head(
                "<html><head data-value='a>b'><title>x</title></head></html>",
                "INJECT"
            ),
            "<html><head data-value='a>b'>INJECT<title>x</title></head></html>"
        );
        assert!(inject_document_head("中文 > result", "INJECT").ends_with("中文 > result</html>"));
    }

    #[test]
    fn complete_documents_have_one_head_with_policy_before_plugin_resources() {
        let policy = WebDocumentPolicy::new(ORIGIN, "index.html").unwrap();
        for markup in ["<!doctype html><html><head><script src='/app.js'></script></head><body>ok</body></html>",
                       "<!-- <head> --><HTML lang='en'><HEAD><title>ok</title></HEAD></HTML>",
                       "<!doctype html><html><body>ok</body></html>"] {
            let protected = String::from_utf8(policy.protect_html(markup.as_bytes())).unwrap();
            let lower = protected.to_ascii_lowercase();
            assert_eq!(lower.matches("<html").count(), 1);
            assert_eq!(lower.matches("<head>").count(), if markup.starts_with("<!--") { 2 } else { 1 });
            assert!(protected.find("Content-Security-Policy").unwrap() < protected.find("</head>").or_else(|| protected.find("</HEAD>")).unwrap());
        }
    }

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
            policy.protect_html(b"<body><script src=\"/app.js\"></script></body>"),
        )
        .unwrap();
        assert!(protected.starts_with("<!doctype html><html><head>"));
        assert!(protected.contains("Content-Security-Policy"));
        assert!(protected.contains("frame-src 'none'"));
        assert!(protected.contains("--aw-on-dark-primary"));
        assert!(protected.contains("dialog button"));
        assert!(protected.contains("<body><script src=\"/app.js\"></script></body>"));
    }

    #[test]
    fn reply_and_state_scripts_use_string_transport_and_structured_details() {
        assert_eq!(
            reply_script("{\"x\":\"中\"}"),
            r#"window.anywhereNative.onmessage({data:"{\"x\":\"中\"}"});"#
        );
        let script = state_script("{\"event\":\"state.changed\"}");
        assert!(script.contains("\"{\\\"event\\\":\\\"state.changed\\\"}\""));
        assert!(script.contains("anywhere:state"));
    }

    #[test]
    fn web_semantic_colors_and_color_scheme_follow_the_material_surface() {
        use crate::platform::WindowAppearance;
        use crate::ui::theme;

        let cases = [
            (
                WindowAppearance::Light,
                false,
                false,
                true,
                "#14181fff",
                "#e8ebf0d9",
            ),
            (
                WindowAppearance::Light,
                false,
                true,
                true,
                "#14181fff",
                "#e8ebf0d9",
            ),
            (
                WindowAppearance::Dark,
                false,
                false,
                false,
                "#f9fafcff",
                "#20232a80",
            ),
            (
                WindowAppearance::Dark,
                false,
                true,
                false,
                "#f9fafcff",
                "#20232a80",
            ),
            (
                WindowAppearance::Dark,
                true,
                false,
                false,
                "#f9fafcff",
                "#20232a80",
            ),
            (
                WindowAppearance::Dark,
                true,
                true,
                false,
                "#f9fafcff",
                "#20232a80",
            ),
            (
                WindowAppearance::Light,
                true,
                true,
                true,
                "#14181fff",
                "#e8ebf0d9",
            ),
        ];
        for (appearance, opaque, contrast, light_surface, primary, field) in cases {
            let palette = theme::palette_for(appearance, opaque, contrast);
            let script = web_theme_script(palette, theme::fixed_for(contrast), light_surface);
            for (name, _) in palette.web_variables() {
                assert!(script.contains(name), "missing {name}");
            }
            assert!(script.contains(primary));
            assert!(script.contains(field));
            for (name, normal_value, contrast_value) in [
                ("--aw-on-dark-control-border", "#ffffff30", "#ffffff52"),
                ("--aw-on-dark-selection-outline", "#ffffff30", "#ffffff68"),
                ("--aw-on-dark-divider", "#ffffff1f", "#ffffff42"),
            ] {
                let value = if contrast {
                    contrast_value
                } else {
                    normal_value
                };
                assert!(script.contains(&format!("setProperty('{name}','{value}','important')")));
            }
            assert!(script.contains(if light_surface {
                "'color-scheme','light'"
            } else {
                "'color-scheme','dark'"
            }));
            assert!(script.contains(if light_surface {
                "'--aw-web-fallback-surface','var(--aw-light-fallback-surface)'"
            } else {
                "'--aw-web-fallback-surface','var(--aw-fallback-surface)'"
            }));
        }
        let light = web_theme_script(
            theme::palette_for(WindowAppearance::Light, false, false),
            theme::fixed_for(false),
            true,
        );
        let dark = web_theme_script(
            theme::palette_for(WindowAppearance::Dark, false, false),
            theme::fixed_for(false),
            false,
        );
        let opaque = web_theme_script(
            theme::palette_for(WindowAppearance::Dark, true, false),
            theme::fixed_for(false),
            false,
        );
        let contrast = web_theme_script(
            theme::palette_for(WindowAppearance::Dark, false, true),
            theme::fixed_for(true),
            false,
        );
        assert_ne!(light, dark);
        assert_ne!(dark, opaque);
        assert_ne!(dark, contrast);
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
