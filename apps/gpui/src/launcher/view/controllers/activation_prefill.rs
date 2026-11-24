use super::*;

impl LauncherView {
    pub(super) fn begin_activation_prefill(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let epoch = self.activation_prefill.begin(enabled);
        if !enabled {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(
                    anywhere_protocol::policy::SELECTION_PREFILL_GRACE_MS,
                ))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.activation_prefill.epoch != epoch || !this.activation_prefill.enabled {
                    return;
                }
                this.activation_prefill.grace_passed = true;
                this.maybe_request_activation_prefill(cx);
            });
        })
        .detach();
    }

    pub(super) fn cancel_activation_prefill(&mut self) {
        self.activation_prefill.cancel();
    }

    fn can_apply_activation_prefill(&self) -> bool {
        self.activation_prefill.enabled
            && self.presence.presented
            && self.screen == Screen::Launcher
            && self.input_mode == InputMode::Environment
            && self.model.query.trim().is_empty()
            && !self.root_navigation_open
            && !self.model.object_actions_active
            && self.model.menu_path().is_none()
    }

    pub(super) fn maybe_request_activation_prefill(&mut self, cx: &mut Context<Self>) {
        if !self.can_apply_activation_prefill() {
            return;
        }
        let captures = self.model.context_captures().to_vec();
        if self.activation_prefill.pending {
            if self.activation_prefill.inflight_captures.as_ref() != Some(&captures) {
                self.activation_prefill.reconsider = true;
            }
            return;
        }
        if captures.is_empty() {
            return;
        }
        let allow_pending_selection = self.activation_prefill.grace_passed;
        if self.activation_prefill.last_captures.as_ref() == Some(&captures)
            && self.activation_prefill.last_allow_pending_selection == allow_pending_selection
        {
            return;
        }
        let Some(live) = self.live.clone() else {
            return;
        };
        let selected_enabled = self.preferences.capture_selected_text;
        let clipboard_enabled = self.preferences.capture_clipboard;
        let epoch = self.activation_prefill.epoch;
        let window_handle = self.window_handle;
        self.activation_prefill.pending = true;
        self.activation_prefill.reconsider = false;
        self.activation_prefill.inflight_captures = Some(captures.clone());
        self.activation_prefill.last_captures = Some(captures.clone());
        self.activation_prefill.last_allow_pending_selection = allow_pending_selection;
        let mut receiver = live.choose_activation_input(
            captures,
            unix_time_ms(),
            selected_enabled,
            clipboard_enabled,
            allow_pending_selection,
        );
        cx.spawn(async move |this, cx| {
            let Some(event) = receiver.recv().await else {
                return;
            };
            let offer = this
                .update(cx, |this, cx| {
                    if this.activation_prefill.epoch != epoch || !this.activation_prefill.enabled {
                        return None;
                    }
                    this.activation_prefill.pending = false;
                    let current = this.model.context_captures().to_vec();
                    let captures_changed = this
                        .activation_prefill
                        .inflight_captures
                        .as_ref()
                        .is_some_and(|inflight| inflight != &current);
                    this.activation_prefill.inflight_captures = None;
                    if captures_changed {
                        this.activation_prefill.reconsider = true;
                    }
                    if this.activation_prefill.reconsider {
                        this.maybe_request_activation_prefill(cx);
                        return None;
                    }
                    match event {
                        LiveEvent::ActivationInput(Some(offer))
                            if this.can_apply_activation_prefill() =>
                        {
                            Some(offer)
                        }
                        LiveEvent::ActivationInput(_) => {
                            this.maybe_request_activation_prefill(cx);
                            None
                        }
                        LiveEvent::Error(error) => {
                            tracing::debug!(%error, "activation_prefill_failed");
                            None
                        }
                        _ => None,
                    }
                })
                .ok()
                .flatten();
            let Some(offer) = offer else {
                return;
            };
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if !this.can_apply_activation_prefill()
                        || offer
                            .valid_until_ms
                            .is_some_and(|valid_until| unix_time_ms() > valid_until)
                    {
                        this.maybe_request_activation_prefill(cx);
                        return;
                    }
                    this.activation_prefill.enabled = false;
                    this.activation_prefill.reconsider = false;
                    this.activation_prefill.last_captures = None;
                    this.activation_prefill.last_allow_pending_selection = false;
                    this.apply_activation_prefill(&offer.text, window, cx);
                });
            });
        })
        .detach();
    }

    pub(super) fn apply_activation_prefill(
        &mut self,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous_mode = self.input_mode;
        self.input_mode = InputMode::Content;
        self.root_navigation_mode = InputMode::Search;
        self.root_navigation_open = false;
        self.begin_scope_selection_for_mode(InputMode::Content);
        self.model.hide_object_actions();
        self.model.select_scope(None);
        self.model.set_query(text.to_owned());
        self._editor_sync_guard.set(true);
        set_input_value(&self.input, text.to_owned(), window, cx);
        set_textarea_value(&self.content_input, text.to_owned(), window, cx);
        let editor_sync_guard = self._editor_sync_guard.clone();
        cx.defer(move |_| editor_sync_guard.set(false));
        self.sync_mode_editor(previous_mode, InputMode::Content, window, cx);
        self.notice = None;
        self.focus_launcher_input(window, cx);
        self.request_live_update(cx);
        self.resize_window_for_screen(window, cx);
        cx.notify();
    }
}

fn unix_time_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
