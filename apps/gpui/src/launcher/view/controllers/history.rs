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
        self.app_history.page_actions_open = false;
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
            self.show_hud_toast(
                l10n::copied(self.locale),
                IconName::Check,
                components::NoticeKind::Success,
                cx,
            );
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
        self.scrolls
            .history_actions
            .set_offset(gpui_kit::Point::default());
        let receiver = live.app_history_object_actions(request, session_id, revision, item.id);
        self.poll(receiver, self.live_state.generation, cx);
        cx.notify();
    }

    pub(super) fn toggle_app_history_page_actions(&mut self, cx: &mut Context<Self>) {
        self.app_history.close_object_actions();
        self.app_history.fallback_actions_open = false;
        self.app_history.page_actions_open = !self.app_history.page_actions_open;
        self.notice = None;
        cx.notify();
    }

    pub(super) fn open_fallback_app_history_actions(&mut self, cx: &mut Context<Self>) {
        self.app_history.close_object_actions();
        self.app_history.page_actions_open = false;
        self.app_history.fallback_actions_open = true;
        self.app_history.fallback_action_selected = 0;
        self.notice = None;
        cx.notify();
    }

    pub(super) fn toggle_fallback_app_history_actions(&mut self, cx: &mut Context<Self>) {
        let open = !self.app_history.fallback_actions_open;
        self.app_history.close_object_actions();
        self.app_history.page_actions_open = false;
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

    pub(super) fn refresh_app_history_from_page_menu(&mut self, cx: &mut Context<Self>) {
        self.app_history.page_actions_open = false;
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

    pub(in crate::launcher::view) fn handle_app_history_overlay_keys(
        &mut self,
        input: &crate::launcher::view::input::KeyInput<'_>,
        cx: &mut Context<Self>,
    ) -> crate::launcher::view::input::KeyFlow {
        use crate::launcher::view::input::KeyFlow;
        if self.screen != Screen::ApplicationHistory || input.modifiers.modified {
            return KeyFlow::Continue;
        }
        if self.app_history.fallback_actions_open {
            match input.key {
                "down" | "up" => {
                    self.app_history.fallback_action_selected =
                        1 - self.app_history.fallback_action_selected;
                }
                "enter" | "return" => {
                    self.execute_fallback_app_history_action(
                        self.app_history.fallback_action_selected,
                        cx,
                    );
                }
                "escape" => self.app_history.fallback_actions_open = false,
                _ => return KeyFlow::Handled,
            }
            cx.notify();
            return KeyFlow::Handled;
        }
        if self.app_history.page_actions_open {
            match input.key {
                "enter" | "return" => self.refresh_app_history_from_page_menu(cx),
                "escape" => self.app_history.page_actions_open = false,
                "down" | "up" => {}
                _ => return KeyFlow::Handled,
            }
            cx.notify();
            return KeyFlow::Handled;
        }
        if self.app_history.object_actions_active {
            match input.key {
                "down" | "up" => {
                    if input.key == "down" {
                        self.app_history.select_object_action_next();
                    } else {
                        self.app_history.select_object_action_previous();
                    }
                    self.scrolls
                        .history_actions
                        .scroll_to_item(self.app_history.object_action_selected);
                }
                "enter" | "return" => self.execute_selected_app_history_action(cx),
                "escape" => self.app_history.close_object_actions(),
                _ => return KeyFlow::Handled,
            }
            cx.notify();
            return KeyFlow::Handled;
        }
        KeyFlow::Continue
    }

    pub(in crate::launcher::view) fn handle_app_history_key(
        &mut self,
        input: &crate::launcher::view::input::KeyInput<'_>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match input.key {
            "pageup" | "pagedown" => {
                let height = self.scrolls.history_base().bounds().size.height;
                let count = self.app_history.items.len();
                let step = (f32::from(height) / 64.).floor().clamp(1., 100.) as usize;
                let target = if input.key == "pageup" {
                    self.app_history.selected.saturating_sub(step)
                } else {
                    self.app_history
                        .selected
                        .saturating_add(step)
                        .min(count.saturating_sub(1))
                };
                self.app_history.close_object_actions();
                self.app_history.selected = target;
                self.reveal_result_row(target);
                cx.notify();
            }
            "down" => {
                self.app_history.select_next();
                self.reveal_result_row(self.app_history.selected);
                cx.notify();
            }
            "up" => {
                self.app_history.select_previous();
                self.reveal_result_row(self.app_history.selected);
                cx.notify();
            }
            "enter" | "return" => {
                if self
                    .app_history
                    .selected_item()
                    .is_some_and(|item| item.can_open)
                {
                    self.open_selected_history_item(cx);
                } else {
                    self.copy_selected_history_item(cx);
                }
            }
            "escape" => {
                if !self.app_history.query.is_empty() {
                    self.history_input
                        .update(cx, |input, cx| input.set_value("", window, cx));
                    self.app_history.set_query(String::new());
                    self.schedule_app_history_search(cx);
                    cx.notify();
                } else {
                    self.go_back(window, cx);
                }
            }
            _ => {}
        }
    }
}
