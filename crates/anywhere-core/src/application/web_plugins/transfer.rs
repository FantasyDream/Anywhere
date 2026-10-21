//! One expiring text buffer per document; this is not a generic stream transport.
use super::rpc_state::{RpcState, Transfer};
use anywhere_protocol::web_bridge::*;
use serde_json::{json, Value};
use std::io::Write;
use tokio::time::{Duration, Instant};

pub(super) fn bounded_json(value: &impl serde::Serialize, limit: usize) -> WebResult<String> {
    struct Buffer {
        bytes: Vec<u8>,
        limit: usize,
    }
    impl Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
                return Err(std::io::Error::other("quota"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut buffer = Buffer {
        bytes: vec![],
        limit,
    };
    serde_json::to_writer(&mut buffer, value).map_err(|_| WebError::new(WebErrorCode::TooLarge))?;
    String::from_utf8(buffer.bytes).map_err(|_| WebError::new(WebErrorCode::HostFailure))
}

pub(super) fn failure(id: Option<&str>, error: WebError) -> String {
    serde_json::to_string(&WebResponse {
        request_id: id.map(str::to_owned),
        outcome: WebOutcome::Failure { error },
    })
    .expect("fixed error serializes")
}

impl RpcState {
    pub fn begin_text(&mut self, bytes: usize) -> WebResult<Value> {
        self.prune();
        if self.transfer.is_some() || self.confirmation.is_some() {
            return Err(WebError::new(WebErrorCode::Busy));
        }
        if bytes > MAX_TEXT_BYTES {
            return Err(WebError::new(WebErrorCode::TooLarge));
        }
        let id = super::token().map_err(|_| WebError::new(WebErrorCode::HostFailure))?;
        self.transfer = Some(Transfer::Upload {
            id: id.clone(),
            bytes,
            sequence: 0,
            text: String::new(),
            expires: Instant::now() + Duration::from_millis(REQUEST_TIMEOUT_MS),
        });
        Ok(json!({"ticket":id}))
    }
    pub fn push_text(&mut self, ticket: &str, index: u32, chunk: &str) -> WebResult<Value> {
        self.prune();
        let Some(Transfer::Upload {
            id,
            bytes,
            sequence,
            text,
            ..
        }) = &mut self.transfer
        else {
            return Err(WebError::new(WebErrorCode::Expired));
        };
        if ticket != id {
            return Err(WebError::new(WebErrorCode::Expired));
        }
        if *sequence != index
            || chunk.len() > MAX_CHUNK_BYTES
            || chunk.len() > bytes.saturating_sub(text.len())
        {
            return Err(WebError::new(WebErrorCode::InvalidRequest));
        }
        text.push_str(chunk);
        *sequence += 1;
        Ok(Value::Null)
    }
    pub fn commit_text(&mut self, ticket: &str) -> WebResult<String> {
        self.prune();
        if !self.transfer.as_ref().is_some_and(|t| t.id() == ticket) {
            return Err(WebError::new(WebErrorCode::Expired));
        }
        match self.transfer.take() {
            Some(Transfer::Upload { bytes, text, .. }) if text.len() == bytes => Ok(text),
            _ => Err(WebError::new(WebErrorCode::InvalidRequest)),
        }
    }
    pub fn read_text(&mut self, ticket: &str, index: u32) -> WebResult<Value> {
        self.prune();
        let Some(Transfer::Download {
            id,
            sequence,
            offset,
            text,
            ..
        }) = &mut self.transfer
        else {
            return Err(WebError::new(WebErrorCode::Expired));
        };
        if ticket != id {
            return Err(WebError::new(WebErrorCode::Expired));
        }
        if index != *sequence {
            return Err(WebError::new(WebErrorCode::InvalidRequest));
        }
        let mut end = (*offset + MAX_CHUNK_BYTES).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let result =
            json!({"text": &text[*offset..end], "sequence":index, "done":end == text.len()});
        *sequence += 1;
        *offset = end;
        if end == text.len() {
            self.transfer = None;
        }
        Ok(result)
    }
    pub fn cancel_text(&mut self, ticket: &str) -> WebResult<Value> {
        if !self.transfer.as_ref().is_some_and(|t| t.id() == ticket) {
            return Err(WebError::new(WebErrorCode::Expired));
        }
        self.transfer = None;
        Ok(Value::Null)
    }
    pub fn pack(&mut self, request_id: &str, value: Value) -> WebResult<String> {
        let payload = bounded_json(&value, MAX_TRANSFER_BYTES)?;
        if payload.len() + request_id.len() + 64 < MAX_FRAME_BYTES {
            return Ok(format!(
                "{{\"requestId\":\"{request_id}\",\"result\":{{\"value\":{payload}}}}}"
            ));
        }
        self.prune();
        if self.transfer.is_some() || self.confirmation.is_some() {
            return Err(WebError::new(WebErrorCode::Busy));
        }
        let id = super::token().map_err(|_| WebError::new(WebErrorCode::HostFailure))?;
        let bytes = payload.len();
        self.transfer = Some(Transfer::Download {
            id: id.clone(),
            sequence: 0,
            offset: 0,
            text: payload,
            expires: Instant::now() + Duration::from_millis(REQUEST_TIMEOUT_MS),
        });
        Ok(
            json!({"requestId":request_id,"result":{"transfer":{"ticket":id,"bytes":bytes}}})
                .to_string(),
        )
    }
}
