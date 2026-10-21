//! Bounded input recognition owned by Context matching, not package loading.
//! No plugin code or business transformation runs during capture/search.
use anywhere_protocol::web_plugin::WebInputMatcher;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Only recognition is cached, for the duration of one input evaluation. The source
/// is borrowed; neither a parsed document nor a persistent copy of user data survives.
pub struct InputPatternMatches<'a> {
    text: &'a str,
    results: BTreeMap<WebInputMatcher, bool>,
}

impl<'a> InputPatternMatches<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            results: BTreeMap::new(),
        }
    }

    pub fn any(&mut self, matchers: &[WebInputMatcher]) -> bool {
        matchers.iter().any(|matcher| {
            *self
                .results
                .entry(*matcher)
                .or_insert_with(|| match matcher {
                    WebInputMatcher::Json => json_container(self.text),
                    WebInputMatcher::JsonEscaped => escaped_json_container(self.text),
                })
        })
    }
}

fn escaped_json_container(text: &str) -> bool {
    if text.len() > 16 * 1024 || json_container(text) {
        return false;
    }
    let mut decoded = text.trim_matches([' ', '\t', '\r', '\n']).to_owned();
    for _ in 0..4 {
        let next = if decoded.starts_with('"') {
            serde_json::from_str::<String>(&decoded)
        } else {
            serde_json::from_str::<String>(&format!("\"{decoded}\""))
        };
        let Ok(next) = next else {
            return false;
        };
        if next == decoded || next.len() > 16 * 1024 {
            return false;
        }
        if json_container(&next) {
            return true;
        }
        decoded = next;
    }
    false
}

fn json_container(text: &str) -> bool {
    // Keep automatic recognition small even though an explicitly opened workspace
    // can process larger documents. This also bounds depth validation and CPU work.
    if text.len() > 16 * 1024 {
        return false;
    }
    let text = text.trim_matches([' ', '\t', '\r', '\n']);
    if !matches!(text.as_bytes().first(), Some(b'{' | b'[')) {
        return false;
    }
    // IgnoredAny bypasses serde_json's normal nesting guard. Check depth before
    // parsing so the format budget also applies when no document tree is built.
    let (mut depth, mut quoted, mut escaped) = (0u16, false, false);
    for byte in text.bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > 128 {
                        return false;
                    }
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    // Validate syntax without constructing Value, converting numbers or changing
    // duplicate keys, escapes, whitespace or the original invocation text.
    let mut parser = serde_json::Deserializer::from_str(text);
    serde::de::IgnoredAny::deserialize(&mut parser).is_ok() && parser.end().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaped_containers_match_without_treating_normal_json_or_strings_as_escaped() {
        let source = r#" {"n":900719925474099312345,"s":"line\n\"quote\"","items":[1,2]} "#;
        let mut encoded = source.to_owned();
        for _ in 0..4 {
            encoded = serde_json::to_string(&encoded).unwrap();
            assert!(escaped_json_container(&encoded));
            assert!(escaped_json_container(&encoded[1..encoded.len() - 1]));
            assert!(!json_container(&encoded));
        }
        encoded = serde_json::to_string(&encoded).unwrap();
        assert!(!escaped_json_container(&encoded));
        for text in [
            source,
            "[1,2]",
            "json",
            r#""hello""#,
            r#""123""#,
            r#""{broken}""#,
            r#"{\"a\":1,}"#,
            r#"{\"a\":\x41}"#,
        ] {
            assert!(!escaped_json_container(text), "{text}");
        }
    }

    #[test]
    fn escaped_recognition_obeys_encoded_byte_and_decoded_depth_limits() {
        let encoded = serde_json::to_string(&format!("[\"{}\"]", "x".repeat(16380))).unwrap();
        assert!(!escaped_json_container(&encoded));
        for (depth, expected) in [(128, true), (129, false)] {
            let raw = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
            assert_eq!(
                escaped_json_container(&serde_json::to_string(&raw).unwrap()),
                expected
            );
        }
        let mut cached = InputPatternMatches::new(r#"{\"a\":1}"#);
        assert!(cached.any(&[WebInputMatcher::Json, WebInputMatcher::JsonEscaped]));
        assert!(cached.any(&[WebInputMatcher::JsonEscaped]));
        assert_eq!(cached.results.len(), 2);
    }

    #[test]
    fn strict_containers_preserve_large_numbers_and_reject_search_terms() {
        for text in [
            r#" {"n":123456789012345678901,"n":1e9999} "#,
            "[true,null,\"中文\"]",
            "{}",
            "[]",
            r#"{"escaped":"\\\"["}"#,
        ] {
            assert!(
                InputPatternMatches::new(text).any(&[WebInputMatcher::Json]),
                "{text}"
            );
        }
        for text in [
            "",
            "123",
            "true",
            "null",
            "\"hello\"",
            "json",
            "{broken}",
            "[1,]",
            "{} extra",
            "{} {}",
            "{\"a\":NaN}",
            "/* comment */ {}",
        ] {
            assert!(
                !InputPatternMatches::new(text).any(&[WebInputMatcher::Json]),
                "{text}"
            );
        }
    }

    #[test]
    fn bounds_depth_and_bytes_and_reuses_results() {
        for (depth, expected) in [(128, true), (129, false), (8000, false)] {
            let input = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
            assert_eq!(json_container(&input), expected);
        }
        assert!(json_container(&format!("[\"{}\"]", "x".repeat(16380))));
        assert!(!json_container(&format!("[\"{}\"]", "x".repeat(16381))));
        let mut cached = InputPatternMatches::new("{}");
        for _ in 0..64 {
            assert!(cached.any(&[WebInputMatcher::Json]));
        }
        assert_eq!(cached.results.len(), 1);
        assert!(!cached.any(&[]));
    }
}
