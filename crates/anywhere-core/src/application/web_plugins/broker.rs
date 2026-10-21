//! Closed Rust capability broker. Only trusted Host code can approve clipboard operations.
use super::{
    rpc_state::{ClipboardOperation, Confirmation, RequestTicket, RpcState},
    transfer::{bounded_json, failure},
    WebDocumentBinding, WebMessageSource, WebPluginService,
};
use crate::core::storage::ExtensionStorage;
use anywhere_protocol::{web_bridge::*, web_plugin::WebPermission};
use serde_json::{json, Value};
use std::sync::Arc;

/// This value is not serializable and must go to Host chrome, never to JS.
pub struct WebConfirmation {
    pub ticket: String,
    pub request_id: String,
    pub title: &'static str,
    pub preview: Option<String>,
}
pub enum WebDispatch {
    Reply(String),
    Confirmation(WebConfirmation),
}

#[derive(Clone)]
pub(super) struct Source {
    runtime: String,
    epoch: u64,
    origin: String,
    main: bool,
}
impl Source {
    pub(super) fn new(source: WebMessageSource<'_>) -> Self {
        Self {
            runtime: source.runtime_id.into(),
            epoch: source.document_epoch,
            origin: source.origin.into(),
            main: source.main_frame,
        }
    }
    pub(super) fn get(&self) -> WebMessageSource<'_> {
        WebMessageSource {
            runtime_id: &self.runtime,
            document_epoch: self.epoch,
            origin: &self.origin,
            main_frame: self.main,
        }
    }
}
pub(super) enum Plan {
    Value(Value),
    Ready,
    Get(Arc<dyn ExtensionStorage>, String),
    Set(Arc<dyn ExtensionStorage>, String, Vec<u8>),
    Remove(Arc<dyn ExtensionStorage>, String),
    Confirmation(WebConfirmation),
}
pub(super) fn error(code: WebErrorCode) -> WebError {
    WebError::new(code)
}

