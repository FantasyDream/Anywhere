//! Native window commands and consent requests are separate from JavaScript reply frames.
use super::{
    broker::{error, Source},
    invocation::{BackgroundState, RuntimeLifecycle},
    rpc_state::{RequestControl, RequestTicket},
    WebDocumentBinding, WebMessageSource,
};
use crate::{
    features::preview::{PreviewResource, PreviewStore},
    Application,
};
use anywhere_protocol::web_bridge::{WebErrorCode, WebResult};
use serde_json::Value;
use std::{future::Future, sync::Arc};
use tokio::sync::{mpsc, oneshot, watch, Mutex};
use url::Url;

/// A window action qualified by the binding that requested it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebHostAction {
    pub handle: String,
    pub document_epoch: u64,
    pub request_id: String,
    pub kind: WebHostActionKind,
}

/// Native actions accepted from the current WebView only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebHostActionKind {
    Back,
    Close,
}

/// The wire reply and the trusted, optional frontend action.
#[derive(Debug)]
pub struct WebDispatchOutcome {
    pub reply_frame: String,
    pub host_action: Option<WebHostAction>,
}

impl From<String> for WebDispatchOutcome {
    fn from(reply_frame: String) -> Self {
        Self {
            reply_frame,
            host_action: None,
        }
    }
}

/// A request for user consent, scoped to the current native window.
pub struct HostInteraction {
    pub request_id: String,
    pub document_handle: Option<String>,
    pub kind: HostInteractionKind,
    control: Arc<RequestControl>,
    response: oneshot::Sender<Result<(), WebErrorCode>>,
    completion: watch::Receiver<()>,
}

/// Frontend interactions originating in the Rust broker.
pub enum HostInteractionKind {
    OpenUrl {
        origin: String,
        destination: String,
        query_bytes: usize,
    },
    Preview(PreviewResource),
}

pub(super) struct PendingPreview {
    store: Arc<std::sync::Mutex<PreviewStore>>,
    resource: Option<PreviewResource>,
}

impl PendingPreview {
    pub(super) fn new(
        store: Arc<std::sync::Mutex<PreviewStore>>,
        resource: PreviewResource,
    ) -> Self {
        Self {
            store,
            resource: Some(resource),
        }
    }

    fn resource(&self) -> WebResult<&PreviewResource> {
        self.resource
            .as_ref()
            .ok_or_else(|| error(WebErrorCode::Expired))
    }

    fn claim(mut self) -> WebResult<()> {
        let resource = self.resource()?;
        crate::core::sync::lock_or_deny(&self.store, "previews")
            .and_then(|mut store| store.retain(&resource.id))
            .map_err(|_| error(WebErrorCode::Expired))?;
        self.resource = None;
        Ok(())
    }
}

impl Drop for PendingPreview {
    fn drop(&mut self) {
        if let Some(resource) = self.resource.take() {
            crate::core::sync::lock_for_revoke(&self.store, "previews").release(&resource.id);
        }
    }
}

impl HostInteraction {
    /// Return true only while the originating request can still accept input.
    pub fn is_current(&self) -> bool {
        self.control.check().is_ok() && !self.response.is_closed()
    }

    /// Wait until the originating request has expired.
    pub fn expiry(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut completion = self.completion.clone();
        async move {
            let _ = completion.changed().await;
        }
    }

    /// Reply once with the user's decision.
    pub fn respond(self, response: Result<(), WebErrorCode>) {
        if self.is_current() {
            let _ = self.response.send(response);
        }
    }
}

#[derive(Default)]
pub(super) struct InteractionPort {
    sender: Option<mpsc::Sender<HostInteraction>>,
    gate: Arc<Mutex<()>>,
}

impl Application {
    /// Connect one interaction queue to the frontend for this application window.
    pub fn web_host_interactions(&self) -> anyhow::Result<mpsc::Receiver<HostInteraction>> {
        let mut web = crate::core::sync::lock_or_deny(&self.web_plugins, "web_plugins")?;
        anyhow::ensure!(
            web.interactions
                .sender
                .as_ref()
                .is_none_or(mpsc::Sender::is_closed),
            "Web host interaction receiver is already attached"
        );
        let (sender, receiver) = mpsc::channel(1);
        web.interactions.sender = Some(sender);
        Ok(receiver)
    }

    pub(super) async fn show_web_preview(
        &self,
        binding: &WebDocumentBinding,
        source: &Source,
        request: &RequestTicket,
        preview: PendingPreview,
    ) -> WebResult<Value> {
        self.request_host_interaction(
            binding,
            source,
            request,
            HostInteractionKind::Preview(preview.resource()?.clone()),
        )
        .await?;
        request.control.check()?;
        preview.claim()?;
        Ok(Value::Null)
    }

