use super::*;
use crate::host::WebPackageOperation;
use std::sync::Mutex;

impl LauncherView {
    pub(super) fn close_web_surface(&mut self, cx: &mut Context<Self>) {
        tracing::debug!(
            screen = ?self.screen,
            has_view = self.web_surface.view.is_some(),
            document = self.web_surface.document.as_ref().map(|document| document.handle.as_str()),
            "web_surface_closing"
        );
        if let Some(view) = self.web_surface.view.as_ref() {
            let _ = view
                .read(cx)
                .raw()
                .evaluate_script(&visibility_script(false));
        }
        if let (Some(live), Some(document)) =
            (self.live.clone(), self.web_surface.document.as_ref())
        {
            let _ = live.close_web_document(document.handle.clone());
        }
        self.web_surface.document = None;
        self.web_surface.view = None;
        self.web_surface.ready = false;
        self.web_surface.error = None;
        self.web_surface.invocation_id = None;
        self.web_surface.actions_open = false;
        self.web_surface.action_selected = 0;
        self.web_surface.state_handle = None;
        self.web_surface.state_version = 0;
        self.web_surface.binding = None;
        self.web_surface.resume_generation = self.web_surface.resume_generation.wrapping_add(1);
        self.web_surface.resume_busy = false;
        self.web_surface.needs_resume = false;
        self.web_surface.open_generation = self.web_surface.open_generation.wrapping_add(1);
        self.web_surface.opening = false;
    }