impl WebPluginService {
    pub(super) fn sweep_document(&mut self, binding: &WebDocumentBinding) -> bool {
        let source = WebMessageSource {
            runtime_id: binding.runtime_id(),
            document_epoch: binding.document_epoch(),
            origin: binding.origin(),
            main_frame: true,
        };
        self.rpc(binding, source).is_ok()
    }
    pub(super) fn rpc(
        &mut self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
    ) -> WebResult<&mut RpcState> {
        self.authorize(binding, source)
            .map_err(|_| error(WebErrorCode::UntrustedDocument))?;
        let rpc = &mut self
            .invocations
            .get_mut(&binding.invocation_id)
            .ok_or_else(|| error(WebErrorCode::Inactive))?
            .rpc;
        rpc.prune();
        Ok(rpc)
    }
    pub(super) fn require(
        &mut self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
        permission: WebPermission,
    ) -> WebResult<()> {
        let plugin = self
            .authorize(binding, source)
            .map_err(|_| error(WebErrorCode::UntrustedDocument))?
            .command
            .plugin_id
            .clone();
        if !self.catalog.entries[&plugin]
            .manifest
            .permissions
            .contains(&permission)
        {
            return Err(error(WebErrorCode::PermissionDenied));
        }
        Ok(())
    }
    fn storage_for(
        &mut self,
        binding: &WebDocumentBinding,
        source: &Source,
        key: &str,
    ) -> WebResult<Arc<dyn ExtensionStorage>> {
        self.require(binding, source.get(), WebPermission::LocalStorage {})?;
        if key.is_empty()
            || key.len() > crate::core::storage::MAX_KEY_BYTES
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err(error(WebErrorCode::InvalidRequest));
        }
        let plugin = self
            .authorize(binding, source.get())
            .map_err(|_| error(WebErrorCode::UntrustedDocument))?
            .command
            .plugin_id
            .clone();
        let entry = &self.catalog.entries[&plugin];
        let namespace = super::storage_namespace(&plugin, &entry.source_id);
        self.storage
            .as_ref()
            .ok_or_else(|| error(WebErrorCode::StorageUnavailable))?
            .bind_with_quota(&namespace, 1024 * 1024, 256)
            .map_err(|_| error(WebErrorCode::StorageUnavailable))
    }
    fn confirm(
        &mut self,
        binding: &WebDocumentBinding,
        source: &Source,
        request: &RequestTicket,
        operation: ClipboardOperation,
    ) -> WebResult<Plan> {
        let rpc = self.rpc(binding, source.get())?;
        if rpc.confirmation.is_some() || rpc.transfer.is_some() {
            return Err(error(WebErrorCode::Busy));
        }
        let ticket = super::token().map_err(|_| error(WebErrorCode::HostFailure))?;
        let (title, preview) = match &operation {
            ClipboardOperation::Read => ("Read clipboard text", None),
            ClipboardOperation::Write(text) => (
                "Copy text to clipboard",
                Some(text.chars().take(120).collect()),
            ),
        };
        rpc.confirmation = Some(Confirmation {
            id: ticket.clone(),
            request: request.clone(),
            operation,
        });
        Ok(Plan::Confirmation(WebConfirmation {
            ticket,
            request_id: request.id.clone(),
            title,
            preview,
        }))
    }
    pub(super) fn prepare_call(
        &mut self,
        binding: &WebDocumentBinding,
        source: &Source,
        request: &RequestTicket,
        call: WebCall,
    ) -> WebResult<Plan> {
        request.control.check()?;
        self.rpc(binding, source.get())?;
        let value = match call {
            WebCall::InvocationGet {} => serde_json::to_value(
                self.invocation(binding, source.get())
                    .map_err(|_| error(WebErrorCode::Inactive))?,
            )
            .map_err(|_| error(WebErrorCode::HostFailure))?,
            WebCall::CurrentApp {} => {
                self.require(binding, source.get(), WebPermission::CurrentApp {})?;
                serde_json::to_value(
                    self.current_app(binding, source.get())
                        .map_err(|_| error(WebErrorCode::Inactive))?,
                )
                .map_err(|_| error(WebErrorCode::HostFailure))?
            }
            WebCall::Ready {} => return Ok(Plan::Ready),
            WebCall::ClipboardRead {} => {
                self.require(binding, source.get(), WebPermission::ClipboardReadText {})?;
                return self.confirm(binding, source, request, ClipboardOperation::Read);
            }
            WebCall::ClipboardWrite { text } => {
                self.require(binding, source.get(), WebPermission::ClipboardWriteText {})?;
                if text.len() > MAX_TEXT_BYTES {
                    return Err(error(WebErrorCode::TooLarge));
                }
                return self.confirm(binding, source, request, ClipboardOperation::Write(text));
            }
            WebCall::StorageGet { key } => {
                return Ok(Plan::Get(self.storage_for(binding, source, &key)?, key))
            }
            WebCall::StorageSet { key, value } => {
                let store = self.storage_for(binding, source, &key)?;
                let bytes =
                    bounded_json(&value, crate::core::storage::MAX_VALUE_BYTES)?.into_bytes();
                return Ok(Plan::Set(store, key, bytes));
            }
            WebCall::StorageRemove { key } => {
                return Ok(Plan::Remove(self.storage_for(binding, source, &key)?, key))
            }
            WebCall::TextBegin { bytes } => {
                self.require(binding, source.get(), WebPermission::ClipboardWriteText {})?;
                self.rpc(binding, source.get())?.begin_text(bytes)?
            }
            WebCall::TextChunk {
                ticket,
                sequence,
                text,
            } => {
                self.require(binding, source.get(), WebPermission::ClipboardWriteText {})?;
                self.rpc(binding, source.get())?
                    .push_text(&ticket, sequence, &text)?
            }
            WebCall::TextCommit { ticket } => {
                self.require(binding, source.get(), WebPermission::ClipboardWriteText {})?;
                let text = self.rpc(binding, source.get())?.commit_text(&ticket)?;
                return self.confirm(binding, source, request, ClipboardOperation::Write(text));
            }
            WebCall::TextRead { ticket, sequence } => self
                .rpc(binding, source.get())?
                .read_text(&ticket, sequence)?,
            WebCall::TextCancel { ticket } => {
                self.rpc(binding, source.get())?.cancel_text(&ticket)?
            }
            WebCall::Cancel { request_id } => {
                if !anywhere_plugin::web::wire::valid_id(&request_id) || request_id == request.id {
                    return Err(error(WebErrorCode::InvalidRequest));
                }
                json!({"cancelled": self.rpc(binding, source.get())?.cancel(&request_id)})
            }
        };
        Ok(Plan::Value(value))
    }
    pub(super) fn finish_call(
        &mut self,
        binding: &WebDocumentBinding,
        source: &Source,
        request: &RequestTicket,
        value: WebResult<Value>,
    ) -> String {
        let result = request.control.check().and_then(|_| {
            let rpc = self.rpc(binding, source.get())?;
            value.and_then(|value| rpc.pack(&request.id, value))
        });
        if let Some(invocation) = self.invocations.get_mut(&binding.invocation_id) {
            invocation.rpc.finish(request);
        }
        result.unwrap_or_else(|e| failure(Some(&request.id), e))
    }
}
