use super::*;

#[derive(Default)]
pub(super) struct ToolListScrolls {
    pub(super) launcher: gpui_kit::UniformListScrollHandle,
    pub(super) file_search: gpui_kit::UniformListScrollHandle,
    pub(super) clipboard: gpui_kit::UniformListScrollHandle,
    pub(super) clipboard_shelf: gpui_kit::UniformListScrollHandle,
    pub(super) history: gpui_kit::UniformListScrollHandle,
    pub(super) tasks: gpui_kit::UniformListScrollHandle,
    pub(super) tasks_output: gpui_kit::UniformListScrollHandle,
}

#[derive(Default)]
pub(super) struct ScrollHandles {
    pub(super) scope: ScrollHandle,
    pub(super) notice: ScrollHandle,
    pub(super) file_filter: ScrollHandle,
    pub(super) web_packages: ScrollHandle,
    pub(super) unsupported_tool: ScrollHandle,
    pub(super) history_actions: ScrollHandle,
    pub(super) tool_lists: ToolListScrolls,
}

impl ScrollHandles {
    pub(super) fn base(handle: &gpui_kit::UniformListScrollHandle) -> ScrollHandle {
        handle.0.borrow().base_handle.clone()
    }

    pub(super) fn launcher_base(&self) -> ScrollHandle {
        Self::base(&self.tool_lists.launcher)
    }

    pub(super) fn file_search_base(&self) -> ScrollHandle {
        Self::base(&self.tool_lists.file_search)
    }

    pub(super) fn clipboard_base(&self) -> ScrollHandle {
        Self::base(&self.tool_lists.clipboard)
    }

    pub(super) fn clipboard_shelf_base(&self) -> ScrollHandle {
        Self::base(&self.tool_lists.clipboard_shelf)
    }

    pub(super) fn history_base(&self) -> ScrollHandle {
        Self::base(&self.tool_lists.history)
    }

    pub(super) fn tasks_base(&self) -> ScrollHandle {
        Self::base(&self.tool_lists.tasks)
    }

    pub(super) fn tasks_output_base(&self) -> ScrollHandle {
        Self::base(&self.tool_lists.tasks_output)
    }
}

#[derive(Default)]
pub(super) struct IconLoader {
    pub(super) cache: ApplicationIconCache,
    pub(super) requested: HashMap<String, ApplicationIconRequest>,
    pub(super) generation: u64,
}

impl IconLoader {
    pub(super) fn trim_for_idle(&mut self) -> Vec<Arc<gpui_kit::RenderImage>> {
        self.requested.clear();
        self.cache.trim_for_idle()
    }
}

#[derive(Default)]
pub(super) struct PreviewState {
    pub(super) host: Option<PreviewHost>,
    pub(super) busy: bool,
    pub(super) generation: u64,
    pub(super) request_generation: u64,
    pub(super) open: bool,
    pub(super) selection_task: Option<gpui_kit::Task<()>>,
}

#[derive(Default)]
pub(super) struct HudToastState {
    pub(super) message: Option<String>,
    pub(super) icon: Option<IconName>,
    pub(super) kind: components::NoticeKind,
    pub(super) generation: u64,
    pub(super) task: Option<gpui_kit::Task<()>>,
}

#[derive(Default)]
pub(super) struct WindowPresence {
    pub(super) presented: bool,
    pub(super) retained_workspace: bool,
    pub(super) hidden_at: Option<Instant>,
    pub(super) retention_task: Option<gpui_kit::Task<()>>,
    pub(super) resize_pending: bool,
}

impl WindowPresence {
    pub(super) fn clear_retention(&mut self) {
        self.retention_task = None;
        self.hidden_at = None;
        self.retained_workspace = false;
    }
}

#[derive(Default)]
pub(super) struct ActivationPrefill {
    pub(super) epoch: u64,
    pub(super) enabled: bool,
    pub(super) grace_passed: bool,
    pub(super) pending: bool,
    pub(super) reconsider: bool,
    pub(super) inflight_captures: Option<Vec<anywhere_protocol::ContextCapturePresentation>>,
    pub(super) last_captures: Option<Vec<anywhere_protocol::ContextCapturePresentation>>,
    pub(super) last_allow_pending_selection: bool,
}

impl ActivationPrefill {
    pub(super) fn begin(&mut self, enabled: bool) -> u64 {
        self.cancel();
        self.enabled = enabled;
        self.epoch
    }

    pub(super) fn cancel(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.enabled = false;
        self.grace_passed = false;
        self.pending = false;
        self.reconsider = false;
        self.inflight_captures = None;
        self.last_captures = None;
        self.last_allow_pending_selection = false;
    }
}

pub(super) struct ShortcutEditor {
    pub(super) action_input: Entity<InputState>,
    pub(super) context_name_input: Entity<InputState>,
    pub(super) current_input: Entity<InputState>,
    pub(super) universal_input: Entity<InputState>,
    pub(super) recording: Option<ShortcutSlot>,
    pub(super) pending_action: Option<ShortcutCatalogAction>,
    pub(super) pending_value: Option<String>,
    pub(super) pending_global: bool,
    pub(super) context_draft: Option<ContextShortcutDraft>,
    pub(super) catalog: Vec<ShortcutCatalogAction>,
    pub(super) catalog_loading: bool,
    pub(super) resolving: Option<String>,
}

