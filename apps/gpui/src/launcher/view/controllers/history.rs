use super::*;

impl LauncherView {
    pub(super) fn poll_app_history_session(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        request: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let event = receiver.recv().await.unwrap_or_else(|| {
                LiveEvent::Error("application history session channel closed".to_owned())
            });
            let _ = this.update(cx, |this, cx| {
                tracing::debug!(
                    request,
                    current_request = this.app_history.session_generation,
                    screen = ?this.screen,
                    "application_history_session_event"
                );
                if request != this.app_history.session_generation
                    || this.screen != Screen::ApplicationHistory
                {
                    return;
                }
                match event {
                    LiveEvent::AppHistorySources {
                        request: event_request,
                        session_id,
                        sources,
                    } if event_request == request => {
                        tracing::debug!(
                            request,
                            session_id = ?session_id,
                            source_count = sources.len(),
                            selected_source = ?this.app_history.selected_source,
                            "application_history_sources_loaded"
                        );
                        this.app_history.apply_sources(session_id, sources);
                        this.request_app_history_search(cx);
                    }
                    LiveEvent::Error(error) => {
                        tracing::warn!(%error, "app_history_session_failed");
                        this.app_history.fail();
                        this.set_notice(
                            l10n::app_history_operation_failed(this.locale),
                            components::NoticeKind::Error,
                        );
                    }
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn request_app_history_search(&mut self, cx: &mut Context<Self>) {
        self.request_app_history_search_with_refresh(false, cx);
    }

    pub(super) fn request_app_history_refresh(&mut self, cx: &mut Context<Self>) {
        self.request_app_history_search_with_refresh(true, cx);
    }

    fn request_app_history_search_with_refresh(
        &mut self,
        force_refresh: bool,
        cx: &mut Context<Self>,
    ) {
        self.app_history.query_generation = self.app_history.query_generation.wrapping_add(1);
        let request_id = self.app_history.query_generation;
        if self.app_history.selected_source.is_none() {
            self.app_history.fail();
            self.app_history.notice = Some(if self.app_history.sources.is_empty() {
                anywhere_core::features::application_history::model::Notice::Unconfigured
            } else {
                anywhere_core::features::application_history::model::Notice::Disabled
            });
            return;
        }
        let (Some(live), Some(session_id), Some(source_id)) = (
            self.live.clone(),
            self.app_history.session_id,
            self.app_history.selected_source.clone(),
        ) else {
            tracing::debug!(
                has_live = self.live.is_some(),
                session_id = ?self.app_history.session_id,
                selected_source = ?self.app_history.selected_source,
                "application_history_search_skipped"
            );
            return;
        };
        self.app_history.close_object_actions();
        self.app_history.empty_actions_open = false;
        self.app_history.fallback_actions_open = false;
        self.app_history.failed_sources = false;
        self.app_history.notice = None;
        self.app_history.loading = true;
        let query = self.app_history.query.clone();
        tracing::debug!(
            request_id,
            session_id,
            source_id = %source_id,
            query = %query,
            force_refresh,
            "application_history_search_started"
        );
        let receiver = live.search_app_history(
            session_id,
            request_id,
            query.clone(),
            source_id.clone(),
            force_refresh,
        );
        self.poll_app_history_search(receiver, request_id, source_id, query, cx);
    }

    pub(super) fn schedule_app_history_search(&mut self, cx: &mut Context<Self>) {
        self.app_history.query_generation = self.app_history.query_generation.wrapping_add(1);
        let generation = self.app_history.query_generation;
        self.app_history.close_object_actions();
        self.app_history.loading = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.app_history.query_generation
                    || this.screen != Screen::ApplicationHistory
                {
                    return;
                }
                this.request_app_history_search(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn poll_app_history_search(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        request_id: u64,
        source_id: String,
        query: String,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let event = receiver.recv().await.unwrap_or_else(|| {
                LiveEvent::Error("application history search channel closed".to_owned())
            });
            let _ = this.update(cx, |this, cx| {
                tracing::debug!(
                    request_id,
                    current_request = this.app_history.query_generation,
                    screen = ?this.screen,
                    selected_source = ?this.app_history.selected_source,
                    current_query = %this.app_history.query,
                    "application_history_search_event"
                );
                if request_id != this.app_history.query_generation
                    || this.screen != Screen::ApplicationHistory
                    || this.app_history.selected_source.as_deref() != Some(source_id.as_str())
                    || this.app_history.query != query
                {
                    return;
                }
                match event {
                    LiveEvent::AppHistory(snapshot) if snapshot.source_id == source_id => {
                        this.app_history.apply(snapshot);
                        this.reveal_result_row(this.app_history.selected);
                        if let Some(message) = this.app_history.notice_text(this.locale) {
                            this.set_notice(message, components::NoticeKind::Warning);
                        } else {
                            this.clear_notice();
                        }
                    }
                    LiveEvent::Error(error) => {
                        tracing::warn!(%error, "app_history_search_failed");
                        this.app_history.fail();
                        this.set_notice(
                            l10n::app_history_operation_failed(this.locale),
                            components::NoticeKind::Error,
                        );
                    }
                    LiveEvent::Ignored => {
                        this.app_history.loading = false;
                    }
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn release_app_history_session(&mut self) {
        self.app_history.session_generation = self.app_history.session_generation.wrapping_add(1);
        self.app_history.query_generation = self.app_history.query_generation.wrapping_add(1);
        self.app_history.close_object_actions();
        self.app_history.loading = false;
        if let (Some(live), Some(session_id)) =
            (self.live.clone(), self.app_history.session_id.take())
        {
            live.cancel_app_history_session(session_id);
        }
    }

    pub(super) fn start_app_history_session(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            tracing::warn!("application_history_session_missing_live");
            return;
        };
        self.app_history.session_generation = self.app_history.session_generation.wrapping_add(1);
        let request = self.app_history.session_generation;
        tracing::debug!(request, "application_history_session_started");
        let receiver = live.open_app_history_session(request);
        self.poll_app_history_session(receiver, request, cx);
    }

    pub(super) fn open_selected_history_item(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(item)) = (self.live.clone(), self.app_history.selected_item()) {
            let receiver = live.use_app_history_item(item.id.clone(), false);
            self.poll(receiver, self.live_state.generation, cx);
            self.set_notice(l10n::open(self.locale), components::NoticeKind::Info);
            cx.notify();
        }
    }

    pub(super) fn copy_selected_history_item(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(item)) = (self.live.clone(), self.app_history.selected_item()) {
            let receiver = live.use_app_history_item(item.id.clone(), true);
            self.poll(receiver, self.live_state.generation, cx);
            self.notice = None;
            cx.notify();
        }
    }

    pub(super) fn request_app_history_object_actions(&mut self, cx: &mut Context<Self>) {
        let (Some(live), Some(session_id), Some(item)) = (
            self.live.clone(),
            self.live_state.session_id,
            self.app_history.selected_item().cloned(),
        ) else {
            return;
        };
        if !item.can_open {
            return;
        }
        self.notice = None;
        let revision = self.model.revision;
        let request = self
            .app_history
            .begin_object_actions_request(item.id.clone(), revision);
        let receiver =
            live.app_history_object_actions(request, session_id, revision, item.id.clone());
        self.poll(receiver, self.live_state.generation, cx);
        cx.notify();
    }

    pub(super) fn toggle_empty_app_history_actions(&mut self, cx: &mut Context<Self>) {
        self.app_history.close_object_actions();
        self.app_history.empty_actions_open = !self.app_history.empty_actions_open;
        self.notice = None;
        cx.notify();
    }

    pub(super) fn open_fallback_app_history_actions(&mut self, cx: &mut Context<Self>) {
        self.app_history.close_object_actions();
        self.app_history.empty_actions_open = false;
        self.app_history.fallback_actions_open = true;
        self.app_history.fallback_action_selected = 0;
        self.notice = None;
        cx.notify();
    }

    pub(super) fn toggle_fallback_app_history_actions(&mut self, cx: &mut Context<Self>) {
        let open = !self.app_history.fallback_actions_open;
        self.app_history.close_object_actions();
        self.app_history.empty_actions_open = false;
        self.app_history.fallback_actions_open = open;
        self.app_history.fallback_action_selected = 0;
        self.notice = None;
        cx.notify();
    }

    pub(super) fn execute_fallback_app_history_action(
        &mut self,
        action: usize,
        cx: &mut Context<Self>,
    ) {
        self.app_history.fallback_actions_open = false;
        self.app_history.fallback_action_selected = 0;
        match action {
            0 => self.copy_selected_history_item(cx),
            1 => self.request_app_history_refresh(cx),
            _ => {}
        }
    }

    pub(super) fn refresh_empty_app_history(&mut self, cx: &mut Context<Self>) {
        self.app_history.empty_actions_open = false;
        self.request_app_history_refresh(cx);
        cx.notify();
    }

    pub(super) fn execute_selected_app_history_action(&mut self, cx: &mut Context<Self>) {
        if self.app_history.object_action_selected >= self.app_history.object_actions.len() {
            if self.app_history.object_actions_fallback_copy()
                && self.app_history.object_action_selected == self.app_history.object_actions.len()
            {
                self.copy_selected_history_item(cx);
                return;
            }
            self.request_app_history_refresh(cx);
            return;
        }
        let Some(action) = self.app_history.selected_object_action().cloned() else {
            return;
        };
        if !action.enabled {
            return;
        }
        let Some(live) = self.live.clone() else {
            return;
        };
        let revision = self.model.revision;
        self.app_history.close_object_actions();
        let receiver = live.execute(revision, action.id);
        self.poll(receiver, self.live_state.generation, cx);
        cx.notify();
    }
}
