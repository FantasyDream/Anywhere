use super::{
    excluded_app_ids, l10n, lines, locale_for_language, set_input_value, shortcut_action_title,
    ActionShortcutBinding, ActionShortcutRoute, ContextShortcutDraft, HostPreferences,
    LauncherEvent, LauncherView, LiveEvent, Screen, SettingsExitIntent, ShortcutCatalogAction,
};
use gpui_kit::{Context, Focusable as _, Window};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::Receiver;

use crate::host::HotKeyController;
use crate::launcher::components;
use std::sync::{Arc, Mutex};

impl LauncherView {
    pub(super) fn preferences_from_editors(&self, cx: &Context<Self>) -> HostPreferences {
        let mut preferences = self.settings_draft.clone();
        preferences.excluded_apps =
            excluded_app_ids(self.excluded_apps_input.read(cx).value().as_ref());
        preferences.file_search_roots = lines(self.file_roots_input.read(cx).value().as_ref());
        preferences.file_search_excluded_names =
            lines(self.file_exclusions_input.read(cx).value().as_ref());
        preferences.current_app_shortcut = self
            .current_shortcut_input
            .read(cx)
            .value()
            .trim()
            .to_owned();
        preferences.universal_shortcut = self
            .universal_shortcut_input
            .read(cx)
            .value()
            .trim()
            .to_owned();
        preferences
    }

    pub(super) fn open_context_shortcut_draft(
        &mut self,
        existing: Option<&ActionShortcutBinding>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.shortcut_recording = None;
        self.settings_action_picker_open = false;
        self.pending_action_shortcut = None;
        self.pending_action_shortcut_value = None;
        let draft = if let Some(existing) = existing {
            ContextShortcutDraft {
                action_id: existing.action_id.clone(),
                title: existing.title.clone(),
                shortcut: Some(existing.shortcut.clone()),
                global: existing.global,
                routes: existing.routes.clone(),
            }
        } else {
            let sequence = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            ContextShortcutDraft {
                action_id: format!("intent:{sequence:x}"),
                title: String::new(),
                shortcut: None,
                global: true,
                routes: Vec::new(),
            }
        };
        set_input_value(
            &self.context_shortcut_name_input,
            draft.title.clone(),
            window,
            cx,
        );
        self.context_shortcut_name_input
            .update(cx, |input, cx| input.focus(window, cx));
        self.context_shortcut_draft = Some(draft);
        self.notice = None;
        cx.notify();
    }

    pub(super) fn add_context_shortcut_action(
        &mut self,
        action: &ShortcutCatalogAction,
        cx: &mut Context<Self>,
    ) {
        let Some(draft) = self.context_shortcut_draft.as_mut() else {
            return;
        };
        let mut context_types = action
            .context_types
            .iter()
            .filter(|context_type| context_type.as_str() != "core.launcher")
            .cloned()
            .collect::<Vec<_>>();
        if context_types.is_empty() {
            context_types.push(String::new());
        }
        let title = shortcut_action_title(action, self.locale);
        for context_type in context_types {
            let context_type = (!context_type.is_empty()).then_some(context_type);
            if draft.routes.iter().any(|route| {
                route.action_id == action.action_id
                    && route.app_bundle_id.is_none()
                    && route.context_type == context_type
            }) {
                continue;
            }
            if draft.routes.len() >= anywhere_protocol::policy::MAX_SHORTCUT_ROUTES as usize {
                break;
            }
            draft.routes.push(ActionShortcutRoute {
                action_id: action.action_id.clone(),
                title: title.clone(),
                app_bundle_id: None,
                app_name: None,
                context_type,
            });
        }
        cx.notify();
    }

