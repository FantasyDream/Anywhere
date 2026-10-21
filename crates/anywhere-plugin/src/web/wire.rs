//! Bounded parsing only; no Host identity or permission decisions.
use anywhere_protocol::web_bridge::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<'a> {
    protocol: u32,
    #[serde(rename = "requestId")]
    request_id: String,
    method: String,
    #[serde(borrow)]
    params: &'a serde_json::value::RawValue,
}
pub struct Request {
    pub id: String,
    pub call: WebCall,
}

/// For an error response only; this never contributes to request authorization.
pub fn request_id_hint(raw: &str) -> Option<String> {
    if raw.len() > MAX_FRAME_BYTES {
        return None;
    }
    let envelope: Envelope<'_> = serde_json::from_str(raw).ok()?;
    valid_id(&envelope.request_id).then_some(envelope.request_id)
}

pub fn parse(raw: &str) -> WebResult<Request> {
    if raw.len() > MAX_FRAME_BYTES {
        return Err(WebError::new(WebErrorCode::TooLarge));
    }
    let envelope: Envelope<'_> =
        serde_json::from_str(raw).map_err(|_| WebError::new(WebErrorCode::InvalidRequest))?;
    if envelope.protocol != PROTOCOL {
        return Err(WebError::new(WebErrorCode::IncompatibleProtocol));
    }
    if !valid_id(&envelope.request_id) {
        return Err(WebError::new(WebErrorCode::InvalidRequest));
    }
    let method = serde_json::to_string(&envelope.method)
        .map_err(|_| WebError::new(WebErrorCode::InvalidRequest))?;
    let call = serde_json::from_str(&format!(
        "{{\"method\":{method},\"params\":{}}}",
        envelope.params.get()
    ))
    .map_err(|_| WebError::new(WebErrorCode::InvalidRequest))?;
    // Bound arbitrary storage JSON's nesting and entry count too, not just envelope bytes.
    let value: serde_json::Value = serde_json::from_str(envelope.params.get())
        .map_err(|_| WebError::new(WebErrorCode::InvalidRequest))?;
    let mut fields = 0;
    fn shape(value: &serde_json::Value, depth: usize, fields: &mut usize) -> bool {
        if depth > 16 {
            return false;
        }
        match value {
            serde_json::Value::Object(map) => {
                *fields += map.len();
                *fields <= 1024 && map.values().all(|v| shape(v, depth + 1, fields))
            }
            serde_json::Value::Array(values) => {
                *fields += values.len();
                *fields <= 1024 && values.iter().all(|v| shape(v, depth + 1, fields))
            }
            _ => true,
        }
    }
    if !shape(&value, 0, &mut fields) {
        return Err(WebError::new(WebErrorCode::TooLarge));
    }
    Ok(Request {
        id: envelope.request_id,
        call,
    })
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(method: &str, params: &str) -> String {
        format!(r#"{{"protocol":1,"requestId":"r1","method":"{method}","params":{params}}}"#)
    }
    #[test]
    fn rejects_identity_gesture_unknown_methods_duplicate_fields_and_extra_params() {
        for raw in [
            frame("host.confirm", r#"{"approved":true}"#),
            frame("clipboard.readText", r#"{"userGesture":true}"#),
            frame("context.getCurrentApp", r#"{"pluginId":"other"}"#),
            frame("storage.get", r#"{"key":"a","namespace":"other"}"#),
            frame("storage.get", r#"{"key":"a","key":"b"}"#),
            frame("invocation.get", "{}")
                .replace("\"protocol\":1", "\"protocol\":1,\"protocol\":1"),
            frame("invocation.get", "{}")
                .replace("\"protocol\":1", "\"runtimeId\":\"forged\",\"protocol\":1"),
        ] {
            assert!(parse(&raw).is_err(), "accepted {raw}");
        }
    }
    #[test]
    fn bounds_utf8_frames_nesting_field_count_and_protocol() {
        assert!(parse(&frame(
            "clipboard.writeText",
            &serde_json::json!({"text":"中".repeat(MAX_FRAME_BYTES / 2)}).to_string()
        ))
        .is_err());
        let nested = format!("{}0{}", "[".repeat(20), "]".repeat(20));
        assert!(parse(&frame(
            "storage.set",
            &format!(r#"{{"key":"a","value":{nested}}}"#)
        ))
        .is_err());
        assert!(parse(&frame(
            "storage.set",
            &serde_json::json!({"key":"a","value": vec![0;1025]}).to_string()
        ))
        .is_err());
        let raw = frame("invocation.get", "{}").replace("\"protocol\":1", "\"protocol\":2");
        assert_eq!(
            parse(&raw).err().unwrap().code,
            WebErrorCode::IncompatibleProtocol
        );
        assert_eq!(request_id_hint(&raw).as_deref(), Some("r1"));
    }
    #[test]
    fn accepts_only_exact_typed_request_shapes() {
        assert!(matches!(
            parse(&frame(
                "storage.set",
                r#"{"key":"prefs","value":{"indent":2}}"#
            ))
            .unwrap()
            .call,
            WebCall::StorageSet { .. }
        ));
        assert!(parse(&frame(
            "text.chunk",
            r#"{"ticket":"x","sequence":-1,"text":"a"}"#
        ))
        .is_err());
        assert!(parse(&frame("invocation.get", "null")).is_err());
        assert!(parse(&frame("invocation.get", "{}").replace("\"r1\"", "\"../id\"")).is_err());
    }
}