    pub(super) fn toggle_web_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.web_surface.actions_open = !self.web_surface.actions_open;
        if let Some(view) = &self.web_surface.view {
            if let Err(error) = view.read(cx).set_visible(!self.web_surface.actions_open) {
                tracing::warn!(%error, "web_tool_visibility_failed");
            }
        }
        self.surface_focus_handle.focus(window, cx);
        self.web_surface.action_selected = 0;
        cx.notify();
    }

    pub(super) fn reload_web_tool(&mut self, cx: &mut Context<Self>) {
        if self.web_surface.opening || self.web_surface.resume_busy {
            return;
        }
        let Some(invocation_id) = self.web_surface.invocation_id.clone() else {
            return;
        };
        // Binding the replacement revokes the old document. Closing first would
        // delete the invocation that the replacement needs to bind.
        if let Some(live) = self.live.clone() {
            let receiver = live.open_web_document(invocation_id, None);
            self.poll_web_document(receiver, cx);
        }
        cx.notify();
    }

    pub(super) fn execute_web_action(
        &mut self,
        action: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_web_actions(window, cx);
        match action {
            0 => self.reload_web_tool(cx),
            1 => self.go_back(window, cx),
            _ => {}
        }
    }

    pub(super) fn resume_retained_web(&mut self, cx: &mut Context<Self>) {
        let (Some(live), Some(document)) = (self.live.clone(), self.web_surface.document.clone())
        else {
            return;
        };
        let receiver = live.resume_web_document(document.handle.clone());
        self.poll_web_resume(receiver, document.handle, cx);
    }

    pub(super) fn toggle_selected_web_package(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(package)) =
            (self.live.clone(), self.web.packages.get(self.web.selected))
        {
            let id = package.id.clone();
            let operation = if package.enabled {
                WebPackageOperation::Disable
            } else {
                WebPackageOperation::Enable
            };
            let receiver = live.change_web_package(id, operation);
            self.poll(receiver, self.live_state.generation, cx);
        }
    }

    pub(super) fn cancel_web_package_review(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(review) = self.web_surface.review.take() else {
            return false;
        };
        if let Some(live) = &self.live {
            let receiver = live.cancel_web_review(review.token);
            self.poll(receiver, self.live_state.generation, cx);
        }
        self.surface_focus_handle.focus(window, cx);
        cx.notify();
        true
    }

    pub(super) fn change_web_package_operation(
        &mut self,
        id: String,
        operation: WebPackageOperation,
        cx: &mut Context<Self>,
    ) {
        if let Some(live) = self.live.clone() {
            let receiver = live.change_web_package(id, operation);
            self.poll(receiver, self.live_state.generation, cx);
        }
    }

    pub(super) fn ensure_web_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.presence.presented
            || self.web_surface.opening
            || self.web_surface.resume_busy
            || self.web_surface.needs_resume
            || self.web_surface.view.is_some()
        {
            return;
        }
        let (Some(document), Some(live)) = (self.web_surface.document.clone(), self.live.clone())
        else {
            return;
        };
        let policy = match WebDocumentPolicy::new(document.origin.clone(), document.entry.clone()) {
            Ok(policy) => policy,
            Err(error) => {
                self.web_surface.error = Some(l10n::web_document_failed(
                    self.locale,
                    &format!("{error:#}"),
                ));
                return;
            }
        };
        let runtime = live.web_runtime();
        let protocol_policy = policy.clone();
        let entry_url = policy.url();
        let protocol_runtime = runtime;
        let nav_policy = policy.clone();
        let ipc_policy = policy;
        let active_binding = Arc::new(Mutex::new(ActiveWebBinding {
            handle: document.handle.clone(),
            origin: document.origin.clone(),
        }));
        let protocol_binding = active_binding.clone();
        let ipc_binding = active_binding.clone();
        let (ipc_sender, ipc_receiver) = tokio::sync::mpsc::channel(WEB_IPC_QUEUE_CAPACITY);
        let language = match self.locale {
            Locale::En => "en",
            Locale::Zh => "zh",
        };
        let builder = crate::platform::configure_webview_builder(wry::WebViewBuilder::new())
            .with_transparent(true)
            .with_incognito(true)
            .with_devtools(cfg!(debug_assertions))
            .with_autoplay(false)
            .with_initialization_script(bootstrap_script(language))
            .with_navigation_handler({
                move |candidate| nav_policy.accepts_navigation(&candidate)
            })
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .with_download_started_handler(|_, _| false)
            .with_custom_protocol("anywhere-plugin".to_owned(), move |_, request| {
                let Some(path) = protocol_policy.resource_path(&request.uri().to_string()) else {
                    return plain_response(404, "Not found");
                };
                let handle = protocol_binding
                    .lock()
                    .map(|binding| binding.handle.clone())
                    .unwrap_or_default();
                match protocol_runtime.web_resource(handle.as_str(), &path) {
                    Ok(bytes) => protocol_policy.response(&path, bytes),
                    Err(_) => plain_response(404, "Not found"),
                }
            })
            .with_ipc_handler(move |request| {
                let source = request.uri().to_string();
                let frame = request.body();
                if frame.len() > WEB_FRAME_BYTES {
                    tracing::warn!(
                        source = %source,
                        frame_bytes = frame.len(),
                        "web_ipc_frame_rejected"
                    );
                    return;
                }
                if !ipc_policy.accepts_source(&source) {
                    tracing::warn!(source = %source, "web_ipc_source_rejected");
                    return;
                }
                let Ok(binding) = ipc_binding.lock() else {
                    tracing::warn!("web_ipc_binding_unavailable");
                    return;
                };
                if ipc_sender
                    .try_send(WebIpcRequest {
                        handle: binding.handle.clone(),
                        origin: binding.origin.clone(),
                        frame: frame.clone(),
                    })
                    .is_err()
                {
                    tracing::warn!("web_ipc_queue_full");
                }
            })
            .with_url(entry_url);
        let web_view = match gpui_webview::WebView::build(builder, window, cx) {
            Ok(web_view) => web_view,
            Err(error) => {
                self.web_surface.error =
                    Some(l10n::web_view_start_failed(self.locale, &error.to_string()));
                return;
            }
        };
        let web_theme = if theme::surface_is_light() {
            wry::Theme::Light
        } else {
            wry::Theme::Dark
        };
        if let Err(error) = web_view.set_theme(web_theme) {
            self.web_surface.error = Some(l10n::web_theme_failed(self.locale, &error.to_string()));
        }
        tracing::debug!(
            document = %document.handle,
            origin = %document.origin,
            entry = %document.entry,
            "web_view_created"
        );
        self.web_surface.view = Some(cx.new(|_| web_view));
        self.web_surface.binding = Some(active_binding);
        self.platform_appearance_changed(cx);
        self.poll_web_ipc(ipc_receiver, cx);
    }

    pub(super) fn evaluate_web_script(
        &self,
        script: &str,
        cx: &Context<Self>,
    ) -> anyhow::Result<()> {
        let web_view = self
            .web_surface
            .view
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Web view is not mounted"))?;
        web_view
            .read(cx)
            .raw()
            .evaluate_script(script)
            .map_err(anyhow::Error::from)
    }

    pub(super) fn poll_web_ipc(
        &mut self,
        mut receiver: Receiver<WebIpcRequest>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Some(request) = receiver.recv().await {
                let reply = this.update(cx, |this, cx| {
                    tracing::debug!(
                        document = %request.handle,
                        origin = %request.origin,
                        frame_bytes = request.frame.len(),
                        current_document = this
                            .web_surface
                            .document
                            .as_ref()
                            .map(|document| document.handle.as_str()),
                        "web_ipc_request_received"
                    );
                    if this
                        .web_surface
                        .document
                        .as_ref()
                        .map(|document| document.handle.as_str())
                        != Some(request.handle.as_str())
                    {
                        return None;
                    }
                    if crate::web_tool::is_host_escape_frame(&request.frame) {
                        tracing::debug!(
                            document = %request.handle,
                            "web_view_escape_requested"
                        );
                        cx.emit(LauncherEvent::WebViewEscapeRequested);
                        return Some(());
                    }
                    let live = this.live.clone()?;
                    let generation = this.live_state.generation;
                    let receiver = live.dispatch_web_frame(
                        request.handle.clone(),
                        request.origin.clone(),
                        true,
                        request.frame.clone(),
                    );
                    this.poll_web_reply(receiver, request.handle, request.frame, generation, cx);
                    Some(())
                });
                if reply.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    pub(super) fn poll_web_reply(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        handle: String,
        request: String,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Some(event) = receiver.recv().await {
                match event {
                    LiveEvent::WebFrameReply {
                        handle: event_handle,
                        frame,
                    } => {
                        let request_value =
                            serde_json::from_str::<serde_json::Value>(&request).ok();
                        let reply_value = serde_json::from_str::<serde_json::Value>(&frame).ok();
                        let acknowledgement = request_value
                            .as_ref()
                            .zip(reply_value.as_ref())
                            .and_then(|(request, response)| {
                                accepted_web_acknowledgement_values(request, response)
                            });
                        if tracing::enabled!(tracing::Level::DEBUG) {
                            let request_method = request_value.as_ref().and_then(|value| {
                                value
                                    .get("method")
                                    .and_then(|method| method.as_str())
                                    .map(str::to_owned)
                            });
                            let reply = &reply_value;
                            let reply_error = reply
                                .as_ref()
                                .and_then(|value| value.get("error"))
                                .and_then(|error| error.get("code"))
                                .and_then(|code| code.as_str());
                            let reply_result_kind = reply
                                .as_ref()
                                .and_then(|value| value.get("result"))
                                .map(|result| match result {
                                    serde_json::Value::Null => "null",
                                    serde_json::Value::Bool(_) => "bool",
                                    serde_json::Value::Number(_) => "number",
                                    serde_json::Value::String(_) => "string",
                                    serde_json::Value::Array(_) => "array",
                                    serde_json::Value::Object(_) => "object",
                                })
                                .unwrap_or("missing");
                            tracing::debug!(
                                document = %event_handle,
                                method = request_method.as_deref().unwrap_or("unknown"),
                                reply_bytes = frame.len(),
                                reply_error = reply_error.unwrap_or("none"),
                                reply_result = reply_result_kind,
                                "web_ipc_reply_received"
                            );
                        }
                        let _ = this.update(cx, |this, cx| {
                            if generation != this.live_state.generation
                                || event_handle != handle
                                || this
                                    .web_surface
                                    .document
                                    .as_ref()
                                    .map(|document| document.handle.as_str())
                                    != Some(handle.as_str())
                            {
                                return;
                            }
                            if let Err(error) = this.evaluate_web_script(&reply_script(&frame), cx)
                            {
                                tracing::warn!(
                                    %error,
                                    document = %event_handle,
                                    "web_ipc_reply_delivery_failed"
                                );
                                this.web_surface.error =
                                    Some(l10n::web_tool_closed_unexpectedly(this.locale));
                            }
                            tracing::debug!(
                                document = %event_handle,
                                method = acknowledgement.as_deref().unwrap_or("other"),
                                acknowledgement = acknowledgement.as_deref().unwrap_or("none"),
                                "web_ipc_reply_applied"
                            );
                            match acknowledgement.as_deref() {
                                Some("lifecycle.ready") => {
                                    this.web_surface.ready = true;
                                    let _ = this.evaluate_web_script(
                                        &crate::web_tool::web_theme_script(
                                            theme::palette(),
                                            theme::fixed(),
                                            theme::surface_is_light(),
                                        ),
                                        cx,
                                    );
                                    if this
                                        .evaluate_web_script(
                                            &canvas_script(
                                                !crate::platform::uses_opaque_fallback(),
                                            ),
                                            cx,
                                        )
                                        .is_err()
                                    {
                                        this.web_surface.error =
                                            Some(l10n::web_tool_background_failed(this.locale));
                                    }
                                }
                                Some("state.subscribe")
                                    if this.web_surface.state_handle.as_deref()
                                        != Some(handle.as_str()) =>
                                {
                                    this.web_surface.state_handle = Some(handle.clone());
                                    this.web_surface.state_version = 0;
                                    this.follow_web_state(handle.clone(), 0, generation, cx);
                                }
                                _ => {}
                            }
                            cx.notify();
                        });
                        break;
                    }
                    LiveEvent::Error(error) => {
                        tracing::warn!(
                            %error,
                            document = %handle,
                            "web_ipc_dispatch_failed"
                        );
                        let _ = this.update(cx, |this, cx| {
                            if generation == this.live_state.generation
                                && event_handle_matches(this, &handle)
                            {
                                this.web_surface.error = Some(error);
                                this.web_surface.state_handle = None;
                                cx.notify();
                            }
                        });
                        break;
                    }
                    LiveEvent::Ignored => break,
                    _ => {}
                }
            }
        })
        .detach();
    }

    pub(super) fn follow_web_state(
        &mut self,
        handle: String,
        after: u64,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(live) = self.live.clone() else {
            return;
        };
        let mut receiver = live.wait_web_state(handle.clone(), after);
        cx.spawn(async move |this, cx| loop {
            match receiver.recv().await {
                Some(LiveEvent::WebState {
                    handle: event_handle,
                    frame,
                }) => {
                    let next = this.update(cx, |this, cx| {
                        if generation != this.live_state.generation
                            || event_handle != handle
                            || !event_handle_matches(this, &handle)
                        {
                            return None;
                        }
                        let version = web_state_version(&frame).unwrap_or(after);
                        if let Err(error) = this.evaluate_web_script(&state_script(&frame), cx) {
                            this.web_surface.error = Some(error.to_string());
                            this.web_surface.state_handle = None;
                            cx.notify();
                            return None;
                        }
                        this.web_surface.state_version = version;
                        cx.notify();
                        Some(version)
                    });
                    match next {
                        Ok(Some(version)) => {
                            let continuation = this.update(cx, |this, cx| {
                                if generation == this.live_state.generation
                                    && event_handle_matches(this, &handle)
                                {
                                    this.follow_web_state(handle.clone(), version, generation, cx);
                                }
                            });
                            if continuation.is_err() {
                                break;
                            }
                        }
                        _ => break,
                    }
                    break;
                }
                Some(LiveEvent::Ignored) => {
                    let restart = this.update(cx, |this, cx| {
                        if generation == this.live_state.generation
                            && event_handle_matches(this, &handle)
                        {
                            this.follow_web_state(handle.clone(), after, generation, cx);
                            true
                        } else {
                            false
                        }
                    });
                    if !matches!(restart, Ok(true)) {
                        break;
                    }
                    break;
                }
                Some(LiveEvent::Error(error)) => {
                    let _ = this.update(cx, |this, cx| {
                        if generation == this.live_state.generation
                            && event_handle_matches(this, &handle)
                        {
                            this.web_surface.error = Some(error);
                            this.web_surface.state_handle = None;
                            cx.notify();
                        }
                    });
                    break;
                }
                Some(_) => continue,
                None => break,
            }
        })
        .detach();
    }

    pub(in crate::launcher::view) fn handle_web_overlay_keys(
        &mut self,
        input: &crate::launcher::view::input::KeyInput<'_>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> crate::launcher::view::input::KeyFlow {
        use crate::launcher::view::input::KeyFlow;
        if self.screen != Screen::WebTool
            || !self.web_surface.actions_open
            || input.modifiers.modified
        {
            return KeyFlow::Continue;
        }
        match input.key {
            "down" | "up" => {
                self.web_surface.action_selected = 1 - self.web_surface.action_selected;
                cx.notify();
            }
            "enter" | "return" => {
                self.execute_web_action(self.web_surface.action_selected, window, cx);
            }
            "escape" => {
                self.toggle_web_actions(window, cx);
            }
            _ => {}
        }
        KeyFlow::Handled
    }

    pub(in crate::launcher::view) fn handle_web_packages_key(
        &mut self,
        input: &crate::launcher::view::input::KeyInput<'_>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modified = input.modifiers.modified;
        match input.key {
            "escape" => self.go_back(window, cx),
            _ if self.web_surface.review.is_some() => {}
            "down" if !modified => {
                self.web.select_next();
                self.reveal_result_row(self.web.selected);
                cx.notify();
            }
            "up" if !modified => {
                self.web.select_previous();
                self.reveal_result_row(self.web.selected);
                cx.notify();
            }
            "enter" | "return" => self.toggle_selected_web_package(cx),
            _ => {}
        }
    }
}
