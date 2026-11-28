use super::*;

impl LauncherView {
    pub(super) fn open_screen(
        &mut self,
        screen: Screen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_activation_prefill();
        tracing::debug!(
            ?screen,
            current_screen = ?self.screen,
            has_live = self.live.is_some(),
            "application_history_open_screen"
        );
        if screen != Screen::Tasks {
            self.cancel_tasks_refresh();
            self.tasks.refresh_generation = self.tasks.refresh_generation.wrapping_add(1);
        }
        if self.screen == screen {
            return;
        }
        if self.screen == Screen::WebPackages {
            self.retire_package_review();
        }
        if self.screen == Screen::ApplicationHistory {
            self.app_history.close_object_actions();
            self.app_history.page_actions_open = false;
            self.app_history.fallback_actions_open = false;
            self.release_app_history_session();
        }
        self.capture_parent_workspace(cx);
        if self.screen == Screen::FileSearch {
            self.reset_file_search_browsing(cx);
        }
        self.screen = screen;
        self.notice = None;
        self.model.hide_object_actions();
        if screen == Screen::ApplicationHistory {
            if self.app_history.session_id.is_some() {
                self.release_app_history_session();
            }
            self.app_history.begin_open();
            set_input_value(&self.history_input, "", window, cx);
            self.start_app_history_session(cx);
        }
        if matches!(screen, Screen::Clipboard | Screen::ClipboardShelf) {
            self.request_clipboard_search(cx);
        }
        let receiver = match (screen, self.live.clone()) {
            (Screen::WebPackages, Some(live)) => Some(live.web_packages()),
            _ => None,
        };
        if let Some(receiver) = receiver {
            self.poll(receiver, self.live_state.generation, cx);
        }
        if screen == Screen::Tasks {
            self.start_tasks_refresh(cx);
        }
        if screen == Screen::FileSearch {
            self.file_search.browsing = false;
            self.model.selected = 0;
            self.input.update(cx, |input, cx| input.focus(window, cx));
            self.request_live_update(cx);
        } else if matches!(screen, Screen::Clipboard | Screen::ClipboardShelf) {
            self.reset_clipboard_browsing(cx);
            self.clipboard_input
                .update(cx, |input, cx| input.focus(window, cx));
        } else if screen == Screen::ApplicationHistory {
            self.history_input
                .update(cx, |input, cx| input.focus(window, cx));
        } else if matches!(
            screen,
            Screen::Tasks | Screen::WebPackages | Screen::WebTool | Screen::UnsupportedTool
        ) {
            self.surface_focus_handle.focus(window, cx);
        }
        self.resize_window_for_screen(window, cx);
        cx.notify();
    }

