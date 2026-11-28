//! Bounded scheduling and cancellation around the capability planner.
use super::{
    broker::{error, Plan, Source},
    interaction::PendingPreview,
    resources::MAX_PREVIEW_BYTES,
    rpc_state::{RequestControl, RequestTicket},
    transfer::failure,
    WebDispatchOutcome, WebDocumentBinding, WebHostAction, WebHostActionKind, WebMessageSource,
};
use crate::{application::Application, persistence::module_storage::ModuleStorageHost};
use anywhere_protocol::web_bridge::*;
use serde_json::{json, Value};

fn update_web_storage(
    store: &dyn crate::core::storage::ExtensionStorage,
    key: &str,
    control: &RequestControl,
    mut value: Option<Vec<u8>>,
) -> WebResult<Value> {
    let mut rejected = None;
    let updated = store.update(key, &mut |_| {
        control.check().map_err(|error| {
            rejected = Some(error);
            anyhow::anyhow!("Web storage request expired")
        })?;
        Ok(value.take())
    });
    if let Some(error) = rejected {
        return Err(error);
    }
    updated.map(|_| Value::Null).map_err(|error_value| {
        if error_value.is::<crate::core::storage::StorageQuotaExceeded>() {
            error(WebErrorCode::QuotaExceeded)
        } else {
            error(WebErrorCode::StorageUnavailable)
        }
    })
}

impl Application {
    pub fn clear_web_storage_namespace(&self, namespace: &str) -> anyhow::Result<()> {
        let storage = self
            .web_plugins
            .lock()
            .map_err(|_| anyhow::anyhow!("Web service unavailable"))?
            .storage
            .clone();
        // SQLite can wait for another connection. Never hold document lifecycle
        // coordination while doing disk I/O.
        if let Some(storage) = storage {
            storage.clear_namespace(namespace)?;
        }
        Ok(())
    }
    /// Host composition attaches the persistent database. JS never receives a factory.
    pub fn configure_web_storage(&self, storage: ModuleStorageHost) -> anyhow::Result<()> {
        let mut web = self
            .web_plugins
            .lock()
            .map_err(|_| anyhow::anyhow!("Web service unavailable"))?;
        web.invalidate();
        web.storage = Some(storage);
        Ok(())
    }
    pub async fn dispatch_web_message(
        &self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
        raw: &str,
    ) -> String {
        self.dispatch_web_outcome(binding, source, raw)
            .await
            .reply_frame
    }

    /// Dispatch a frame with any qualified native window intent.
    pub async fn dispatch_web_outcome(
        &self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
        raw: &str,
    ) -> WebDispatchOutcome {
        let parsed = anywhere_plugin::web::wire::parse(raw);
        let hint = parsed
            .as_ref()
            .ok()
            .map(|request| request.id.clone())
            .or_else(|| anywhere_plugin::web::wire::request_id_hint(raw));
        self.dispatch_web_request(binding, source, parsed, hint)
            .await
    }

    pub(super) async fn dispatch_web_call(
        &self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
        id: String,
        call: WebCall,
    ) -> String {
        self.dispatch_web_request(
            binding,
            source,
            Ok(anywhere_plugin::web::wire::Request {
                id: id.clone(),
                call,
            }),
            Some(id),
        )
        .await
        .reply_frame
    }

