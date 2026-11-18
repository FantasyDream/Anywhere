use super::*;

impl LauncherView {
    pub(super) fn begin_stream(&mut self, receiver: Receiver<LiveEvent>, cx: &mut Context<Self>) {
        let (generation, cancelled) = self.live_state.begin_stream();
        self.poll_with_cancel(receiver, generation, Some(cancelled), cx);
    }

    pub(super) fn cancel_stream(&mut self) {
        self.live_state.cancel_stream();
    }

    pub(super) fn poll(
        &mut self,
        receiver: Receiver<LiveEvent>,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        self.poll_with_cancel(receiver, generation, None, cx);
    }

    pub(super) fn poll_with_cancel(
        &mut self,
        receiver: Receiver<LiveEvent>,
        generation: u64,
        cancelled: Option<oneshot::Receiver<()>>,
        cx: &mut Context<Self>,
    ) {
        self.poll_events(receiver, generation, true, cancelled, cx);
    }

    #[cfg(test)]
    pub(super) fn poll_follow(
        &mut self,
        receiver: Receiver<LiveEvent>,
        generation: u64,
        cancelled: Option<oneshot::Receiver<()>>,
        cx: &mut Context<Self>,
    ) {
        self.poll_events(receiver, generation, false, cancelled, cx);
    }

    pub(super) fn poll_events(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        generation: u64,
        mut request_reply: bool,
        mut cancelled: Option<oneshot::Receiver<()>>,
        cx: &mut Context<Self>,
    ) {
        let window_handle = self.window_handle;
        cx.spawn(async move |this, cx| loop {
            let event = if let Some(cancel) = cancelled.as_mut() {
                tokio::select! {
                    _ = cancel => break,
                    event = receiver.recv() => event,
                }
            } else {
                receiver.recv().await
            };
            match event {
                Some(LiveEvent::SharedState(state)) => {
                    let outcome = this
                        .update(cx, |this, cx| {
                            if generation != this.live_state.generation
                                || this.live_state.session_id != Some(state.session_id)
                            {
                                return (false, None);
                            }
                            let accepted =
                                this.live_state.snapshot_gate.as_mut().is_some_and(|gate| {
                                    if request_reply {
                                        gate.accept_request(&state)
                                    } else {
                                        gate.accept_update(&state)
                                    }
                                });
                            if accepted {
                                let query_layout_ready =
                                    std::mem::take(&mut this.live_state.awaiting_query_layout);
                                let applied = this.apply_shared_state(state, cx);
                                this.request_application_icons(cx);
                                if query_layout_ready
                                    || applied.rows_changed
                                    || applied.scopes_changed
                                {
                                    this.resize_current_window(cx);
                                }
                                cx.notify();
                            } else if request_reply
                                && std::mem::take(&mut this.live_state.awaiting_query_layout)
                            {
                                this.resize_current_window(cx);
                            }
                            let follow = if request_reply && accepted {
                                this.live.as_ref().map(|live| {
                                    live.follow_snapshots(
                                        this.live_state
                                            .snapshot_gate
                                            .as_ref()
                                            .map_or(0, SnapshotGate::cursor),
                                    )
                                })
                            } else {
                                None
                            };
                            (!request_reply || accepted, follow)
                        })
                        .unwrap_or((false, None));
                    if !outcome.0 {
                        break;
                    }
                    if request_reply {
                        let Some(follow) = outcome.1 else { break };
                        receiver = follow;
                        request_reply = false;
                    }
                }
                Some(LiveEvent::Ignored) | None => {
                    let _ = this.update(cx, |this, cx| {
                        if generation == this.live_state.generation
                            && std::mem::take(&mut this.live_state.awaiting_query_layout)
                        {
                            this.resize_current_window(cx);
                        }
                    });
                    break;
                }
                Some(LiveEvent::ActionResult(result)) => {
                    let result_session_id = result.session_id;
                    let destination = result.view.clone();
                    let dismiss = this
                        .update(cx, |this, cx| {
                            if generation != this.live_state.generation
                                || this.live_state.session_id != Some(result_session_id)
                            {
                                return None;
                            }
                            Some(this.finish_action_result(&result, destination.is_some(), cx))
                        })
                        .unwrap_or(None);
                    let Some(dismiss) = dismiss else {
                        break;
                    };
                    if dismiss {
                        crate::platform::hide_main_window();
                    }
                    if let Some(destination) = destination {
                        let _ = cx.update_window(window_handle, |_, window, cx| {
                            let _ = this.update(cx, |this, cx| {
                                if generation == this.live_state.generation
                                    && this.live_state.session_id == Some(result_session_id)
                                {
                                    this.open_action_destination(destination, window, cx);
                                }
                            });
                        });
                    }
                    break;
                }
                Some(LiveEvent::Error(error)) => {
                    this.update(cx, |this, cx| {
                        if generation == this.live_state.generation {
                            this.live_state.awaiting_query_layout = false;
                            this.model.loading = false;
                            this.shortcut_catalog_loading = false;
                            this.set_notice(error, components::NoticeKind::Error);
                            cx.notify();
                        }
                    })
                    .ok();
                    break;
                }
                Some(event) => {
                    tracing::debug!("launcher_live_event_received");
                    this.update(cx, |this, cx| {
                        if generation == this.live_state.generation {
                            this.apply_auxiliary_event(event, cx);
                            cx.notify();
                        }
                    })
                    .ok();
                    break;
                }
            }
        })
        .detach();
    }

