//! Scheduling, cancellation and trusted Host confirmation around the capability planner.
use super::{
    broker::{error, Plan, Source},
    rpc_state::{ClipboardOperation, RequestTicket},
    transfer::failure,
    WebConfirmation, WebDispatch, WebDocumentBinding, WebMessageSource,
};
use crate::{application::Application, persistence::module_storage::ModuleStorageHost};
use anywhere_protocol::{web_bridge::*, web_plugin::WebPermission};
use serde_json::{json, Value};

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
    ) -> WebDispatch {
        let source = Source::new(source);
        let admission = (|| {
            let mut web = self
                .web_plugins
                .lock()
                .map_err(|_| error(WebErrorCode::HostFailure))?;
            web.rpc(binding, source.get())?;
            let parsed = anywhere_plugin::web::wire::parse(raw)?;
            let confirmation = matches!(
                parsed.call,
                WebCall::ClipboardRead {}
                    | WebCall::ClipboardWrite { .. }
                    | WebCall::TextCommit { .. }
            );
            let queued = !matches!(parsed.call, WebCall::Cancel { .. });
            let request =
                web.rpc(binding, source.get())?
                    .admit(&parsed.id, confirmation, queued)?;
            Ok::<_, WebError>((request, parsed.call))
        })();
        let (request, call) = match admission {
            Ok(v) => v,
            Err(e) => {
                return WebDispatch::Reply(failure(
                    anywhere_plugin::web::wire::request_id_hint(raw).as_deref(),
                    e,
                ))
            }
        };
        // Cancellation is never queued behind the operation it is meant to cancel.
        if matches!(call, WebCall::Cancel { .. }) {
            let mut web = match self.web_plugins.lock() {
                Ok(web) => web,
                Err(_) => {
                    return WebDispatch::Reply(failure(
                        Some(&request.id),
                        error(WebErrorCode::HostFailure),
                    ))
                }
            };
            let result = web
                .prepare_call(binding, &source, &request, call)
                .and_then(|plan| {
                    if let Plan::Value(v) = plan {
                        Ok(v)
                    } else {
                        Err(error(WebErrorCode::HostFailure))
                    }
                });
            return WebDispatch::Reply(web.finish_call(binding, &source, &request, result));
        }
        let permit = tokio::time::timeout_at(
            request.control.deadline,
            self.web_rpc_workers.clone().acquire_owned(),
        )
        .await;
        let permit = match permit {
            Ok(Ok(p)) => p,
            _ => {
                return self.finish_web_reply(
                    binding,
                    &source,
                    &request,
                    Err(error(WebErrorCode::Timeout)),
                )
            }
        };
        request
            .control
            .running
            .store(true, std::sync::atomic::Ordering::Release);
        let web = self.web_plugins.clone();
        let history = self.resolver.history.clone();
        let platform = self.platform.clone();
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
                Plan::Snapshot | Plan::Context | Plan::Input | Plan::SetInput(_) => {
                    return Ok(Work::Host(plan))
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
                Plan::Confirmation(confirmation) => return Ok(Work::Confirmation(confirmation)),
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
                    store.put(&key, &bytes).map_err(|e| {
                        if e.is::<crate::core::storage::StorageQuotaExceeded>() {
                            error(WebErrorCode::QuotaExceeded)
                        } else {
                            error(WebErrorCode::StorageUnavailable)
                        }
                    })?;
                    Value::Null
                }
                Plan::Remove(store, key) => {
                    store
                        .remove(&key)
                        .map_err(|_| error(WebErrorCode::StorageUnavailable))?;
                    Value::Null
                }
            };
            Ok::<_, WebError>(Work::Value(value))
        });
        let result = tokio::time::timeout_at(request.control.deadline, worker).await;
        match result {
            Ok(Ok(Ok(Work::Host(plan)))) => {
                let result = match plan {
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
                self.finish_web_reply(binding, &source, &request, result)
            }
            Ok(Ok(Ok(Work::Confirmation(confirmation)))) => {
                request
                    .control
                    .running
                    .store(false, std::sync::atomic::Ordering::Release);
                let active = request.control.check().and_then(|_| {
                    let mut web = self
                        .web_plugins
                        .lock()
                        .map_err(|_| error(WebErrorCode::HostFailure))?;
                    if web
                        .rpc(binding, source.get())?
                        .confirmation
                        .as_ref()
                        .is_some_and(|c| c.id == confirmation.ticket)
                    {
                        Ok(())
                    } else {
                        Err(error(WebErrorCode::Inactive))
                    }
                });
                if let Err(e) = active {
                    return self.finish_web_reply(binding, &source, &request, Err(e));
                }
                WebDispatch::Confirmation(confirmation)
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
    }
    fn finish_web_reply(
        &self,
        binding: &WebDocumentBinding,
        source: &Source,
        request: &RequestTicket,
        result: WebResult<Value>,
    ) -> WebDispatch {
        match self.web_plugins.lock() {
            Ok(mut web) => WebDispatch::Reply(web.finish_call(binding, source, request, result)),
            Err(_) => {
                WebDispatch::Reply(failure(Some(&request.id), error(WebErrorCode::HostFailure)))
            }
        }
    }
    /// Trusted Host confirmation only. No WebCall can invoke this method or forge its ticket.
    pub async fn confirm_web_operation(
        &self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
        ticket: &str,
        approved: bool,
    ) -> WebDispatch {
        let source = Source::new(source);
        let pending = (|| {
            let mut web = self
                .web_plugins
                .lock()
                .map_err(|_| error(WebErrorCode::HostFailure))?;
            let rpc = web.rpc(binding, source.get())?;
            if !rpc.confirmation.as_ref().is_some_and(|c| c.id == ticket) {
                return Err(error(WebErrorCode::Expired));
            }
            rpc.confirmation
                .take()
                .ok_or_else(|| error(WebErrorCode::Expired))
        })();
        let confirmation = match pending {
            Ok(v) => v,
            Err(e) => return WebDispatch::Reply(failure(None, e)),
        };
        if !approved {
            return self.finish_web_reply(
                binding,
                &source,
                &confirmation.request,
                Err(error(WebErrorCode::UserCancelled)),
            );
        }
        let request = confirmation.request;
        let permit = match tokio::time::timeout_at(
            request.control.deadline,
            self.web_rpc_workers.clone().acquire_owned(),
        )
        .await
        {
            Ok(Ok(p)) => p,
            _ => {
                return self.finish_web_reply(
                    binding,
                    &source,
                    &request,
                    Err(error(WebErrorCode::Timeout)),
                )
            }
        };
        let platform = self.platform.clone();
        request
            .control
            .running
            .store(true, std::sync::atomic::Ordering::Release);
        let web = self.web_plugins.clone();
        let worker_binding = binding.clone();
        let worker_source = source.clone();
        let worker_request = request.clone();
        let worker = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let permission = match &confirmation.operation {
                ClipboardOperation::Read => WebPermission::ClipboardReadText {},
                ClipboardOperation::Write(_) => WebPermission::ClipboardWriteText {},
            };
            web.lock()
                .map_err(|_| error(WebErrorCode::HostFailure))?
                .require(&worker_binding, worker_source.get(), permission)?;
            worker_request.control.check()?;
            match confirmation.operation {
                ClipboardOperation::Read => {
                    let text = platform
                        .clipboard_text()
                        .map_err(|_| error(WebErrorCode::HostFailure))?;
                    if text.as_ref().is_some_and(|s| s.len() > MAX_TEXT_BYTES) {
                        return Err(error(WebErrorCode::TooLarge));
                    }
                    Ok(json!(text))
                }
                ClipboardOperation::Write(text) => {
                    platform
                        .copy_text(&text)
                        .map_err(|_| error(WebErrorCode::HostFailure))?;
                    Ok(Value::Null)
                }
            }
        });
        let result = match tokio::time::timeout_at(request.control.deadline, worker).await {
            Ok(Ok(value)) => value,
            Ok(Err(_)) => Err(error(WebErrorCode::HostFailure)),
            Err(_) => Err(error(WebErrorCode::Timeout)),
        };
        self.finish_web_reply(binding, &source, &request, result)
    }
}
enum Work {
    Host(Plan),
    Value(Value),
    Confirmation(WebConfirmation),
}
