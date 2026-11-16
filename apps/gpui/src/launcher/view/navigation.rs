use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LauncherStreamRecovery {
    KeepActive,
    Resume,
    Restart,
}

impl LauncherView {
    pub(super) fn is_settings_surface(&self) -> bool {
        matches!(self.screen, Screen::Settings | Screen::WebPackages)
    }

    pub fn activate_from_hotkey(
        &mut self,
        input_mode: InputMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.activate_from_entry(input_mode, true, window, cx)
    }

    pub fn activate_from_menu(
        &mut self,
        input_mode: InputMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.activate_from_entry(input_mode, false, window, cx)
    }

    pub fn activate_action_shortcut(
        &mut self,
        binding: ActionShortcutBinding,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.is_settings_surface() || self.shortcut_recording.is_some() {
            return false;
        }
        self.live_state.generation = self.live_state.generation.wrapping_add(1);
        self.reset_for_activation(InputMode::Search, window, cx);
        self.presented = true;
        self.pending_shortcut_execution = Some(binding);
        self.action_shortcut_resolving = None;
        self.begin_new_activation(InputMode::Search, cx);
        true
    }

    pub(super) fn activate_from_entry(
        &mut self,
        input_mode: InputMode,
        toggle: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if matches!(
            self.runtime_state,
            RuntimeState::Starting | RuntimeState::Failed
        ) {
            self.presented = true;
            self.notice = Some(match self.runtime_state {
                RuntimeState::Starting => l10n::starting_anywhere(self.locale),
                RuntimeState::Failed => self
                    .notice
                    .clone()
                    .unwrap_or_else(|| l10n::launcher_startup_failed(self.locale)),
                RuntimeState::Preview | RuntimeState::Ready => unreachable!(),
            });
            self.resize_window_for_screen(window);
            self.focus_launcher_input(window, cx);
            cx.notify();
            return true;
        }
        if self.is_settings_surface() {
            let exited =
                self.request_settings_exit(SettingsExitIntent::Wake(input_mode), window, cx);
            if !exited {
                crate::platform::activate_window_after_update(crate::PRODUCT_NAME);
            }
            return exited;
        }
        if self.presented
            && toggle
            && self.input_mode == input_mode
            && self.screen == Screen::Launcher
        {
            self.prepare_hide(cx);
            crate::platform::hide_main_window();
            return false;
        }

        let was_presented = self.presented;
        self.presented = true;
        if was_presented && self.screen == Screen::Launcher && self.input_mode == input_mode {
            self.focus_launcher_input(window, cx);
            return true;
        }

        if was_presented && self.screen == Screen::Launcher {
            let previous_mode = self.input_mode;
            self.input_mode = input_mode;
            self.root_navigation_mode = input_mode;
            self.model.hide_object_actions();
            self.model.select_scope(None);
            self.begin_navigation_scope_selection(input_mode);
            self.notice = None;
            self.sync_mode_editor(previous_mode, input_mode, window, cx);
            self.focus_launcher_input(window, cx);
            self.request_live_update(cx);
            cx.notify();
            return true;
        }

        let can_resume = self.retained_workspace
            && self.hidden_at.is_some_and(|hidden_at| {
                hidden_at.elapsed()
                    < Duration::from_millis(anywhere_protocol::policy::LAUNCHER_RETENTION_MS)
            })
            && self.live_state.session_id.is_some();
        if can_resume {
            self.hidden_at = None;
            self.retained_workspace = false;
            self.begin_resume(cx);
            if self.screen == Screen::Tasks {
                self.start_tasks_refresh(cx);
            } else if self.screen == Screen::ApplicationHistory {
                if self.app_history.session_id.is_some() {
                    self.request_app_history_search(cx);
                } else {
                    self.start_app_history_session(cx);
                }
            }
            self.focus_active_editor(window, cx);
            return true;
        }

        self.reset_for_activation(input_mode, window, cx);
        self.begin_new_activation(input_mode, cx);
        true
    }

    fn sync_mode_editor(
        &self,
        previous_mode: InputMode,
        input_mode: InputMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if previous_mode == InputMode::Content && input_mode != InputMode::Content {
            set_input_value(
                &self.input,
                self.content_input.read(cx).value().to_string(),
                window,
                cx,
            );
        } else if previous_mode != InputMode::Content && input_mode == InputMode::Content {
            set_textarea_value(
                &self.content_input,
                self.input.read(cx).value().to_string(),
                window,
                cx,
            );
        }
    }

