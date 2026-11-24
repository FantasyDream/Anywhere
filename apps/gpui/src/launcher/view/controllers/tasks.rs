use super::*;

impl LauncherView {
    pub fn tasks_window(
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
            Some(live),
            None,
            ViewKind::Tasks,
        );
        view.screen = Screen::Tasks;
        view.tasks_filter_input.update(cx, |input, cx| {
            input.set_placeholder(l10n::find_atask(locale), window, cx)
        });
        view.standalone_tasks = true;
        view.presence.presented = true;
        view.surface_focus_handle.focus(window, cx);
        view.resize_window_for_screen(window, cx);
        view
    }

    pub fn initialize_tasks_window(
        &mut self,
        selected_task_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.live.is_none() {
            return;
        }
        self.tasks.pending_selection = selected_task_id;
        self.start_tasks_refresh(cx);
        cx.notify();
    }

    pub fn select_execution(&mut self, task_id: String, cx: &mut Context<Self>) {
        self.tasks.pending_selection = Some(task_id);
        if self.apply_pending_task_selection(cx) {
            self.refresh_selected_task_detail(cx);
            self.reveal_result_row(self.tasks.selected);
        }
        cx.notify();
    }

    pub(crate) fn handle_tasks_navigation_key(
        &mut self,
        key: &str,
        repeated: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.handle_key_with_repeat(key, KeyModifiers::default(), repeated, window, cx);
    }

    #[cfg(test)]
    pub(crate) fn prepare_tasks_launcher_restore_for_test(&mut self) {
        self.screen = Screen::Tasks;
        self.tasks.tool_options_open = true;
        self.tasks.tool_option_selected = 1;
    }

    pub(super) fn open_selected_execution(&mut self, cx: &mut Context<Self>) {
        if let Some(row_id) = self.tasks.selected_row().map(|row| row.id.clone()) {
            self.request_task_detail(row_id, cx);
        }
    }

    pub(super) fn request_task_detail(&mut self, task_id: String, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            return;
        };
        let Some(request_id) = self.tasks.begin_detail_request() else {
            return;
        };
        let receiver = live.execution_detail(task_id.clone());
        self.poll_task_detail(receiver, request_id, task_id, cx);
    }

    pub(super) fn refresh_selected_task_detail(&mut self, cx: &mut Context<Self>) {
        if !self.tasks.detail_is_current() {
            if let Some(task_id) = self.tasks.selected_row().map(|row| row.id.clone()) {
                self.request_task_detail(task_id, cx);
            }
        }
    }

    pub(super) fn poll_task_detail(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        request_id: u64,
        task_id: String,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let event = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                if !this.tasks.finish_detail_request(request_id)
                    || this.screen != Screen::Tasks
                    || !this.presence.presented
                    || !this
                        .tasks
                        .selected_row()
                        .is_some_and(|row| row.id == task_id)
                {
                    return;
                }
                match event {
                    Some(LiveEvent::ExecutionDetail(detail)) => {
                        this.tasks.apply_detail(detail);
                        if this.tasks.follow_output {
                            this.tasks.request_follow_output();
                        }
                        // The list may have advanced while the detail was loading.
                        this.refresh_selected_task_detail(cx);
                    }
                    Some(LiveEvent::Error(error)) => {
                        this.tasks.clear_detail();
                        this.set_notice(error, components::NoticeKind::Error);
                    }
                    Some(event) => this.apply_auxiliary_event(event, cx),
                    None => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn stop_selected_execution(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(row)) = (self.live.clone(), self.tasks.selected_row()) {
            let receiver = live.stop_execution(row.id.clone());
            self.poll(receiver, self.live_state.generation, cx);
        }
    }

    pub(super) fn remove_selected_execution(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(row)) = (self.live.clone(), self.tasks.selected_row()) {
            if row.status.is_active() {
                return;
            }
            let receiver = live.remove_execution(row.id.clone());
            self.poll(receiver, self.live_state.generation, cx);
            cx.notify();
        }
    }

    pub(super) fn open_tool_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tasks.tool_options_open = true;
        self.tasks.tool_option_selected = 0;
        self.tasks.task_actions_open = false;
        self.tasks.task_action_selected = 0;
        self.notice = None;
        self.surface_focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(super) fn close_tool_options(&mut self, cx: &mut Context<Self>) {
        self.tasks.tool_options_open = false;
        self.tasks.tool_option_selected = 0;
        cx.notify();
    }

    pub(super) fn move_tool_option(&mut self, cx: &mut Context<Self>) {
        self.tasks.tool_option_selected = 1 - self.tasks.tool_option_selected;
        cx.notify();
    }

    pub(super) fn execute_tool_option(&mut self, option: usize, cx: &mut Context<Self>) {
        self.tasks.tool_options_open = false;
        self.tasks.tool_option_selected = 0;
        match option {
            0 => self.start_tasks_refresh(cx),
            1 => cx.emit(LauncherEvent::RestoreLauncher),
            _ => {}
        }
        cx.notify();
    }

    pub(super) fn open_task_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.tasks.detail.as_ref() else {
            return;
        };
        if detail.row.status.is_active() {
            self.tasks.task_action_selected = 0;
        } else {
            self.tasks.task_action_selected = self
                .tasks
                .task_action_selected
                .min(Self::task_action_count(detail.row.status) - 1);
        }
        self.tasks.task_actions_open = true;
        self.tasks.tool_options_open = false;
        self.tasks.tool_option_selected = 0;
        self.notice = None;
        self.surface_focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(super) fn close_task_actions(&mut self, cx: &mut Context<Self>) {
        self.tasks.task_actions_open = false;
        self.tasks.task_action_selected = 0;
        cx.notify();
    }

    pub(super) fn task_action_count(status: anywhere_protocol::ExecutionSessionStatus) -> usize {
        if status.is_active() {
            1
        } else {
            2
        }
    }

    pub(super) fn move_task_action(&mut self, direction: i8, cx: &mut Context<Self>) {
        let Some(detail) = self.tasks.detail.as_ref() else {
            self.close_task_actions(cx);
            return;
        };
        let actions = Self::task_action_count(detail.row.status);
        if actions == 0 {
            return;
        }
        self.tasks.task_action_selected = if direction < 0 {
            (self.tasks.task_action_selected + actions - 1) % actions
        } else {
            (self.tasks.task_action_selected + 1) % actions
        };
        cx.notify();
    }

    pub(super) fn execute_task_action(&mut self, action: usize, cx: &mut Context<Self>) {
        let Some(detail) = self.tasks.detail.clone() else {
            self.close_task_actions(cx);
            return;
        };
        self.tasks.task_actions_open = false;
        self.tasks.task_action_selected = 0;
        match action {
            0 => {
                cx.write_to_clipboard(ClipboardItem::new_string(detail.row.command));
                self.set_notice(
                    l10n::command_copied(self.locale),
                    components::NoticeKind::Success,
                );
                cx.notify();
            }
            1 if !detail.row.status.is_active() => self.remove_selected_execution(cx),
            _ => {}
        }
    }

    pub(super) fn start_tasks_refresh(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            return;
        };
        self.stop_tasks_follow();
        self.clear_notice();
        self.tasks.refresh_generation = self.tasks.refresh_generation.wrapping_add(1);
        let generation = self.tasks.refresh_generation;
        let (cancel, mut cancelled) = oneshot::channel();
        self.tasks.refresh_cancel = Some(cancel);
        let mut receiver = live.follow_executions();
        cx.spawn(async move |this, cx| loop {
            let event = tokio::select! {
                _ = &mut cancelled => break,
                event = receiver.recv() => event,
            };
            let Some(event) = event else {
                break;
            };
            let should_continue = this
                .update(cx, |this, cx| {
                    if generation != this.tasks.refresh_generation
                        || this.screen != Screen::Tasks
                        || !this.presence.presented
                    {
                        return false;
                    }
                    match event {
                        LiveEvent::Executions(rows) => {
                            this.apply_tasks_rows(rows, cx);
                            cx.notify();
                        }
                        LiveEvent::Error(error) => {
                            this.set_notice(error, components::NoticeKind::Error);
                            cx.notify();
                        }
                        event => this.apply_auxiliary_event(event, cx),
                    }
                    true
                })
                .unwrap_or(false);
            if !should_continue {
                break;
            }
        })
        .detach();
    }

    pub(super) fn apply_tasks_rows(
        &mut self,
        rows: Vec<crate::host::ExecutionRow>,
        cx: &mut Context<Self>,
    ) {
        let had_tasks = !self.tasks.rows.is_empty();
        let previous_selection = self.tasks.selected_row().map(|row| row.id.clone());
        self.tasks.apply_rows(rows);
        if had_tasks && self.tasks.rows.is_empty() {
            let window_handle = self.window_handle;
            let view = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = window_handle.update(cx, |_, window, cx| {
                    let _ = view.update(cx, |this, cx| {
                        if this.screen == Screen::Tasks && this.tasks.rows.is_empty() {
                            this.surface_focus_handle.focus(window, cx);
                        }
                    });
                });
            });
        }
        let reveal_selection = self.apply_pending_task_selection(cx);
        self.refresh_selected_task_detail(cx);
        if reveal_selection
            || self.tasks.selected_row().map(|row| row.id.as_str()) != previous_selection.as_deref()
        {
            self.reveal_result_row(self.tasks.selected);
        }
    }

    fn apply_pending_task_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(task_id) = self.tasks.pending_selection.as_deref() else {
            return false;
        };
        let Some(index) = self.tasks.rows.iter().position(|row| row.id == task_id) else {
            return false;
        };
        self.tasks.pending_selection = None;
        if !self.tasks.visible_indices().contains(&index) {
            self.tasks.set_filter(String::new());
            let previous_input = self.tasks_filter_input.read(cx).value().to_string();
            let window_handle = self.window_handle;
            let view = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = window_handle.update(cx, |_, window, cx| {
                    let _ = view.update(cx, |this, cx| {
                        if this.screen == Screen::Tasks
                            && this.tasks.filter.is_empty()
                            && this.tasks_filter_input.read(cx).value().as_str() == previous_input
                        {
                            set_input_value(&this.tasks_filter_input, "", window, cx);
                        }
                    });
                });
            });
        }
        self.tasks.selected = index;
        self.tasks.clear_detail();
        self.tasks.tool_options_open = false;
        self.tasks.tool_option_selected = 0;
        self.tasks.task_actions_open = false;
        self.tasks.task_action_selected = 0;
        true
    }

    pub(super) fn cancel_tasks_refresh(&mut self) {
        self.stop_tasks_follow();
        self.tasks.clear_detail();
    }

    fn stop_tasks_follow(&mut self) {
        if let Some(cancel) = self.tasks.refresh_cancel.take() {
            let _ = cancel.send(());
        }
    }

    pub(in crate::launcher::view) fn handle_tasks_overlay_keys(
        &mut self,
        input: &crate::launcher::view::input::KeyInput<'_>,
        cx: &mut Context<Self>,
    ) -> crate::launcher::view::input::KeyFlow {
        use crate::launcher::view::input::KeyFlow;
        if self.screen != Screen::Tasks {
            return KeyFlow::Continue;
        }
        if self.tasks.tool_options_open {
            if input.modifiers.command && input.key.eq_ignore_ascii_case("k") {
                self.close_tool_options(cx);
                return KeyFlow::Handled;
            }
            if !input.modifiers.modified {
                match input.key {
                    "down" | "up" => self.move_tool_option(cx),
                    "enter" | "return" => {
                        self.execute_tool_option(self.tasks.tool_option_selected, cx)
                    }
                    "escape" => self.close_tool_options(cx),
                    _ => {}
                }
            }
            return KeyFlow::Handled;
        }
        if self.tasks.task_actions_open {
            if input.modifiers.command && input.key.eq_ignore_ascii_case("k") {
                self.close_task_actions(cx);
                return KeyFlow::Handled;
            }
            if !input.modifiers.modified {
                match input.key {
                    "down" => self.move_task_action(1, cx),
                    "up" => self.move_task_action(-1, cx),
                    "enter" | "return" => {
                        self.execute_task_action(self.tasks.task_action_selected, cx)
                    }
                    "escape" => self.close_task_actions(cx),
                    _ => {}
                }
            }
            return KeyFlow::Handled;
        }
        KeyFlow::Continue
    }

    pub(in crate::launcher::view) fn handle_tasks_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match key {
            "down" => {
                self.tasks.pending_selection = None;
                if !self.tasks.visible_indices().is_empty() {
                    let previous = self.tasks.selected;
                    self.tasks.select_next_visible();
                    self.reveal_result_row(self.tasks.selected);
                    if self.tasks.selected != previous {
                        self.tasks.clear_detail();
                        if let Some(task_id) = self.tasks.selected_row().map(|row| row.id.clone()) {
                            self.request_task_detail(task_id, cx);
                        }
                    }
                    cx.notify();
                }
            }
            "up" => {
                self.tasks.pending_selection = None;
                if !self.tasks.visible_indices().is_empty() {
                    let previous = self.tasks.selected;
                    self.tasks.select_previous_visible();
                    self.reveal_result_row(self.tasks.selected);
                    if self.tasks.selected != previous {
                        self.tasks.clear_detail();
                        if let Some(task_id) = self.tasks.selected_row().map(|row| row.id.clone()) {
                            self.request_task_detail(task_id, cx);
                        }
                    }
                    cx.notify();
                }
            }
            "enter" | "return" => {
                if self
                    .tasks_filter_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
                {
                    self.surface_focus_handle.focus(window, cx);
                } else {
                    self.open_selected_execution(cx);
                }
            }
            "escape" => {
                if !self.tasks.filter.is_empty() {
                    self.tasks_filter_input
                        .update(cx, |input, cx| input.set_value("", window, cx));
                    self.tasks.set_filter(String::new());
                    self.surface_focus_handle.focus(window, cx);
                    cx.notify();
                } else {
                    self.go_back(window, cx);
                }
            }
            _ => {}
        }
    }
}