    pub(super) fn commit_context_shortcut_draft(
        &mut self,
        cx: &Context<Self>,
    ) -> Result<(), String> {
        let Some(draft) = self.context_shortcut_draft.as_ref() else {
            return Ok(());
        };
        let title = self
            .context_shortcut_name_input
            .read(cx)
            .value()
            .trim()
            .to_owned();
        let shortcut = draft
            .shortcut
            .clone()
            .ok_or_else(|| l10n::shortcut_recording_failed_try_again(self.locale))?;
        if title.is_empty() {
            return Err(l10n::shortcut_recording_failed_try_again(self.locale));
        }
        if draft.routes.is_empty() {
            return Err(l10n::no_rules_yet_add_an_action_and(self.locale));
        }
        let binding = ActionShortcutBinding {
            action_id: draft.action_id.clone(),
            title,
            shortcut,
            global: draft.global,
            routes: draft.routes.clone(),
        };
        let mut next = self.settings_draft.clone();
        next.action_shortcuts
            .retain(|candidate| candidate.action_id != binding.action_id);
        next.action_shortcuts.push(binding);
        next.validate().map_err(|error| format!("{error:#}"))?;
        self.settings_draft = next;
        self.context_shortcut_draft = None;
        self.shortcut_recording = None;
        self.settings_action_picker_open = false;
        self.pending_action_shortcut = None;
        self.pending_action_shortcut_value = None;
        Ok(())
    }

    pub(super) fn settings_dirty(&self, cx: &Context<Self>) -> bool {
        self.context_shortcut_draft.is_some()
            || self.preferences_from_editors(cx) != self.preferences
    }

    pub(super) fn save_settings(&mut self, cx: &mut Context<Self>) {
        if self.settings.saving {
            return;
        }
        if let Err(error) = self.commit_context_shortcut_draft(cx) {
            self.set_notice(
                format!("{} {error}", l10n::settings_save_failed(self.locale)),
                components::NoticeKind::Error,
            );
            cx.notify();
            return;
        }
        let preferences = self.preferences_from_editors(cx);
        if let Err(error) = preferences.validate() {
            self.set_notice(
                format!("{} {error:#}", l10n::settings_save_failed(self.locale)),
                components::NoticeKind::Error,
            );
            cx.notify();
            return;
        }
        self.settings_draft = preferences.clone();
        let operation_id = self.begin_settings_operation();
        self.save_preferences(preferences, cx);
        tracing::debug!(operation_id, "settings_save_started");
        cx.notify();
    }

    pub(super) fn begin_settings_operation(&mut self) -> u64 {
        self.settings.saving = true;
        self.settings.error = None;
        self.settings_save_generation = self.settings_save_generation.wrapping_add(1);
        self.settings_save_generation
    }

    pub(super) fn set_clipboard_recording(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.settings.saving {
            return;
        }
        self.settings.clipboard_recording = enabled;
        if let Some(live) = self.live.clone() {
            let operation_id = self.begin_settings_operation();
            self.poll_settings_save(live.set_clipboard_recording(enabled), operation_id, cx);
        }
        cx.notify();
    }

    pub(super) fn change_feature_setting(
        &mut self,
        key: &str,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if self.settings.saving {
            return;
        }
        self.settings.set_feature_enabled(key, enabled);
        if let Some(live) = self.live.clone() {
            let operation_id = self.begin_settings_operation();
            self.poll_settings_save(
                live.change_feature(key.to_owned(), enabled),
                operation_id,
                cx,
            );
        }
        cx.notify();
    }

    fn save_preferences(&mut self, preferences: HostPreferences, cx: &mut Context<Self>) {
        if let Some(live) = self.live.clone() {
            let receiver = live.set_preferences(preferences);
            self.poll_settings_save(receiver, self.settings_save_generation, cx);
        } else {
            self.preferences = preferences;
            self.settings.saving = false;
            if self.settings_exit_intent.is_some() {
                self.finish_settings_exit_after_save(cx);
            }
        }
    }

    pub(super) fn retry_login_item(&mut self, cx: &mut Context<Self>) {
        if self.settings.saving || self.settings_dirty(cx) {
            return;
        }
        if let Some(live) = self.live.clone() {
            let operation_id = self.begin_settings_operation();
            self.poll_settings_save(live.retry_login_item(), operation_id, cx);
            cx.notify();
        }
    }