    fn reset_for_activation(
        &mut self,
        input_mode: InputMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pending_shortcut_execution = None;
        self.action_shortcut_resolving = None;
        self.release_app_history_session();
        self.close_web_surface(cx);
        self.parent_workspace = None;
        let previous_mode = self.input_mode;
        self.screen = Screen::Launcher;
        self.input_mode = input_mode;
        self.root_navigation_open = false;
        self.root_navigation_mode = input_mode;
        self.model.hide_object_actions();
        self.model.select_scope(None);
        self.begin_scope_selection_for_mode(input_mode);
        self.reveal_selected_scope();
        self.notice = None;
        self.resize_window_for_screen(window);
        self.sync_mode_editor(previous_mode, input_mode, window, cx);
        self.focus_launcher_input(window, cx);
    }

    fn begin_new_activation(&mut self, input_mode: InputMode, cx: &mut Context<Self>) {
        if let Some(live) = self.live.clone() {
            match live.reserve_activation() {
                Ok(session_id) => {
                    self.live_state.session_id = Some(session_id);
                    self.live_state.snapshot_gate = Some(SnapshotGate::new(session_id));
                    self.live_state.request_revision = 0;
                    self.retained_workspace = false;
                    self.hidden_at = None;
                    self.begin_stream(live.activate_session(session_id, input_mode), cx);
                }
                Err(error) => {
                    self.notice = Some(l10n::could_not_activate_anywhere(
                        self.locale,
                        &format!("{error:#}"),
                    ));
                }
            }
        }
        cx.notify();
    }