    pub(super) fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.standalone_tool {
            self.prepare_hide(cx);
            window.remove_window();
            return;
        }
        self.cancel_tasks_refresh();
        self.tasks.refresh_generation = self.tasks.refresh_generation.wrapping_add(1);
        if self.screen == Screen::WebPackages {
            if self.cancel_web_package_review(window, cx) {
                return;
            }
            self.screen = Screen::Settings;
            self.notice = None;
            self.resize_window_for_screen(window, cx);
            crate::platform::configure_settings_window_after_update(PRODUCT_NAME);
            self.settings_editor.focus_handle.focus(window, cx);
            cx.notify();
            return;
        }
        if self.standalone_tasks && self.screen == Screen::Tasks {
            self.prepare_hide(cx);
            window.remove_window();
            return;
        }
        if self.standalone_shelf && self.screen == Screen::ClipboardShelf {
            self.prepare_hide(cx);
            window.remove_window();
            return;
        }
        if self.screen == Screen::ApplicationHistory {
            self.app_history.close_object_actions();
            self.app_history.page_actions_open = false;
            self.app_history.fallback_actions_open = false;
            self.release_app_history_session();
        }
        if self.screen == Screen::Launcher {
            if self.model.object_actions_active {
                self.model.hide_object_actions();
                self.reveal_result_row(self.model.selected);
                cx.notify();
            } else if self.model.menu_path().is_some() {
                self.model.back_menu();
                self.reveal_result_row(self.model.selected);
                cx.notify();
            } else if !self.model.query.is_empty() {
                self.clear_launcher_query(window, cx);
            } else {
                self.prepare_hide(cx);
                crate::platform::hide_main_window();
            }
            return;
        }
        if self.screen == Screen::FileSearch {
            self.reset_file_search_browsing(cx);
        } else if matches!(self.screen, Screen::Clipboard | Screen::ClipboardShelf) {
            self.reset_clipboard_browsing(cx);
        }
        if self.screen == Screen::WebTool {
            self.close_web_surface(cx);
        }
        self.web_surface.unsupported_tool = None;
        self.screen = Screen::Launcher;
        self.restore_parent_workspace(window, cx);
        self.request_application_icons(cx);
        self.notice = None;
        self.focus_launcher_input(window, cx);
        self.resize_window_for_screen(window, cx);
        cx.notify();
    }

    pub(super) fn capture_parent_workspace(&mut self, cx: &Context<Self>) {
        if self.screen != Screen::Launcher || self.parent_workspace.is_some() {
            return;
        }
        self.parent_workspace = Some(ParentWorkspace {
            input_mode: self.input_mode,
            input_value: self.input.read(cx).value().to_string(),
            content_value: self.content_input.read(cx).value().to_string(),
            launcher: self.model.workspace(),
        });
    }

    pub(super) fn restore_parent_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.parent_workspace.take() else {
            return;
        };
        let previous_mode = self.input_mode;
        self.input_mode = workspace.input_mode;
        self.root_navigation_mode = workspace.input_mode;
        self.model.restore_workspace(workspace.launcher);
        set_input_value(&self.input, workspace.input_value, window, cx);
        set_textarea_value(&self.content_input, workspace.content_value, window, cx);
        if previous_mode != self.input_mode {
            self.sync_search_placeholder(window, cx);
        }
        self.reveal_selected_scope();
        self.reveal_result_row(self.model.selected);
    }

    pub(super) fn toggle_root_navigation(&mut self, cx: &mut Context<Self>) {
        self.cancel_activation_prefill();
        if self.screen != Screen::Launcher
            || self.model.object_actions_active
            || self.input_mode == InputMode::Content
        {
            return;
        }
        self.root_navigation_open = !self.root_navigation_open;
        if self.root_navigation_open {
            self.root_navigation_mode = self.input_mode;
            self.notice = None;
        }
        self.resize_current_window(cx);
        cx.notify();
    }

    pub(super) fn escape_launcher_root(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.model.object_actions_active {
            self.go_back(window, cx);
            return;
        }
        if self.input_mode == InputMode::Content {
            self.return_to_current_app(window, cx);
            return;
        }
        if !self.model.query.is_empty() {
            self.clear_launcher_query(window, cx);
            return;
        }
        if self.model.menu_path().is_some() {
            self.model.back_menu();
            self.resize_current_window(cx);
            cx.notify();
            return;
        }
        if self.input_mode == InputMode::Environment {
            self.select_root_mode(InputMode::Search, cx);
            self.focus_launcher_input(window, cx);
            return;
        }
        self.prepare_hide(cx);
        crate::platform::hide_main_window();
    }

    pub(super) fn return_to_current_app(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_mode = InputMode::Environment;
        self.root_navigation_mode = InputMode::Environment;
        self.root_navigation_open = false;
        self._editor_sync_guard.set(true);
        set_input_value(&self.input, "", window, cx);
        set_textarea_value(&self.content_input, "", window, cx);
        let editor_sync_guard = self._editor_sync_guard.clone();
        cx.defer(move |_| editor_sync_guard.set(false));
        self.model.set_query(String::new());
        self.model.select_scope(None);
        self.begin_navigation_scope_selection(InputMode::Environment);
        self.notice = None;
        self.sync_search_placeholder(window, cx);
        self.focus_launcher_input(window, cx);
        self.request_live_update(cx);
        self.resize_current_window(cx);
        cx.notify();
    }

    pub(super) fn clear_launcher_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self._editor_sync_guard.set(true);
        if self.input_mode == InputMode::Content {
            set_textarea_value(&self.content_input, "", window, cx);
        } else {
            set_input_value(&self.input, "", window, cx);
        }
        let editor_sync_guard = self._editor_sync_guard.clone();
        cx.defer(move |_| editor_sync_guard.set(false));
        self.model.set_query(String::new());
        self.notice = None;
        self.focus_launcher_input(window, cx);
        self.request_live_update(cx);
        self.resize_current_window(cx);
        cx.notify();
    }

    pub(super) fn move_root_navigation(&mut self, direction: i8, cx: &mut Context<Self>) {
        if !self.root_navigation_open {
            return;
        }
        self.root_navigation_mode = if direction < 0 {
            match self.root_navigation_mode {
                InputMode::Search => InputMode::Environment,
                _ => InputMode::Search,
            }
        } else {
            match self.root_navigation_mode {
                InputMode::Environment => InputMode::Search,
                _ => InputMode::Environment,
            }
        };
        cx.notify();
    }

    pub(super) fn choose_root_navigation(&mut self, cx: &mut Context<Self>) {
        if !self.root_navigation_open {
            return;
        }
        self.select_root_mode(self.root_navigation_mode, cx);
    }

    pub(super) fn select_root_mode(&mut self, input_mode: InputMode, cx: &mut Context<Self>) {
        self.cancel_activation_prefill();
        if self.input_mode == InputMode::Content {
            return;
        }
        self.input_mode = input_mode;
        self.root_navigation_mode = input_mode;
        self.root_navigation_open = false;
        self.model.select_scope(None);
        self.begin_navigation_scope_selection(input_mode);
        self.request_live_update(cx);
        self.notice = None;
        self.resize_current_window(cx);
        cx.notify();
    }

    pub(super) fn render_root_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("launcher-mode-chooser")
            .role(gpui_kit::Role::ListBox)
            .test_support()
            .h(px(108.))
            .flex_shrink_0()
            .px(px(10.))
            .py(px(metrics::SPACE_XS))
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_XS))
            .child(self.root_navigation_row(InputMode::Search, cx))
            .child(self.root_navigation_row(InputMode::Environment, cx))
    }

    pub(super) fn root_navigation_row(
        &self,
        mode: InputMode,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.root_navigation_mode == mode;
        let active = self.input_mode == mode;
        let (label, detail, icon) = match mode {
            InputMode::Search => (
                l10n::global_search(self.locale),
                l10n::find_apps_and_actions(self.locale),
                IconName::Search,
            ),
            _ => (
                l10n::current_app(self.locale),
                l10n::actions_for_your_current_environment(self.locale),
                IconName::AppWindow,
            ),
        };
        let id = match mode {
            InputMode::Search => "launcher-navigation-search",
            _ => "launcher-navigation-environment",
        };
        let mut row = components::ListRow::new(id)
            .density(components::RowDensity::Compact)
            .selected(selected)
            .title(label)
            .title_weight(FontWeight::SEMIBOLD)
            .subtitle(detail)
            .subtitle_id(format!("{id}-detail"))
            .leading(
                Icon::new(icon)
                    .size(px(metrics::ICON_SIZE))
                    .text_color(theme::secondary()),
            );
        let hotkey = if mode == InputMode::Search {
            &self.preferences.universal_shortcut
        } else {
            &self.preferences.current_app_shortcut
        };
        if let Some(label) = crate::platform::shortcut_label(hotkey) {
            row = row.trailing(components::keycap(label));
        }
        if active {
            row = row.trailing(
                Icon::new(IconName::Check)
                    .size(px(metrics::ICON_SIZE))
                    .text_color(theme::secondary()),
            );
        }
        row.build()
            .test_support()
            .on_click(cx.listener(move |this, _, _, cx| this.select_root_mode(mode, cx)))
    }

    pub(in crate::launcher::view) fn handle_root_navigation_key(
        &mut self,
        input: &crate::launcher::view::input::KeyInput<'_>,
        cx: &mut Context<Self>,
    ) -> crate::launcher::view::input::KeyFlow {
        use crate::launcher::view::input::KeyFlow;
        if self.screen != Screen::Launcher || !self.root_navigation_open {
            return KeyFlow::Continue;
        }
        if input.modifiers.modified {
            return KeyFlow::Handled;
        }
        match input.key {
            "escape" => self.toggle_root_navigation(cx),
            "down" => self.move_root_navigation(1, cx),
            "up" => self.move_root_navigation(-1, cx),
            "enter" | "return" => self.choose_root_navigation(cx),
            _ => {}
        }
        KeyFlow::Handled
    }
}
