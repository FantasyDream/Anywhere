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
                            && advance_from.as_deref().is_some_and(|id| {
                                this.model.selected_row().is_some_and(|row| row.id == id)
                            })
                            && this.model.selected + 1 < this.model.rows.len()
                        {
                            this.model.select_next();
                            this.reveal_result_row(this.model.selected);
                            if this.preview_open {
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

    fn remember_file_page_advance(&mut self) {
        self.file_search.advance_from = self.model.selected_row().map(|row| row.id.clone());
    }
}
