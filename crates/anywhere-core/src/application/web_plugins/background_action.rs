//! A headless command uses the same broker and trusted consent as a Web document.
//! The async driver owns Host work; the blocking VM only exchanges bounded messages.
use super::{
    background_runtime, WebConfirmation, WebDispatch, WebDocumentBinding, WebMessageSource,
};
use crate::application::Application;
use anyhow::{ensure, Context, Result};
use anywhere_protocol::{web_bridge::*, web_plugin::PluginRuntime};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio::sync::{mpsc, oneshot, watch};

type ConsentReply = (String, oneshot::Sender<WebDispatch>);
pub(super) struct BackgroundJob {
    active: Arc<AtomicBool>,
    notices: watch::Sender<Option<WebConfirmation>>,
    consent: Mutex<Option<ConsentReply>>,
}
impl BackgroundJob {
    pub(super) fn cancel(&self) {
        self.active.store(false, Ordering::Release);
        self.notices.send_replace(None);
        if let Ok(mut pending) = self.consent.lock() {
            pending.take();
        }
    }
}
impl Drop for BackgroundJob {
    fn drop(&mut self) {
        self.cancel();
    }
}
struct Running(Arc<BackgroundJob>);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
fn source(binding: &WebDocumentBinding) -> WebMessageSource<'_> {
    WebMessageSource {
        runtime_id: binding.runtime_id(),
        document_epoch: binding.document_epoch(),
        origin: binding.origin(),
        main_frame: true,
    }
}

impl Application {
    pub async fn run_background_command(&self, binding: &WebDocumentBinding) -> Result<String> {
        let (file, entrypoint, input, registered, job) = {
            let mut web = self
                .web_plugins
                .lock()
                .map_err(|_| anyhow::anyhow!("plugin service unavailable"))?;
            ensure!(
                web.is_background(binding, source(binding))?,
                "not a background command"
            );
            ensure!(
                !web.backgrounds.contains_key(binding.runtime_id()),
                "background command already started"
            );
            let (reference, input) = {
                let invocation = web.authorize(binding, source(binding))?;
                (invocation.command.clone(), json!(invocation.input))
            };
            let registration = &web.catalog.entries[&reference.plugin_id];
            let command = registration
                .manifest
                .commands
                .iter()
                .find(|c| c.id == reference.command_id && c.runtime == PluginRuntime::Background)
                .context("background command missing")?;
            let file = registration
                .background_file
                .clone()
                .context("background resource unavailable")?;
            let values = (
                file,
                command.entrypoint.clone(),
                input,
                registration.active.clone(),
            );
            let job = Arc::new(BackgroundJob {
                active: Arc::new(AtomicBool::new(true)),
                notices: watch::channel(None).0,
                consent: Mutex::new(None),
            });
            web.backgrounds
                .insert(binding.runtime_id().into(), job.clone());
            (values.0, values.1, values.2, values.3, job)
        };
        let _running = Running(job.clone());
        // Ready records usage once, without creating any WebKit resource.
        let ready =
            json!({"protocol":1,"requestId":"bg_ready","method":"lifecycle.ready","params":{}})
                .to_string();
        let _ = self
            .dispatch_web_message(binding, source(binding), &ready)
            .await;
        let (sender, mut calls) = mpsc::channel::<(WebCall, oneshot::Sender<WebResult<Value>>)>(1);
        let invoke: background_runtime::HostInvoke = Arc::new(move |call| {
            let (reply, result) = oneshot::channel();
            sender
                .blocking_send((call, reply))
                .map_err(|_| WebError::new(WebErrorCode::Cancelled))?;
            result
                .blocking_recv()
                .unwrap_or_else(|_| Err(WebError::new(WebErrorCode::Cancelled)))
        });
        let active = job.active.clone();
        let current =
            Arc::new(move || active.load(Ordering::Acquire) && registered.load(Ordering::Acquire));
        let mut worker = tokio::task::spawn_blocking(move || {
            let code = String::from_utf8(file.read()?)?;
            background_runtime::run(&code, &entrypoint, &input, invoke, current)
        });
        let mut sequence = 0u64;
        let mut declined = false;
        let result = loop {
            tokio::select! {
                result = &mut worker => break result.context("background worker failed")?,
                call = calls.recv() => {
                    let Some((call, reply)) = call else { break worker.await.context("background worker failed")? };
                    sequence += 1;
                    let mut frame = serde_json::to_value(call)?;
                    frame["protocol"] = json!(PROTOCOL);
                    frame["requestId"] = json!(format!("background_{sequence}"));
                    let response = self.dispatch_web_message(binding, source(binding), &frame.to_string()).await;
                    let response = match response {
                        WebDispatch::Confirmation(notice) => {
                            let (answer, result) = oneshot::channel();
                            *job.consent.lock().map_err(|_| anyhow::anyhow!("consent unavailable"))? = Some((notice.ticket.clone(), answer));
                            job.notices.send_replace(Some(notice));
                            match tokio::time::timeout(std::time::Duration::from_secs(30), result).await {
                                Ok(Ok(value)) => value,
                                _ => { let _ = reply.send(Err(WebError::new(WebErrorCode::Timeout))); break Err(anyhow::anyhow!("background consent expired")); }
                            }
                        }
                        value => value,
                    };
                    let WebDispatch::Reply(raw) = response else { unreachable!("consent returns a reply") };
                    let result = self.background_reply_value(binding, &raw, &mut sequence).await;
                    declined |= result.as_ref().is_err_and(|error| error.code == WebErrorCode::UserCancelled);
                    let _ = reply.send(result);
                }
            }
        };
        self.web_plugins
            .lock()
            .map_err(|_| anyhow::anyhow!("plugin service unavailable"))?
            .backgrounds
            .remove(binding.runtime_id());
        // Declining a Host operation is a normal exit, not a crashed-tool page.
        if declined && result.is_err() {
            return Ok("null".into());
        }
        Ok(serde_json::to_string(&result?)?)
    }

