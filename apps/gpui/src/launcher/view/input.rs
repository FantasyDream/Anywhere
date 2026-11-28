use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum KeyFlow {
    Handled,
    Continue,
}

pub(super) struct KeyInput<'a> {
    pub(super) key: &'a str,
    pub(super) modifiers: KeyModifiers,
    pub(super) shortcut: Option<String>,
    pub(super) repeated: bool,
}

fn ignore_repeated_key(
    repeated: bool,
    key: &str,
    primary: bool,
    screen: Screen,
    recording_shortcut: bool,
    shortcut: Option<&str>,
    action_shortcuts: &[ActionShortcutBinding],
) -> bool {
    repeated
        && (matches!(key, "enter" | "return")
            || (primary && matches!(screen, Screen::Clipboard | Screen::ClipboardShelf))
            || (primary
                && key.eq_ignore_ascii_case("y")
                && matches!(screen, Screen::Launcher | Screen::FileSearch))
            || (primary && key.eq_ignore_ascii_case("k") && screen == Screen::Tasks)
            || (key == "space" && matches!(screen, Screen::Clipboard | Screen::ClipboardShelf))
            || (screen == Screen::Settings && (recording_shortcut || key == "escape"))
            || shortcut.is_some_and(|shortcut| {
                action_shortcuts
                    .iter()
                    .any(|binding| !binding.global && binding.shortcut.as_str() == shortcut)
            }))
}

impl LauncherView {
    pub(super) fn handle_key(
        &mut self,
        key: &str,
        modifiers: KeyModifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.handle_key_with_repeat(key, modifiers, false, window, cx);
    }

    pub(super) fn handle_key_with_repeat(
        &mut self,
        key: &str,
        modifiers: KeyModifiers,
        repeated: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_activation_prefill();
        if self.web_surface.interaction.is_some() {
            if !repeated && !modifiers.modified {
                match key {
                    "enter" | "return" => self.respond_web_host_interaction(Ok(()), window, cx),
                    "escape" => self.respond_web_host_interaction(
                        Err(anywhere_protocol::web_bridge::WebErrorCode::UserCancelled),
                        window,
                        cx,
                    ),
                    _ => {}
                }
            }
            return;
        }
        let input = KeyInput {
            key,
            modifiers,
            shortcut: normalized_shortcut(
                key,
                modifiers.command,
                modifiers.control,
                modifiers.alt,
                modifiers.shift,
            ),
            repeated,
        };
        if self.filter_repeated_key(&input) == KeyFlow::Handled
            || self.handle_shortcut_recording_key(&input, window, cx) == KeyFlow::Handled
            || self.handle_overlay_keys(&input, window, cx) == KeyFlow::Handled
            || self.handle_user_shortcut_key(&input, cx) == KeyFlow::Handled
            || self.handle_preview_key_overlay(&input, window, cx) == KeyFlow::Handled
            || self.handle_root_navigation_key(&input, cx) == KeyFlow::Handled
            || self.handle_launcher_quick_index_shortcut(&input, cx) == KeyFlow::Handled
            || self.handle_shelf_navigation_shortcut(&input, cx) == KeyFlow::Handled
        {
            return;
        }
        if input.modifiers.modified && !is_scope_tab_shortcut(input.key, input.modifiers) {
            return;
        }
        self.handle_navigation_key(&input, window, cx);
    }

    fn handle_launcher_quick_index_shortcut(
        &mut self,
        input: &KeyInput<'_>,
        cx: &mut Context<Self>,
    ) -> KeyFlow {
        if self.screen != Screen::Launcher
            || self.root_navigation_open
            || !input.modifiers.primary_only()
        {
            return KeyFlow::Continue;
        }
        let Some(digit) = input
            .key
            .bytes()
            .next()
            .filter(|byte| input.key.len() == 1 && (b'1'..=b'9').contains(byte))
            .map(|byte| usize::from(byte - b'1'))
        else {
            return KeyFlow::Continue;
        };
        if digit >= self.model.rows.len()
            || !self.model.rows.get(digit).is_some_and(|row| row.enabled)
        {
            return KeyFlow::Continue;
        }
        self.model.select_row(digit);
        self.reveal_result_row(digit);
        self.activate_selection(cx);
        KeyFlow::Handled
    }

