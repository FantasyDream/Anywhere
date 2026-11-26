use super::*;

gpui_kit::actions!(
    anywhere,
    [
        OpenSettings,
        TogglePreview,
        ClosePreviewToEditor,
        ToggleObjectActions,
        RefreshPage,
        FocusSearch,
        SaveSettings,
        QuitApp,
        ToggleRootNavigation,
        CopyAndReturn,
        RestoreLauncher,
        StopSelectedTask,
        DeleteShelfItem,
        SelectNextTaskFromDetail,
        SelectPreviousTaskFromDetail,
    ]
);

#[derive(Default)]
struct LauncherKeyBindingsInstalled;

impl gpui_kit::Global for LauncherKeyBindingsInstalled {}

pub(crate) fn bind_keys(cx: &mut gpui_kit::App) {
    if cx.has_global::<LauncherKeyBindingsInstalled>() {
        return;
    }
    cx.set_global(LauncherKeyBindingsInstalled);

    macro_rules! binding {
        ($key:literal, $action:expr, $context:literal) => {
            crate::platform::primary_key_binding(concat!("secondary-", $key), $action, $context, cx)
        };
    }
    let bindings = [
        binding!(",", OpenSettings, "Anywhere"),
        binding!("q", QuitApp, "Anywhere"),
        binding!("y", TogglePreview, "Launcher"),
        binding!("y", TogglePreview, "FileSearch"),
        binding!("y", TogglePreview, "Clipboard"),
        binding!("y", TogglePreview, "ClipboardShelf"),
        binding!("l", ClosePreviewToEditor, "Launcher"),
        binding!("l", ClosePreviewToEditor, "FileSearch"),
        binding!("l", ClosePreviewToEditor, "Clipboard"),
        binding!("l", ClosePreviewToEditor, "ClipboardShelf"),
        binding!("k", ToggleObjectActions, "Launcher"),
        binding!("k", ToggleObjectActions, "FileSearch"),
        binding!("k", ToggleObjectActions, "Clipboard"),
        binding!("k", ToggleObjectActions, "ClipboardShelf"),
        binding!("k", ToggleObjectActions, "ApplicationHistory"),
        binding!("k", ToggleObjectActions, "Tasks"),
        binding!("r", RefreshPage, "Tasks"),
        binding!("r", RefreshPage, "Clipboard"),
        binding!("r", RefreshPage, "ClipboardShelf"),
        binding!("r", RefreshPage, "ApplicationHistory"),
        binding!("f", FocusSearch, "Tasks"),
        binding!("f", FocusSearch, "Clipboard"),
        binding!("f", FocusSearch, "ClipboardShelf"),
        binding!("f", FocusSearch, "ApplicationHistory"),
        binding!("s", SaveSettings, "Settings"),
        binding!("enter", ToggleRootNavigation, "Launcher"),
        binding!("enter", CopyAndReturn, "ClipboardShelf"),
        binding!("shift-backspace", DeleteShelfItem, "ClipboardShelf"),
        binding!("l", RestoreLauncher, "Tasks"),
        binding!(".", StopSelectedTask, "Tasks"),
        binding!("alt-down", SelectNextTaskFromDetail, "Tasks"),
        binding!("alt-up", SelectPreviousTaskFromDetail, "Tasks"),
    ];
    cx.bind_keys(bindings);
}

impl LauncherView {
    pub(super) fn on_action_open_settings(
        &mut self,
        _: &OpenSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen == Screen::Settings
            || self.standalone_shelf
            || self.standalone_tasks
            || self.standalone_tool
            || self.preview.open
        {
            cx.propagate();
            return;
        }
        self.open_settings(window, cx);
    }

    pub(super) fn on_action_quit_app(
        &mut self,
        _: &QuitApp,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen == Screen::Settings {
            self.request_settings_exit(SettingsExitIntent::Quit, window, cx);
        } else {
            cx.emit(LauncherEvent::QuitRequested);
        }
    }

    pub(super) fn on_action_toggle_preview(
        &mut self,
        _: &TogglePreview,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.root_navigation_open || !self.toggle_preview_shortcut(window, cx) {
            cx.propagate();
        }
    }

    pub(super) fn on_action_close_preview_to_editor(
        &mut self,
        _: &ClosePreviewToEditor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.preview.open {
            cx.propagate();
            return;
        }
        self.close_preview(cx);
        self.focus_active_editor(window, cx);
    }