    pub(super) fn poll_settings_save(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        operation_id: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let event = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                if operation_id != this.settings_save_generation {
                    return;
                }
                this.settings.saving = false;
                match event {
                    Some(LiveEvent::Preferences {
                        preferences,
                        host_error,
                    }) => {
                        this.apply_preferences_result(preferences, host_error, cx);
                        tracing::debug!(operation_id, "settings_save_finished");
                    }
                    Some(LiveEvent::Settings(snapshot)) => {
                        this.settings.apply_snapshot(snapshot);
                        this.notice = None;
                        if this.settings_close_pending && !this.settings_dirty(cx) {
                            this.finish_settings_exit_after_save(cx);
                        }
                    }
                    Some(LiveEvent::LoginItemReconciled(error)) => {
                        if let Some(error) = error.as_ref() {
                            this.set_notice(error.clone(), components::NoticeKind::Error);
                        } else {
                            this.clear_notice();
                            if this.settings_close_pending && !this.settings_dirty(cx) {
                                this.finish_settings_exit_after_save(cx);
                            }
                        }
                        this.settings.login_item_error = error;
                    }
                    Some(LiveEvent::SettingsError {
                        message: error,
                        snapshot,
                    }) => {
                        if let Some(snapshot) = snapshot {
                            this.settings.apply_snapshot(snapshot);
                        }
                        this.settings.error = Some(error.clone());
                        this.set_notice(error, components::NoticeKind::Error);
                        tracing::warn!(operation_id, "settings_save_failed");
                    }
                    Some(LiveEvent::Error(error)) => {
                        this.settings.error = Some(error.clone());
                        this.set_notice(error, components::NoticeKind::Error);
                        tracing::warn!(operation_id, "settings_save_failed");
                    }
                    Some(event) => this.apply_auxiliary_event(event, cx),
                    None => {
                        let error = l10n::settings_save_failed(this.locale);
                        this.settings.error = Some(error.clone());
                        this.set_notice(error, components::NoticeKind::Error);
                        tracing::warn!(operation_id, "settings_save_channel_closed");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn request_close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.request_settings_exit(SettingsExitIntent::Close, window, cx);
    }

    pub(super) fn request_settings_exit(
        &mut self,
        intent: SettingsExitIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.settings.saving {
            self.settings_close_pending = true;
            self.settings_exit_intent = Some(intent);
            self.set_notice(l10n::saving(self.locale), components::NoticeKind::Info);
            self.present_settings_close_prompt(window, cx);
            cx.notify();
            return false;
        }
        if self.settings_dirty(cx) {
            self.settings_close_pending = true;
            self.settings_exit_intent = Some(intent);
            self.set_notice(
                l10n::save_before_close(self.locale),
                components::NoticeKind::Warning,
            );
            self.present_settings_close_prompt(window, cx);
            cx.notify();
            return false;
        }
        self.settings_close_pending = false;
        self.settings_exit_intent = None;
        self.execute_settings_exit(intent, window, cx)
    }

    fn present_settings_close_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.clipboard_confirmation = None;
        self.clipboard_settings_choice = None;
        self.shortcut_recording = None;
        self.settings_action_picker_open = false;
        if self.settings_modal_return_focus.is_none() {
            self.settings_modal_return_focus = window.focused(cx);
        }
        let focus = self.settings_keep_editing_focus.clone();
        cx.defer_in(window, move |this, window, cx| {
            if this.settings_close_pending && this.clipboard_confirmation.is_none() {
                focus.focus(window, cx);
            }
        });
    }

    pub(super) fn cancel_settings_close_prompt(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_close_pending = false;
        self.settings_exit_intent = None;
        self.notice = None;
        self.restore_settings_modal_focus(window, cx);
        cx.notify();
    }

    pub(super) fn restore_settings_modal_focus(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus = self
            .settings_modal_return_focus
            .take()
            .unwrap_or_else(|| self.settings_focus_handle.clone());
        // The scrim is still present in the current frame. Restore only after
        // the modal has been removed from the render tree so the target is
        // focusable and cannot be masked by the tab trap.
        cx.defer_in(window, move |this, window, cx| {
            if this.settings_close_pending || this.clipboard_confirmation.is_some() {
                return;
            }
            focus.focus(window, cx);
        });
    }

    pub(super) fn cycle_settings_modal_focus(
        &self,
        backwards: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_action_picker_open
            && !self.settings_close_pending
            && self.clipboard_confirmation.is_none()
        {
            let original = window.focused(cx);
            self.settings_action_picker_focus.focus(window, cx);
            let mut handles = Vec::new();
            for _ in 0..128 {
                window.focus_next(cx);
                let Some(focused) = window.focused(cx) else {
                    break;
                };
                if !self
                    .settings_action_picker_focus
                    .contains_focused(window, cx)
                    || handles.contains(&focused)
                {
                    break;
                }
                handles.push(focused);
            }
            if handles.is_empty() {
                self.action_shortcut_input
                    .focus_handle(cx)
                    .focus(window, cx);
            } else {
                let current = original
                    .as_ref()
                    .and_then(|original| handles.iter().position(|handle| handle == original));
                let next = match (current, backwards) {
                    (Some(index), true) => (index + handles.len() - 1) % handles.len(),
                    (Some(index), false) => (index + 1) % handles.len(),
                    (None, true) => handles.len() - 1,
                    (None, false) => 0,
                };
                handles[next].focus(window, cx);
            }
            return;
        }
        let handles = if self.settings_close_pending {
            let mut handles = vec![self.settings_keep_editing_focus.clone()];
            if !self.settings.saving {
                handles.push(self.settings_discard_focus.clone());
                if self.settings_dirty(cx) {
                    handles.push(self.settings_save_focus.clone());
                }
            }
            handles
        } else if self.clipboard_confirmation.is_some() {
            let mut handles = vec![self.settings_confirm_cancel_focus.clone()];
            if !self.clipboard.loading {
                handles.push(self.settings_confirm_accept_focus.clone());
            }
            handles
        } else if self.settings_action_picker_open {
            vec![self.action_shortcut_input.focus_handle(cx)]
        } else {
            return;
        };
        let current = window.focused(cx);
        let index = current
            .as_ref()
            .and_then(|current| handles.iter().position(|handle| handle == current))
            .unwrap_or(if backwards { 0 } else { handles.len() - 1 });
        let next = if backwards {
            (index + handles.len() - 1) % handles.len()
        } else {
            (index + 1) % handles.len()
        };
        handles[next].focus(window, cx);
    }

    pub(super) fn save_settings_and_close(&mut self, cx: &mut Context<Self>) {
        if !self.settings_dirty(cx) {
            self.finish_settings_exit_after_save(cx);
            return;
        }
        self.settings_close_pending = true;
        self.save_settings(cx);
    }

    pub(super) fn finish_settings_exit_after_save(&mut self, cx: &mut Context<Self>) {
        self.settings_close_pending = true;
        let window_handle = self.window_handle;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(0))
                .await;
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if this.screen != Screen::Settings
                        || !this.settings_close_pending
                        || this.settings.saving
                        || this.settings_dirty(cx)
                    {
                        return;
                    }
                    let intent = this
                        .settings_exit_intent
                        .take()
                        .unwrap_or(SettingsExitIntent::Close);
                    this.settings_close_pending = false;
                    let _ = this.execute_settings_exit(intent, window, cx);
                });
            });
        })
        .detach();
    }

    fn execute_settings_exit(
        &mut self,
        intent: SettingsExitIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.shortcut_recording = None;
        self.settings_action_picker_open = false;
        self.pending_action_shortcut = None;
        self.pending_action_shortcut_value = None;
        self.context_shortcut_draft = None;
        self.clipboard_settings_choice = None;
        self.clipboard_confirmation = None;
        self.settings_modal_return_focus = None;
        match intent {
            SettingsExitIntent::Close => {
                self.toggle_screen(window, cx);
                true
            }
            SettingsExitIntent::Wake(mode) => {
                self.toggle_screen(window, cx);
                self.activate_from_entry(mode, false, window, cx)
            }
            SettingsExitIntent::Quit => {
                cx.emit(LauncherEvent::QuitRequested);
                false
            }
        }
    }

    pub(super) fn discard_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_draft = self.preferences.clone();
        set_input_value(
            &self.excluded_apps_input,
            self.settings_draft.excluded_apps.join("\n"),
            window,
            cx,
        );
        set_input_value(
            &self.file_roots_input,
            self.settings_draft.file_search_roots.join("\n"),
            window,
            cx,
        );
        set_input_value(
            &self.file_exclusions_input,
            self.settings_draft.file_search_excluded_names.join("\n"),
            window,
            cx,
        );
        set_input_value(
            &self.current_shortcut_input,
            self.settings_draft.current_app_shortcut.clone(),
            window,
            cx,
        );
        set_input_value(
            &self.universal_shortcut_input,
            self.settings_draft.universal_shortcut.clone(),
            window,
            cx,
        );
        self.settings_close_pending = false;
        self.settings_modal_return_focus = None;
        self.clipboard_confirmation = None;
        self.clipboard_settings_choice = None;
        let intent = self.settings_exit_intent.take();
        self.notice = None;
        self.shortcut_recording = None;
        self.settings_action_picker_open = false;
        self.pending_action_shortcut = None;
        self.pending_action_shortcut_value = None;
        self.context_shortcut_draft = None;
        let saved_locale = locale_for_language(self.preferences.ui_language);
        if self.locale != saved_locale {
            self.locale = saved_locale;
            self.model.set_locale(saved_locale);
            cx.emit(LauncherEvent::PreferencesApplied {
                locale: saved_locale,
                preferences: self.registered_preferences(),
            });
        }
        self.sync_search_placeholder(window, cx);
        self.toggle_screen(window, cx);
        if let Some(intent) = intent {
            match intent {
                SettingsExitIntent::Close => {}
                SettingsExitIntent::Wake(mode) => {
                    let _ = self.activate_from_entry(mode, false, window, cx);
                }
                SettingsExitIntent::Quit => cx.emit(LauncherEvent::QuitRequested),
            }
        }
    }

    pub(super) fn apply_preferences_result(
        &mut self,
        preferences: HostPreferences,
        host_error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let next_locale = locale_for_language(preferences.ui_language);
        if self.locale != next_locale {
            self.locale = next_locale;
            self.model.set_locale(self.locale);
            let window_handle = self.window_handle;
            let weak = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = window_handle.update(cx, |_, window, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.refresh_editor_placeholders(window, cx);
                    });
                });
            });
        }
        // Keep a failed startup controller for retries. Manager creation itself
        // can also fail, so recover it here when the live runtime is available.
        self.hotkey_error = if let Some(hotkeys) = self.hotkeys.as_ref() {
            hotkeys
                .lock()
                .map_err(|_| l10n::hotkey_state_unavailable(self.locale))
                .and_then(|mut hotkeys| {
                    hotkeys
                        .register(&preferences)
                        .map_err(|error| format!("{error:#}"))
                })
                .err()
        } else if self.live.is_some() {
            match HotKeyController::new(&preferences) {
                Ok(controller) => {
                    let error = controller.registration_error().map(str::to_owned);
                    self.hotkeys = Some(Arc::new(Mutex::new(controller)));
                    error
                }
                Err(error) => Some(format!("{error:#}")),
            }
        } else {
            self.hotkey_error.clone()
        };
        let current_draft = self.preferences_from_editors(cx);
        let draft_matches_saved = current_draft == preferences;
        let login_item_changed = self.preferences.launch_at_login != preferences.launch_at_login;
        self.preferences = preferences.clone();
        if draft_matches_saved {
            self.settings_draft = preferences.clone();
        }
        self.settings.saving = false;

        let effective = self.registered_preferences();
        let combined_error = host_error.clone().or_else(|| self.hotkey_error_message());
        if login_item_changed || host_error.is_some() {
            self.settings.login_item_error = host_error;
        }
        if !draft_matches_saved {
            self.set_notice(
                l10n::new_edits_were_made_while_saving_save(self.locale),
                components::NoticeKind::Warning,
            );
        } else if let Some(error) = combined_error {
            self.set_notice(error, components::NoticeKind::Error);
        } else {
            self.clear_notice();
        }
        cx.emit(LauncherEvent::PreferencesApplied {
            locale: self.locale,
            preferences: effective,
        });
        if self.settings_close_pending && draft_matches_saved && self.notice.is_none() {
            self.finish_settings_exit_after_save(cx);
        }
    }
}