    pub(super) fn filter_repeated_key(&self, input: &KeyInput<'_>) -> KeyFlow {
        if ignore_repeated_key(
            input.repeated,
            input.key,
            input.modifiers.primary(),
            self.screen,
            self.shortcuts.recording.is_some(),
            input.shortcut.as_deref(),
            &self.preferences.action_shortcuts,
        ) {
            KeyFlow::Handled
        } else {
            KeyFlow::Continue
        }
    }

    fn handle_shortcut_recording_key(
        &mut self,
        input: &KeyInput<'_>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> KeyFlow {
        if self.screen != Screen::Settings || self.shortcuts.recording.is_none() {
            return KeyFlow::Continue;
        }
        if input.key == "escape" {
            self.shortcuts.recording = None;
            self.notice = None;
            cx.notify();
            return KeyFlow::Handled;
        }
        let Some(shortcut) = input.shortcut.clone() else {
            self.set_notice(
                l10n::use_command_option_or_control_with_a(self.locale),
                components::NoticeKind::Warning,
            );
            cx.notify();
            return KeyFlow::Handled;
        };
        let target = self.shortcuts.recording.expect("recording active");
        if matches!(target, ShortcutSlot::CurrentApp | ShortcutSlot::Universal) {
            let other = match target {
                ShortcutSlot::CurrentApp => &self.shortcuts.universal_input,
                _ => &self.shortcuts.current_input,
            };
            if other.read(cx).value().as_ref() == shortcut
                || self
                    .settings_editor
                    .draft
                    .action_shortcuts
                    .iter()
                    .any(|binding| binding.global && binding.shortcut == shortcut)
            {
                self.set_notice(
                    l10n::shortcut_already_used(self.locale),
                    components::NoticeKind::Warning,
                );
                cx.notify();
                return KeyFlow::Handled;
            }
        }
        self.shortcuts.recording = None;
        match target {
            ShortcutSlot::CurrentApp => {
                set_input_value(&self.shortcuts.current_input, shortcut, window, cx);
                self.notice = None;
            }
            ShortcutSlot::Universal => {
                set_input_value(&self.shortcuts.universal_input, shortcut, window, cx);
                self.notice = None;
            }
            ShortcutSlot::Action | ShortcutSlot::ContextShortcut => {
                self.record_letter_shortcut(target, shortcut);
            }
        }
        cx.notify();
        KeyFlow::Handled
    }

    fn record_letter_shortcut(&mut self, target: ShortcutSlot, shortcut: String) {
        let is_letter = shortcut
            .rsplit('+')
            .next()
            .is_some_and(|key| key.len() == 4 && key.starts_with("Key"));
        if !is_letter {
            self.set_notice(
                l10n::letter_with_modifier(self.locale),
                components::NoticeKind::Warning,
            );
            self.shortcuts.recording = Some(target);
            return;
        }
        if reserved_action_shortcut(&shortcut) {
            self.set_notice(
                l10n::reserved_editing_shortcut(self.locale),
                components::NoticeKind::Warning,
            );
            self.shortcuts.recording = Some(target);
            return;
        }
        match target {
            ShortcutSlot::Action => {
                self.notice = None;
                if self.shortcuts.pending_action.is_some() {
                    self.shortcuts.pending_value = Some(shortcut);
                } else {
                    self.set_notice(
                        l10n::shortcut_recording_failed_try_again(self.locale),
                        components::NoticeKind::Warning,
                    );
                }
            }
            ShortcutSlot::ContextShortcut => {
                if let Some(draft) = self.shortcuts.context_draft.as_mut() {
                    draft.shortcut = Some(shortcut);
                    self.notice = None;
                } else {
                    self.set_notice(
                        l10n::shortcut_recording_failed_try_again(self.locale),
                        components::NoticeKind::Warning,
                    );
                }
            }
            ShortcutSlot::CurrentApp | ShortcutSlot::Universal => {}
        }
    }

    pub(super) fn page_actions_visible(&self) -> bool {
        match self.screen {
            Screen::ApplicationHistory => {
                self.app_history.object_actions_active
                    || self.app_history.page_actions_open
                    || self.app_history.fallback_actions_open
            }
            Screen::Clipboard | Screen::ClipboardShelf => self.clipboard.actions_open,
            Screen::Tasks => self.tasks.tool_options_open || self.tasks.task_actions_open,
            Screen::WebTool => self.web_surface.actions_open,
            Screen::FileSearch => self.model.object_actions_active,
            _ => false,
        }
    }