    fn begin_resume(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            return;
        };
        if let Some(session_id) = self.live_state.session_id {
            self.live_state.snapshot_gate = Some(SnapshotGate::new(session_id));
            self.live_state.request_revision = self.model.revision;
            self.begin_stream(live.resume_current(), cx);
        }
        cx.notify();
    }

    pub(super) fn try_start_pending_action_shortcut(&mut self, cx: &mut Context<Self>) {
        if self.action_shortcut_resolving.is_some() {
            return;
        }
        let Some(binding) = self.pending_shortcut_execution.take() else {
            return;
        };
        if self.model.revision == 0 {
            self.pending_shortcut_execution = Some(binding);
            return;
        }
        self.start_action_shortcut_resolution(binding, cx);
    }

    pub(super) fn start_action_shortcut_resolution(
        &mut self,
        binding: ActionShortcutBinding,
        cx: &mut Context<Self>,
    ) {
        if self.action_shortcut_resolving.is_some() {
            return;
        }
        let (Some(live), Some(session_id)) = (self.live.clone(), self.live_state.session_id) else {
            return;
        };
        self.action_shortcut_resolving = Some(binding.action_id.clone());
        let receiver = live.wait_action_shortcut(session_id, self.model.revision, binding);
        self.poll(receiver, self.live_state.generation, cx);
    }

    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.runtime_state,
            RuntimeState::Starting | RuntimeState::Failed
        ) {
            self.notice = Some(match self.runtime_state {
                RuntimeState::Starting => l10n::starting_anywhere(self.locale),
                RuntimeState::Failed => self
                    .notice
                    .clone()
                    .unwrap_or_else(|| l10n::launcher_startup_failed(self.locale)),
                RuntimeState::Preview | RuntimeState::Ready => unreachable!(),
            });
            cx.notify();
            return;
        }
        if self.is_settings_surface() {
            return;
        }
        self.screen = Screen::Settings;
        self.root_navigation_open = false;
        self.shortcut_recording = None;
        self.settings_action_picker_open = false;
        self.pending_action_shortcut = None;
        self.pending_action_shortcut_value = None;
        self.context_shortcut_draft = None;
        self.presented = true;
        self.notice = None;
        self.resize_window_for_screen(window);
        crate::platform::configure_settings_window_after_update(PRODUCT_NAME);
        self.request_shortcut_catalog(cx);
        if self.settings.selected == SettingsCategory::ApplicationHistory {
            self.request_app_history_settings_sources(cx);
        }
        let settings_focus_handle = self.settings_focus_handle.clone();
        cx.defer_in(window, move |this, window, cx| {
            if this.screen == Screen::Settings {
                settings_focus_handle.focus(window, cx);
                cx.notify();
            }
        });
        cx.notify();
    }

    pub(super) fn settings_launcher_recovery(&self) -> LauncherStreamRecovery {
        if self.live_state.cancel.is_some() {
            return LauncherStreamRecovery::KeepActive;
        }
        if self.retained_workspace
            && self.live_state.session_id.is_some()
            && self.hidden_at.is_some_and(|hidden_at| {
                hidden_at.elapsed()
                    < Duration::from_millis(anywhere_protocol::policy::LAUNCHER_RETENTION_MS)
            })
        {
            return LauncherStreamRecovery::Resume;
        }
        LauncherStreamRecovery::Restart
    }

    pub(super) fn recover_launcher_session_after_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.settings_launcher_recovery() {
            LauncherStreamRecovery::KeepActive => {}
            LauncherStreamRecovery::Resume => {
                self.hidden_at = None;
                self.retained_workspace = false;
                self.begin_resume(cx);
            }
            LauncherStreamRecovery::Restart => {
                self.hidden_at = None;
                self.retained_workspace = false;
                let mode = self.input_mode;
                self.reset_for_activation(mode, window, cx);
                self.begin_new_activation(mode, cx);
            }
        }
    }

    pub(crate) fn focus_settings_navigation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_settings_surface() {
            self.settings_focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    fn request_shortcut_catalog(&mut self, cx: &mut Context<Self>) {
        if self.shortcut_catalog_loading {
            return;
        }
        let Some(live) = self.live.clone() else {
            return;
        };
        self.shortcut_catalog_loading = true;
        let receiver = live.shortcut_catalog();
        self.poll(receiver, self.live_state.generation, cx);
    }

    pub(super) fn request_app_history_settings_sources(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            return;
        };
        let receiver = live.app_history_sources();
        self.poll(receiver, self.live_state.generation, cx);
    }

    pub(crate) fn request_main_window_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.is_settings_surface() {
            self.request_close_settings(window, cx);
            false
        } else {
            self.prepare_hide(cx);
            true
        }
    }

    pub(crate) fn request_application_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_settings_surface() {
            self.request_settings_exit(SettingsExitIntent::Quit, window, cx);
        } else {
            cx.emit(LauncherEvent::QuitRequested);
        }
    }

    pub fn handle_settings_escape_from_platform(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.is_settings_surface() {
            return false;
        }
        self.handle_key(
            "escape",
            KeyModifiers {
                command: false,
                control: false,
                shift: false,
                alt: false,
                modified: false,
            },
            window,
            cx,
        );
        true
    }

    pub fn handle_web_view_escape_from_platform(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.screen != Screen::WebTool {
            return false;
        }
        self.handle_key(
            "escape",
            KeyModifiers {
                command: false,
                control: false,
                shift: false,
                alt: false,
                modified: false,
            },
            window,
            cx,
        );
        true
    }

    pub fn dismiss_on_focus_loss(&self) -> bool {
        !self.preview_open
            && !self.preview_busy
            && !self.is_settings_surface()
            && !self.standalone_shelf
            && !self.standalone_tasks
    }

    pub fn prepare_hide(&mut self, cx: &mut Context<Self>) {
        self.presented = false;
        self.pending_shortcut_execution = None;
        self.action_shortcut_resolving = None;
        self.cancel_stream();
        self.live_state.generation = self.live_state.generation.wrapping_add(1);
        self.cancel_tasks_refresh();
        self.tasks.refresh_generation = self.tasks.refresh_generation.wrapping_add(1);
        self.clipboard.operation_generation = self.clipboard.operation_generation.wrapping_add(1);
        self.app_history.session_generation = self.app_history.session_generation.wrapping_add(1);
        self.app_history.query_generation = self.app_history.query_generation.wrapping_add(1);
        if self.screen == Screen::ApplicationHistory {
            if let (Some(live), Some(session_id)) = (self.live.clone(), self.app_history.session_id)
            {
                live.cancel_app_history_session(session_id);
            }
        }
        if !self.standalone_shelf && !self.standalone_tasks {
            if let Some(live) = &self.live {
                if let Err(error) = live.suspend_current_now() {
                    tracing::warn!(%error, "launcher_suspend_failed");
                }
            }
            self.retained_workspace = self.should_retain_workspace();
            self.hidden_at = self.retained_workspace.then(Instant::now);
        }
        self.root_navigation_open = false;
        self.model.hide_object_actions();
        self.app_history.close_object_actions();
        self.app_history.empty_actions_open = false;
        self.app_history.fallback_actions_open = false;
        self.web_surface.actions_open = false;
        self.web_surface.action_selected = 0;
        self.tasks.tool_options_open = false;
        self.tasks.tool_option_selected = 0;
        self.tasks.task_actions_open = false;
        self.tasks.task_action_selected = 0;
        self.close_preview(cx);
        if !self.retained_workspace || self.screen != Screen::WebTool {
            self.close_web_surface(cx);
        } else if self.web_surface.view.is_some() {
            let _ = self.evaluate_web_script(&visibility_script(false), cx);
            self.web_surface.ready = false;
            self.web_surface.state_handle = None;
            self.web_surface.state_version = 0;
        }
        if !self.retained_workspace {
            self.application_icons_requested.clear();
            self.application_icons.clear();
        }
    }

    fn should_retain_workspace(&self) -> bool {
        if self.standalone_shelf || self.standalone_tasks {
            return false;
        }
        self.screen != Screen::Launcher
            || self.input_mode == InputMode::Content
            || !self.model.query.trim().is_empty()
            || self.model.object_actions_active
            || self.model.menu_path().is_some()
            || self
                .model
                .selected_scope
                .as_deref()
                .is_some_and(|selected| {
                    self.model
                        .scopes
                        .iter()
                        .any(|scope| scope.id == selected && !scope.is_current_app)
                })
    }
}
