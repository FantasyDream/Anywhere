use super::{components, visibility_script, LauncherView, LiveEvent, Screen};
use crate::host::{WebDocument, WebPackageReview};
use crate::l10n;
use gpui_kit::Context;
use gpui_kit::Entity;
use gpui_webview::WebView;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::Receiver;

pub(super) struct WebIpcRequest {
    pub handle: String,
    pub origin: String,
    pub frame: String,
}

pub(super) struct ActiveWebBinding {
    pub handle: String,
    pub origin: String,
}

#[derive(Default)]
pub(super) struct WebSurfaceState {
    pub review: Option<WebPackageReview>,
    pub invocation_id: Option<String>,
    pub document: Option<WebDocument>,
    pub view: Option<Entity<WebView>>,
    pub ready: bool,
    pub error: Option<String>,
    pub actions_open: bool,
    pub action_selected: usize,
    pub state_handle: Option<String>,
    pub state_version: u64,
    pub binding: Option<Arc<Mutex<ActiveWebBinding>>>,
    pub resume_generation: u64,
    pub resume_busy: bool,
    pub needs_resume: bool,
    pub open_generation: u64,
    pub opening: bool,
    pub unsupported_tool: Option<(String, String)>,
}

impl LauncherView {
    pub(super) fn poll_web_document(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        cx: &mut Context<Self>,
    ) {
        self.web_surface.open_generation = self.web_surface.open_generation.wrapping_add(1);
        self.web_surface.opening = true;
        self.web_surface.needs_resume = false;
        let request_id = self.web_surface.open_generation;
        let invocation_id = self.web_surface.invocation_id.clone();
        let live = self.live.clone();
        cx.spawn(async move |this, cx| {
            let mut event = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                if request_id != this.web_surface.open_generation
                    || this.screen != Screen::WebTool
                    || this.web_surface.invocation_id != invocation_id
                    || (!this.presented && !this.retained_workspace)
                {
                    return;
                }
                this.web_surface.opening = false;
                match event.take() {
                    Some(event @ LiveEvent::WebDocument(_)) => {
                        this.apply_auxiliary_event(event, cx);
                    }
                    Some(LiveEvent::Error(error)) => this.web_surface.error = Some(error),
                    _ => this.web_surface.error = Some(crate::l10n::action_failed(this.locale)),
                }
                // Core binds only while the session is interactive. A bind
                // delivered after hide therefore carries a revoked epoch;
                // retain its metadata and resume before mounting or IPC.
                this.resume_web_surface_if_needed(cx);
                cx.notify();
            });
            // Also release an undisplayed binding if the view itself was dropped.
            if let (Some(LiveEvent::WebDocument(document)), Some(live)) = (event, live) {
                let _ = live.close_web_document(document.handle);
            }
        })
        .detach();
    }

    pub(super) fn poll_web_resume(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        previous_handle: String,
        cx: &mut Context<Self>,
    ) {
        self.web_surface.resume_generation = self.web_surface.resume_generation.wrapping_add(1);
        let generation = self.web_surface.resume_generation;
        self.web_surface.resume_busy = true;
        self.web_surface.needs_resume = false;
        let live = self.live.clone();
        cx.spawn(async move |this, cx| {
            let mut event = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.web_surface.resume_generation
                    || this.screen != Screen::WebTool
                    || this
                        .web_surface
                        .document
                        .as_ref()
                        .map(|document| document.handle.as_str())
                        != Some(previous_handle.as_str())
                {
                    return;
                }
                this.web_surface.resume_busy = false;
                match event.take() {
                    Some(LiveEvent::WebDocument(document)) => {
                        if let Some(binding) = &this.web_surface.binding {
                            if let Ok(mut binding) = binding.lock() {
                                binding.handle = document.handle.clone();
                                binding.origin = document.origin.clone();
                            }
                        }
                        this.web_surface.document = Some(document);
                        this.web_surface.ready = false;
                        this.web_surface.error = None;
                        this.web_surface.state_handle = None;
                        this.web_surface.state_version = 0;
                        if this.presented && !this.web_surface.needs_resume {
                            let _ = this.evaluate_web_script(&visibility_script(true), cx);
                        }
                    }
                    Some(LiveEvent::Error(error)) => {
                        this.web_surface.error = Some(error.clone());
                        this.set_notice(error, components::NoticeKind::Error);
                    }
                    _ => this.web_surface.error = Some(l10n::action_failed(this.locale)),
                }
                this.resume_web_surface_if_needed(cx);
                cx.notify();
            });
            if let (Some(LiveEvent::WebDocument(document)), Some(live)) = (event, live) {
                let _ = live.close_web_document(document.handle);
            }
        })
        .detach();
    }

    pub(super) fn resume_web_surface_if_needed(&mut self, cx: &mut Context<Self>) {
        if self.screen != Screen::WebTool
            || !self.presented
            || !self.web_surface.needs_resume
            || self.web_surface.opening
            || self.web_surface.resume_busy
        {
            return;
        }
        if self.web_surface.document.is_some() {
            self.resume_retained_web(cx);
        } else {
            self.reload_web_tool(cx);
        }
    }
}