    pub(super) fn apply_auxiliary_event(&mut self, event: LiveEvent, cx: &mut Context<Self>) {
        match event {
            LiveEvent::Preferences {
                preferences,
                host_error,
            } => self.apply_preferences_result(preferences, host_error, cx),
            LiveEvent::Clipboard(snapshot) => {
                self.clipboard.apply(snapshot);
                self.notice = None;
            }
            LiveEvent::AppHistorySources {
                session_id,
                sources,
                ..
            } => {
                self.app_history.apply_sources(session_id, sources);
                if self.screen == Screen::ApplicationHistory {
                    self.request_app_history_search(cx);
                }
            }
            LiveEvent::AppHistory(snapshot) => {
                self.app_history.apply(snapshot);
                if let Some(message) = self.app_history.notice_text(self.locale) {
                    self.set_notice(message, components::NoticeKind::Warning);
                } else {
                    self.clear_notice();
                }
            }
            LiveEvent::AppHistoryAction { item_id, copy } => {
                if copy
                    && self
                        .app_history
                        .selected_item()
                        .is_some_and(|item| item.id == item_id)
                {
                    self.set_notice(l10n::copied(self.locale), components::NoticeKind::Success);
                }
            }
            LiveEvent::AppHistoryObjectActions {
                request,
                item_id,
                revision,
                actions,
            } => {
                self.app_history
                    .apply_object_actions(request, &item_id, revision, actions);
            }
            LiveEvent::AppHistoryObjectActionsFailed { request } => {
                if self.app_history.fail_object_actions(request) {
                    self.set_notice(
                        l10n::app_history_operation_failed(self.locale),
                        components::NoticeKind::Error,
                    );
                }
            }
            LiveEvent::AppHistoryAdaptersOpened { directory } => {
                self.set_notice(
                    l10n::opened_folder(self.locale, &directory),
                    components::NoticeKind::Success,
                );
            }
            LiveEvent::ShortcutCatalog(actions) => {
                self.shortcut_catalog = actions;
                self.shortcut_catalog_loading = false;
            }
            LiveEvent::ActionShortcutReady {
                action_id,
                revision,
                candidates,
            } => {
                if self.action_shortcut_resolving.as_deref() != Some(action_id.as_str()) {
                    return;
                }
                self.action_shortcut_resolving = None;
                match candidates.as_slice() {
                    [candidate] => {
                        self.set_notice(
                            l10n::running_action(self.locale, &candidate.title),
                            components::NoticeKind::Info,
                        );
                        if let Some(live) = self.live.clone() {
                            let receiver = live.execute(revision, candidate.candidate_id.clone());
                            self.poll(receiver, self.live_state.generation, cx);
                        }
                    }
                    [] => {
                        self.set_notice(
                            l10n::no_matching_actions(self.locale),
                            components::NoticeKind::Warning,
                        );
                    }
                    _ => {
                        self.set_notice(
                            l10n::choose_another_action(self.locale),
                            components::NoticeKind::Info,
                        );
                    }
                }
            }
            LiveEvent::ActionShortcutFailed { action_id, message } => {
                if self.action_shortcut_resolving.as_deref() == Some(action_id.as_str()) {
                    self.action_shortcut_resolving = None;
                    self.set_notice(
                        l10n::action_failed_with(self.locale, &message),
                        components::NoticeKind::Error,
                    );
                }
            }
            LiveEvent::WebPackages(packages) => {
                self.web.apply(packages);
                self.web_surface.review = None;
                self.notice = None;
            }
            LiveEvent::WebReview(review) => {
                self.web_surface.review = Some(review);
                self.notice = None;
            }
            LiveEvent::WebDocument(document) => {
                tracing::debug!(
                    screen = ?self.screen,
                    had_view = self.web_surface.view.is_some(),
                    previous_document = self
                        .web_surface
                        .document
                        .as_ref()
                        .map(|previous| previous.handle.as_str()),
                    document = %document.handle,
                    "web_document_event_applied"
                );
                if let (Some(previous), Some(live)) =
                    (self.web_surface.document.as_ref(), self.live.clone())
                {
                    if previous.handle != document.handle {
                        let _ = live.close_web_document(previous.handle.clone());
                    }
                }
                if self.web_surface.view.is_some() {
                    let _ = self.web_surface.view.as_ref().map(|view| {
                        view.read(cx)
                            .raw()
                            .evaluate_script(&visibility_script(false))
                    });
                }
                self.web_surface.view = None;
                self.web_surface.ready = false;
                self.web_surface.error = None;
                self.web_surface.state_handle = None;
                self.web_surface.state_version = 0;
                self.web_surface.document = Some(document);
                self.notice = None;
            }
            LiveEvent::WebFrameReply { handle, frame } => {
                if self
                    .web_surface
                    .document
                    .as_ref()
                    .is_some_and(|document| document.handle == handle)
                {
                    let _ = self.evaluate_web_script(&reply_script(&frame), cx);
                }
            }
            LiveEvent::WebState { handle, frame } => {
                if self
                    .web_surface
                    .document
                    .as_ref()
                    .is_some_and(|document| document.handle == handle)
                {
                    let _ = self.evaluate_web_script(&state_script(&frame), cx);
                }
            }
            LiveEvent::DirectoriesAuthorized { .. }
            | LiveEvent::LoginItemReconciled(_)
            | LiveEvent::ApplicationIcon { .. }
            | LiveEvent::Preview(_)
            | LiveEvent::SharedState(_)
            | LiveEvent::ActionResult(_)
            | LiveEvent::Settings(_)
            | LiveEvent::SettingsError { .. }
            | LiveEvent::Executions(_)
            | LiveEvent::ExecutionDetail(_)
            | LiveEvent::Ignored
            | LiveEvent::Error(_) => {}
        }
        cx.notify();
    }

