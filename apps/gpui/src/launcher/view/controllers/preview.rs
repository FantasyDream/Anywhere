use super::*;

impl LauncherView {
    pub(super) fn schedule_selected_preview(&mut self, cx: &mut Context<Self>) {
        let generation = self.live_state.generation;
        let screen = self.screen;
        self.preview.selection_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(
                    anywhere_protocol::policy::PREVIEW_SELECTION_DEBOUNCE_MS,
                ))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.preview.open
                    && this.live_state.generation == generation
                    && this.screen == screen
                {
                    this.request_preview(cx);
                }
            });
        }));
    }

    pub(super) fn can_toggle_preview_shortcut(&self) -> bool {
        let file_preview = matches!(self.screen, Screen::Launcher | Screen::FileSearch)
            && (self.preview.open || self.can_preview_selected_file());
        let clipboard_preview = matches!(self.screen, Screen::Clipboard | Screen::ClipboardShelf)
            && (self.preview.open || self.clipboard_selection_active());
        file_preview || clipboard_preview
    }

    pub(super) fn toggle_preview_shortcut(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.can_toggle_preview_shortcut() {
            return false;
        }
        let file_preview = matches!(self.screen, Screen::Launcher | Screen::FileSearch);
        if self.close_preview(cx) {
            self.focus_active_editor(window, cx);
        } else if file_preview {
            self.request_preview(cx);
        } else {
            self.preview_selected_clipboard(cx);
        }
        true
    }

    pub(super) fn request_preview(&mut self, cx: &mut Context<Self>) {
        if self.screen == Screen::Launcher || self.screen == Screen::FileSearch {
            if let (Some(live), Some(row)) = (self.live.clone(), self.selected_row_for_action()) {
                if row.kind == anywhere_protocol::CandidateKind::File {
                    let receiver = live.preview_file(self.model.revision, row.id.to_string());
                    self.poll_preview(receiver, cx);
                }
            }
        }
    }

    pub(super) fn current_preview_target(&self) -> Option<PreviewTarget> {
        match self.screen {
            Screen::Launcher | Screen::FileSearch => self
                .selected_row_for_action()
                .filter(|row| row.kind == CandidateKind::File)
                .map(|row| PreviewTarget::File {
                    candidate_id: row.id.to_string(),
                    revision: self.model.revision,
                }),
            Screen::Clipboard | Screen::ClipboardShelf => {
                self.clipboard.selected_id().map(PreviewTarget::Clipboard)
            }
            _ => None,
        }
    }

    pub(super) fn preview_request_is_current(&self, request: &PreviewRequest) -> bool {
        request.generation == self.preview.request_generation
            && request.session_generation == self.live_state.generation
            && request.screen == self.screen
            && self.current_preview_target().as_ref() == Some(&request.target)
    }

    pub(super) fn poll_preview(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.current_preview_target() else {
            return;
        };
        self.preview.request_generation = self.preview.request_generation.wrapping_add(1);
        let request = PreviewRequest {
            generation: self.preview.request_generation,
            session_generation: self.live_state.generation,
            screen: self.screen,
            target,
        };
        let live = self.live.clone();
        cx.spawn(async move |this, cx| {
            let Some(mut event) = receiver.recv().await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                if !this.preview_request_is_current(&request) {
                    return;
                }
                match std::mem::replace(&mut event, LiveEvent::Ignored) {
                    LiveEvent::Preview(preview) => this.show_preview(preview, request, cx),
                    LiveEvent::Error(error) => {
                        this.set_notice(error, components::NoticeKind::Error);
                        cx.notify();
                    }
                    _ => {}
                }
            });
            if let (Some(live), LiveEvent::Preview(preview)) = (live, event) {
                let _ = live.release_preview(preview.id);
            }
        })
        .detach();
    }

    pub(super) fn show_preview(
        &mut self,
        preview: crate::host::PreviewFile,
        request: PreviewRequest,
        cx: &mut Context<Self>,
    ) {
        self.notice = None;
        tracing::debug!(
            preview_id = %preview.id,
            path_count = preview.paths.len(),
            "quick_look_show_started"
        );
        let owner_title = if self.screen == Screen::ClipboardShelf {
            crate::CLIPBOARD_SHELF_TITLE
        } else {
            crate::PRODUCT_NAME
        };
        let live = self.live.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            // AppKit's shared Quick Look panel pumps its event loop while showing.
            // Keep that call outside the mutable GPUI app borrow held by `update`.
            cx.background_executor()
                .timer(std::time::Duration::from_millis(0))
                .await;
            let (mut host, generation) = loop {
                let reservation = this
                    .update(cx, |this, cx| {
                        if !this.preview_request_is_current(&request) {
                            return Ok(PreviewReservation::Stale);
                        }
                        if this.preview.busy {
                            return Ok(PreviewReservation::Busy);
                        }
                        this.preview.busy = true;
                        if this.preview.host.is_none() {
                            this.preview.host = Some(PreviewHost::new()?);
                        }
                        let host = this.preview.host.take().ok_or_else(|| {
                            anyhow::anyhow!("Quick Look host was not initialized")
                        })?;
                        this.preview.generation = this.preview.generation.wrapping_add(1);
                        this.preview.open = false;
                        cx.notify();
                        Ok(PreviewReservation::Ready(host, this.preview.generation))
                    })
                    .and_then(|reservation| reservation);
                match reservation {
                    Ok(PreviewReservation::Ready(host, generation)) => break (host, generation),
                    Ok(PreviewReservation::Stale) => {
                        if let Some(live) = &live {
                            let _ = live.release_preview(preview.id);
                        }
                        return;
                    }
                    Ok(PreviewReservation::Busy) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(1))
                            .await;
                    }
                    Err(error) => {
                        if let Some(live) = &live {
                            let _ = live.release_preview(preview.id);
                        }
                        let _ = this.update(cx, |this, cx| {
                            this.preview.busy = false;
                            if this.preview_request_is_current(&request) {
                                this.set_notice(
                                    preview_error_message(this.locale, &error),
                                    components::NoticeKind::Error,
                                );
                            }
                            cx.notify();
                        });
                        return;
                    }
                }
            };
            let result = host.show(preview.id.clone(), &preview.paths, owner_title);
            let owns_panel = host.owns_panel();
            tracing::debug!(
                preview_id = %preview.id,
                success = result.is_ok(),
                "quick_look_show_finished"
            );
            let mut reserved_host = Some(host);
            let applied = this
                .update(cx, |this, cx| {
                    if generation != this.preview.generation
                        || !this.preview_request_is_current(&request)
                    {
                        return false;
                    }
                    this.preview.host = reserved_host.take();
                    this.preview.busy = false;
                    match &result {
                        Ok(previous) => {
                            this.preview.open = true;
                            if let Some(live) = this.live.clone() {
                                if let Some(previous) = previous {
                                    let _ = live.release_preview(previous.clone());
                                }
                                let _ = live.retain_preview(preview.id.clone());
                            }
                            this.set_notice(
                                l10n::previewing_title(this.locale, &preview.title),
                                components::NoticeKind::Info,
                            );
                            this.start_preview_monitor(generation, cx);
                        }
                        Err(error) => {
                            this.preview.open = owns_panel;
                            if owns_panel {
                                this.start_preview_monitor(generation, cx);
                            }
                            this.set_notice(
                                preview_error_message(this.locale, error),
                                components::NoticeKind::Error,
                            );
                        }
                    }
                    cx.notify();
                    this.refresh_current_window(cx);
                    tracing::debug!(
                        preview_id = %preview.id,
                        preview_open = this.preview.open,
                        "quick_look_state_applied"
                    );
                    true
                })
                .unwrap_or(false);
            if result.is_err() {
                if let Some(live) = &live {
                    let _ = live.release_preview(preview.id);
                }
            }
            if !applied {
                let resource_id = reserved_host.as_mut().and_then(|host| host.close());
                if let Some(live) = &live {
                    for id in resource_id.into_iter().chain(result.ok().flatten()) {
                        let _ = live.release_preview(id);
                    }
                }
                let _ = this.update(cx, |this, cx| {
                    this.preview.host = reserved_host.take();
                    this.preview.busy = false;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(super) fn start_preview_monitor(&mut self, generation: u64, cx: &mut Context<Self>) {
        let window_handle = self.window_handle;
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            let host = this
                .update(cx, |this, _cx| this.reserve_preview_for_poll(generation))
                .ok()
                .flatten();
            let Some(mut host) = host else {
                let continue_polling = this
                    .update(cx, |this, _cx| {
                        generation == this.preview.generation
                            && (this.preview.busy || this.preview.host.is_some())
                    })
                    .unwrap_or(false);
                if !continue_polling {
                    break;
                }
                continue;
            };
            let poll = host.poll();
            let resource_id = matches!(poll, PreviewPoll::Closed { .. })
                .then(|| host.close())
                .flatten();
            let mut reserved_host = Some(host);
            let state = this
                .update(cx, |this, cx| {
                    this.apply_preview_poll(
                        generation,
                        &poll,
                        reserved_host.take(),
                        resource_id.clone(),
                        cx,
                    )
                })
                .ok()
                .flatten();
            let Some(restore_input) = state else {
                if let Some(mut host) = reserved_host.take() {
                    let stale_resource_id = host.close();
                    let _ = this.update(cx, |this, cx| {
                        this.finish_preview_close(Some(host), stale_resource_id, cx);
                    });
                }
                break;
            };
            if let Some(input) = restore_input {
                let _ = window_handle.update(cx, |_, window, cx| {
                    input.update(cx, |input, cx| input.focus(window, cx));
                });
            }
            if !matches!(poll, PreviewPoll::Open) {
                break;
            }
        })
        .detach();
    }

    pub(super) fn reserve_preview_for_poll(&mut self, generation: u64) -> Option<PreviewHost> {
        if generation != self.preview.generation || self.preview.busy {
            return None;
        }
        let host = self.preview.host.take()?;
        self.preview.busy = true;
        Some(host)
    }

    pub(super) fn apply_preview_poll(
        &mut self,
        generation: u64,
        poll: &PreviewPoll,
        host: Option<PreviewHost>,
        resource_id: Option<String>,
        cx: &mut Context<Self>,
    ) -> Option<Option<Entity<InputState>>> {
        if generation != self.preview.generation {
            let (host, closed_id) = match host {
                Some(mut host) => {
                    let closed = host.close().or(resource_id);
                    (Some(host), closed)
                }
                None => (None, resource_id),
            };
            self.finish_preview_close(host, closed_id, cx);
            return None;
        }
        self.preview.host = host;
        self.preview.busy = false;
        match poll {
            PreviewPoll::Open => Some(None),
            PreviewPoll::Closed {
                resource_id: active_resource_id,
                application_active,
            } => {
                self.preview.request_generation = self.preview.request_generation.wrapping_add(1);
                debug_assert_eq!(resource_id.as_ref(), Some(active_resource_id));
                if let (Some(live), Some(resource_id)) = (self.live.clone(), resource_id.as_ref()) {
                    let _ = live.release_preview(resource_id.clone());
                }
                self.preview.open = false;
                self.notice = None;
                cx.notify();
                self.refresh_current_window(cx);
                let restore = application_active
                    .then(|| self.active_editor_input().cloned())
                    .flatten();
                Some(restore)
            }
        }
    }

    pub(super) fn close_preview(&mut self, cx: &mut Context<Self>) -> bool {
        self.preview.request_generation = self.preview.request_generation.wrapping_add(1);
        let was_open = self.preview.open;
        self.preview.generation = self.preview.generation.wrapping_add(1);
        let generation = self.preview.generation;
        self.preview.open = false;
        if was_open {
            self.notice = None;
            cx.notify();
            self.refresh_current_window(cx);
        }
        let view = cx.entity();
        cx.spawn(async move |_, cx| {
            // `hide` and `makeKeyAndOrderFront` may synchronously restore AppKit
            // focus, so perform them after the current GPUI update has ended.
            // Keep the host alive until cleanup even if the shelf window closes.
            cx.background_executor()
                .timer(std::time::Duration::from_millis(0))
                .await;
            let host = view.update(cx, |this, _cx| this.reserve_preview_for_close(generation));
            if let Some(mut host) = host {
                let resource_id = host.close();
                view.update(cx, |this, cx| {
                    this.finish_preview_close(Some(host), resource_id, cx);
                });
            }
        })
        .detach();
        was_open
    }

    pub(super) fn reserve_preview_for_close(&mut self, generation: u64) -> Option<PreviewHost> {
        if generation != self.preview.generation || self.preview.busy {
            return None;
        }
        let host = self.preview.host.take()?;
        self.preview.busy = true;
        Some(host)
    }

    pub(super) fn finish_preview_close(
        &mut self,
        host: Option<PreviewHost>,
        resource_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.preview.host = host;
        self.preview.busy = false;
        if let (Some(live), Some(resource_id)) = (self.live.clone(), resource_id) {
            let _ = live.release_preview(resource_id);
        }
        cx.notify();
        self.refresh_current_window(cx);
    }

    pub(super) fn handle_preview_keystroke(
        &mut self,
        key: &str,
        modified: bool,
        prefer_character_input: bool,
        repeated: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.preview.open && key == "escape" {
            self.close_preview(cx);
            self.focus_active_editor(window, cx);
            return true;
        }
        if key != "space"
            || prefer_character_input
            || modified
            || !self.active_editor_focused(window, cx)
            || (!self.preview.open && !self.file_search_selection_active())
        {
            return false;
        }
        if repeated {
            return true;
        }
        if self.close_preview(cx) {
            self.focus_active_editor(window, cx);
        } else {
            self.request_preview(cx);
        }
        true
    }

    pub(super) fn can_preview_selected_file(&self) -> bool {
        matches!(self.screen, Screen::Launcher | Screen::FileSearch)
            && self
                .selected_row_for_action()
                .is_some_and(|row| row.kind == anywhere_protocol::CandidateKind::File)
    }

    pub(in crate::launcher::view) fn handle_preview_key_overlay(
        &mut self,
        input: &crate::launcher::view::input::KeyInput<'_>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> crate::launcher::view::input::KeyFlow {
        use crate::launcher::view::input::KeyFlow;
        if !self.preview.open {
            return KeyFlow::Continue;
        }
        let KeyModifiers {
            command,
            control,
            alt,
            shift,
            modified,
        } = input.modifiers;
        if input.key == "escape"
            || (command && !control && !alt && !shift && input.key.eq_ignore_ascii_case("l"))
        {
            self.close_preview(cx);
            self.focus_active_editor(window, cx);
            return KeyFlow::Handled;
        }
        if (!modified
            && matches!(
                input.key,
                "left" | "right" | "home" | "end" | "backspace" | "delete"
            ))
            || (command && !control && !alt && !shift && input.key.eq_ignore_ascii_case("a"))
        {
            if matches!(self.screen, Screen::Launcher | Screen::FileSearch) {
                self.file_search.browsing = false;
            }
            if matches!(self.screen, Screen::Clipboard | Screen::ClipboardShelf) {
                self.clipboard.browsing = false;
            }
            self.close_preview(cx);
        }
        KeyFlow::Continue
    }
}