impl ShortcutEditor {
    pub(super) fn new(
        auxiliary: bool,
        fallback_input: &Entity<InputState>,
        preferences: &HostPreferences,
        locale: Locale,
        window: &mut Window,
        cx: &mut Context<LauncherView>,
    ) -> (Self, Vec<Subscription>) {
        let action_input = if auxiliary {
            fallback_input.clone()
        } else {
            cx.new(|cx| InputState::new(window, cx).placeholder(l10n::find_an_action(locale)))
        };
        let context_name_input = if auxiliary {
            fallback_input.clone()
        } else {
            cx.new(|cx| InputState::new(window, cx).placeholder(l10n::e_gopen_terminal(locale)))
        };
        let current_input = if auxiliary {
            fallback_input.clone()
        } else {
            cx.new(|cx| {
                InputState::new(window, cx).default_value(preferences.current_app_shortcut.clone())
            })
        };
        let universal_input = if auxiliary {
            fallback_input.clone()
        } else {
            cx.new(|cx| {
                InputState::new(window, cx).default_value(preferences.universal_shortcut.clone())
            })
        };
        let subscriptions = vec![
            cx.subscribe(
                &action_input,
                move |_: &mut LauncherView, _, event: &InputEvent, cx| {
                    if !auxiliary && matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                },
            ),
            cx.subscribe(
                &context_name_input,
                move |_: &mut LauncherView, _, event: &InputEvent, cx| {
                    if !auxiliary && matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                },
            ),
        ];
        let editor = Self {
            action_input,
            context_name_input,
            current_input,
            universal_input,
            recording: None,
            pending_action: None,
            pending_value: None,
            pending_global: false,
            context_draft: None,
            catalog: Vec::new(),
            catalog_loading: false,
            resolving: None,
        };
        (editor, subscriptions)
    }
}

pub(super) struct SettingsEditor {
    pub(super) model: SettingsModel,
    pub(super) draft: HostPreferences,
    pub(super) excluded_apps_input: Entity<TextareaState>,
    pub(super) file_roots_input: Entity<TextareaState>,
    pub(super) file_exclusions_input: Entity<TextareaState>,
    pub(super) history_path_input: Entity<InputState>,
    pub(super) history_filter_input: Entity<InputState>,
    pub(super) history_enabled_only: bool,
    pub(super) action_picker_open: bool,
    pub(super) action_picker_focus: FocusHandle,
    pub(super) close_pending: bool,
    pub(super) exit_intent: Option<SettingsExitIntent>,
    pub(super) dismiss_on_done: bool,
    pub(super) modal_return_focus: Option<FocusHandle>,
    pub(super) keep_editing_focus: FocusHandle,
    pub(super) discard_focus: FocusHandle,
    pub(super) save_focus: FocusHandle,
    pub(super) confirm_cancel_focus: FocusHandle,
    pub(super) confirm_accept_focus: FocusHandle,
    pub(super) clipboard_choice: Option<ClipboardSettingsChoice>,
    pub(super) save_generation: u64,
    pub(super) draft_save_queued: bool,
    pub(super) scrolls: [ScrollHandle; 7],
    pub(super) focus_handle: FocusHandle,
}

impl SettingsEditor {
    pub(super) fn new(
        auxiliary: bool,
        fallback_input: &Entity<InputState>,
        model: SettingsModel,
        preferences: &HostPreferences,
        locale: Locale,
        window: &mut Window,
        cx: &mut Context<LauncherView>,
    ) -> (Self, Subscription) {
        let excluded_apps_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .default_value(preferences.excluded_apps.join("\n"))
                .auto_grow(2, 4)
        });
        let file_roots_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .default_value(readable_search_roots(&preferences.file_search_roots).join("\n"))
                .auto_grow(2, 4)
        });
        let file_exclusions_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .default_value(preferences.file_search_excluded_names.join("\n"))
                .auto_grow(2, 4)
        });
        let history_path_input = if auxiliary {
            fallback_input.clone()
        } else {
            cx.new(|cx| InputState::new(window, cx))
        };
        let history_filter_input = if auxiliary {
            fallback_input.clone()
        } else {
            cx.new(|cx| {
                InputState::new(window, cx).placeholder(l10n::app_history_filter_sources(locale))
            })
        };
        let history_filter_subscription = cx.subscribe(
            &history_filter_input,
            move |_: &mut LauncherView, _, event: &InputEvent, cx| {
                if !auxiliary && matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
        let editor = Self {
            model,
            draft: preferences.clone(),
            excluded_apps_input,
            file_roots_input,
            file_exclusions_input,
            history_path_input,
            history_filter_input,
            history_enabled_only: false,
            action_picker_open: false,
            action_picker_focus: cx.focus_handle(),
            close_pending: false,
            exit_intent: None,
            dismiss_on_done: false,
            modal_return_focus: None,
            keep_editing_focus: cx.focus_handle(),
            discard_focus: cx.focus_handle(),
            save_focus: cx.focus_handle(),
            confirm_cancel_focus: cx.focus_handle(),
            confirm_accept_focus: cx.focus_handle(),
            clipboard_choice: None,
            save_generation: 0,
            draft_save_queued: false,
            scrolls: std::array::from_fn(|_| ScrollHandle::new()),
            focus_handle: cx.focus_handle(),
        };
        (editor, history_filter_subscription)
    }
}