    async fn dispatch_web_request(
        &self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
        parsed: WebResult<anywhere_plugin::web::wire::Request>,
        hint: Option<String>,
    ) -> WebDispatchOutcome {
        let source = Source::new(source);
        let (request, call) = match self.admit_web_request(binding, &source, parsed) {
            Ok(value) => value,
            Err(error) => return failure(hint.as_deref(), error).into(),
        };
        let window_action = match call {
            WebCall::WindowBack {} => Some(WebHostActionKind::Back),
            WebCall::WindowClose {} => Some(WebHostActionKind::Close),
            _ => None,
        };
        if window_action.is_some() || matches!(call, WebCall::Cancel { .. }) {
            return self.dispatch_immediate(binding, &source, request, call, window_action);
        }
        let permit = tokio::select! {
            _ = request.control.cancelled() => {
                return self.finish_web_reply(binding, &source, &request,
                    Err(error(WebErrorCode::Cancelled))).into();
            }
            permit = tokio::time::timeout_at(
                request.control.deadline,
                self.web_rpc_workers.clone().acquire_owned(),
            ) => permit,
        };
        let permit = match permit {
            Ok(Ok(p)) => p,
            _ => {
                return self
                    .finish_web_reply(
                        binding,
                        &source,
                        &request,
                        Err(error(WebErrorCode::Timeout)),
                    )
                    .into()
            }
        };
        request
            .control
            .running
            .store(true, std::sync::atomic::Ordering::Release);
        let web = self.web_plugins.clone();
        let history = self.resolver.history.clone();
        let platform = self.platform.clone();
        let previews = self.previews.clone();
        let worker_binding = binding.clone();
        let worker_source = source.clone();
        let worker_request = request.clone();
        let worker = tokio::task::spawn_blocking(move || {
            let _permit = permit; // Retained until actual I/O ends, including after a timeout.
            let plan = web
                .lock()
                .map_err(|_| error(WebErrorCode::HostFailure))?
                .prepare_call(&worker_binding, &worker_source, &worker_request, call)?;
            worker_request.control.check()?;
            let value = match plan {
                Plan::Value(value) => value,
                Plan::Snapshot
                | Plan::Context
                | Plan::Input
                | Plan::SetInput(_)
                | Plan::OpenUrl(_) => return Ok(Work::Host(plan)),
                Plan::Stat(file) => serde_json::to_value(file.stat(&worker_request.control)?)
                    .map_err(|_| error(WebErrorCode::HostFailure))?,
                Plan::ReadText(file, limit) => {
                    Value::String(file.text(limit, &worker_request.control)?)
                }
                Plan::Preview(file) => {
                    let bytes = file.bytes(MAX_PREVIEW_BYTES, &worker_request.control)?;
                    worker_request.control.check()?;
                    let snapshot = crate::core::sync::lock_or_deny(&previews, "previews")
                        .and_then(|mut store| store.web_snapshot(&file.name, &bytes))
                        .map_err(|err| {
                            if err.to_string().contains("too many active previews") {
                                error(WebErrorCode::Busy)
                            } else {
                                error(WebErrorCode::HostFailure)
                            }
                        })?;
                    return Ok(Work::Preview(PendingPreview::new(previews, snapshot)));
                }
                Plan::FrontmostApp => {
                    let app = platform
                        .frontmost_application()
                        .map_err(|_| error(WebErrorCode::HostFailure))?;
                    json!(app.map(|app| anywhere_protocol::web_plugin::WebCurrentApp {
                        id: app.bundle_id,
                        name: app.name
                    }))
                }
                Plan::ReadClipboard => {
                    let text = platform
                        .clipboard_text()
                        .map_err(|_| error(WebErrorCode::HostFailure))?;
                    if text
                        .as_ref()
                        .is_some_and(|text| text.len() > MAX_TEXT_BYTES)
                    {
                        return Err(error(WebErrorCode::TooLarge));
                    }
                    json!(text)
                }
                Plan::WriteClipboard(text) => {
                    platform
                        .copy_text(&text)
                        .map_err(|_| error(WebErrorCode::HostFailure))?;
                    Value::Null
                }
                Plan::Ready => {
                    let usage = web
                        .lock()
                        .map_err(|_| error(WebErrorCode::HostFailure))?
                        .ready(&worker_binding, worker_source.get())
                        .map_err(|_| error(WebErrorCode::Expired))?;
                    if let Some(usage) = usage {
                        history
                            .lock()
                            .map_err(|_| error(WebErrorCode::HostFailure))?
                            .record(
                                &usage.action_id,
                                &usage.context_type,
                                &usage.source_app,
                                crate::application::session_runtime::now(),
                            )
                            .map_err(|_| error(WebErrorCode::HostFailure))?;
                    }
                    Value::Null
                }
                Plan::Get(store, key) => match store
                    .get(&key)
                    .map_err(|_| error(WebErrorCode::StorageUnavailable))?
                {
                    Some(bytes) => serde_json::from_slice(&bytes)
                        .map_err(|_| error(WebErrorCode::StorageUnavailable))?,
                    None => Value::Null,
                },
                Plan::Set(store, key, bytes) => {
                    update_web_storage(store.as_ref(), &key, &worker_request.control, Some(bytes))?
                }
                Plan::Remove(store, key) => {
                    update_web_storage(store.as_ref(), &key, &worker_request.control, None)?
                }
            };
            Ok::<_, WebError>(Work::Value(value))
        });
        let result = tokio::time::timeout_at(request.control.deadline, worker).await;
        match result {
            Ok(Ok(Ok(Work::Host(plan)))) => {
                let mut committed_handoff = false;
                let result = match plan {
                    Plan::OpenUrl(url) => {
                        let outcome = self.confirm_web_url(binding, &source, &request, url).await;
                        committed_handoff = outcome.is_ok();
                        outcome
                    }
                    Plan::SetInput(text) => {
                        self.set_web_input(binding, source.get(), text, &request.control)
                            .await
                    }
                    _ => self.web_snapshot(binding, source.get()).and_then(|state| {
                        match plan {
                            Plan::Snapshot => serde_json::to_value(state),
                            Plan::Context => serde_json::to_value(state.context),
                            Plan::Input => serde_json::to_value(state.input),
                            _ => unreachable!("Host state plan"),
                        }
                        .map_err(|_| error(WebErrorCode::HostFailure))
                    }),
                };
                self.finish_web_reply_inner(binding, &source, &request, result, committed_handoff)
            }
            Ok(Ok(Ok(Work::Preview(preview)))) => {
                let result = self
                    .show_web_preview(binding, &source, &request, preview)
                    .await;
                let committed = result.is_ok();
                self.finish_web_reply_inner(binding, &source, &request, result, committed)
            }
            Ok(Ok(Ok(Work::Value(value)))) => {
                self.finish_web_reply(binding, &source, &request, Ok(value))
            }
            Ok(Ok(Err(e))) => self.finish_web_reply(binding, &source, &request, Err(e)),
            Ok(Err(_)) => self.finish_web_reply(
                binding,
                &source,
                &request,
                Err(error(WebErrorCode::HostFailure)),
            ),
            Err(_) => self.finish_web_reply(
                binding,
                &source,
                &request,
                Err(error(WebErrorCode::Timeout)),
            ),
        }
        .into()
    }
    fn admit_web_request(
        &self,
        binding: &WebDocumentBinding,
        source: &Source,
        parsed: WebResult<anywhere_plugin::web::wire::Request>,
    ) -> WebResult<(RequestTicket, WebCall)> {
        let mut web = self
            .web_plugins
            .lock()
            .map_err(|_| error(WebErrorCode::HostFailure))?;
        let invocation = web
            .authorize(binding, source.get())
            .map_err(|_| error(WebErrorCode::UntrustedDocument))?;
        if invocation.is_draining() {
            return Err(error(WebErrorCode::Inactive));
        }
        let parsed = parsed?;
        let request = web
            .rpc(binding, source.get())?
            .admit_call(&parsed.id, &parsed.call)?;
        Ok((request, parsed.call))
    }