    pub(super) fn handle_overlay_keys(
        &mut self,
        input: &KeyInput<'_>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> KeyFlow {
        if input.modifiers.primary()
            && input.modifiers.shift
            && !input.modifiers.alt
            && input.key.eq_ignore_ascii_case("o")
            && self.can_open_tool_window()
        {
            self.open_tool_window(window, cx);
            return KeyFlow::Handled;
        }
        if self.screen == Screen::WebPackages {
            if let Some(id) = self.web_surface.uninstall_target.clone() {
                let cancel = &self.settings_editor.confirm_cancel_focus;
                let accept = &self.settings_editor.confirm_accept_focus;
                if is_scope_tab_shortcut(input.key, input.modifiers) {
                    let next = if accept.is_focused(window) {
                        cancel
                    } else {
                        accept
                    };
                    next.focus(window, cx);
                } else if !input.modifiers.modified && !input.repeated {
                    match input.key {
                        "escape" => self.close_web_uninstall_prompt(window, cx),
                        "enter" | "return" if accept.is_focused(window) => {
                            self.confirm_web_uninstall(&id, window, cx);
                        }
                        "enter" | "return" => self.close_web_uninstall_prompt(window, cx),
                        _ => {}
                    }
                }
                return KeyFlow::Handled;
            }
        }
        let had_candidates = self.page_actions_visible();
        if self.handle_app_history_overlay_keys(input, cx) == KeyFlow::Handled
            || self.handle_clipboard_overlay_keys(input, cx) == KeyFlow::Handled
            || self.handle_tasks_overlay_keys(input, cx) == KeyFlow::Handled
            || self.handle_web_overlay_keys(input, window, cx) == KeyFlow::Handled
        {
            if had_candidates && !self.page_actions_visible() && input.key == "escape" {
                if self.screen == Screen::Tasks {
                    if let Some(focus) = self.tasks_actions_return_focus.take() {
                        focus.focus(window, cx);
                    }
                } else if self.screen != Screen::WebTool {
                    self.refocus_active_editor(window, cx);
                }
            }
            KeyFlow::Handled
        } else {
            KeyFlow::Continue
        }
    }

    pub(super) fn handle_user_shortcut_key(
        &mut self,
        input: &KeyInput<'_>,
        cx: &mut Context<Self>,
    ) -> KeyFlow {
        if let Some(shortcut) = input.shortcut.as_deref() {
            if self.screen == Screen::Settings
                && shortcut == crate::platform::settings_save_shortcut()
            {
                return KeyFlow::Continue;
            }
            if let Some(binding) = self
                .preferences
                .action_shortcuts
                .iter()
                .find(|binding| !binding.global && binding.shortcut == shortcut)
                .cloned()
            {
                self.notice = None;
                self.start_action_shortcut_resolution(binding, cx);
                return KeyFlow::Handled;
            }
        }
        KeyFlow::Continue
    }

    fn handle_shelf_navigation_shortcut(
        &mut self,
        input: &KeyInput<'_>,
        cx: &mut Context<Self>,
    ) -> KeyFlow {
        if self.screen == Screen::ClipboardShelf
            && input.modifiers.primary_only()
            && self.handle_shelf_command_key(input.key, cx) == KeyFlow::Handled
        {
            return KeyFlow::Handled;
        }
        KeyFlow::Continue
    }