    pub(super) async fn confirm_web_url(
        &self,
        binding: &WebDocumentBinding,
        source: &Source,
        request: &RequestTicket,
        url: Url,
    ) -> WebResult<Value> {
        let background = {
            let mut web = crate::core::sync::lock_or_deny(&self.web_plugins, "web_plugins")
                .map_err(|_| error(WebErrorCode::HostFailure))?;
            web.is_background(binding, source.get())
                .map_err(|_| error(WebErrorCode::UntrustedDocument))?
        };
        let decision = self
            .request_host_interaction(
                binding,
                source,
                request,
                HostInteractionKind::OpenUrl {
                    origin: url.origin().ascii_serialization(),
                    destination: url.as_str().to_owned(),
                    query_bytes: url.query().map_or(0, str::len),
                },
            )
            .await;
        if decision.is_err() && background {
            if let Ok(mut web) = crate::core::sync::lock_or_deny(&self.web_plugins, "web_plugins") {
                let _ =
                    web.change_background_state(binding, source.get(), BackgroundState::Running);
            }
        }
        decision?;
        let binding = binding.clone();
        let source = source.clone();
        let control = request.control.clone();
        let web = self.web_plugins.clone();
        let platform = self.platform.clone();
        tokio::task::spawn_blocking(move || {
            control.check()?;
            {
                let mut web = crate::core::sync::lock_or_deny(&web, "web_plugins")
                    .map_err(|_| error(WebErrorCode::HostFailure))?;
                // The binding also qualifies the installed package, its digest
                // and grants. Revalidate at the commit point after consent.
                web.authorize_url(&binding, source.get(), url.as_str())?;
                if background {
                    web.change_background_state(
                        &binding,
                        source.get(),
                        BackgroundState::HandoffCompleted,
                    )?;
                }
            }
            let result = platform.open_url(url.as_str());
            if result.is_err() && background {
                if let Ok(mut web) = crate::core::sync::lock_or_deny(&web, "web_plugins") {
                    web.fail_background_handoff(&binding);
                }
            }
            result
                .map(|()| Value::Null)
                .map_err(|_| error(WebErrorCode::HostFailure))
        })
        .await
        .map_err(|_| error(WebErrorCode::HostFailure))?
    }

    async fn request_host_interaction(
        &self,
        binding: &WebDocumentBinding,
        source: &Source,
        request: &RequestTicket,
        kind: HostInteractionKind,
    ) -> WebResult<()> {
        request.control.check()?;
        let (sender, _guard, background) = {
            let mut web = crate::core::sync::lock_or_deny(&self.web_plugins, "web_plugins")
                .map_err(|_| error(WebErrorCode::HostFailure))?;
            let background = web
                .is_background(binding, source.get())
                .map_err(|_| error(WebErrorCode::UntrustedDocument))?;
            let guard = web
                .interactions
                .gate
                .clone()
                .try_lock_owned()
                .map_err(|_| error(WebErrorCode::Busy))?;
            let sender = web
                .interactions
                .sender
                .clone()
                .ok_or_else(|| error(WebErrorCode::UnsupportedMethod))?;
            if background {
                web.change_background_state(
                    binding,
                    source.get(),
                    BackgroundState::AwaitingInteraction,
                )?;
            }
            (sender, guard, background)
        };
        let (response, decision) = oneshot::channel();
        let (_complete, completion) = watch::channel(());
        sender
            .try_send(HostInteraction {
                request_id: request.id.clone(),
                document_handle: (!background).then(|| binding.runtime_id().to_owned()),
                kind,
                control: request.control.clone(),
                response,
                completion,
            })
            .map_err(|_| error(WebErrorCode::Inactive))?;
        let decision = tokio::select! {
            biased;
            _ = request.control.cancelled() => Err(WebErrorCode::Cancelled),
            _ = tokio::time::sleep_until(request.control.deadline) => Err(WebErrorCode::Timeout),
            reply = decision => reply.unwrap_or(Err(WebErrorCode::UserCancelled)),
        };
        decision.map_err(error)
    }
}

impl super::WebPluginService {
    pub(super) fn change_background_state(
        &mut self,
        binding: &WebDocumentBinding,
        source: WebMessageSource<'_>,
        next: BackgroundState,
    ) -> WebResult<()> {
        self.authorize(binding, source)
            .map_err(|_| error(WebErrorCode::UntrustedDocument))?;
        let invocation = self
            .invocations
            .get_mut(&binding.invocation_id)
            .ok_or_else(|| error(WebErrorCode::Inactive))?;
        let RuntimeLifecycle::Background { state } = &mut invocation.lifecycle else {
            return Err(error(WebErrorCode::PermissionDenied));
        };
        *state = next;
        Ok(())
    }

    pub(super) fn fail_background_handoff(&mut self, binding: &WebDocumentBinding) {
        if let Some(invocation) = self.invocations.get_mut(&binding.invocation_id) {
            if invocation.is_draining() {
                self.close_document(binding);
            } else if let RuntimeLifecycle::Background { state } = &mut invocation.lifecycle {
                *state = BackgroundState::Running;
            }
        }
    }
}