    pub(super) fn poll_auxiliary_state(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        self.object_action_generation = self.object_action_generation.wrapping_add(1);
        let request_id = self.object_action_generation;
        let screen = self.screen;
        let revision = self.model.revision;
        let parent = self.model.object_action_parent().map(str::to_owned);
        cx.spawn(async move |this, cx| {
            let Some(event) = receiver.recv().await else {
                return;
            };
            this.update(cx, |this, cx| {
                if generation != this.live_state.generation
                    || request_id != this.object_action_generation
                    || !this.presented
                    || this.screen != screen
                    || this.model.revision != revision
                    || !this.model.object_actions_active
                    || this.model.object_action_parent() != parent.as_deref()
                {
                    return;
                }
                match event {
                    LiveEvent::SharedState(state) => {
                        if !this
                            .live_state
                            .snapshot_gate
                            .as_mut()
                            .is_some_and(|gate| gate.accept_request(&state))
                        {
                            return;
                        }
                        let applied = this.apply_shared_state(state, cx);
                        this.request_application_icons(cx);
                        if applied.rows_changed || applied.scopes_changed {
                            this.resize_current_window(cx);
                        }
                    }
                    LiveEvent::Error(error) => {
                        this.model.hide_object_actions();
                        this.set_notice(error, components::NoticeKind::Error);
                    }
                    event => this.apply_auxiliary_event(event, cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