    fn handle_navigation_key(
        &mut self,
        input: &KeyInput<'_>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.screen {
            Screen::Settings if input.key == "escape" => self.handle_settings_escape(window, cx),
            Screen::Launcher => self.handle_launcher_key(input, window, cx),
            Screen::FileSearch => self.handle_file_search_key(input, window, cx),
            Screen::Clipboard | Screen::ClipboardShelf => {
                self.handle_clipboard_key(input, window, cx)
            }
            Screen::ApplicationHistory => self.handle_app_history_key(input, window, cx),
            Screen::Tasks => self.handle_tasks_key(input.key, window, cx),
            Screen::WebPackages => self.handle_web_packages_key(input, window, cx),
            Screen::WebTool | Screen::UnsupportedTool if input.key == "escape" => {
                self.go_back(window, cx);
            }
            _ => {}
        }
    }

    fn handle_launcher_key(
        &mut self,
        input: &KeyInput<'_>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modified = input.modifiers.modified;
        match input.key {
            "down" if !modified => {
                self.mark_scope_user_controlled();
                self.model.select_next();
                self.reveal_result_row(self.model.selected);
                self.notice = None;
                cx.notify();
            }
            "up" if !modified => {
                self.mark_scope_user_controlled();
                self.model.select_previous();
                self.reveal_result_row(self.model.selected);
                self.notice = None;
                cx.notify();
            }
            "tab"
                if !modified
                    && self.launcher_empty_action()
                        == Some(LauncherEmptyAction::SearchAllScopes) =>
            {
                self.use_launcher_empty_action(window, cx);
            }
            "tab"
                if is_scope_tab_shortcut(input.key, input.modifiers)
                    && !self.model.object_actions_active
                    && self.model.menu_path().is_none()
                    && self.has_scope_bar()
                    && self.active_editor_focused(window, cx) =>
            {
                self.mark_scope_user_controlled();
                if input.modifiers.shift {
                    self.model.cycle_scope_previous();
                } else {
                    self.model.cycle_scope();
                }
                self.reveal_selected_scope();
                self.request_live_update(cx);
                self.notice = None;
                cx.notify();
            }
            "enter" | "return" if self.launcher_empty_action().is_some() => {
                self.use_launcher_empty_action(window, cx);
            }
            "enter" | "return" => self.activate_selection(cx),
            "escape" => self.escape_launcher_root(window, cx),
            "right"
                if !modified
                    && self.model.query.is_empty()
                    && self.model.selected_row().is_some_and(|row| row.is_menu) =>
            {
                self.browse_selected_menu(cx);
            }
            "left"
                if !modified && self.model.query.is_empty() && self.model.menu_path().is_some() =>
            {
                self.model.back_menu();
                self.resize_current_window(cx);
                cx.notify();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_shortcut(shortcut: &str, global: bool) -> ActionShortcutBinding {
        ActionShortcutBinding {
            action_id: "test.action".to_owned(),
            title: "Test Action".to_owned(),
            shortcut: shortcut.to_owned(),
            global,
            routes: Vec::new(),
        }
    }

    #[test]
    fn repeated_enter_is_ignored_for_every_page() {
        assert!(ignore_repeated_key(
            true,
            "enter",
            false,
            Screen::ApplicationHistory,
            false,
            None,
            &[],
        ));
        assert!(!ignore_repeated_key(
            false,
            "enter",
            false,
            Screen::ApplicationHistory,
            false,
            None,
            &[],
        ));
    }

    #[test]
    fn repeated_clipboard_commands_do_not_toggle_or_refresh_again() {
        for screen in [Screen::Clipboard, Screen::ClipboardShelf] {
            assert!(ignore_repeated_key(
                true,
                "k",
                true,
                screen,
                false,
                Some("meta+KeyK"),
                &[],
            ));
            assert!(ignore_repeated_key(
                true,
                "space",
                false,
                screen,
                false,
                None,
                &[],
            ));
        }
        assert!(ignore_repeated_key(
            true,
            "y",
            true,
            Screen::FileSearch,
            false,
            Some("meta+KeyY"),
            &[],
        ));
        assert!(!ignore_repeated_key(
            true,
            "k",
            true,
            Screen::ApplicationHistory,
            false,
            Some("meta+KeyK"),
            &[],
        ));
        assert!(ignore_repeated_key(
            true,
            "k",
            true,
            Screen::Tasks,
            false,
            Some("meta+KeyK"),
            &[],
        ));
    }

    #[test]
    fn repeated_local_action_shortcuts_are_ignored_but_global_ones_are_not() {
        let local = [action_shortcut("alt+KeyT", false)];
        let global = [action_shortcut("alt+KeyT", true)];
        assert!(ignore_repeated_key(
            true,
            "t",
            false,
            Screen::Launcher,
            false,
            Some("alt+KeyT"),
            &local,
        ));
        assert!(!ignore_repeated_key(
            true,
            "t",
            false,
            Screen::Launcher,
            false,
            Some("alt+KeyT"),
            &global,
        ));
    }

    #[test]
    fn repeated_settings_keys_do_not_finish_recording_or_close_the_page() {
        assert!(ignore_repeated_key(
            true,
            "escape",
            false,
            Screen::Settings,
            true,
            None,
            &[],
        ));
        assert!(ignore_repeated_key(
            true,
            "escape",
            false,
            Screen::Settings,
            false,
            None,
            &[],
        ));
        assert!(ignore_repeated_key(
            true,
            "k",
            true,
            Screen::Settings,
            true,
            Some("meta+KeyK"),
            &[],
        ));
        assert!(!ignore_repeated_key(
            true,
            "down",
            false,
            Screen::Settings,
            false,
            None,
            &[],
        ));
    }
}