    pub(super) fn on_action_toggle_object_actions(
        &mut self,
        _: &ToggleObjectActions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.root_navigation_open {
            cx.propagate();
            return;
        }
        match self.screen {
            Screen::Tasks => {
                if self.tasks.detail.is_some() {
                    self.open_task_actions(window, cx);
                } else {
                    self.open_tool_options(window, cx);
                }
            }
            Screen::Clipboard | Screen::ClipboardShelf => {
                if self.clipboard.actions_open {
                    self.close_clipboard_actions(cx);
                } else if let Some(item_id) = self.clipboard.selected_item().map(|item| item.id) {
                    self.open_clipboard_actions(item_id, cx);
                } else {
                    self.open_empty_clipboard_actions(cx);
                }
            }
            Screen::ApplicationHistory => {
                if self.app_history.object_actions_active {
                    self.app_history.close_object_actions();
                    cx.notify();
                } else if self.app_history.fallback_actions_open {
                    self.app_history.fallback_actions_open = false;
                    cx.notify();
                } else if let Some(item) = self.app_history.selected_item() {
                    if item.can_open {
                        self.request_app_history_object_actions(cx);
                    } else {
                        self.toggle_fallback_app_history_actions(cx);
                    }
                } else {
                    self.toggle_app_history_page_actions(cx);
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
            _ => cx.propagate(),
        }
    }

    pub(super) fn on_action_refresh_page(
        &mut self,
        _: &RefreshPage,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.screen {
            Screen::Tasks => self.start_tasks_refresh(cx),
            Screen::Clipboard | Screen::ClipboardShelf => self.request_clipboard_search(cx),
            Screen::ApplicationHistory => self.refresh_app_history_from_page_menu(cx),
            _ => cx.propagate(),
        }
    }

    pub(super) fn on_action_focus_search(
        &mut self,
        _: &FocusSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen == Screen::Tasks {
            if !self.tasks.rows.is_empty() {
                self.tasks_filter_input.update(cx, |input, cx| {
                    input.focus(window, cx);
                    input.select_all(window, cx);
                });
            }
        } else if matches!(self.screen, Screen::Clipboard | Screen::ClipboardShelf) {
            self.clipboard_input.update(cx, |input, cx| {
                input.focus(window, cx);
                input.select_all(window, cx);
            });
        } else if self.screen == Screen::ApplicationHistory {
            self.history_input.update(cx, |input, cx| {
                input.focus(window, cx);
                input.select_all(window, cx);
            });
        } else {
            cx.propagate();
        }
    }

    pub(super) fn on_action_save_settings(
        &mut self,
        _: &SaveSettings,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen == Screen::Settings {
            self.save_settings(cx);
        } else {
            cx.propagate();
        }
    }

    pub(super) fn on_action_toggle_root_navigation(
        &mut self,
        _: &ToggleRootNavigation,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen == Screen::Launcher {
            self.toggle_root_navigation(cx);
        } else {
            cx.propagate();
        }
    }

    pub(super) fn on_action_copy_and_return(
        &mut self,
        _: &CopyAndReturn,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen == Screen::ClipboardShelf {
            self.copy_selected_clipboard_and_return(cx);
        } else {
            cx.propagate();
        }
    }

    pub(super) fn on_action_restore_launcher(
        &mut self,
        _: &RestoreLauncher,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen == Screen::Tasks {
            cx.emit(LauncherEvent::RestoreLauncher);
        } else {
            cx.propagate();
        }
    }

    pub(super) fn on_action_stop_selected_task(
        &mut self,
        _: &StopSelectedTask,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen == Screen::Tasks {
            self.stop_selected_execution(cx);
        } else {
            cx.propagate();
        }
    }

    pub(super) fn on_action_delete_shelf_item(
        &mut self,
        _: &DeleteShelfItem,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen == Screen::ClipboardShelf {
            if let Some(item_id) = self.clipboard.selected_item().map(|item| item.id) {
                self.request_delete_clipboard_item(item_id, cx);
            }
        } else {
            cx.propagate();
        }
    }

    pub(super) fn on_action_select_next_task_from_detail(
        &mut self,
        _: &SelectNextTaskFromDetail,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen == Screen::Tasks {
            self.handle_tasks_key("down", window, cx);
        } else {
            cx.propagate();
        }
    }

    pub(super) fn on_action_select_previous_task_from_detail(
        &mut self,
        _: &SelectPreviousTaskFromDetail,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.screen == Screen::Tasks {
            self.handle_tasks_key("up", window, cx);
        } else {
            cx.propagate();
        }
    }
}
