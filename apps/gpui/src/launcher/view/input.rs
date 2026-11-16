use super::*;

fn ignore_repeated_key(
    repeated: bool,
    key: &str,
    command: bool,
    screen: Screen,
    recording_shortcut: bool,
    shortcut: Option<&str>,
    action_shortcuts: &[ActionShortcutBinding],
) -> bool {
    repeated
        && (matches!(key, "enter" | "return")
            || (command && matches!(screen, Screen::Clipboard | Screen::ClipboardShelf))
            || (command
                && key.eq_ignore_ascii_case("y")
                && matches!(screen, Screen::Launcher | Screen::FileSearch))
            || (command && key.eq_ignore_ascii_case("k") && screen == Screen::Tasks)
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
        let KeyModifiers {
            command,
            control,
            shift,
            alt,
            modified,
        } = modifiers;
        let shortcut = normalized_shortcut(key, command, control, alt, shift);
        if ignore_repeated_key(
            repeated,
            key,
            command,
            self.screen,
            self.shortcut_recording.is_some(),
            shortcut.as_deref(),
            &self.preferences.action_shortcuts,
        ) {
            return;
        }
        if self.screen == Screen::Settings && self.shortcut_recording.is_some() {
            if key == "escape" {
                self.shortcut_recording = None;
                self.notice = None;
                cx.notify();
                return;
            }
            let Some(shortcut) = normalized_shortcut(key, command, control, alt, shift) else {
                self.set_notice(
                    l10n::use_command_option_or_control_with_a(self.locale),
                    components::NoticeKind::Warning,
                );
                cx.notify();
                return;
            };
            let target = self
                .shortcut_recording
                .take()
                .unwrap_or(ShortcutSlot::CurrentApp);
            match target {
                ShortcutSlot::CurrentApp => {
                    set_input_value(&self.current_shortcut_input, shortcut, window, cx);
                    self.notice = None;
                }
                ShortcutSlot::Universal => {
                    set_input_value(&self.universal_shortcut_input, shortcut, window, cx);
                    self.notice = None;
                }
                ShortcutSlot::Action => {
                    self.notice = None;
                    let is_letter = shortcut
                        .rsplit('+')
                        .next()
                        .is_some_and(|key| key.len() == 4 && key.starts_with("Key"));
                    if !is_letter {
                        self.set_notice(
                            l10n::letter_with_modifier(self.locale),
                            components::NoticeKind::Warning,
                        );
                        self.shortcut_recording = Some(ShortcutSlot::Action);
                        cx.notify();
                        return;
                    }
                    if reserved_action_shortcut(&shortcut) {
                        self.set_notice(
                            l10n::reserved_editing_shortcut(self.locale),
                            components::NoticeKind::Warning,
                        );
                        self.shortcut_recording = Some(ShortcutSlot::Action);
                        cx.notify();
                        return;
                    }
                    if self.pending_action_shortcut.is_some() {
                        self.pending_action_shortcut_value = Some(shortcut);
                    } else {
                        self.set_notice(
                            l10n::shortcut_recording_failed_try_again(self.locale),
                            components::NoticeKind::Warning,
                        );
                    }
                }
                ShortcutSlot::ContextShortcut => {
                    let is_letter = shortcut
                        .rsplit('+')
                        .next()
                        .is_some_and(|key| key.len() == 4 && key.starts_with("Key"));
                    if !is_letter {
                        self.set_notice(
                            l10n::letter_with_modifier(self.locale),
                            components::NoticeKind::Warning,
                        );
                        self.shortcut_recording = Some(ShortcutSlot::ContextShortcut);
                        cx.notify();
                        return;
                    }
                    if reserved_action_shortcut(&shortcut) {
                        self.set_notice(
                            l10n::reserved_editing_shortcut(self.locale),
                            components::NoticeKind::Warning,
                        );
                        self.shortcut_recording = Some(ShortcutSlot::ContextShortcut);
                        cx.notify();
                        return;
                    }
                    if let Some(draft) = self.context_shortcut_draft.as_mut() {
                        draft.shortcut = Some(shortcut);
                        self.notice = None;
                    } else {
                        self.set_notice(
                            l10n::shortcut_recording_failed_try_again(self.locale),
                            components::NoticeKind::Warning,
                        );
                    }
                }
            }
            cx.notify();
            return;
        }
        if matches!(self.screen, Screen::Clipboard | Screen::ClipboardShelf)
            && self.clipboard.delete_pending.is_some()
        {
            match key {
                "left" if !modified => {
                    self.clipboard.delete_confirm_selected = false;
                    cx.notify();
                }
                "right" if !modified => {
                    self.clipboard.delete_confirm_selected = true;
                    cx.notify();
                }
                "enter" | "return" if !modified => {
                    if self.clipboard.delete_confirm_selected {
                        self.confirm_clipboard_delete(cx);
                    }
                    self.clipboard.delete_pending = None;
                    self.clipboard.delete_confirm_selected = false;
                    cx.notify();
                }
                "escape" if !modified => {
                    self.clipboard.delete_pending = None;
                    self.clipboard.delete_confirm_selected = false;
                    cx.notify();
                }
                _ => {}
            }
            return;
        }
        if self.screen == Screen::Tasks && self.tasks.tool_options_open {
            if command && key.eq_ignore_ascii_case("k") {
                self.close_tool_options(cx);
                return;
            }
            if !modified {
                match key {
                    "down" | "up" => self.move_tool_option(cx),
                    "enter" | "return" => {
                        self.execute_tool_option(self.tasks.tool_option_selected, cx)
                    }
                    "escape" => self.close_tool_options(cx),
                    _ => {}
                }
            }
            return;
        }
        if self.screen == Screen::Tasks && self.tasks.task_actions_open {
            if command && key.eq_ignore_ascii_case("k") {
                self.close_task_actions(cx);
                return;
            }
            if !modified {
                match key {
                    "down" => self.move_task_action(1, cx),
                    "up" => self.move_task_action(-1, cx),
                    "enter" | "return" => {
                        self.execute_task_action(self.tasks.task_action_selected, cx)
                    }
                    "escape" => self.close_task_actions(cx),
                    _ => {}
                }
            }
            return;
        }
        if matches!(self.screen, Screen::Clipboard | Screen::ClipboardShelf)
            && self.clipboard.actions_open
        {
            if command && key.eq_ignore_ascii_case("k") {
                self.close_clipboard_actions(cx);
                return;
            }
            let action_count = if self.screen == Screen::ClipboardShelf {
                5
            } else {
                4
            };
            if !modified {
                match key {
                    "down" => {
                        self.move_clipboard_action(1, action_count, cx);
                        return;
                    }
                    "up" => {
                        self.move_clipboard_action(-1, action_count, cx);
                        return;
                    }
                    "enter" | "return" => {
                        self.execute_clipboard_action(self.clipboard.action_selected, cx);
                        return;
                    }
                    "escape" => {
                        self.close_clipboard_actions(cx);
                        return;
                    }
                    _ => {}
                }
            }
            self.close_clipboard_actions(cx);
        }
        if self.screen == Screen::WebTool && self.web_surface.actions_open && !modified {
            match key {
                "down" | "up" => {
                    self.web_surface.action_selected = 1 - self.web_surface.action_selected;
                    cx.notify();
                }
                "enter" | "return" => {
                    self.execute_web_action(self.web_surface.action_selected, window, cx);
                }
                "escape" => {
                    self.web_surface.actions_open = false;
                    self.web_surface.action_selected = 0;
                    cx.notify();
                }
                _ => {}
            }
            return;
        }
        if let Some(shortcut) = shortcut {
            if let Some(binding) = self
                .preferences
                .action_shortcuts
                .iter()
                .find(|binding| !binding.global && binding.shortcut == shortcut)
                .cloned()
            {
                self.notice = None;
                self.start_action_shortcut_resolution(binding, cx);
                return;
            }
        }
        if self.preview_open && key == "escape" {
            self.close_preview(cx);
            self.focus_active_editor(window, cx);
            return;
        }
        if self.preview_open
            && command
            && !control
            && !alt
            && !shift
            && key.eq_ignore_ascii_case("l")
        {
            self.close_preview(cx);
            self.focus_active_editor(window, cx);
            return;
        }
        if self.preview_open
            && ((!modified
                && matches!(
                    key,
                    "left" | "right" | "home" | "end" | "backspace" | "delete"
                ))
                || (command && !control && !alt && !shift && key.eq_ignore_ascii_case("a")))
        {
            if matches!(self.screen, Screen::Launcher | Screen::FileSearch) {
                self.file_search.browsing = false;
            }
            if matches!(self.screen, Screen::Clipboard | Screen::ClipboardShelf) {
                self.clipboard.browsing = false;
            }
            self.close_preview(cx);
        }
        if self.screen == Screen::Launcher && self.root_navigation_open {
            if modified {
                return;
            }
            match key {
                "escape" => self.toggle_root_navigation(cx),
                "down" => self.move_root_navigation(1, cx),
                "up" => self.move_root_navigation(-1, cx),
                "enter" | "return" => self.choose_root_navigation(cx),
                _ => {}
            }
            return;
        }
        if command
            && !control
            && !alt
            && !shift
            && key.eq_ignore_ascii_case("y")
            && self.toggle_preview_shortcut(window, cx)
        {
            return;
        }
        if command && key.eq_ignore_ascii_case("k") {
            match self.screen {
                Screen::Tasks => {
                    if self.tasks.detail.is_some() {
                        self.open_task_actions(window, cx);
                    } else {
                        self.open_tool_options(window, cx);
                    }
                }
                Screen::Clipboard | Screen::ClipboardShelf => {
                    if matches!(self.screen, Screen::Clipboard | Screen::ClipboardShelf)
                        && self.clipboard.actions_open
                    {
                        self.close_clipboard_actions(cx);
                    } else if let Some(item_id) = self.clipboard.selected_item().map(|item| item.id)
                    {
                        self.open_clipboard_actions(item_id, cx);
                    } else {
                        self.open_empty_clipboard_actions(cx);
                    }
                }
                Screen::Launcher | Screen::FileSearch => {
                    self.show_object_actions(cx);
                    if self.screen == Screen::Launcher
                        && self.model.query.is_empty()
                        && !self.model.object_actions_active
                    {
                        self.browse_selected_menu(cx);
                    }
                }
                _ => {}
            }
            return;
        }
        if self.screen == Screen::Tasks && command && !control && !shift {
            if !alt && key.eq_ignore_ascii_case("l") {
                cx.emit(LauncherEvent::RestoreLauncher);
                return;
            }
            if !alt && key == "." {
                self.stop_selected_execution(cx);
                return;
            }
            if alt && matches!(key, "up" | "down") {
                self.handle_key(key, KeyModifiers::default(), window, cx);
                return;
            }
        }
        if command && key.eq_ignore_ascii_case("r") {
            if self.screen == Screen::Tasks {
                self.start_tasks_refresh(cx);
            } else if self.screen == Screen::ClipboardShelf {
                self.request_clipboard_search(cx);
            }
            return;
        }
        if command && key.eq_ignore_ascii_case("f") {
            if self.screen == Screen::Tasks {
                if !self.tasks.rows.is_empty() {
                    self.tasks_filter_input.update(cx, |input, cx| {
                        input.focus(window, cx);
                        input.select_all(window, cx);
                    });
                }
            } else if self.screen == Screen::ClipboardShelf {
                self.clipboard_input.update(cx, |input, cx| {
                    input.focus(window, cx);
                    input.select_all(window, cx);
                });
            }
            return;
        }
        if command && shift && key == "backspace" && self.screen == Screen::ClipboardShelf {
            if let Some(item_id) = self.clipboard.selected_item().map(|item| item.id) {
                self.request_delete_clipboard_item(item_id, cx);
            }
            return;
        }
        if command && self.screen == Screen::ClipboardShelf {
            if let Ok(index) = key.parse::<usize>() {
                if (1..=9).contains(&index) && index <= self.clipboard.items().len() {
                    self.clipboard.browsing = true;
                    self.clipboard.selected = index - 1;
                    self.copy_selected_clipboard(cx);
                    return;
                }
            }
            if key == "up" {
                if !self.clipboard.items().is_empty() {
                    self.clipboard.browsing = true;
                    self.clipboard.selected = 0;
                    cx.notify();
                }
                return;
            }
            if key == "down" {
                if !self.clipboard.items().is_empty() {
                    self.clipboard.browsing = true;
                    self.clipboard.selected = self.clipboard.items().len() - 1;
                    cx.notify();
                }
                return;
            }
        }
        if command && key.eq_ignore_ascii_case("s") && self.screen == Screen::Settings {
            self.save_settings(cx);
            return;
        }
        if command && key.eq_ignore_ascii_case("q") {
            if self.screen == Screen::Settings {
                self.request_settings_exit(SettingsExitIntent::Quit, window, cx);
            } else {
                cx.emit(LauncherEvent::QuitRequested);
            }
            return;
        }
        if command && matches!(key, "enter" | "return") {
            if self.screen == Screen::ClipboardShelf {
                self.copy_selected_clipboard_and_return(cx);
            } else {
                self.toggle_root_navigation(cx);
            }
            return;
        }
        if self.screen == Screen::ApplicationHistory
            && self.app_history.fallback_actions_open
            && !modified
        {
            match key {
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
                _ => return,
            }
            cx.notify();
            return;
        }
        if self.screen == Screen::ApplicationHistory
            && self.app_history.empty_actions_open
            && !modified
        {
            match key {
                "enter" | "return" => self.refresh_empty_app_history(cx),
                "escape" => self.app_history.empty_actions_open = false,
                "down" | "up" => {}
                _ => return,
            }
            cx.notify();
            return;
        }
        if self.screen == Screen::ApplicationHistory
            && self.app_history.object_actions_active
            && !modified
        {
            match key {
                "down" => self.app_history.select_object_action_next(),
                "up" => self.app_history.select_object_action_previous(),
                "enter" | "return" => self.execute_selected_app_history_action(cx),
                "escape" => self.app_history.close_object_actions(),
                _ => return,
            }
            cx.notify();
            return;
        }
        if modified && !is_scope_tab_shortcut(key, modifiers) {
            return;
        }
        match (self.screen, key) {
            (Screen::Settings, "escape") => self.handle_settings_escape(window, cx),
            (Screen::Launcher, "down") if !modified => {
                self.mark_scope_user_controlled();
                self.model.select_next();
                self.reveal_result_row(self.model.selected);
                self.notice = None;
                cx.notify();
            }
            (Screen::Launcher, "up") if !modified => {
                self.mark_scope_user_controlled();
                self.model.select_previous();
                self.reveal_result_row(self.model.selected);
                self.notice = None;
                cx.notify();
            }
            (Screen::FileSearch, "down") if !modified => self.select_file_search(1, cx),
            (Screen::FileSearch, "up") if !modified => self.select_file_search(-1, cx),
            (Screen::Launcher, "tab")
                if is_scope_tab_shortcut(key, modifiers)
                    && !self.model.object_actions_active
                    && self.model.menu_path().is_none()
                    && self.has_scope_bar()
                    && self.active_editor_focused(window, cx) =>
            {
                self.mark_scope_user_controlled();
                if shift {
                    self.model.cycle_scope_previous();
                } else {
                    self.model.cycle_scope();
                }
                self.reveal_selected_scope();
                self.request_live_update(cx);
                self.notice = None;
                cx.notify();
            }
            (Screen::FileSearch, "tab")
                if is_scope_tab_shortcut(key, modifiers)
                    && self.active_editor_focused(window, cx) =>
            {
                if shift {
                    self.file_search.previous_filter();
                } else {
                    self.file_search.next_filter();
                }
                self.reveal_selected_file_filter();
                self.reset_file_search_browsing(cx);
                self.request_live_update(cx);
                self.notice = None;
                cx.notify();
            }
            (Screen::Launcher, "enter" | "return") => self.activate_selection(cx),
            (Screen::FileSearch, "enter" | "return") => self.activate_selection(cx),
            (Screen::FileSearch, "escape") => {
                if self.model.object_actions_active {
                    self.model.hide_object_actions();
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
            (Screen::Launcher, "escape") => {
                self.escape_launcher_root(window, cx);
            }
            (Screen::Launcher, "right")
                if !modified
                    && self.model.query.is_empty()
                    && self.model.selected_row().is_some_and(|row| row.is_menu) =>
            {
                self.browse_selected_menu(cx);
            }
            (Screen::Launcher, "left")
                if !modified && self.model.query.is_empty() && self.model.menu_path().is_some() =>
            {
                self.model.back_menu();
                self.resize_current_window(cx);
                cx.notify();
            }
            (Screen::Clipboard | Screen::ClipboardShelf, "down") => {
                self.select_clipboard(1, cx);
            }
            (Screen::Clipboard | Screen::ClipboardShelf, "up") => {
                self.select_clipboard(-1, cx);
            }
            (Screen::Clipboard | Screen::ClipboardShelf, "enter" | "return") => {
                self.copy_selected_clipboard(cx)
            }
            (Screen::Clipboard | Screen::ClipboardShelf, "space")
                if self.active_editor_focused(window, cx) =>
            {
                self.preview_selected_clipboard(cx)
            }
            (Screen::Clipboard | Screen::ClipboardShelf, "escape") => {
                if !self.clipboard.query.is_empty() {
                    self.clipboard_input
                        .update(cx, |input, cx| input.set_value("", window, cx));
                    self.clipboard.query.clear();
                    self.reset_clipboard_browsing(cx);
                    self.request_clipboard_search(cx);
                    cx.notify();
                } else {
                    self.go_back(window, cx);
                }
            }
            (Screen::ApplicationHistory | Screen::ClipboardShelf, "pageup" | "pagedown") => {
                let history = self.screen == Screen::ApplicationHistory;
                let (height, row_height, selected, count) = if history {
                    (
                        self.history_scroll.bounds().size.height,
                        64.,
                        self.app_history.selected,
                        self.app_history.items.len(),
                    )
                } else {
                    (
                        self.clipboard_shelf_scroll.bounds().size.height,
                        48.,
                        self.clipboard.selected,
                        self.clipboard.items().len(),
                    )
                };
                let step = (f32::from(height) / row_height).floor().clamp(1., 100.) as usize;
                let target = if key == "pageup" {
                    selected.saturating_sub(step)
                } else {
                    selected.saturating_add(step).min(count.saturating_sub(1))
                };
                if history {
                    self.app_history.close_object_actions();
                    self.app_history.selected = target;
                } else if count > 0 {
                    self.clipboard.browsing = true;
                    self.clipboard.selected = target;
                    if self.preview_open {
                        self.preview_selected_clipboard(cx);
                    }
                }
                self.reveal_result_row(target);
                cx.notify();
            }
            (Screen::ApplicationHistory, "down") => {
                self.app_history.select_next();
                self.reveal_result_row(self.app_history.selected);
                cx.notify();
            }
            (Screen::ApplicationHistory, "up") => {
                self.app_history.select_previous();
                self.reveal_result_row(self.app_history.selected);
                cx.notify();
            }
            (Screen::ApplicationHistory, "enter" | "return") => {
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
            (Screen::ApplicationHistory, "escape") => {
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
            (Screen::Tasks, "down") => {
                if !self.tasks.visible_indices().is_empty() {
                    let previous = self.tasks.selected;
                    self.tasks.select_next_visible();
                    self.reveal_result_row(self.tasks.selected);
                    if self.tasks.selected != previous {
                        self.tasks.detail = None;
                        self.tasks.detail_request = self.tasks.detail_request.wrapping_add(1);
                        if let Some(task_id) = self.tasks.selected_row().map(|row| row.id.clone()) {
                            self.request_task_detail(task_id, cx);
                        }
                    }
                    cx.notify();
                }
            }
            (Screen::Tasks, "up") => {
                if !self.tasks.visible_indices().is_empty() {
                    let previous = self.tasks.selected;
                    self.tasks.select_previous_visible();
                    self.reveal_result_row(self.tasks.selected);
                    if self.tasks.selected != previous {
                        self.tasks.detail = None;
                        self.tasks.detail_request = self.tasks.detail_request.wrapping_add(1);
                        if let Some(task_id) = self.tasks.selected_row().map(|row| row.id.clone()) {
                            self.request_task_detail(task_id, cx);
                        }
                    }
                    cx.notify();
                }
            }
            (Screen::Tasks, "enter" | "return") => {
                if self
                    .tasks_filter_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
                {
                    window.blur(cx);
                } else {
                    self.open_selected_execution(cx);
                }
            }
            (Screen::Tasks, "escape") => {
                if !self.tasks.filter.is_empty() {
                    self.tasks_filter_input
                        .update(cx, |input, cx| input.set_value("", window, cx));
                    self.tasks.set_filter(String::new());
                    window.blur(cx);
                    cx.notify();
                } else {
                    self.go_back(window, cx);
                }
            }
            (Screen::WebPackages, "down") if !modified => {
                self.web.select_next();
                self.reveal_result_row(self.web.selected);
                cx.notify();
            }
            (Screen::WebPackages, "up") if !modified => {
                self.web.select_previous();
                self.reveal_result_row(self.web.selected);
                cx.notify();
            }
            (Screen::WebPackages, "enter" | "return") => self.toggle_selected_web_package(cx),
            (Screen::WebPackages, "escape") => self.go_back(window, cx),
            (Screen::WebTool, "escape") => self.go_back(window, cx),
            (Screen::PluginDetail | Screen::UnsupportedTool, "escape") => self.go_back(window, cx),
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
