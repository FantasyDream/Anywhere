//! A headless command uses the same grant-checked broker as a Web document.
//! The async driver owns Host work; the blocking VM only exchanges bounded messages.
use super::{background_runtime, WebDocumentBinding, WebMessageSource};
use crate::application::Application;
use anyhow::{ensure, Context, Result};
use anywhere_protocol::{web_bridge::*, web_plugin::PluginRuntime};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio::sync::{mpsc, oneshot};

pub(super) struct BackgroundJob {
    active: Arc<AtomicBool>,
}
impl BackgroundJob {
    pub(super) fn cancel(&self) {
        self.active.store(false, Ordering::Release);
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
// A cancelled execution future must retire its binding as well as its VM job.
struct HeadlessDocument {
    service: Arc<Mutex<super::WebPluginService>>,
    binding: WebDocumentBinding,
}
impl Drop for HeadlessDocument {
    fn drop(&mut self) {
        if let Ok(mut service) = self.service.lock() {
            service.close_document(&self.binding);
        }
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
    pub(crate) async fn run_headless_web_action(
        &self,
        view: Option<&anywhere_protocol::HostView>,
        session_id: anywhere_protocol::SessionId,
        revision: anywhere_protocol::Revision,
    ) -> Result<bool> {
        let Some(anywhere_protocol::HostView::WebPlugin { invocation_id }) = view else {
            return Ok(false);
        };
        let binding = {
            let session = self.session()?;
            ensure!(
                !session.suspended
                    && session.state.session_id == session_id
                    && session.state.revision == revision,
                "plugin execution expired"
            );
            let mut web = self
                .web_plugins
                .lock()
                .map_err(|_| anyhow::anyhow!("plugin service unavailable"))?;
            let invocation = web
                .invocations
                .get(invocation_id)
                .context("plugin invocation expired")?;
            let entry = &web.catalog.entries[&invocation.command.plugin_id];
            if !entry.manifest.commands.iter().any(|command| {
                command.id == invocation.command.command_id
                    && command.runtime == PluginRuntime::Background
            }) {
                return Ok(false);
            }
            let platform = entry.platform;
            web.bind(invocation_id, platform)?
        };
        let document = HeadlessDocument {
            service: self.web_plugins.clone(),
            binding,
        };
        self.run_background_command(&document.binding).await?;
        Ok(true)
    }

    pub(super) async fn run_background_command(
        &self,
        binding: &WebDocumentBinding,
    ) -> Result<String> {
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
        let result = loop {
            tokio::select! {
                result = &mut worker => break result.context("background worker failed")?,
                call = calls.recv() => {
                    let Some((call, reply)) = call else { break worker.await.context("background worker failed")? };
                    sequence += 1;
                    let mut frame = serde_json::to_value(call)?;
                    frame["protocol"] = json!(PROTOCOL);
                    frame["requestId"] = json!(format!("background_{sequence}"));
                    let raw = self.dispatch_web_message(binding, source(binding), &frame.to_string()).await;
                    let result = self.background_reply_value(binding, &raw, &mut sequence).await;
                    let _ = reply.send(result);
                }
            }
        };
        self.web_plugins
            .lock()
            .map_err(|_| anyhow::anyhow!("plugin service unavailable"))?
            .backgrounds
            .remove(binding.runtime_id());
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
            let raw = self
                .dispatch_web_message(binding, source(binding), &frame)
                .await;
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
}
