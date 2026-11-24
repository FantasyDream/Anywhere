use super::*;

impl LauncherView {
    pub(super) fn request_more_files(&mut self, keyboard: bool, cx: &mut Context<Self>) {
        if self.screen != Screen::FileSearch
            || self.model.object_actions_active
            || !self
                .model
                .file_search_progress()
                .is_some_and(|progress| progress.next_cursor.is_some())
        {
            return;
        }
        if self.file_search.page_in_flight {
            if keyboard {
                self.remember_file_page_advance();
            }
            return;
        }
        let Some(live) = self.live.clone() else {
            return;
        };
        let receiver = live.load_more_files(self.model.revision);
        self.poll_file_page(receiver, keyboard, cx);
    }

    pub(super) fn poll_file_page(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        keyboard: bool,
        cx: &mut Context<Self>,
    ) {
        self.file_search.page_generation = self.file_search.page_generation.wrapping_add(1);
        let generation = self.file_search.page_generation;
        self.file_search.page_in_flight = true;
        self.file_search.advance_from = None;
        if keyboard {
            self.remember_file_page_advance();
        }
        let revision = self.model.revision;
        let stream_generation = self.live_state.generation;
        let session_id = self.live_state.session_id;

        cx.spawn(async move |this, cx| {
            let event = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.file_search.page_generation {
                    return;
                }
                this.file_search.page_in_flight = false;
                let advance_from = this.file_search.advance_from.take();
                if stream_generation != this.live_state.generation
                    || this.screen != Screen::FileSearch
                    || this.live_state.session_id != session_id
                    || this.model.revision != revision
                {
                    return;
                }

                match event {
                    Some(LiveEvent::SharedState(state))
                        if Some(state.session_id) == session_id
                            && state.revision == revision
                            && state.input == this.model.query =>
                    {
                        if this
                            .live_state
                            .snapshot_gate
                            .as_mut()
                            .is_some_and(|gate| gate.accept_update(&state))
                        {
                            this.apply_shared_state(state, cx);
                            this.resize_current_window(cx);
                        }
                        // The follow stream can publish the appended rows before
                        // this reply. Use the current rows in either delivery order.
                        if this.file_search.browsing
                            && !this.model.object_actions_active
                            && advance_from
                                .as_deref()
                                .is_some_and(|id| this.model.selection_id() == Some(id))
                            && this.model.selected + 1 < this.model.rows.len()
                        {
                            this.model.select_next();
                            this.reveal_result_row(this.model.selected);
                            if this.preview.open {
                                this.schedule_selected_preview(cx);
                            }
                        }
                    }
                    Some(LiveEvent::Error(error)) => {
                        this.set_notice(error, components::NoticeKind::Error);
                    }
                    Some(LiveEvent::SharedState(_) | LiveEvent::Ignored) => {}
                    Some(event) => this.apply_auxiliary_event(event, cx),
                    None => this.set_notice(
                        l10n::file_page_load_failed(this.locale),
                        components::NoticeKind::Error,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(in crate::launcher::view) fn select_file_filter(
        &mut self,
        filter: FileSearchFilter,
        cx: &mut Context<Self>,
    ) {
        if self.file_search.filter != filter {
            self.file_search.filter = filter;
            self.apply_file_filter_change(cx);
        }
    }

    fn apply_file_filter_change(&mut self, cx: &mut Context<Self>) {
        self.model.hide_object_actions();
        self.reveal_selected_file_filter();
        self.reset_file_search_browsing(cx);
        self.request_live_update(cx);
        self.notice = None;
        cx.notify();
    }

    fn remember_file_page_advance(&mut self) {
        self.file_search.advance_from = self.model.selection_id().map(str::to_owned);
    }

    pub(in crate::launcher::view) fn handle_file_search_key(
        &mut self,
        input: &crate::launcher::view::input::KeyInput<'_>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modified = input.modifiers.modified;
        match input.key {
            "down" if !modified => self.select_file_search(1, cx),
            "up" if !modified => self.select_file_search(-1, cx),
            "tab"
                if is_scope_tab_shortcut(input.key, input.modifiers)
                    && self.active_editor_focused(window, cx) =>
            {
                if input.modifiers.shift {
                    self.file_search.previous_filter();
                } else {
                    self.file_search.next_filter();
                }
                self.apply_file_filter_change(cx);
            }
            "enter" | "return" => self.activate_selection(cx),
            "escape" => {
                if self.model.object_actions_active {
                    self.model.hide_object_actions();
                    self.reveal_result_row(self.model.selected);
                    cx.notify();
                } else if !self.model.query.is_empty() {
                    self.reset_file_search_browsing(cx);
                    self.input
                        .update(cx, |input, cx| input.set_value("", window, cx));
                    self.model.set_query(String::new());
                    self.request_live_update(cx);
                    cx.notify();
                } else {
                    self.go_back(window, cx);
                }
            }
            "pageup" | "pagedown" => {
                let count = self.model.rows.len();
                if count > 0 {
                    let height = self.scrolls.file_search_base().bounds().size.height;
                    let step = (f32::from(height) / metrics::ROW_HEIGHT)
                        .floor()
                        .clamp(1., 100.) as usize;
                    let selected = if self.file_search.browsing {
                        self.model.selected
                    } else {
                        0
                    };
                    let target = if input.key == "pageup" {
                        selected.saturating_sub(step)
                    } else {
                        selected.saturating_add(step).min(count - 1)
                    };
                    self.file_search.browsing = true;
                    self.model.selected = target;
                    self.notice = None;
                    if self.preview.open {
                        self.request_preview(cx);
                    }
                    self.reveal_result_row(target);
                    cx.notify();
                }
            }
            _ => {}
        }
    }
}
