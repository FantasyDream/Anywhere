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
use anywhere_protocol::{
    web_bridge::{WebErrorCode, WebResult},
    web_plugin::WebPermission,
};
use serde_json::Value;
use std::{
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
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
    preview_claim: Option<PreviewClaim>,
}

struct PreviewClaim {
    store: Arc<std::sync::Mutex<PreviewStore>>,
    id: String,
    claimed: Arc<AtomicBool>,
    web: Arc<std::sync::Mutex<super::WebPluginService>>,
    binding: WebDocumentBinding,
    source: Source,
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
    resource: PreviewResource,
    claimed: Arc<AtomicBool>,
}

impl PendingPreview {
    pub(super) fn new(
        store: Arc<std::sync::Mutex<PreviewStore>>,
        resource: PreviewResource,
    ) -> Self {
        Self {
            store,
            resource,
            claimed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn claim_handle(
        &self,
        web: Arc<std::sync::Mutex<super::WebPluginService>>,
        binding: WebDocumentBinding,
        source: Source,
    ) -> PreviewClaim {
        PreviewClaim {
            store: self.store.clone(),
            id: self.resource.id.clone(),
            claimed: self.claimed.clone(),
            web,
            binding,
            source,
        }
    }
}

impl Drop for PendingPreview {
    fn drop(&mut self) {
        if !self.claimed.load(Ordering::Acquire) {
            crate::core::sync::lock_for_revoke(&self.store, "previews").release(&self.resource.id);
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

    /// Reply once with the user's decision. Returns `true` only when the host
    /// accepted the reply (and atomically retained any preview lease).
    pub fn respond(self, response: Result<(), WebErrorCode>) -> bool {
        if !self.is_current() {
            return false;
        }
        if response.is_ok() {
            if let Some(claim) = self.preview_claim {
                let allowed = crate::core::sync::lock_or_deny(&claim.web, "web_plugins")
                    .ok()
                    .and_then(|mut web| {
                        web.require(
                            &claim.binding,
                            claim.source.get(),
                            WebPermission::PreviewOpen {},
                        )
                        .ok()
                    })
                    .is_some();
                if !allowed {
                    let _ = self.response.send(Err(WebErrorCode::PermissionDenied));
                    return false;
                }
                let retained = crate::core::sync::lock_or_deny(&claim.store, "previews")
                    .and_then(|mut store| store.retain(&claim.id))
                    .is_ok();
                if !retained {
                    let _ = self.response.send(Err(WebErrorCode::Expired));
                    return false;
                }
                claim.claimed.store(true, Ordering::Release);
                if self.response.send(Ok(())).is_err() {
                    crate::core::sync::lock_for_revoke(&claim.store, "previews").release(&claim.id);
                    return false;
                }
                return true;
            }
        }
        self.response.send(response).is_ok()
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
        let claim = preview.claim_handle(self.web_plugins.clone(), binding.clone(), source.clone());
        self.request_host_interaction(
            binding,
            source,
            request,
            HostInteractionKind::Preview(preview.resource.clone()),
            Some(claim),
        )
        .await?;
        if !preview.claimed.load(Ordering::Acquire) {
            return Err(error(WebErrorCode::Expired));
        }
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
                None,
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
        let request_id = request.id.clone();
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
                        BackgroundState::HandoffCompleted {
                            request_id: Some(request_id),
                        },
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
        preview_claim: Option<PreviewClaim>,
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
        let interaction = HostInteraction {
            request_id: request.id.clone(),
            document_handle: (!background).then(|| binding.runtime_id().to_owned()),
            kind,
            control: request.control.clone(),
            response,
            completion,
            preview_claim,
        };
        let sent = tokio::select! {
            biased;
            _ = request.control.cancelled() => return Err(error(WebErrorCode::Cancelled)),
            _ = tokio::time::sleep_until(request.control.deadline) => return Err(error(WebErrorCode::Timeout)),
            result = sender.send(interaction) => result,
        };
        if sent.is_err() {
            return Err(error(WebErrorCode::Inactive));
        }
        let decision = tokio::select! {
            biased;
            _ = request.control.cancelled() => Err(WebErrorCode::Cancelled),
            reply = decision => reply.unwrap_or(Err(WebErrorCode::UserCancelled)),
            _ = tokio::time::sleep_until(request.control.deadline) => Err(WebErrorCode::Timeout),
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
            if invocation.document_epoch != binding.document_epoch() {
                return;
            }
            if invocation.is_draining() {
                self.close_document(binding);
            } else if let RuntimeLifecycle::Background { state } = &mut invocation.lifecycle {
                *state = BackgroundState::Running;
            }
        }
    }

    pub(super) fn clear_handoff_request(&mut self, binding: &WebDocumentBinding, request_id: &str) {
        if let Some(invocation) = self.invocations.get_mut(&binding.invocation_id) {
            if invocation.document_epoch == binding.document_epoch() {
                if let RuntimeLifecycle::Background {
                    state: BackgroundState::HandoffCompleted { request_id: active },
                } = &mut invocation.lifecycle
                {
                    if active.as_deref() == Some(request_id) {
                        *active = None;
                    }
                }
            }
        }
    }
}