    async fn background_reply_value(
        &self,
        binding: &WebDocumentBinding,
        raw: &str,
        sequence: &mut u64,
    ) -> WebResult<Value> {
        let parsed: Value =
            serde_json::from_str(raw).map_err(|_| WebError::new(WebErrorCode::HostFailure))?;
        if parsed.get("error").is_some() {
            let code = serde_json::from_value(parsed["error"]["code"].clone())
                .map_err(|_| WebError::new(WebErrorCode::HostFailure))?;
            return Err(WebError::new(code));
        }
        if let Some(value) = parsed["result"].get("value") {
            return Ok(value.clone());
        }
        let transfer = &parsed["result"]["transfer"];
        let ticket = transfer["ticket"]
            .as_str()
            .ok_or_else(|| WebError::new(WebErrorCode::HostFailure))?;
        let bytes = transfer["bytes"]
            .as_u64()
            .filter(|bytes| *bytes <= MAX_TRANSFER_BYTES as u64)
            .ok_or_else(|| WebError::new(WebErrorCode::TooLarge))?;
        let mut text = String::new();
        for chunk in 0..=(MAX_TRANSFER_BYTES / MAX_CHUNK_BYTES) {
            *sequence += 1;
            let frame = json!({"protocol":PROTOCOL,"requestId":format!("background_{}",sequence),"method":"text.read","params":{"ticket":ticket,"sequence":chunk}}).to_string();
            let WebDispatch::Reply(raw) = self
                .dispatch_web_message(binding, source(binding), &frame)
                .await
            else {
                return Err(WebError::new(WebErrorCode::HostFailure));
            };
            let response: Value =
                serde_json::from_str(&raw).map_err(|_| WebError::new(WebErrorCode::HostFailure))?;
            let value = &response["result"]["value"];
            let part = value["text"]
                .as_str()
                .filter(|part| part.len() <= MAX_CHUNK_BYTES)
                .ok_or_else(|| WebError::new(WebErrorCode::Expired))?;
            if value["sequence"] != chunk || text.len() + part.len() > bytes as usize {
                return Err(WebError::new(WebErrorCode::HostFailure));
            }
            text.push_str(part);
            if value["done"] == true && text.len() == bytes as usize {
                return serde_json::from_str(&text)
                    .map_err(|_| WebError::new(WebErrorCode::HostFailure));
            }
        }
        Err(WebError::new(WebErrorCode::TooLarge))
    }

    pub async fn wait_background_confirmation(
        &self,
        binding: &WebDocumentBinding,
        after: &str,
    ) -> Result<Option<WebConfirmation>> {
        self.validate_web_document(binding, source(binding))?;
        let job = self
            .web_plugins
            .lock()
            .map_err(|_| anyhow::anyhow!("plugin service unavailable"))?
            .backgrounds
            .get(binding.runtime_id())
            .cloned();
        let Some(job) = job else { return Ok(None) };
        let mut notices = job.notices.subscribe();
        loop {
            if !job.active.load(Ordering::Acquire) {
                return Ok(None);
            }
            if let Some(notice) = notices
                .borrow_and_update()
                .clone()
                .filter(|n| n.ticket != after)
            {
                return Ok(Some(notice));
            }
            if tokio::time::timeout(std::time::Duration::from_secs(15), notices.changed())
                .await
                .is_err()
            {
                return Ok(None);
            }
        }
    }

    pub async fn confirm_background_operation(
        &self,
        binding: &WebDocumentBinding,
        ticket: &str,
        approved: bool,
    ) -> Result<()> {
        let job = self
            .web_plugins
            .lock()
            .map_err(|_| anyhow::anyhow!("plugin service unavailable"))?
            .backgrounds
            .get(binding.runtime_id())
            .cloned()
            .context("background command expired")?;
        let answer = {
            let mut pending = job
                .consent
                .lock()
                .map_err(|_| anyhow::anyhow!("consent unavailable"))?;
            ensure!(
                pending.as_ref().is_some_and(|(id, _)| id == ticket),
                "consent ticket expired"
            );
            pending.take().expect("checked ticket").1
        };
        let reply = self
            .confirm_web_operation(binding, source(binding), ticket, approved)
            .await;
        let _ = answer.send(reply);
        job.notices.send_replace(None);
        Ok(())
    }
}