    // Cancellation and native window intents require no data worker.
    fn dispatch_immediate(
        &self,
        binding: &WebDocumentBinding,
        source: &Source,
        request: RequestTicket,
        call: WebCall,
        window_action: Option<WebHostActionKind>,
    ) -> WebDispatchOutcome {
        let mut web = match crate::core::sync::lock_or_deny(&self.web_plugins, "web_plugins") {
            Ok(web) => web,
            Err(_) => return failure(Some(&request.id), error(WebErrorCode::HostFailure)).into(),
        };
        let result = web
            .prepare_call(binding, source, &request, call)
            .and_then(|plan| {
                if let Plan::Value(v) = plan {
                    Ok(v)
                } else {
                    Err(error(WebErrorCode::HostFailure))
                }
            });
        let (reply_frame, accepted) = web.finish_call(binding, source, &request, result);
        WebDispatchOutcome {
            reply_frame,
            host_action: window_action
                .filter(|_| accepted)
                .map(|kind| WebHostAction {
                    kind,
                    request_id: request.id,
                    handle: binding.runtime_id().into(),
                    document_epoch: binding.document_epoch(),
                }),
        }
    }

    fn finish_web_reply(
        &self,
        binding: &WebDocumentBinding,
        source: &Source,
        request: &RequestTicket,
        result: WebResult<Value>,
    ) -> String {
        self.finish_web_reply_inner(binding, source, request, result, false)
    }

    fn finish_web_reply_inner(
        &self,
        binding: &WebDocumentBinding,
        source: &Source,
        request: &RequestTicket,
        result: WebResult<Value>,
        committed: bool,
    ) -> String {
        match crate::core::sync::lock_or_deny(&self.web_plugins, "web_plugins") {
            Ok(mut web) => {
                web.clear_handoff_request(binding, &request.id);
                if committed {
                    web.finish_call_after_commit(binding, source, request, result)
                        .0
                } else {
                    web.finish_call(binding, source, request, result).0
                }
            }
            Err(_) => failure(Some(&request.id), error(WebErrorCode::HostFailure)),
        }
    }
}
enum Work {
    Host(Plan),
    Preview(PendingPreview),
    Value(Value),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::web_plugins::rpc_state::RpcState;

    #[test]
    fn storage_update_rechecks_the_ticket_after_namespace_revocation() {
        let host = ModuleStorageHost::memory().unwrap();
        let store = host.bind("web.test").unwrap();
        store.put("key", b"before").unwrap();
        let mut rpc = RpcState::default();
        let write = rpc.admit("write", true).unwrap();
        let remove = rpc.admit("remove", true).unwrap();
        assert!(write.control.check().is_ok());
        assert!(remove.control.check().is_ok());

        drop(rpc);
        host.clear_namespace("web.test").unwrap();
        let result = update_web_storage(
            store.as_ref(),
            "key",
            &write.control,
            Some(b"stale".to_vec()),
        );
        assert_eq!(result.unwrap_err().code, WebErrorCode::Cancelled);
        assert_eq!(store.get("key").unwrap(), None);
        store.put("key", b"current").unwrap();
        let result = update_web_storage(store.as_ref(), "key", &remove.control, None);
        assert_eq!(result.unwrap_err().code, WebErrorCode::Cancelled);
        assert_eq!(
            store.get("key").unwrap().as_deref(),
            Some(b"current".as_slice())
        );
    }
}
