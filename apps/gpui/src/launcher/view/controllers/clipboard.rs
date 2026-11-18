use super::*;

impl LauncherView {
    pub fn clipboard_shelf(
        window: &mut Window,
        cx: &mut Context<Self>,
        live: Arc<LiveLauncher>,
        locale: Locale,
    ) -> Self {
        let locale = live
            .preferences()
            .map(|preferences| locale_for_language(preferences.ui_language))
            .unwrap_or(locale);
        let mut view = Self::build(
            window,
            cx,
            LauncherModel::preview(locale),
            locale,
            Some(live.clone()),
            None,
            ViewKind::ClipboardShelf,
        );
        view.screen = Screen::ClipboardShelf;
        view.clipboard_input.update(cx, |input, cx| {
            input.set_placeholder(l10n::search_clipboard(locale), window, cx)
        });
        view.clipboard_input
            .update(cx, |input, cx| input.focus(window, cx));
        view
    }

    pub fn initialize_clipboard_shelf(&mut self, cx: &mut Context<Self>) {
        tracing::debug!("clipboard_shelf_initial_snapshot_requested");
        self.request_clipboard_search(cx);
        cx.notify();
    }

    pub(super) fn schedule_clipboard_search(&mut self, cx: &mut Context<Self>) {
        self.clipboard.query_generation = self.clipboard.query_generation.wrapping_add(1);
        self.clipboard.loading = true;
        let query_generation = self.clipboard.query_generation;
        let generation = self.live_state.generation;
        let screen = self.screen;
        self.clipboard_search_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(80))
                .await;
            let _ = this.update(cx, |this, cx| {
                if query_generation == this.clipboard.query_generation
                    && generation == this.live_state.generation
                    && screen == this.screen
                {
                    this.request_clipboard_search(cx);
                }
            });
        }));
    }

    pub(super) fn request_clipboard_search(&mut self, cx: &mut Context<Self>) {
        if let Some(live) = self.live.as_ref() {
            let receiver = live.clipboard_snapshot(self.clipboard.query.clone());
            self.begin_clipboard_read(receiver, cx);
        }
    }

    pub(super) fn request_clipboard_settings_status(&mut self, cx: &mut Context<Self>) {
        if let Some(live) = self.live.as_ref() {
            let receiver = live.clipboard_settings_snapshot();
            self.begin_clipboard_read(receiver, cx);
            cx.notify();
        }
    }

    pub(super) fn begin_clipboard_read(
        &mut self,
        receiver: Receiver<LiveEvent>,
        cx: &mut Context<Self>,
    ) {
        self.clipboard.query_generation = self.clipboard.query_generation.wrapping_add(1);
        self.clipboard.loading();
        self.poll_clipboard_search(receiver, self.clipboard.query_generation, cx);
    }

    pub(super) fn poll_clipboard_search(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        request_id: u64,
        cx: &mut Context<Self>,
    ) {
        let screen = self.screen;
        let generation = self.live_state.generation;
        cx.spawn(async move |this, cx| {
            let Some(event) = receiver.recv().await else {
                let _ = this.update(cx, |this, cx| {
                    if request_id != this.clipboard.query_generation
                        || screen != this.screen
                        || generation != this.live_state.generation
                    {
                        return;
                    }
                    this.clipboard.loading = false;
                    let message = if this.screen == Screen::Settings {
                        l10n::clipboard_settings_failed(this.locale)
                    } else {
                        l10n::history_operation_failed(this.locale)
                    };
                    this.set_notice(message, components::NoticeKind::Error);
                    cx.notify();
                });
                return;
            };
            let _ = this.update(cx, |this, cx| {
                if request_id != this.clipboard.query_generation
                    || screen != this.screen
                    || generation != this.live_state.generation
                {
                    return;
                }
                match event {
                    LiveEvent::Clipboard(snapshot) => this.clipboard.apply(snapshot),
                    LiveEvent::Error(error) => {
                        this.clipboard.loading = false;
                        this.set_notice(error, components::NoticeKind::Error);
                    }
                    event => this.apply_auxiliary_event(event, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn poll_clipboard_operation(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        operation_id: u64,
        copied_item: Option<i64>,
        return_to_application: bool,
        failure: ClipboardOperationFailure,
        cx: &mut Context<Self>,
    ) {
        let screen = self.screen;
        let generation = self.live_state.generation;
        let query_generation = self.clipboard.query_generation;
        cx.spawn(async move |this, cx| {
            let event = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                if operation_id != this.clipboard.operation_generation
                    || screen != this.screen
                    || generation != this.live_state.generation
                {
                    return;
                }
                let same_query = query_generation == this.clipboard.query_generation;
                match event {
                    Some(LiveEvent::Clipboard(snapshot)) if same_query => {
                        this.clipboard.apply(snapshot);
                        // Any earlier in-flight search was read before this operation.
                        this.clipboard.query_generation =
                            this.clipboard.query_generation.wrapping_add(1);
                        if copied_item.is_none() || copied_item == this.clipboard.selected_id() {
                            this.clipboard.copied_item = copied_item;
                            this.clear_notice();
                            if return_to_application {
                                this.return_after_clipboard_copy();
                            }
                        }
                    }
                    Some(LiveEvent::Clipboard(_)) => {
                        // The command has already run. Read the current query
                        // again so an earlier read cannot resurrect a deleted row.
                        if screen == Screen::Settings {
                            this.request_clipboard_settings_status(cx);
                        } else {
                            this.request_clipboard_search(cx);
                        }
                    }
                    Some(LiveEvent::Error(error)) if same_query => {
                        this.clipboard.loading = false;
                        let message = this.clipboard_operation_failure(failure);
                        this.set_notice(
                            format!("{message} {error}"),
                            components::NoticeKind::Error,
                        );
                    }
                    None if same_query => {
                        this.clipboard.loading = false;
                        let message = this.clipboard_operation_failure(failure);
                        this.set_notice(message, components::NoticeKind::Error);
                    }
                    Some(LiveEvent::Ignored) if same_query => this.clipboard.loading = false,
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn clipboard_operation_failure(&self, failure: ClipboardOperationFailure) -> String {
        match failure {
            ClipboardOperationFailure::Copy => l10n::copy_failed(self.locale),
            ClipboardOperationFailure::Delete => l10n::history_operation_failed(self.locale),
            ClipboardOperationFailure::Settings => l10n::clipboard_settings_failed(self.locale),
        }
    }

    fn return_after_clipboard_copy(&mut self) {
        if !crate::platform::clipboard_shelf_is_key_window() {
            return;
        }
        match crate::platform::return_to_previous_application() {
            crate::platform::ApplicationReturnResult::Returned => {}
            crate::platform::ApplicationReturnResult::Unavailable => self.set_notice(
                l10n::shelf_return_unavailable(self.locale),
                components::NoticeKind::Warning,
            ),
            crate::platform::ApplicationReturnResult::Failed => self.set_notice(
                l10n::shelf_return_failed(self.locale),
                components::NoticeKind::Warning,
            ),
        }
    }

    pub(super) fn copy_selected_clipboard(&mut self, cx: &mut Context<Self>) {
        self.copy_selected_clipboard_with_destination(false, cx);
    }

    pub(super) fn copy_selected_clipboard_and_return(&mut self, cx: &mut Context<Self>) {
        self.copy_selected_clipboard_with_destination(true, cx);
    }

    pub(super) fn copy_selected_clipboard_with_destination(
        &mut self,
        return_to_application: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(live) = self.live.clone() else {
            return;
        };
        let Some(item_id) = self.clipboard.selected_item().map(|item| item.id) else {
            return;
        };
        self.clipboard.operation_generation = self.clipboard.operation_generation.wrapping_add(1);
        let operation_id = self.clipboard.operation_generation;
        let receiver = live.copy_clipboard(item_id, self.clipboard.query.clone());
        self.poll_clipboard_operation(
            receiver,
            operation_id,
            Some(item_id),
            return_to_application,
            ClipboardOperationFailure::Copy,
            cx,
        );
        self.clipboard.loading();
        cx.notify();
    }

    pub(super) fn preview_selected_clipboard(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(item)) = (self.live.clone(), self.clipboard.selected_item()) {
            let receiver = live.preview_clipboard(item.id);
            self.poll_preview(receiver, cx);
        }
    }

    pub(super) fn confirm_clipboard_delete(&mut self, cx: &mut Context<Self>) {
        // Selection may change when a new snapshot arrives. Authorization is
        // tied to the item displayed by the confirmation, never the new row.
        if let Some(item_id) = self.clipboard.delete_pending {
            if self.select_clipboard_item(item_id) {
                self.delete_selected_clipboard(cx);
            }
        }
    }

    pub(super) fn delete_selected_clipboard(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            return;
        };
        let Some(item_id) = self.clipboard.selected_item().map(|item| item.id) else {
            return;
        };
        self.clipboard.operation_generation = self.clipboard.operation_generation.wrapping_add(1);
        let operation_id = self.clipboard.operation_generation;
        let receiver = live.delete_clipboard(item_id, self.clipboard.query.clone());
        self.poll_clipboard_operation(
            receiver,
            operation_id,
            None,
            false,
            ClipboardOperationFailure::Delete,
            cx,
        );
        self.clipboard.loading();
        cx.notify();
    }

    pub(super) fn select_clipboard_item(&mut self, item_id: i64) -> bool {
        let Some(index) = self
            .clipboard
            .items()
            .iter()
            .position(|item| item.id == item_id)
        else {
            return false;
        };
        self.clipboard.selected = index;
        self.clipboard.browsing = true;
        true
    }

    pub(super) fn open_clipboard_actions(&mut self, item_id: i64, cx: &mut Context<Self>) {
        if !self.select_clipboard_item(item_id) {
            return;
        }
        if self.preview_open {
            self.close_preview(cx);
        }
        self.clipboard.actions_open = true;
        self.clipboard.action_target = Some(item_id);
        self.clipboard.action_selected = 0;
        self.clipboard.delete_pending = None;
        self.notice = None;
        cx.notify();
    }

    pub(super) fn open_empty_clipboard_actions(&mut self, cx: &mut Context<Self>) {
        if self.preview_open {
            self.close_preview(cx);
        }
        self.clipboard.actions_open = true;
        self.clipboard.action_target = None;
        self.clipboard.action_selected = Self::clipboard_refresh_action(self.screen);
        self.clipboard.delete_pending = None;
        self.notice = None;
        cx.notify();
    }

    pub(super) fn clipboard_refresh_action(screen: Screen) -> usize {
        if screen == Screen::ClipboardShelf {
            4
        } else {
            3
        }
    }

    pub(super) fn close_clipboard_actions(&mut self, cx: &mut Context<Self>) {
        self.clipboard.actions_open = false;
        self.clipboard.action_target = None;
        self.clipboard.action_selected = 0;
        cx.notify();
    }

    pub(super) fn move_clipboard_action(
        &mut self,
        direction: i8,
        actions: usize,
        cx: &mut Context<Self>,
    ) {
        if actions == 0 {
            return;
        }
        let target_exists = self
            .clipboard
            .action_target
            .is_some_and(|id| self.clipboard.items().iter().any(|item| item.id == id));
        let refresh_action = Self::clipboard_refresh_action(self.screen);
        for step in 1..=actions {
            let candidate = if direction < 0 {
                (self.clipboard.action_selected + actions - step) % actions
            } else {
                (self.clipboard.action_selected + step) % actions
            };
            if target_exists || candidate == refresh_action {
                self.clipboard.action_selected = candidate;
                break;
            }
        }
        cx.notify();
    }

    pub(super) fn request_delete_clipboard_item(&mut self, item_id: i64, cx: &mut Context<Self>) {
        if !self.select_clipboard_item(item_id) {
            return;
        }
        self.clipboard.actions_open = false;
        self.clipboard.action_target = None;
        self.clipboard.delete_pending = Some(item_id);
        self.clipboard.delete_confirm_selected = false;
        self.notice = None;
        cx.notify();
    }

    pub(super) fn execute_clipboard_action(&mut self, action: usize, cx: &mut Context<Self>) {
        if action == Self::clipboard_refresh_action(self.screen) {
            self.clipboard.actions_open = false;
            self.clipboard.action_target = None;
            self.clipboard.action_selected = 0;
            self.request_clipboard_search(cx);
            cx.notify();
            return;
        }
        let Some(item_id) = self.clipboard.action_target else {
            return;
        };
        if !self.select_clipboard_item(item_id) {
            self.close_clipboard_actions(cx);
            return;
        }
        self.clipboard.actions_open = false;
        self.clipboard.action_target = None;
        self.clipboard.action_selected = 0;
        match action {
            0 => self.copy_selected_clipboard(cx),
            1 if self.screen == Screen::ClipboardShelf => {
                self.copy_selected_clipboard_and_return(cx)
            }
            1 => self.preview_selected_clipboard(cx),
            2 if self.screen == Screen::ClipboardShelf => self.preview_selected_clipboard(cx),
            2 => self.delete_selected_clipboard(cx),
            3 if self.screen == Screen::ClipboardShelf => {
                self.request_delete_clipboard_item(item_id, cx)
            }
            3 => self.request_clipboard_search(cx),
            4 if self.screen == Screen::ClipboardShelf => self.request_clipboard_search(cx),
            _ => {}
        }
    }

    pub(super) fn run_clipboard_command(
        &mut self,
        receiver: Receiver<LiveEvent>,
        cx: &mut Context<Self>,
    ) {
        self.clipboard.operation_generation = self.clipboard.operation_generation.wrapping_add(1);
        let operation_id = self.clipboard.operation_generation;
        self.poll_clipboard_operation(
            receiver,
            operation_id,
            None,
            false,
            ClipboardOperationFailure::Settings,
            cx,
        );
        self.clipboard.loading();
        cx.notify();
    }

    pub(super) fn reset_clipboard_browsing(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.screen, Screen::Clipboard | Screen::ClipboardShelf) {
            return;
        }
        self.clipboard.reset_selection();
        self.clipboard.actions_open = false;
        self.clipboard.action_target = None;
        self.clipboard.action_selected = 0;
        self.clipboard.delete_pending = None;
        self.clipboard.delete_confirm_selected = false;
        self.clipboard.copied_item = None;
        self.close_preview(cx);
    }

    pub(super) fn clipboard_selection_active(&self) -> bool {
        matches!(self.screen, Screen::Clipboard | Screen::ClipboardShelf)
            && self.clipboard.selected_item().is_some()
    }

    pub(super) fn select_clipboard(&mut self, direction: i8, cx: &mut Context<Self>) {
        if direction < 0 {
            self.clipboard.select_previous();
        } else {
            self.clipboard.select_next();
        }
        self.refresh_clipboard_selection(cx);
    }

    pub(super) fn refresh_clipboard_selection(&mut self, cx: &mut Context<Self>) {
        self.reveal_result_row(self.clipboard.selected);
        self.clipboard.copied_item = None;
        self.notice = None;
        if self.preview_open {
            self.preview_selected_clipboard(cx);
        }
        cx.notify();
        self.refresh_current_window(cx);
    }

    pub(super) fn select_clipboard_row(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clipboard.browsing = true;
        self.clipboard.selected = index;
        self.refresh_clipboard_selection(cx);
        let clipboard_input = self.clipboard_input.clone();
        cx.defer_in(window, move |this, window, cx| {
            if matches!(this.screen, Screen::Clipboard | Screen::ClipboardShelf)
                && this.clipboard.selected == index
            {
                clipboard_input.update(cx, |input, cx| input.focus(window, cx));
                cx.notify();
            }
        });
        cx.notify();
    }
}
