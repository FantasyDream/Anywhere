use gpui_kit::assets::IconName;
use gpui_kit::base::{InteractiveElementExt as _, Selectable as _};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputEditsEvent, InputEvent, InputState, Textarea, TextareaState},
        scroll::ScrollableElement as _,
        spinner::Spinner,
        switch::Switch,
        Disableable as _, Icon, Sizable as _,
    },
    div, img,
    prelude::*,
    px, AnyElement, AnyWindowHandle, ClipboardItem, Context, Div, Entity, FocusHandle,
    Focusable as _, FontWeight, Image, ImageFormat, IntoElement, MouseButton, ObjectFit, Render,
    ScrollHandle, SharedString, Styled, Subscription, TestSupportExt as _, Window,
};

use crate::{
    host::{
        effective_shortcuts, ActionShortcutBinding, ActionShortcutRoute, HostPreferences,
        HotKeyController, LiveEvent, LiveLauncher, ShortcutCatalogAction, UiLanguage,
    },
    l10n::{self, Locale},
    launcher::{
        AppHistoryPageModel, ClipboardPageModel, FileSearchPageModel, TasksPageModel, WebPageModel,
    },
    platform::{open_privacy_settings, PreviewHost, PreviewPoll, PrivacySettingsPane},
    settings::{SettingsCategory, SettingsModel},
    ui::{
        design_tokens::{fonts, metrics},
        styles::AppStyle as _,
        theme, LAUNCHER_CORNER_RADIUS, LAUNCHER_ROW_HEIGHT, LAUNCHER_WIDTH, SETTINGS_HEIGHT,
    },
    web_tool::{
        bootstrap_script, canvas_script, plain_response, reply_script, state_script,
        visibility_script, WebDocumentPolicy,
    },
    PRODUCT_NAME,
};

use super::{
    components, icon_cache::ApplicationIconCache, presentation::*, LauncherModel, LauncherRow,
    LauncherWorkspace, SnapshotGate,
};
use anywhere_protocol::{
    CandidateKind, ContextCaptureStatus, ContextSource, FileSearchFilter, InputMode,
};

use std::{
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc::Receiver, oneshot};

mod candidate_row;
mod input;
mod live_state;
mod navigation;
mod page_shell;
mod pages;
mod settings;
#[path = "settings/controller.rs"]
mod settings_controller;
mod web_surface;

use live_state::LiveState;
use web_surface::{ActiveWebBinding, WebIpcRequest, WebSurfaceState};

const WEB_FRAME_BYTES: usize = 64 * 1024;
const WEB_IPC_QUEUE_CAPACITY: usize = 32;
const MAX_APPLICATION_ICON_REQUESTS: usize = 4;
const SCROLLBAR_OVERLAY_CHILD_COUNT: usize = 1;

#[derive(Clone)]
struct ApplicationIconRequest {
    ticket: u64,
    cache_key: String,
}

#[derive(Clone, Debug)]
struct ParentWorkspace {
    input_mode: InputMode,
    input_value: String,
    content_value: String,
    launcher: LauncherWorkspace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SettingsExitIntent {
    Close,
    Wake(InputMode),
    Quit,
}

#[derive(Clone, Copy, Debug)]
enum ClipboardOperationFailure {
    Copy,
    Delete,
    Settings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShortcutSlot {
    CurrentApp,
    Universal,
    Action,
    ContextShortcut,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClipboardSettingsChoice {
    Retention,
    Pause,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClipboardConfirmation {
    Clear,
    SetRetention(u32),
}

fn clipboard_confirmation_is_submittable(
    loading: bool,
    presented: Option<ClipboardConfirmation>,
    requested: ClipboardConfirmation,
) -> bool {
    !loading && presented == Some(requested)
}

#[derive(Clone, Debug)]
struct ContextShortcutDraft {
    action_id: String,
    title: String,
    shortcut: Option<String>,
    global: bool,
    routes: Vec<ActionShortcutRoute>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct KeyModifiers {
    command: bool,
    control: bool,
    shift: bool,
    alt: bool,
    modified: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ScopeSelectionIntent {
    #[default]
    UserControlled,
    PendingCurrentApp,
    PendingCurrentAppNavigation,
}

fn key_modifiers(event: &gpui_kit::KeyDownEvent) -> KeyModifiers {
    KeyModifiers {
        command: event.keystroke.modifiers.platform,
        control: event.keystroke.modifiers.control,
        shift: event.keystroke.modifiers.shift,
        alt: event.keystroke.modifiers.alt,
        modified: event.keystroke.modifiers.modified(),
    }
}

fn window_size_changed(
    current: gpui_kit::Size<gpui_kit::Pixels>,
    target: gpui_kit::Size<gpui_kit::Pixels>,
) -> bool {
    (current.width.as_f32() - target.width.as_f32()).abs() > 0.5
        || (current.height.as_f32() - target.height.as_f32()).abs() > 0.5
}

fn is_scope_tab_shortcut(key: &str, modifiers: KeyModifiers) -> bool {
    key.eq_ignore_ascii_case("tab") && !modifiers.command && !modifiers.alt
}

fn is_tool_page_tab_shortcut(screen: Screen, key: &str, modifiers: KeyModifiers) -> bool {
    matches!(
        screen,
        Screen::Clipboard
            | Screen::ClipboardShelf
            | Screen::ApplicationHistory
            | Screen::PluginDetail
            | Screen::UnsupportedTool
    ) && key.eq_ignore_ascii_case("tab")
        && !modifiers.command
        && !modifiers.control
        && !modifiers.alt
}

fn is_tasks_global_shortcut(key: &str, modifiers: KeyModifiers) -> bool {
    if !modifiers.modified && !modifiers.command {
        return key.eq_ignore_ascii_case("escape");
    }
    let command = modifiers.command && !modifiers.control && !modifiers.shift;
    if command && !modifiers.alt {
        return matches!(
            key.to_ascii_lowercase().as_str(),
            "k" | "l" | "." | "r" | "f"
        );
    }
    command && modifiers.alt && matches!(key.to_ascii_lowercase().as_str(), "up" | "down")
}

fn should_handle_tasks_shortcut(key: &str, modifiers: KeyModifiers, character_input: bool) -> bool {
    is_tasks_global_shortcut(key, modifiers) && (modifiers.command || !character_input)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Screen {
    #[default]
    Launcher,
    FileSearch,
    Clipboard,
    ClipboardShelf,
    ApplicationHistory,
    Tasks,
    WebPackages,
    WebTool,
    PluginDetail,
    UnsupportedTool,
    Settings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeState {
    Preview,
    Starting,
    Ready,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LauncherEvent {
    RestoreLauncher,
    OpenClipboardShelf,
    OpenTasks(String),
    WebViewEscapeRequested,
    QuitRequested,
    PreferencesApplied {
        locale: Locale,
        preferences: HostPreferences,
    },
}

impl gpui_kit::EventEmitter<LauncherEvent> for LauncherView {}

/// Coordinates the GPUI workspace and delegates matching, ranking and
/// authorization to an authoritative `LauncherState` snapshot.
pub struct LauncherView {
    input: Entity<InputState>,
    content_input: Entity<TextareaState>,
    clipboard_input: Entity<InputState>,
    history_input: Entity<InputState>,
    tasks_filter_input: Entity<InputState>,
    model: LauncherModel,
    file_search: FileSearchPageModel,
    clipboard: ClipboardPageModel,
    app_history: AppHistoryPageModel,
    tasks: TasksPageModel,
    pending_task_selection: Option<String>,
    tasks_output_scroll: ScrollHandle,
    tasks_refresh_cancel: Option<oneshot::Sender<()>>,
    web: WebPageModel,
    web_surface: WebSurfaceState,
    application_icons: ApplicationIconCache,
    application_icons_requested: HashMap<String, ApplicationIconRequest>,
    application_icon_generation: u64,
    preferences: HostPreferences,
    settings_draft: HostPreferences,
    excluded_apps_input: Entity<InputState>,
    file_roots_input: Entity<InputState>,
    file_exclusions_input: Entity<InputState>,
    history_path_input: Entity<InputState>,
    history_settings_filter_input: Entity<InputState>,
    history_settings_enabled_only: bool,
    action_shortcut_input: Entity<InputState>,
    context_shortcut_name_input: Entity<InputState>,
    current_shortcut_input: Entity<InputState>,
    universal_shortcut_input: Entity<InputState>,
    shortcut_recording: Option<ShortcutSlot>,
    settings_action_picker_open: bool,
    pending_action_shortcut: Option<ShortcutCatalogAction>,
    pending_action_shortcut_value: Option<String>,
    pending_action_shortcut_global: bool,
    context_shortcut_draft: Option<ContextShortcutDraft>,
    shortcut_catalog: Vec<ShortcutCatalogAction>,
    shortcut_catalog_loading: bool,
    pending_shortcut_execution: Option<ActionShortcutBinding>,
    action_shortcut_resolving: Option<String>,
    settings_close_pending: bool,
    settings_exit_intent: Option<SettingsExitIntent>,
    settings_modal_return_focus: Option<FocusHandle>,
    settings_keep_editing_focus: FocusHandle,
    settings_discard_focus: FocusHandle,
    settings_save_focus: FocusHandle,
    settings_confirm_cancel_focus: FocusHandle,
    settings_confirm_accept_focus: FocusHandle,
    clipboard_settings_choice: Option<ClipboardSettingsChoice>,
    clipboard_confirmation: Option<ClipboardConfirmation>,
    settings_save_generation: u64,
    parent_workspace: Option<ParentWorkspace>,
    input_mode: InputMode,
    scope_selection_intent: ScopeSelectionIntent,
    input_placeholder: String,
    root_navigation_open: bool,
    root_navigation_mode: InputMode,
    scope_scroll: ScrollHandle,
    notice_scroll: ScrollHandle,
    file_filter_scroll: ScrollHandle,
    result_scroll: ScrollHandle,
    file_search_scroll: ScrollHandle,
    clipboard_scroll: ScrollHandle,
    clipboard_shelf_scroll: ScrollHandle,
    history_scroll: ScrollHandle,
    tasks_scroll: ScrollHandle,
    web_packages_scroll: ScrollHandle,
    unsupported_tool_scroll: ScrollHandle,
    settings_scrolls: [ScrollHandle; 7],
    settings: SettingsModel,
    settings_focus_handle: FocusHandle,
    screen: Screen,
    locale: Locale,
    notice: Option<String>,
    notice_kind: components::NoticeKind,
    preview: Option<PreviewHost>,
    preview_busy: bool,
    preview_generation: u64,
    preview_open: bool,
    live: Option<Arc<LiveLauncher>>,
    hotkeys: Option<Arc<Mutex<HotKeyController>>>,
    standalone_shelf: bool,
    standalone_tasks: bool,
    runtime_state: RuntimeState,
    live_state: LiveState,
    presented: bool,
    retained_workspace: bool,
    hidden_at: Option<Instant>,
    window_handle: AnyWindowHandle,
    _editor_sync_guard: Rc<Cell<bool>>,
    _input_subscription: Subscription,
    _key_interceptor: Subscription,
    _page_input_subscriptions: Vec<Subscription>,
}

impl LauncherView {
    pub(crate) fn set_startup_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.runtime_state = RuntimeState::Failed;
        self.set_notice(error, components::NoticeKind::Error);
        self.model.loading = false;
        cx.notify();
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let view = Self::build(
            window,
            cx,
            LauncherModel::preview(Locale::En),
            Locale::En,
            None,
            None,
            false,
        );
        view.focus_launcher_input(window, cx);
        view.resize_window_for_screen(window);
        view
    }

    pub fn with_model(
        window: &mut Window,
        cx: &mut Context<Self>,
        model: LauncherModel,
        locale: Locale,
    ) -> Self {
        let view = Self::build(window, cx, model, locale, None, None, false);
        view.focus_launcher_input(window, cx);
        view
    }

    pub fn starting(window: &mut Window, cx: &mut Context<Self>, locale: Locale) -> Self {
        let mut view = Self::build(
            window,
            cx,
            LauncherModel::loading(locale),
            locale,
            None,
            None,
            false,
        );
        view.focus_launcher_input(window, cx);
        view.runtime_state = RuntimeState::Starting;
        view
    }

    pub fn live(
        window: &mut Window,
        cx: &mut Context<Self>,
        live: Arc<LiveLauncher>,
        locale: Locale,
    ) -> Self {
        let locale = live
            .preferences()
            .map(|preferences| locale_for_language(preferences.ui_language))
            .unwrap_or(locale);
        let view = Self::build(
            window,
            cx,
            LauncherModel::loading(locale),
            locale,
            Some(live),
            None,
            false,
        );
        view.focus_launcher_input(window, cx);
        view
    }

    pub fn live_with_hotkeys(
        window: &mut Window,
        cx: &mut Context<Self>,
        live: Arc<LiveLauncher>,
        locale: Locale,
        hotkeys: Arc<Mutex<HotKeyController>>,
    ) -> Self {
        let locale = live
            .preferences()
            .map(|preferences| locale_for_language(preferences.ui_language))
            .unwrap_or(locale);
        let view = Self::build(
            window,
            cx,
            LauncherModel::loading(locale),
            locale,
            Some(live),
            Some(hotkeys),
            false,
        );
        view.focus_launcher_input(window, cx);
        view
    }

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
            true,
        );
        view.screen = Screen::ClipboardShelf;
        view.clipboard_input
            .update(cx, |input, cx| input.focus(window, cx));
        view
    }

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
            false,
        );
        view.screen = Screen::Tasks;
        view.standalone_tasks = true;
        view.presented = true;
        window.blur(cx);
        view.resize_window_for_screen(window);
        view
    }

    pub(crate) fn attach_runtime(
        &mut self,
        live: Arc<LiveLauncher>,
        hotkeys: Option<Arc<Mutex<HotKeyController>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let preferences = live.preferences().unwrap_or_default();
        let locale = locale_for_language(preferences.ui_language);
        let settings_error = live.search_access_error();
        let mut settings = live
            .settings_snapshot()
            .ok()
            .map(SettingsModel::from_snapshot)
            .unwrap_or_default();
        settings.error = settings_error.clone();

        set_input_value(
            &self.excluded_apps_input,
            preferences.excluded_apps.join("\n"),
            window,
            cx,
        );
        set_input_value(
            &self.file_roots_input,
            preferences.file_search_roots.join("\n"),
            window,
            cx,
        );
        set_input_value(
            &self.file_exclusions_input,
            preferences.file_search_excluded_names.join("\n"),
            window,
            cx,
        );
        set_input_value(
            &self.current_shortcut_input,
            preferences.current_app_shortcut.clone(),
            window,
            cx,
        );
        set_input_value(
            &self.universal_shortcut_input,
            preferences.universal_shortcut.clone(),
            window,
            cx,
        );
        self.input.update(cx, |input, cx| {
            input.set_placeholder(l10n::search_apps_actions(locale), window, cx)
        });
        self.content_input.update(cx, |input, cx| {
            input.set_placeholder(l10n::edit_captured_content(locale), window, cx)
        });
        self.clipboard_input.update(cx, |input, cx| {
            input.set_placeholder(l10n::search_history(locale), window, cx)
        });
        self.history_input.update(cx, |input, cx| {
            input.set_placeholder(l10n::app_history_search(locale), window, cx)
        });
        self.tasks_filter_input.update(cx, |input, cx| {
            input.set_placeholder(l10n::find_atask(locale), window, cx)
        });
        self.history_settings_filter_input.update(cx, |input, cx| {
            input.set_placeholder(l10n::app_history_filter_sources(locale), window, cx)
        });
        self.action_shortcut_input.update(cx, |input, cx| {
            input.set_placeholder(l10n::find_an_action(locale), window, cx)
        });
        self.context_shortcut_name_input.update(cx, |input, cx| {
            input.set_placeholder(l10n::e_gopen_terminal(locale), window, cx)
        });

        self.preferences = preferences.clone();
        self.settings_draft = preferences;
        self.settings = settings;
        self.locale = locale;
        self.model.set_locale(locale);
        self.input_placeholder = l10n::search_apps_actions(locale);
        self.notice = settings_error;
        self.notice_kind = components::NoticeKind::Error;
        self.live = Some(live);
        self.hotkeys = hotkeys;
        self.runtime_state = RuntimeState::Ready;
        cx.notify();
    }

    pub fn initialize_clipboard_shelf(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            return;
        };
        tracing::debug!("clipboard_shelf_initial_snapshot_requested");
        self.clipboard.loading();
        let receiver = live.clipboard_snapshot(self.clipboard.query.clone());
        self.poll(receiver, self.live_state.generation, cx);
        cx.notify();
    }

    pub fn initialize_tasks_window(
        &mut self,
        selected_task_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.live.is_none() {
            return;
        }
        self.pending_task_selection = selected_task_id;
        self.start_tasks_refresh(cx);
        cx.notify();
    }

    pub fn select_execution(&mut self, task_id: String, cx: &mut Context<Self>) {
        self.pending_task_selection = Some(task_id);
        if let Some(task_id) = self.pending_task_selection.clone() {
            if let Some(index) = self.tasks.rows.iter().position(|row| row.id == task_id) {
                self.pending_task_selection = None;
                self.tasks.selected = index;
                self.tasks.detail = None;
                self.tasks.tool_options_open = false;
                self.tasks.tool_option_selected = 0;
                self.tasks.task_actions_open = false;
                self.tasks.task_action_selected = 0;
                self.tasks.detail_request = self.tasks.detail_request.wrapping_add(1);
                self.reveal_result_row(index);
                self.request_task_detail(task_id, cx);
            }
        }
        cx.notify();
    }

    fn build(
        window: &mut Window,
        cx: &mut Context<Self>,
        model: LauncherModel,
        locale: Locale,
        live: Option<Arc<LiveLauncher>>,
        hotkeys: Option<Arc<Mutex<HotKeyController>>>,
        standalone_shelf: bool,
    ) -> Self {
        let runtime_state = if live.is_some() {
            RuntimeState::Ready
        } else {
            RuntimeState::Preview
        };
        let mut settings = live
            .as_ref()
            .and_then(|live| live.settings_snapshot().ok())
            .map(SettingsModel::from_snapshot)
            .unwrap_or_default();
        let search_access_error = live.as_ref().and_then(|live| live.search_access_error());
        if let Some(error) = &search_access_error {
            settings.error = Some(error.clone());
        }
        let preferences = live
            .as_ref()
            .and_then(|live| live.preferences().ok())
            .unwrap_or_default();
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder(l10n::search_apps_actions(locale)));
        let content_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder(l10n::edit_captured_content(locale))
                .auto_grow(2, 2)
        });
        let clipboard_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(l10n::search_history(locale)));
        let history_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(l10n::app_history_search(locale)));
        let tasks_filter_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(l10n::find_atask(locale)));
        let editor_sync_guard = Rc::new(Cell::new(false));
        let input_sync_guard = editor_sync_guard.clone();
        let input_edits_subscription =
            cx.subscribe(&input, |this: &mut Self, _, event: &InputEditsEvent, cx| {
                if this.screen != Screen::FileSearch
                    || (!this.preview_open && !this.file_search_selection_active())
                {
                    return;
                }
                let Some((restored_value, cursor)) =
                    plain_space_insertion(event, &this.model.query)
                else {
                    return;
                };
                this.file_search.space_edit = Some((restored_value, cursor));
                if crate::platform::current_key_event_is_repeat() {
                    return;
                }
                if this.preview_open {
                    this.close_preview(cx);
                } else {
                    this.request_preview(cx);
                }
            });
        let _input_subscription =
            cx.subscribe(&input, move |this: &mut Self, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    if input_sync_guard.get() {
                        return;
                    }
                    if let Some((restored_value, cursor)) = this.file_search.space_edit.take() {
                        let input = this.input.clone();
                        let window_handle = this.window_handle;
                        cx.defer(move |cx| {
                            let _ = window_handle.update(cx, |_, window, cx| {
                                input.update(cx, |input, cx| {
                                    input.set_value(restored_value, window, cx);
                                    input.set_selected_range(cursor..cursor, cx);
                                });
                            });
                        });
                        cx.notify();
                        return;
                    }
                    if this.screen == Screen::FileSearch {
                        this.reset_file_search_browsing(cx);
                    }
                    this.mark_query_user_controlled();
                    let value = this.input.read(cx).value().to_string();
                    this.model.set_query(value);
                    if this.model.selected_row().is_some_and(|row| !row.enabled) {
                        this.set_notice(
                            l10n::this_action_unavailable(this.locale),
                            components::NoticeKind::Warning,
                        );
                    } else {
                        this.clear_notice();
                    }
                    this.request_live_update(cx);
                    cx.notify();
                }
            });
        let content_sync_guard = editor_sync_guard.clone();
        let content_subscription = cx.subscribe(
            &content_input,
            move |this: &mut Self, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    if content_sync_guard.get() {
                        return;
                    }
                    this.mark_query_user_controlled();
                    this.model
                        .set_query(this.content_input.read(cx).value().to_string());
                    if this.model.selected_row().is_some_and(|row| !row.enabled) {
                        this.set_notice(
                            l10n::this_action_unavailable(this.locale),
                            components::NoticeKind::Warning,
                        );
                    } else {
                        this.clear_notice();
                    }
                    this.request_live_update(cx);
                    cx.notify();
                }
            },
        );
        let clipboard_edits_subscription = cx.subscribe(
            &clipboard_input,
            |this: &mut Self, _, event: &InputEditsEvent, cx| {
                if !matches!(this.screen, Screen::Clipboard | Screen::ClipboardShelf)
                    || (!this.preview_open && !this.clipboard_selection_active())
                {
                    return;
                }
                let Some((restored_value, cursor)) =
                    plain_space_insertion(event, &this.clipboard.query)
                else {
                    return;
                };
                this.clipboard.space_edit = Some((restored_value, cursor));
                if crate::platform::current_key_event_is_repeat() {
                    return;
                }
                if this.preview_open {
                    this.close_preview(cx);
                } else {
                    this.preview_selected_clipboard(cx);
                }
            },
        );
        let clipboard_subscription = cx.subscribe(
            &clipboard_input,
            |this: &mut Self, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some((restored_value, cursor)) = this.clipboard.space_edit.take() {
                        let input = this.clipboard_input.clone();
                        let window_handle = this.window_handle;
                        cx.defer(move |cx| {
                            let _ = window_handle.update(cx, |_, window, cx| {
                                input.update(cx, |input, cx| {
                                    input.set_value(restored_value, window, cx);
                                    input.set_selected_range(cursor..cursor, cx);
                                });
                            });
                        });
                        cx.notify();
                        return;
                    }
                    this.reset_clipboard_browsing(cx);
                    this.clipboard.query = this.clipboard_input.read(cx).value().to_string();
                    this.request_clipboard_search(cx);
                    cx.notify();
                }
            },
        );
        let history_subscription = cx.subscribe(
            &history_input,
            |this: &mut Self, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.app_history
                        .set_query(this.history_input.read(cx).value().to_string());
                    this.schedule_app_history_search(cx);
                    cx.notify();
                }
            },
        );
        let tasks_filter_subscription = cx.subscribe(
            &tasks_filter_input,
            |this: &mut Self, _, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let previous = this.tasks.selected;
                this.tasks
                    .set_filter(this.tasks_filter_input.read(cx).value().to_string());
                if this.tasks.selected != previous {
                    if let Some(task_id) = this.tasks.selected_row().map(|row| row.id.clone()) {
                        this.request_task_detail(task_id, cx);
                    }
                }
                cx.notify();
            },
        );
        let action_shortcut_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(l10n::find_an_action(locale)));
        let context_shortcut_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(l10n::e_gopen_terminal(locale)));
        let action_shortcut_subscription = cx.subscribe(
            &action_shortcut_input,
            |_: &mut Self, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
        let context_shortcut_name_subscription = cx.subscribe(
            &context_shortcut_name_input,
            |_: &mut Self, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
        let excluded_apps_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(preferences.excluded_apps.join("\n"))
        });
        let file_roots_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(preferences.file_search_roots.join("\n"))
        });
        let file_exclusions_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(preferences.file_search_excluded_names.join("\n"))
        });
        let history_path_input = cx.new(|cx| InputState::new(window, cx));
        let history_settings_filter_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(l10n::app_history_filter_sources(locale))
        });
        let history_settings_filter_subscription = cx.subscribe(
            &history_settings_filter_input,
            |_: &mut Self, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
        let current_shortcut_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(preferences.current_app_shortcut.clone())
        });
        let universal_shortcut_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(preferences.universal_shortcut.clone())
        });
        let key_interceptor = {
            let this = cx.weak_entity();
            cx.intercept_keystrokes(move |event, window, cx| {
                let modifiers = KeyModifiers {
                    command: event.keystroke.modifiers.platform,
                    control: event.keystroke.modifiers.control,
                    shift: event.keystroke.modifiers.shift,
                    alt: event.keystroke.modifiers.alt,
                    modified: event.keystroke.modifiers.modified(),
                };
                let repeated = crate::platform::current_key_event_is_repeat();
                if modifiers.command {
                    tracing::debug!(
                        key = %event.keystroke.key,
                        control = modifiers.control,
                        shift = modifiers.shift,
                        alt = modifiers.alt,
                        "launcher_command_key_observed"
                    );
                }
                let _ = this.update(cx, |this, cx| {
                    // Interceptors are application-wide, including hidden tool views.
                    if this.window_handle != window.window_handle() {
                        return;
                    }
                    if this.shortcut_recording.is_some() {
                        this.handle_key_with_repeat(
                            event.keystroke.key.as_str(),
                            modifiers,
                            repeated,
                            window,
                            cx,
                        );
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    let settings_modal_open = this.screen == Screen::Settings
                        && (this.settings_close_pending || this.clipboard_confirmation.is_some());
                    if settings_modal_open {
                        if is_scope_tab_shortcut(event.keystroke.key.as_str(), modifiers) {
                            this.cycle_settings_modal_focus(
                                event.keystroke.modifiers.shift,
                                window,
                                cx,
                            );
                            window.prevent_default();
                            cx.stop_propagation();
                            return;
                        }
                        if !modifiers.modified && event.keystroke.key.eq_ignore_ascii_case("escape")
                        {
                            if this.clipboard_confirmation.is_some() {
                                this.cancel_clipboard_confirmation(window, cx);
                            } else {
                                this.cancel_settings_close_prompt(window, cx);
                            }
                            window.prevent_default();
                            cx.stop_propagation();
                            return;
                        }
                        let focused_modal_control = window.focused(cx).is_some_and(|focused| {
                            [
                                &this.settings_keep_editing_focus,
                                &this.settings_discard_focus,
                                &this.settings_save_focus,
                                &this.settings_confirm_cancel_focus,
                                &this.settings_confirm_accept_focus,
                            ]
                            .into_iter()
                            .any(|handle| handle == &focused)
                        });
                        let activates_focused_control = !modifiers.modified
                            && matches!(event.keystroke.key.as_str(), "enter" | "space" | "return")
                            && focused_modal_control;
                        if !activates_focused_control {
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                        return;
                    }
                    // Composition belongs to the editor. An open Settings modal
                    // is handled above so IME input cannot reach underlying fields.
                    if matches!(
                        event.input_preference,
                        gpui_kit::InputPreference::CharacterInput
                    ) {
                        return;
                    }
                    if event.keystroke.modifiers.platform
                        && !event.keystroke.modifiers.control
                        && !event.keystroke.modifiers.alt
                        && !event.keystroke.modifiers.shift
                        && event.keystroke.key == ","
                        && !this.standalone_shelf
                        && !this.standalone_tasks
                        && !this.preview_open
                    {
                        this.open_settings(window, cx);
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if this.screen == Screen::Settings
                        && !this.preview_open
                        && event.keystroke.key.eq_ignore_ascii_case("escape")
                    {
                        this.handle_key_with_repeat(
                            event.keystroke.key.as_str(),
                            modifiers,
                            repeated,
                            window,
                            cx,
                        );
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if this.screen == Screen::Settings
                        && is_scope_tab_shortcut(event.keystroke.key.as_str(), modifiers)
                    {
                        let backwards = event.keystroke.modifiers.shift;
                        cx.defer_in(window, move |_, window, cx| {
                            if backwards {
                                window.focus_prev(cx);
                            } else {
                                window.focus_next(cx);
                            }
                        });
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if is_tool_page_tab_shortcut(
                        this.screen,
                        event.keystroke.key.as_str(),
                        modifiers,
                    ) {
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if this.preview_open && event.keystroke.key.eq_ignore_ascii_case("escape") {
                        this.handle_key_with_repeat(
                            event.keystroke.key.as_str(),
                            modifiers,
                            repeated,
                            window,
                            cx,
                        );
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    let command_only = event.keystroke.modifiers.platform
                        && !event.keystroke.modifiers.control
                        && !event.keystroke.modifiers.alt
                        && !event.keystroke.modifiers.shift;
                    if command_only
                        && event.keystroke.key.eq_ignore_ascii_case("y")
                        && this.can_toggle_preview_shortcut()
                    {
                        if !repeated {
                            this.toggle_preview_shortcut(window, cx);
                        }
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if command_only
                        && this.preview_open
                        && event.keystroke.key.eq_ignore_ascii_case("l")
                    {
                        this.close_preview(cx);
                        this.focus_active_editor(window, cx);
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if this.handle_preview_keystroke(
                        event.keystroke.key.as_str(),
                        modifiers.modified,
                        matches!(
                            event.input_preference,
                            gpui_kit::InputPreference::CharacterInput
                        ),
                        repeated,
                        window,
                        cx,
                    ) {
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if this.screen == Screen::Tasks
                        && should_handle_tasks_shortcut(
                            event.keystroke.key.as_str(),
                            modifiers,
                            matches!(
                                event.input_preference,
                                gpui_kit::InputPreference::CharacterInput
                            ),
                        )
                    {
                        this.handle_key_with_repeat(
                            event.keystroke.key.as_str(),
                            modifiers,
                            repeated,
                            window,
                            cx,
                        );
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if matches!(
                        event.input_preference,
                        gpui_kit::InputPreference::CharacterInput
                    ) || !is_scope_tab_shortcut(event.keystroke.key.as_str(), modifiers)
                        || !this.active_editor_focused(window, cx)
                        || !(this.screen == Screen::FileSearch
                            || (this.screen == Screen::Launcher
                                && this.has_scope_bar()
                                && !this.model.object_actions_active
                                && this.model.menu_path().is_none()))
                    {
                        return;
                    }
                    this.handle_key_with_repeat(
                        event.keystroke.key.as_str(),
                        modifiers,
                        repeated,
                        window,
                        cx,
                    );
                    window.prevent_default();
                    cx.stop_propagation();
                });
            })
        };
        Self {
            input,
            content_input,
            clipboard_input,
            history_input,
            tasks_filter_input,
            model,
            file_search: FileSearchPageModel::default(),
            clipboard: ClipboardPageModel::default(),
            app_history: AppHistoryPageModel::default(),
            tasks: TasksPageModel::default(),
            pending_task_selection: None,
            tasks_output_scroll: ScrollHandle::new(),
            tasks_refresh_cancel: None,
            web: WebPageModel::default(),
            web_surface: WebSurfaceState::default(),
            application_icons: ApplicationIconCache::default(),
            application_icons_requested: HashMap::new(),
            application_icon_generation: 0,
            settings_draft: preferences.clone(),
            preferences,
            excluded_apps_input,
            file_roots_input,
            file_exclusions_input,
            history_path_input,
            history_settings_filter_input,
            history_settings_enabled_only: false,
            action_shortcut_input,
            context_shortcut_name_input,
            current_shortcut_input,
            universal_shortcut_input,
            shortcut_recording: None,
            settings_action_picker_open: false,
            pending_action_shortcut: None,
            pending_action_shortcut_value: None,
            pending_action_shortcut_global: false,
            context_shortcut_draft: None,
            shortcut_catalog: Vec::new(),
            shortcut_catalog_loading: false,
            pending_shortcut_execution: None,
            action_shortcut_resolving: None,
            settings_close_pending: false,
            settings_exit_intent: None,
            settings_modal_return_focus: None,
            settings_keep_editing_focus: cx.focus_handle(),
            settings_discard_focus: cx.focus_handle(),
            settings_save_focus: cx.focus_handle(),
            settings_confirm_cancel_focus: cx.focus_handle(),
            settings_confirm_accept_focus: cx.focus_handle(),
            clipboard_settings_choice: None,
            clipboard_confirmation: None,
            settings_save_generation: 0,
            parent_workspace: None,
            input_mode: InputMode::Environment,
            scope_selection_intent: ScopeSelectionIntent::PendingCurrentApp,
            input_placeholder: l10n::search_apps_actions(locale),
            root_navigation_open: false,
            root_navigation_mode: InputMode::Environment,
            scope_scroll: ScrollHandle::new(),
            notice_scroll: ScrollHandle::new(),
            file_filter_scroll: ScrollHandle::new(),
            result_scroll: ScrollHandle::new(),
            file_search_scroll: ScrollHandle::new(),
            clipboard_scroll: ScrollHandle::new(),
            clipboard_shelf_scroll: ScrollHandle::new(),
            history_scroll: ScrollHandle::new(),
            tasks_scroll: ScrollHandle::new(),
            web_packages_scroll: ScrollHandle::new(),
            unsupported_tool_scroll: ScrollHandle::new(),
            settings_scrolls: std::array::from_fn(|_| ScrollHandle::new()),
            settings,
            settings_focus_handle: cx.focus_handle(),
            screen: Screen::Launcher,
            locale,
            notice: search_access_error,
            notice_kind: components::NoticeKind::Error,
            preview: None,
            preview_busy: false,
            preview_generation: 0,
            preview_open: false,
            live,
            hotkeys,
            standalone_shelf,
            standalone_tasks: false,
            runtime_state,
            live_state: LiveState::default(),
            presented: false,
            retained_workspace: false,
            hidden_at: None,
            window_handle: window.window_handle(),
            _editor_sync_guard: editor_sync_guard,
            _input_subscription,
            _key_interceptor: key_interceptor,
            _page_input_subscriptions: vec![
                clipboard_subscription,
                history_subscription,
                input_edits_subscription,
                clipboard_edits_subscription,
                content_subscription,
                action_shortcut_subscription,
                context_shortcut_name_subscription,
                history_settings_filter_subscription,
                tasks_filter_subscription,
            ],
        }
    }

    #[cfg(test)]
    fn apply_state(&mut self, state: anywhere_protocol::LauncherState, cx: &mut Context<Self>) {
        self.apply_shared_state(Arc::new(state), cx);
    }

    fn apply_shared_state(
        &mut self,
        state: Arc<anywhere_protocol::LauncherState>,
        cx: &mut Context<Self>,
    ) -> super::LauncherApply {
        let input = state.input.clone();
        let mut applied = self.model.apply_shared(state);
        if self.apply_pending_current_app_scope(cx) {
            applied.rows_changed = true;
        }
        if applied.input_changed {
            self.sync_launcher_editor(input, cx);
        }
        if applied.rows_changed || applied.scopes_changed {
            self.reveal_selected_scope();
        }
        self.notice = None;
        self.try_start_pending_action_shortcut(cx);
        if self.screen == Screen::WebTool
            && self.web_surface.document.is_some()
            && self.web_surface.view.is_some()
            && !self.web_surface.ready
            && !self.web_surface.resume_busy
        {
            self.resume_retained_web(cx);
        }
        applied
    }

    fn apply_pending_current_app_scope(&mut self, cx: &mut Context<Self>) -> bool {
        let pending_navigation =
            self.scope_selection_intent == ScopeSelectionIntent::PendingCurrentAppNavigation;
        if !matches!(
            self.scope_selection_intent,
            ScopeSelectionIntent::PendingCurrentApp
                | ScopeSelectionIntent::PendingCurrentAppNavigation
        ) || self.screen != Screen::Launcher
            || self.input_mode != InputMode::Environment
            || (!pending_navigation && !self.model.query.trim().is_empty())
            || self.model.object_actions_active
            || self.model.menu_path().is_some()
        {
            return false;
        }
        let Some(current_app) = self.model.scopes.iter().find(|scope| scope.is_current_app) else {
            tracing::debug!(
                scope_count = self.model.scopes.len(),
                query = %self.model.query,
                "launcher_current_app_scope_pending"
            );
            if pending_navigation && self.model.context_complete() {
                self.scope_selection_intent = ScopeSelectionIntent::UserControlled;
                self.model.loading = false;
                self.model.rows.clear();
                self.model.selected = 0;
            }
            return false;
        };
        self.scope_selection_intent = ScopeSelectionIntent::UserControlled;
        if self.model.selected_scope.as_deref() == Some(current_app.id.as_str()) {
            return false;
        }
        tracing::debug!(
            scope_id = %current_app.id,
            scope_count = self.model.scopes.len(),
            "launcher_current_app_scope_selected"
        );
        self.model.select_scope(Some(current_app.id.clone()));
        self.model.loading = true;
        self.model.rows.clear();
        self.model.selected = 0;
        self.request_live_update(cx);
        true
    }

    pub fn restore_from_tool(&mut self, cx: &mut Context<Self>) {
        let handle = self.window_handle;
        let view = cx.entity();
        cx.spawn(async move |_, cx| {
            // ShowMainWindow moves the native panel synchronously. Leave the
            // current entity update first so AppKit cannot re-enter a borrowed
            // GPUI App/Context while Tasks or another tool emits RestoreLauncher.
            cx.background_executor()
                .timer(Duration::from_millis(0))
                .await;
            let _ = handle.update(cx, |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.activate_from_entry(InputMode::Search, false, window, cx)
                })
            });
            crate::platform::show_main_window();
            let _ = handle.update(cx, |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.refocus_active_editor(window, cx);
                });
            });
        })
        .detach();
    }

    fn begin_scope_selection_for_mode(&mut self, input_mode: InputMode) {
        self.scope_selection_intent = if input_mode == InputMode::Environment {
            ScopeSelectionIntent::PendingCurrentApp
        } else {
            ScopeSelectionIntent::UserControlled
        };
    }

    fn begin_navigation_scope_selection(&mut self, input_mode: InputMode) {
        if input_mode != InputMode::Environment {
            self.scope_selection_intent = ScopeSelectionIntent::UserControlled;
            return;
        }
        self.scope_selection_intent = ScopeSelectionIntent::PendingCurrentAppNavigation;
        let Some(current_app) = self
            .model
            .scopes
            .iter()
            .find(|scope| scope.is_current_app)
            .cloned()
        else {
            self.model.loading = true;
            self.model.rows.clear();
            self.model.selected = 0;
            return;
        };
        self.scope_selection_intent = ScopeSelectionIntent::UserControlled;
        self.model.select_scope(Some(current_app.id));
    }

    fn mark_scope_user_controlled(&mut self) {
        self.scope_selection_intent = ScopeSelectionIntent::UserControlled;
    }

    fn mark_query_user_controlled(&mut self) {
        if self.scope_selection_intent == ScopeSelectionIntent::PendingCurrentApp {
            self.scope_selection_intent = ScopeSelectionIntent::UserControlled;
        }
    }

    fn sync_launcher_editor(&mut self, value: String, cx: &mut Context<Self>) {
        let window_handle = self.window_handle;
        if self.input_mode == InputMode::Content {
            if self.content_input.read(cx).value() == value {
                return;
            }
            self._editor_sync_guard.set(true);
            let content_input = self.content_input.clone();
            let _ = cx.update_window(window_handle, |_, window, cx| {
                content_input.update(cx, |input, cx| input.set_value(value, window, cx));
            });
        } else {
            if self.input.read(cx).value() == value {
                return;
            }
            self._editor_sync_guard.set(true);
            let input = self.input.clone();
            let _ = cx.update_window(window_handle, |_, window, cx| {
                input.update(cx, |input, cx| input.set_value(value, window, cx));
            });
        }
        let editor_sync_guard = self._editor_sync_guard.clone();
        cx.defer(move |_| editor_sync_guard.set(false));
    }

    fn desired_search_placeholder(&self) -> String {
        if self.screen == Screen::FileSearch {
            return l10n::search_files_folders(self.locale);
        }
        if self.input_mode == InputMode::Content {
            return l10n::edit_captured_content(self.locale);
        }
        match self
            .model
            .selected_scope
            .as_deref()
            .and_then(|selected| self.model.scopes.iter().find(|scope| scope.id == selected))
        {
            Some(scope) => l10n::search_app_actions(self.locale, &scope.label),
            None => l10n::search_apps_actions(self.locale),
        }
    }

    fn sync_search_placeholder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = self.desired_search_placeholder();
        if placeholder == self.input_placeholder {
            return;
        }
        self.input_placeholder = placeholder.clone();
        self.input.update(cx, |input, cx| {
            input.set_placeholder(placeholder.clone(), window, cx);
        });
        self.content_input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx);
        });
    }

    fn launcher_input_height(&self) -> f32 {
        metrics::PAGE_HEADER_HEIGHT
            + if self.input_mode == InputMode::Content {
                24.0
            } else {
                0.0
            }
    }

    fn content_source_icon(&self) -> IconName {
        if self
            .model
            .context_summary()
            .iter()
            .any(|context| context.source == ContextSource::SelectedText)
        {
            IconName::Quote
        } else {
            IconName::Clipboard
        }
    }

    fn file_range_label(&self) -> String {
        let roots = self
            .model
            .file_search_progress()
            .map(|progress| progress.roots.as_slice())
            .unwrap_or_default();
        if roots.is_empty() {
            return l10n::file_search(self.locale);
        }
        if roots.len() > 1 {
            return l10n::file_locations(self.locale, roots.len() as i64);
        }
        let root = roots.first().map(String::as_str).unwrap_or_default();
        let name = root
            .split(['/', '\\'])
            .rfind(|part| !part.is_empty())
            .unwrap_or(root);
        l10n::file_range(self.locale, name)
    }

    fn header_mode_label(&self) -> Option<String> {
        match self.screen {
            Screen::FileSearch => Some(self.file_range_label()),
            Screen::Launcher if self.input_mode == InputMode::Content => {
                let source = self
                    .model
                    .context_summary()
                    .iter()
                    .find_map(|context| match context.source {
                        ContextSource::SelectedText => Some(l10n::selected_text(self.locale)),
                        ContextSource::Clipboard => Some(l10n::clipboard_text(self.locale)),
                        _ => None,
                    })
                    .unwrap_or_else(|| l10n::clipboard_text(self.locale));
                Some(l10n::content_escape_hint(self.locale, &source))
            }
            Screen::Launcher
                if self.input_mode == InputMode::Environment && !self.has_scope_bar() =>
            {
                Some(
                    self.model
                        .scopes
                        .iter()
                        .find(|scope| scope.is_current_app)
                        .map(|scope| scope.label.clone())
                        .unwrap_or_else(|| l10n::current_app(self.locale)),
                )
            }
            _ => None,
        }
    }

    fn result_heading(&self) -> Option<(String, Option<String>)> {
        if self.screen != Screen::Launcher {
            return None;
        }
        if self.model.object_actions_active {
            return Some((l10n::actions(self.locale), None));
        }
        if let Some(path) = self.model.menu_path() {
            let path = if path.is_empty() {
                l10n::menus(self.locale)
            } else {
                format!("{} › {}", l10n::menus(self.locale), path.join(" › "))
            };
            return Some((path, None));
        }
        if self.input_mode == InputMode::Content {
            return Some((
                l10n::content_actions(self.locale),
                Some(l10n::global_search(self.locale)),
            ));
        }
        None
    }

    fn launcher_empty_message(&self) -> String {
        if self.input_mode == InputMode::Content {
            return l10n::no_content_actions(self.locale);
        }
        if self.model.selected_scope.is_some() {
            return if self.model.query.trim().is_empty() {
                l10n::no_actions_available(self.locale)
            } else {
                l10n::no_matching_actions(self.locale)
            };
        }
        if !self.model.query.trim().is_empty() {
            l10n::no_matching_actions(self.locale)
        } else {
            l10n::no_results(self.locale)
        }
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        let query = query.into();
        self.model.set_query(query);
    }

    pub fn locale(&self) -> Locale {
        self.locale
    }

    pub fn live_launcher(&self) -> Option<Arc<LiveLauncher>> {
        self.live.clone()
    }

    fn close_web_surface(&mut self, cx: &mut Context<Self>) {
        tracing::debug!(
            screen = ?self.screen,
            has_view = self.web_surface.view.is_some(),
            document = self.web_surface.document.as_ref().map(|document| document.handle.as_str()),
            "web_surface_closing"
        );
        if let Some(view) = self.web_surface.view.as_ref() {
            let _ = view
                .read(cx)
                .raw()
                .evaluate_script(&visibility_script(false));
        }
        if let (Some(live), Some(document)) =
            (self.live.clone(), self.web_surface.document.as_ref())
        {
            let _ = live.close_web_document(document.handle.clone());
        }
        self.web_surface.document = None;
        self.web_surface.view = None;
        self.web_surface.ready = false;
        self.web_surface.error = None;
        self.web_surface.invocation_id = None;
        self.web_surface.actions_open = false;
        self.web_surface.action_selected = 0;
        self.web_surface.state_handle = None;
        self.web_surface.state_version = 0;
        self.web_surface.binding = None;
        self.web_surface.resume_generation = self.web_surface.resume_generation.wrapping_add(1);
        self.web_surface.resume_busy = false;
    }

    fn toggle_web_actions(&mut self, cx: &mut Context<Self>) {
        self.web_surface.actions_open = !self.web_surface.actions_open;
        self.web_surface.action_selected = 0;
        cx.notify();
    }

    fn reload_web_tool(&mut self, cx: &mut Context<Self>) {
        let Some(invocation_id) = self.web_surface.invocation_id.clone() else {
            return;
        };
        self.close_web_surface(cx);
        self.web_surface.invocation_id = Some(invocation_id.clone());
        if let Some(live) = self.live.clone() {
            let receiver = live.open_web_document(invocation_id, None);
            self.poll(receiver, self.live_state.generation, cx);
        }
        cx.notify();
    }

    fn execute_web_action(&mut self, action: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.web_surface.actions_open = false;
        self.web_surface.action_selected = 0;
        match action {
            0 => self.reload_web_tool(cx),
            1 => self.go_back(window, cx),
            _ => {}
        }
    }

    fn resume_retained_web(&mut self, cx: &mut Context<Self>) {
        let (Some(live), Some(document)) = (self.live.clone(), self.web_surface.document.clone())
        else {
            return;
        };
        self.web_surface.resume_generation = self.web_surface.resume_generation.wrapping_add(1);
        let generation = self.web_surface.resume_generation;
        self.web_surface.resume_busy = true;
        let previous_handle = document.handle.clone();
        let mut receiver = live.resume_web_document(document.handle);
        cx.spawn(async move |this, cx| {
            let Some(event) = receiver.recv().await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                if generation != this.web_surface.resume_generation
                    || this.screen != Screen::WebTool
                    || this
                        .web_surface
                        .document
                        .as_ref()
                        .map(|document| document.handle.as_str())
                        != Some(previous_handle.as_str())
                {
                    return;
                }
                this.web_surface.resume_busy = false;
                match event {
                    LiveEvent::WebDocument(document) => {
                        if let Some(binding) = &this.web_surface.binding {
                            if let Ok(mut binding) = binding.lock() {
                                binding.handle = document.handle.clone();
                                binding.origin = document.origin.clone();
                            }
                        }
                        this.web_surface.document = Some(document);
                        this.web_surface.ready = false;
                        this.web_surface.error = None;
                        this.web_surface.state_handle = None;
                        this.web_surface.state_version = 0;
                        let _ = this.evaluate_web_script(&visibility_script(true), cx);
                    }
                    LiveEvent::Error(error) => {
                        this.web_surface.error = Some(error.clone());
                        this.set_notice(error, components::NoticeKind::Error);
                    }
                    event => this.apply_auxiliary_event(event, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let leaving_settings = self.is_settings_surface();
        self.screen = if self.screen == Screen::Launcher {
            Screen::Settings
        } else {
            Screen::Launcher
        };
        self.notice = None;
        self.resize_window_for_screen(window);
        if self.screen == Screen::Settings {
            crate::platform::configure_settings_window_after_update(PRODUCT_NAME);
        } else {
            if leaving_settings {
                self.recover_launcher_session_after_settings(window, cx);
            }
            self.focus_launcher_input(window, cx);
            let (configured, wait_for_configuration) = oneshot::channel();
            crate::platform::configure_main_window_after_update_then(PRODUCT_NAME, move || {
                let _ = configured.send(());
            });
            let window_handle = self.window_handle;
            cx.spawn(async move |this, cx| {
                if wait_for_configuration.await.is_err() {
                    return;
                }
                let _ = window_handle.update(cx, |_, window, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if this.screen == Screen::Launcher {
                            this.focus_launcher_input(window, cx);
                            cx.notify();
                        }
                    });
                });
            })
            .detach();
        }
        cx.notify();
    }

    fn desired_window_size(&self) -> gpui_kit::Size<gpui_kit::Pixels> {
        match self.screen {
            Screen::Settings => {
                return gpui_kit::size(px(LAUNCHER_WIDTH), px(SETTINGS_HEIGHT));
            }
            Screen::Clipboard => return gpui_kit::size(px(LAUNCHER_WIDTH), px(600.)),
            Screen::ApplicationHistory => return gpui_kit::size(px(760.), px(520.)),
            Screen::WebTool => return gpui_kit::size(px(920.), px(600.)),
            Screen::WebPackages | Screen::PluginDetail | Screen::UnsupportedTool => {
                return gpui_kit::size(px(LAUNCHER_WIDTH), px(480.));
            }
            Screen::ClipboardShelf => {
                return gpui_kit::size(px(480.), px(520.));
            }
            Screen::Tasks => {
                return gpui_kit::size(px(920.), px(600.));
            }
            Screen::FileSearch => {
                return gpui_kit::size(px(LAUNCHER_WIDTH), px(600.));
            }
            Screen::Launcher => {}
        }
        if self.root_navigation_open {
            return gpui_kit::size(
                px(LAUNCHER_WIDTH),
                px(self.launcher_input_height() + metrics::DIVIDER_WIDTH + 108.),
            );
        }
        let has_controls = self.has_scope_bar();
        let has_panel = self.launcher_has_panel();
        let rows_height = if !self.model.rows.is_empty() {
            self.model.rows.len() as f32 * LAUNCHER_ROW_HEIGHT + 8.0
        } else {
            64.0
        };
        let scope_height = if has_controls {
            metrics::PAGE_TOOLBAR_HEIGHT
        } else {
            0.0
        };
        let notice_height = self.launcher_notice_height();
        let heading_height = if self.result_heading().is_some() {
            24.0
        } else {
            0.0
        };
        let panel_divider = if has_panel || has_controls {
            metrics::DIVIDER_WIDTH
        } else {
            0.0
        };
        let height = (self.launcher_input_height()
            + scope_height
            + panel_divider
            + heading_height
            + notice_height)
            + if has_panel { rows_height } else { 0.0 };
        let height = height.clamp(self.launcher_input_height(), 520.0);
        gpui_kit::size(px(LAUNCHER_WIDTH), px(height))
    }

    fn launcher_notice_height(&self) -> f32 {
        let Some(notice) = self.notice.as_deref() else {
            return 0.0;
        };
        let characters_per_line = if self.locale == Locale::Zh {
            42.0
        } else {
            78.0
        };
        let line_count = (notice.chars().count() as f32 / characters_per_line)
            .ceil()
            .clamp(1.0, 6.0);
        let content_height =
            line_count * metrics::LABEL_SIZE * 1.2 + metrics::SPACE_MD + metrics::SPACE_SM;
        content_height.clamp(metrics::NOTICE_MIN_HEIGHT, metrics::NOTICE_MAX_HEIGHT)
    }

    fn set_notice(&mut self, message: impl Into<String>, kind: components::NoticeKind) {
        self.notice = Some(message.into());
        self.notice_kind = kind;
    }

    fn clear_notice(&mut self) {
        self.notice = None;
        self.notice_kind = components::NoticeKind::Warning;
    }

    fn sync_launcher_window_after_render(&self, window: &Window, cx: &mut Context<Self>) {
        if !matches!(self.screen, Screen::Launcher | Screen::FileSearch)
            || self.standalone_shelf
            || self.standalone_tasks
        {
            return;
        }
        let size = self.desired_window_size();
        if window_size_changed(window.bounds().size, size) {
            self.resize_current_window(cx);
        }
    }

    fn has_scope_bar(&self) -> bool {
        if self.screen != Screen::Launcher || self.model.scopes.is_empty() {
            return false;
        }
        if self.input_mode == InputMode::Search
            && self.model.selected_scope.is_none()
            && self.model.query.trim().is_empty()
        {
            return false;
        }
        self.model.scopes.len() > 1 || !self.model.scopes[0].is_current_app
    }

    fn launcher_has_panel(&self) -> bool {
        self.model.loading
            || !self.model.rows.is_empty()
            || !self.model.query.trim().is_empty()
            || self.model.selected_scope.is_some()
            || self.input_mode == InputMode::Content
            || self.model.object_actions_active
            || self.model.menu_path().is_some()
            || self.notice.is_some()
            || self.result_heading().is_some()
    }

    fn selected_scope_item_index(&self) -> usize {
        self.model
            .selected_scope
            .as_deref()
            .and_then(|selected| {
                self.model
                    .scopes
                    .iter()
                    .position(|scope| scope.id == selected)
            })
            .unwrap_or(0)
    }

    fn reveal_selected_scope(&self) {
        if self.has_scope_bar() {
            // The leading flexible gutter is the scroll container's first
            // child; scope buttons begin at index one.
            self.scope_scroll
                .scroll_to_item(self.selected_scope_item_index() + 1);
        }
    }

    fn selected_file_filter_index(&self) -> usize {
        match self.file_search.filter {
            FileSearchFilter::All => 0,
            FileSearchFilter::Files => 1,
            FileSearchFilter::Folders => 2,
            FileSearchFilter::Documents => 3,
            FileSearchFilter::Pdf => 4,
            FileSearchFilter::Images => 5,
            FileSearchFilter::Audio => 6,
            FileSearchFilter::Video => 7,
            FileSearchFilter::Archives => 8,
            FileSearchFilter::Code => 9,
        }
    }

    fn reveal_selected_file_filter(&self) {
        self.file_filter_scroll
            .scroll_to_item(self.selected_file_filter_index());
    }

    fn reveal_result_row(&self, index: usize) {
        let scroll = match self.screen {
            Screen::Launcher => &self.result_scroll,
            Screen::FileSearch => &self.file_search_scroll,
            Screen::Clipboard => &self.clipboard_scroll,
            Screen::ClipboardShelf => &self.clipboard_shelf_scroll,
            Screen::ApplicationHistory => &self.history_scroll,
            Screen::Tasks => &self.tasks_scroll,
            Screen::WebPackages => &self.web_packages_scroll,
            Screen::Settings | Screen::WebTool | Screen::PluginDetail | Screen::UnsupportedTool => {
                return;
            }
        };
        let index = if self.screen == Screen::Tasks {
            let Some(position) = self
                .tasks
                .visible_indices()
                .iter()
                .position(|item| *item == index)
            else {
                return;
            };
            position
        } else {
            index
        };
        scroll.scroll_to_item(index + SCROLLBAR_OVERLAY_CHILD_COUNT);
    }

    fn focus_launcher_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.input_mode == InputMode::Content {
            self.content_input
                .update(cx, |input, cx| input.focus(window, cx));
        } else {
            self.input.update(cx, |input, cx| input.focus(window, cx));
        }
    }

    pub fn refocus_active_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_active_editor(window, cx);
        tracing::debug!(
            focused = self.active_editor_focused(window, cx),
            "launcher_active_editor_refocused"
        );
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

    pub fn platform_appearance_changed(&self, cx: &mut Context<Self>) {
        if self.screen == Screen::WebTool && self.web_surface.view.is_some() {
            let _ = self
                .evaluate_web_script(&canvas_script(!crate::platform::uses_opaque_fallback()), cx);
        }
        cx.notify();
    }

    fn active_editor_focused(&self, window: &Window, cx: &Context<Self>) -> bool {
        match self.screen {
            Screen::Clipboard | Screen::ClipboardShelf => self
                .clipboard_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window),
            Screen::ApplicationHistory => self
                .history_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window),
            Screen::Launcher if self.input_mode == InputMode::Content => self
                .content_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window),
            Screen::Launcher | Screen::FileSearch => {
                self.input.read(cx).focus_handle(cx).is_focused(window)
            }
            _ => false,
        }
    }

    fn resize_current_window(&self, cx: &mut Context<Self>) {
        // Input/click listeners already hold the window. Resize after that borrow
        // ends, and recompute the size so a newer navigation wins over old work.
        let view = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = view.update(cx, |this, cx| {
                if !matches!(this.screen, Screen::Launcher | Screen::FileSearch) {
                    return;
                }
                let size = this.desired_window_size();
                let needs_resize = cx
                    .update_window(this.window_handle, |_, window, _| {
                        window_size_changed(window.bounds().size, size)
                    })
                    .unwrap_or(false);
                if !needs_resize {
                    return;
                }
                if crate::platform::resize_main_window_preserving_top(
                    size.width.as_f32(),
                    size.height.as_f32(),
                ) {
                    return;
                }
                match cx.update_window(this.window_handle, |_, window, _| {
                    window.resize(size);
                    true
                }) {
                    Ok(true) => this.position_launcher_after_resize(),
                    Ok(false) => {}
                    Err(error) => tracing::warn!(%error, "launcher_window_resize_failed"),
                }
            });
        });
    }

    fn resize_window_for_screen(&self, window: &mut Window) {
        let size = self.desired_window_size();
        if window_size_changed(window.bounds().size, size)
            && !crate::platform::resize_main_window_preserving_top(
                size.width.as_f32(),
                size.height.as_f32(),
            )
        {
            window.resize(size);
            self.position_launcher_after_resize();
        }
    }

    fn position_launcher_after_resize(&self) {
        if !self.standalone_shelf && !self.standalone_tasks && self.screen != Screen::Settings {
            crate::platform::position_main_window_after_update();
        }
    }

    fn refresh_current_window(&self, cx: &mut Context<Self>) {
        let entity_id = cx.entity_id();
        let _ = cx.update_window(self.window_handle, move |_, window, _| {
            window.on_next_frame(move |_, cx| cx.notify(entity_id));
            window.refresh();
        });
    }

    fn request_live_update(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            self.resize_current_window(cx);
            return;
        };
        if self.model.object_actions_active {
            return;
        }
        self.file_search.page_generation = self.file_search.page_generation.wrapping_add(1);
        self.file_search.page_in_flight = false;
        self.file_search.select_after_load = false;
        self.live_state.request_revision = self
            .live_state
            .request_revision
            .max(self.model.revision)
            .saturating_add(1);
        let revision = self.live_state.request_revision;
        if let Some(gate) = &mut self.live_state.snapshot_gate {
            gate.begin_request(revision);
        }
        let receiver = if self.screen == Screen::FileSearch {
            live.update_file_input(self.model.query.clone(), revision, self.file_search.filter)
        } else {
            live.update_input(
                self.model.query.clone(),
                revision,
                self.model.selected_scope.clone(),
                self.input_mode,
            )
        };
        self.begin_stream(receiver, cx);
    }

    fn request_more_files(&mut self, keyboard: bool, cx: &mut Context<Self>) {
        if self.screen != Screen::FileSearch || self.file_search.page_in_flight {
            return;
        }
        let Some(live) = self.live.clone() else {
            return;
        };
        let Some(cursor) = self
            .model
            .file_search_progress()
            .and_then(|progress| progress.next_cursor.clone())
        else {
            return;
        };
        self.file_search.page_generation = self.file_search.page_generation.wrapping_add(1);
        let generation = self.file_search.page_generation;
        self.file_search.page_in_flight = true;
        self.file_search.select_after_load = keyboard;
        let expected_query = self.model.query.clone();
        let expected_revision = self.model.revision;
        let stream_generation = self.live_state.generation;
        let session_id = self.live_state.session_id;
        let previous_selected = self.model.selected;
        let mut receiver = live.load_more_files(expected_revision);
        tracing::debug!(generation, cursor = %cursor, keyboard, "file_page_requested");
        cx.spawn(async move |this, cx| {
            let event = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.file_search.page_generation
                    || stream_generation != this.live_state.generation
                    || this.screen != Screen::FileSearch
                    || this.model.query != expected_query
                {
                    return;
                }
                this.file_search.page_in_flight = false;
                match event {
                    Some(LiveEvent::SharedState(state))
                        if this.live_state.session_id == session_id
                            && Some(state.session_id) == session_id
                            && state.input == expected_query =>
                    {
                        this.apply_shared_state(state, cx);
                        this.notice = None;
                        if this.file_search.select_after_load
                            && this.file_search.browsing
                            && !this.model.rows.is_empty()
                        {
                            this.model.selected = (previous_selected + 1)
                                .min(this.model.rows.len().saturating_sub(1));
                            this.reveal_result_row(this.model.selected);
                            if this.preview_open {
                                this.request_preview(cx);
                            }
                        } else if previous_selected < this.model.rows.len() {
                            this.model.selected = previous_selected;
                        }
                        this.file_search.select_after_load = false;
                        this.resize_current_window(cx);
                    }
                    Some(LiveEvent::Ignored) => {}
                    Some(LiveEvent::Error(error)) => {
                        this.set_notice(error, components::NoticeKind::Error);
                        this.file_search.select_after_load = false;
                    }
                    Some(event) => {
                        this.file_search.select_after_load = false;
                        this.apply_auxiliary_event(event, cx);
                    }
                    None => {
                        this.file_search.select_after_load = false;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn request_clipboard_search(&mut self, cx: &mut Context<Self>) {
        if let Some(live) = self.live.clone() {
            self.clipboard.query_generation = self.clipboard.query_generation.wrapping_add(1);
            let request_id = self.clipboard.query_generation;
            let query = self.clipboard.query.clone();
            self.clipboard.loading();
            let receiver = live.clipboard_snapshot(query);
            self.poll_clipboard_search(receiver, request_id, cx);
        }
    }

    fn request_clipboard_settings_status(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            return;
        };
        self.clipboard.query_generation = self.clipboard.query_generation.wrapping_add(1);
        let request_id = self.clipboard.query_generation;
        self.clipboard.loading = true;
        let receiver = live.clipboard_snapshot(String::new());
        self.poll_clipboard_search(receiver, request_id, cx);
        cx.notify();
    }

    fn poll_clipboard_search(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        request_id: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let Some(event) = receiver.recv().await else {
                let _ = this.update(cx, |this, cx| {
                    if request_id != this.clipboard.query_generation {
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
                if request_id != this.clipboard.query_generation {
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

    fn poll_clipboard_operation(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        operation_id: u64,
        success: Option<(i64, String)>,
        return_to_application: bool,
        failure: ClipboardOperationFailure,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let Some(event) = receiver.recv().await else {
                let _ = this.update(cx, |this, cx| {
                    if operation_id != this.clipboard.operation_generation {
                        return;
                    }
                    this.clipboard.loading = false;
                    let message = match failure {
                        ClipboardOperationFailure::Copy => l10n::copy_failed(this.locale),
                        ClipboardOperationFailure::Delete => {
                            l10n::history_operation_failed(this.locale)
                        }
                        ClipboardOperationFailure::Settings => {
                            l10n::clipboard_settings_failed(this.locale)
                        }
                    };
                    this.set_notice(message, components::NoticeKind::Error);
                    cx.notify();
                });
                return;
            };
            let refresh = this
                .update(cx, |this, cx| {
                    if operation_id != this.clipboard.operation_generation {
                        return false;
                    }
                    match event {
                        LiveEvent::Clipboard(_) => {
                            if success.as_ref().is_none_or(|(item_id, _)| {
                                this.clipboard.selected_item().map(|item| item.id) == Some(*item_id)
                            }) {
                                this.clipboard.copied_item =
                                    success.as_ref().map(|(item_id, _)| *item_id);
                                this.clear_notice();
                                if return_to_application
                                    && crate::platform::clipboard_shelf_is_key_window()
                                {
                                    match crate::platform::return_to_previous_application() {
                                        crate::platform::ApplicationReturnResult::Returned => {}
                                        crate::platform::ApplicationReturnResult::Unavailable => {
                                            this.set_notice(
                                                l10n::shelf_return_unavailable(this.locale),
                                                components::NoticeKind::Warning,
                                            );
                                        }
                                        crate::platform::ApplicationReturnResult::Failed => {
                                            this.set_notice(
                                                l10n::shelf_return_failed(this.locale),
                                                components::NoticeKind::Warning,
                                            );
                                        }
                                    }
                                }
                            }
                            true
                        }
                        LiveEvent::Error(error) => {
                            let message = match failure {
                                ClipboardOperationFailure::Copy => l10n::copy_failed(this.locale),
                                ClipboardOperationFailure::Delete => {
                                    l10n::history_operation_failed(this.locale)
                                }
                                ClipboardOperationFailure::Settings => {
                                    l10n::clipboard_settings_failed(this.locale)
                                }
                            };
                            this.set_notice(
                                format!("{message} {error}"),
                                components::NoticeKind::Error,
                            );
                            false
                        }
                        event => {
                            this.apply_auxiliary_event(event, cx);
                            false
                        }
                    }
                })
                .unwrap_or(false);
            if refresh {
                let _ = this.update(cx, |this, cx| {
                    this.request_clipboard_search(cx);
                    cx.notify();
                });
            } else {
                let _ = this.update(cx, |this, cx| {
                    this.clipboard.loading = false;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn poll_app_history_session(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        request: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let Some(event) = receiver.recv().await else {
                tracing::debug!(request, "application_history_session_channel_closed");
                return;
            };
            let _ = this.update(cx, |this, cx| {
                tracing::debug!(
                    request,
                    current_request = this.app_history.session_generation,
                    screen = ?this.screen,
                    "application_history_session_event"
                );
                if request != this.app_history.session_generation
                    || this.screen != Screen::ApplicationHistory
                {
                    return;
                }
                match event {
                    LiveEvent::AppHistorySources {
                        request: event_request,
                        session_id,
                        sources,
                    } if event_request == request => {
                        tracing::debug!(
                            request,
                            session_id = ?session_id,
                            source_count = sources.len(),
                            selected_source = ?this.app_history.selected_source,
                            "application_history_sources_loaded"
                        );
                        this.app_history.apply_sources(session_id, sources);
                        this.request_app_history_search(cx);
                    }
                    LiveEvent::Error(error) => {
                        tracing::warn!(%error, "app_history_session_failed");
                        this.app_history.fail();
                        this.set_notice(
                            l10n::app_history_operation_failed(this.locale),
                            components::NoticeKind::Error,
                        );
                    }
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn request_app_history_search(&mut self, cx: &mut Context<Self>) {
        self.app_history.query_generation = self.app_history.query_generation.wrapping_add(1);
        let request_id = self.app_history.query_generation;
        let (Some(live), Some(session_id), Some(source_id)) = (
            self.live.clone(),
            self.app_history.session_id,
            self.app_history.selected_source.clone(),
        ) else {
            tracing::debug!(
                has_live = self.live.is_some(),
                session_id = ?self.app_history.session_id,
                selected_source = ?self.app_history.selected_source,
                "application_history_search_skipped"
            );
            return;
        };
        self.app_history.close_object_actions();
        self.app_history.empty_actions_open = false;
        self.app_history.fallback_actions_open = false;
        self.app_history.failed_sources = false;
        self.app_history.notice = None;
        self.app_history.loading = true;
        let query = self.app_history.query.clone();
        tracing::debug!(
            request_id,
            session_id,
            source_id = %source_id,
            query = %query,
            "application_history_search_started"
        );
        let receiver = live.search_app_history(session_id, query.clone(), source_id.clone());
        self.poll_app_history_search(receiver, request_id, source_id, query, cx);
    }

    fn schedule_app_history_search(&mut self, cx: &mut Context<Self>) {
        self.app_history.query_generation = self.app_history.query_generation.wrapping_add(1);
        let generation = self.app_history.query_generation;
        self.app_history.close_object_actions();
        self.app_history.loading = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.app_history.query_generation
                    || this.screen != Screen::ApplicationHistory
                {
                    return;
                }
                this.request_app_history_search(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn poll_app_history_search(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        request_id: u64,
        source_id: String,
        query: String,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let Some(event) = receiver.recv().await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                tracing::debug!(
                    request_id,
                    current_request = this.app_history.query_generation,
                    screen = ?this.screen,
                    selected_source = ?this.app_history.selected_source,
                    current_query = %this.app_history.query,
                    "application_history_search_event"
                );
                if request_id != this.app_history.query_generation
                    || this.screen != Screen::ApplicationHistory
                    || this.app_history.selected_source.as_deref() != Some(source_id.as_str())
                    || this.app_history.query != query
                {
                    return;
                }
                match event {
                    LiveEvent::AppHistory(snapshot) if snapshot.source_id == source_id => {
                        this.app_history.apply(snapshot);
                        this.reveal_result_row(this.app_history.selected);
                        if let Some(message) = this.app_history.notice_text(this.locale) {
                            this.set_notice(message, components::NoticeKind::Warning);
                        } else {
                            this.clear_notice();
                        }
                    }
                    LiveEvent::Error(error) => {
                        tracing::warn!(%error, "app_history_search_failed");
                        this.app_history.fail();
                        this.set_notice(
                            l10n::app_history_operation_failed(this.locale),
                            components::NoticeKind::Error,
                        );
                    }
                    LiveEvent::Ignored => {
                        this.app_history.loading = false;
                    }
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_screen(&mut self, screen: Screen, window: &mut Window, cx: &mut Context<Self>) {
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
        if self.screen == Screen::ApplicationHistory {
            self.app_history.close_object_actions();
            self.app_history.empty_actions_open = false;
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
        let receiver = match (screen, self.live.clone()) {
            (Screen::Clipboard | Screen::ClipboardShelf, Some(live)) => {
                Some(live.clipboard_snapshot(self.clipboard.query.clone()))
            }
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
        }
        self.resize_window_for_screen(window);
        cx.notify();
    }

    fn release_app_history_session(&mut self) {
        self.app_history.session_generation = self.app_history.session_generation.wrapping_add(1);
        self.app_history.query_generation = self.app_history.query_generation.wrapping_add(1);
        self.app_history.close_object_actions();
        self.app_history.loading = false;
        if let (Some(live), Some(session_id)) =
            (self.live.clone(), self.app_history.session_id.take())
        {
            live.cancel_app_history_session(session_id);
        }
    }

    fn start_app_history_session(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            tracing::warn!("application_history_session_missing_live");
            return;
        };
        self.app_history.session_generation = self.app_history.session_generation.wrapping_add(1);
        let request = self.app_history.session_generation;
        tracing::debug!(request, "application_history_session_started");
        let receiver = live.open_app_history_session(request);
        self.poll_app_history_session(receiver, request, cx);
    }

    fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_tasks_refresh();
        self.tasks.refresh_generation = self.tasks.refresh_generation.wrapping_add(1);
        if self.screen == Screen::WebPackages {
            self.screen = Screen::Settings;
            self.notice = None;
            self.resize_window_for_screen(window);
            crate::platform::configure_settings_window_after_update(PRODUCT_NAME);
            self.settings_focus_handle.focus(window, cx);
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
            self.app_history.empty_actions_open = false;
            self.app_history.fallback_actions_open = false;
            self.release_app_history_session();
        }
        if self.screen == Screen::Launcher {
            if self.model.object_actions_active {
                self.model.hide_object_actions();
                cx.notify();
            } else if self.model.menu_path().is_some() {
                self.model.back_menu();
                cx.notify();
            } else if !self.model.query.is_empty() {
                if self.input_mode == InputMode::Content {
                    self.content_input
                        .update(cx, |input, cx| input.set_value("", window, cx));
                } else {
                    self.input
                        .update(cx, |input, cx| input.set_value("", window, cx));
                }
                self.model.set_query(String::new());
                self.request_live_update(cx);
                cx.notify();
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
        self.web_surface.plugin_detail = None;
        self.web_surface.unsupported_tool = None;
        self.screen = Screen::Launcher;
        self.restore_parent_workspace(window, cx);
        self.notice = None;
        self.focus_launcher_input(window, cx);
        self.resize_window_for_screen(window);
        cx.notify();
    }

    fn capture_parent_workspace(&mut self, cx: &Context<Self>) {
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

    fn restore_parent_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    fn begin_stream(&mut self, receiver: Receiver<LiveEvent>, cx: &mut Context<Self>) {
        let (generation, cancelled) = self.live_state.begin_stream();
        self.poll_with_cancel(receiver, generation, Some(cancelled), cx);
    }

    fn cancel_stream(&mut self) {
        self.live_state.cancel_stream();
    }

    fn poll(&mut self, receiver: Receiver<LiveEvent>, generation: u64, cx: &mut Context<Self>) {
        self.poll_with_cancel(receiver, generation, None, cx);
    }

    fn poll_with_cancel(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        generation: u64,
        cancelled: Option<oneshot::Receiver<()>>,
        cx: &mut Context<Self>,
    ) {
        let window_handle = self.window_handle;
        cx.spawn(async move |this, cx| {
            tracing::debug!(generation, "launcher_poll_started");
            let event = receiver.recv().await;
            tracing::debug!(
                generation,
                received = event.is_some(),
                "launcher_poll_received"
            );
            match event {
                Some(LiveEvent::SharedState(state)) => {
                    let accepted = this
                        .update(cx, move |this, cx| {
                            if generation != this.live_state.generation {
                                return None;
                            }
                            let accepted = this
                                .live_state
                                .snapshot_gate
                                .as_mut()
                                .is_some_and(|gate| gate.accept_request(&state));
                            if accepted {
                                let applied = this.apply_shared_state(state, cx);
                                this.request_application_icons(cx);
                                if applied.rows_changed || applied.scopes_changed {
                                    this.resize_current_window(cx);
                                }
                                cx.notify();
                                this.live.as_ref().map(|live| {
                                    live.follow_snapshots(
                                        this.live_state
                                            .snapshot_gate
                                            .as_ref()
                                            .map_or(0, SnapshotGate::cursor),
                                    )
                                })
                            } else {
                                None
                            }
                        })
                        .ok()
                        .flatten();
                    if let Some(follow) = accepted {
                        this.update(cx, move |this, cx| {
                            if generation == this.live_state.generation {
                                this.poll_follow(follow, generation, cancelled, cx);
                            }
                        })
                        .ok();
                    }
                }
                Some(LiveEvent::Ignored) => {}
                Some(LiveEvent::Settings(snapshot)) => {
                    this.update(cx, |this, cx| {
                        if generation == this.live_state.generation {
                            this.settings.apply_snapshot(snapshot);
                            this.notice = None;
                            cx.notify();
                        }
                    })
                    .ok();
                }
                Some(LiveEvent::SettingsError { message, snapshot }) => {
                    this.update(cx, |this, cx| {
                        if generation == this.live_state.generation {
                            if let Some(snapshot) = snapshot {
                                this.settings.apply_snapshot(snapshot);
                            }
                            this.settings.saving = false;
                            this.settings.error = Some(message.clone());
                            this.set_notice(message, components::NoticeKind::Error);
                            cx.notify();
                        }
                    })
                    .ok();
                }
                Some(LiveEvent::DirectoriesAuthorized { roots }) => {
                    let _ = cx.update_window(window_handle, |_, window, cx| {
                        let _ = this.update(cx, |this, cx| {
                            if generation == this.live_state.generation {
                                this.apply_directories_authorized(roots, window, cx);
                            }
                        });
                    });
                }
                Some(LiveEvent::ActionResult(result)) => {
                    let result_session_id = result.session_id;
                    let destination = result.view.clone();
                    let dismiss = this
                        .update(cx, |this, cx| {
                            if generation != this.live_state.generation
                                || this.live_state.session_id != Some(result.session_id)
                            {
                                return None;
                            }
                            Some(this.finish_action_result(&result, destination.is_some(), cx))
                        })
                        .unwrap_or(None);
                    let Some(dismiss) = dismiss else {
                        return;
                    };
                    if dismiss {
                        crate::platform::hide_main_window();
                    }
                    if let Some(destination) = destination {
                        let _ = cx.update_window(window_handle, |_, window, cx| {
                            let _ = this.update(cx, |this, cx| {
                                if generation == this.live_state.generation
                                    && this.live_state.session_id == Some(result_session_id)
                                {
                                    this.open_action_destination(destination, window, cx);
                                }
                            });
                        });
                    }
                }
                Some(LiveEvent::Error(error)) => {
                    this.update(cx, |this, cx| {
                        if generation != this.live_state.generation {
                            return;
                        }
                        this.model.loading = false;
                        this.settings.saving = false;
                        this.shortcut_catalog_loading = false;
                        this.settings.error = Some(error.clone());
                        this.set_notice(error, components::NoticeKind::Error);
                        cx.notify();
                    })
                    .ok();
                }
                Some(event) => {
                    if let Err(error) = this.update(cx, |this, cx| {
                        if generation != this.live_state.generation {
                            return;
                        }
                        this.apply_auxiliary_event(event, cx);
                        cx.notify();
                    }) {
                        tracing::debug!(%error, generation, "launcher_poll_update_failed");
                    } else {
                        let launcher = this.clone();
                        let _ = cx.update_window(window_handle, move |_, window, _| {
                            window.on_next_frame(move |_, cx| {
                                let _ = launcher.update(cx, |_, cx| cx.notify());
                            });
                            window.refresh();
                        });
                    }
                }
                None => {
                    tracing::debug!(generation, "launcher_poll_disconnected");
                }
            }
        })
        .detach();
    }

    fn poll_follow(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        generation: u64,
        mut cancelled: Option<oneshot::Receiver<()>>,
        cx: &mut Context<Self>,
    ) {
        let window_handle = self.window_handle;
        cx.spawn(async move |this, cx| loop {
            let event = if let Some(cancel) = cancelled.as_mut() {
                tokio::select! {
                    _ = cancel => break,
                    event = receiver.recv() => event,
                }
            } else {
                receiver.recv().await
            };
            match event {
                Some(LiveEvent::SharedState(state)) => {
                    let should_continue = this
                        .update(cx, move |this, cx| {
                            if generation != this.live_state.generation
                                || this.live_state.session_id != Some(state.session_id)
                            {
                                return false;
                            }
                            let accepted = this
                                .live_state
                                .snapshot_gate
                                .as_mut()
                                .is_some_and(|gate| gate.accept_update(&state));
                            if accepted {
                                let applied = this.apply_shared_state(state, cx);
                                this.request_application_icons(cx);
                                if applied.rows_changed || applied.scopes_changed {
                                    this.resize_current_window(cx);
                                }
                                cx.notify();
                            }
                            true
                        })
                        .unwrap_or(false);
                    if !should_continue {
                        break;
                    }
                }
                Some(LiveEvent::Ignored) | None => break,
                Some(LiveEvent::Settings(snapshot)) => {
                    this.update(cx, |this, cx| {
                        if generation == this.live_state.generation {
                            this.settings.apply_snapshot(snapshot);
                            this.notice = None;
                            cx.notify();
                        }
                    })
                    .ok();
                    break;
                }
                Some(LiveEvent::SettingsError { message, snapshot }) => {
                    this.update(cx, |this, cx| {
                        if generation == this.live_state.generation {
                            if let Some(snapshot) = snapshot {
                                this.settings.apply_snapshot(snapshot);
                            }
                            this.settings.saving = false;
                            this.settings.error = Some(message.clone());
                            this.set_notice(message, components::NoticeKind::Error);
                            cx.notify();
                        }
                    })
                    .ok();
                    break;
                }
                Some(LiveEvent::DirectoriesAuthorized { roots }) => {
                    let _ = cx.update_window(window_handle, |_, window, cx| {
                        let _ = this.update(cx, |this, cx| {
                            if generation == this.live_state.generation {
                                this.apply_directories_authorized(roots, window, cx);
                            }
                        });
                    });
                    break;
                }
                Some(LiveEvent::ActionResult(result)) => {
                    let result_session_id = result.session_id;
                    let destination = result.view.clone();
                    let dismiss = this
                        .update(cx, |this, cx| {
                            if generation != this.live_state.generation
                                || this.live_state.session_id != Some(result_session_id)
                            {
                                return None;
                            }
                            Some(this.finish_action_result(&result, destination.is_some(), cx))
                        })
                        .unwrap_or(None);
                    let Some(dismiss) = dismiss else {
                        break;
                    };
                    if dismiss {
                        crate::platform::hide_main_window();
                    }
                    if let Some(destination) = destination {
                        let _ = cx.update_window(window_handle, |_, window, cx| {
                            let _ = this.update(cx, |this, cx| {
                                if generation == this.live_state.generation
                                    && this.live_state.session_id == Some(result_session_id)
                                {
                                    this.open_action_destination(destination, window, cx);
                                }
                            });
                        });
                    }
                    break;
                }
                Some(LiveEvent::Error(error)) => {
                    this.update(cx, |this, cx| {
                        if generation == this.live_state.generation {
                            this.model.loading = false;
                            this.settings.saving = false;
                            this.shortcut_catalog_loading = false;
                            this.settings.error = Some(error.clone());
                            this.set_notice(error, components::NoticeKind::Error);
                            cx.notify();
                        }
                    })
                    .ok();
                    break;
                }
                Some(event) => {
                    tracing::debug!("launcher_live_event_received");
                    this.update(cx, |this, cx| {
                        if generation == this.live_state.generation {
                            this.apply_auxiliary_event(event, cx);
                        }
                    })
                    .ok();
                    break;
                }
            }
        })
        .detach();
    }

    fn apply_auxiliary_event(&mut self, event: LiveEvent, cx: &mut Context<Self>) {
        match event {
            LiveEvent::Preferences {
                preferences,
                host_error,
            } => self.apply_preferences_result(preferences, host_error, cx),
            LiveEvent::Clipboard(snapshot) => {
                self.clipboard.apply(snapshot);
                self.notice = None;
            }
            LiveEvent::AppHistorySources {
                session_id,
                sources,
                ..
            } => {
                self.app_history.apply_sources(session_id, sources);
                if self.screen == Screen::ApplicationHistory {
                    self.request_app_history_search(cx);
                }
            }
            LiveEvent::AppHistory(snapshot) => {
                self.app_history.apply(snapshot);
                if let Some(message) = self.app_history.notice_text(self.locale) {
                    self.set_notice(message, components::NoticeKind::Warning);
                } else {
                    self.clear_notice();
                }
            }
            LiveEvent::AppHistoryAction { item_id, copy } => {
                if copy
                    && self
                        .app_history
                        .selected_item()
                        .is_some_and(|item| item.id == item_id)
                {
                    self.set_notice(l10n::copied(self.locale), components::NoticeKind::Success);
                }
            }
            LiveEvent::AppHistoryObjectActions {
                request,
                item_id,
                revision,
                actions,
            } => {
                self.app_history
                    .apply_object_actions(request, &item_id, revision, actions);
            }
            LiveEvent::AppHistoryObjectActionsFailed { request } => {
                if self.app_history.fail_object_actions(request) {
                    self.set_notice(
                        l10n::app_history_operation_failed(self.locale),
                        components::NoticeKind::Error,
                    );
                }
            }
            LiveEvent::AppHistoryAdaptersOpened { directory } => {
                self.set_notice(
                    l10n::opened_folder(self.locale, &directory),
                    components::NoticeKind::Success,
                );
            }
            LiveEvent::ShortcutCatalog(actions) => {
                self.shortcut_catalog = actions;
                self.shortcut_catalog_loading = false;
            }
            LiveEvent::ActionShortcutReady {
                action_id,
                revision,
                candidates,
            } => {
                if self.action_shortcut_resolving.as_deref() != Some(action_id.as_str()) {
                    return;
                }
                self.action_shortcut_resolving = None;
                match candidates.as_slice() {
                    [candidate] => {
                        self.set_notice(
                            l10n::running_action(self.locale, &candidate.title),
                            components::NoticeKind::Info,
                        );
                        if let Some(live) = self.live.clone() {
                            let receiver = live.execute(revision, candidate.candidate_id.clone());
                            self.poll(receiver, self.live_state.generation, cx);
                        }
                    }
                    [] => {
                        self.set_notice(
                            l10n::no_matching_actions(self.locale),
                            components::NoticeKind::Warning,
                        );
                    }
                    _ => {
                        self.set_notice(
                            l10n::choose_another_action(self.locale),
                            components::NoticeKind::Info,
                        );
                    }
                }
            }
            LiveEvent::ActionShortcutFailed { action_id, message } => {
                if self.action_shortcut_resolving.as_deref() == Some(action_id.as_str()) {
                    self.action_shortcut_resolving = None;
                    self.set_notice(
                        l10n::action_failed_with(self.locale, &message),
                        components::NoticeKind::Error,
                    );
                }
            }
            LiveEvent::Executions(rows) => {
                self.tasks.apply_rows(rows);
                self.clear_notice();
            }
            LiveEvent::ExecutionDetail(detail) => {
                let request_id = self.tasks.detail_request;
                self.tasks.apply_detail(request_id, detail);
                if self.tasks.follow_output {
                    self.tasks.request_follow_output();
                }
                self.notice = None;
            }
            LiveEvent::WebPackages(packages) => {
                self.web.apply(packages);
                self.web_surface.review = None;
                self.notice = None;
            }
            LiveEvent::WebReview(review) => {
                self.web_surface.review = Some(review);
                self.notice = None;
            }
            LiveEvent::WebDocument(document) => {
                tracing::debug!(
                    screen = ?self.screen,
                    had_view = self.web_surface.view.is_some(),
                    previous_document = self
                        .web_surface
                        .document
                        .as_ref()
                        .map(|previous| previous.handle.as_str()),
                    document = %document.handle,
                    "web_document_event_applied"
                );
                if let (Some(previous), Some(live)) =
                    (self.web_surface.document.as_ref(), self.live.clone())
                {
                    if previous.handle != document.handle {
                        let receiver = live.close_web_document(previous.handle.clone());
                        self.poll(receiver, self.live_state.generation, cx);
                    }
                }
                if self.web_surface.view.is_some() {
                    let _ = self.web_surface.view.as_ref().map(|view| {
                        view.read(cx)
                            .raw()
                            .evaluate_script(&visibility_script(false))
                    });
                }
                self.web_surface.view = None;
                self.web_surface.ready = false;
                self.web_surface.error = None;
                self.web_surface.state_handle = None;
                self.web_surface.state_version = 0;
                self.web_surface.document = Some(document);
                self.notice = None;
            }
            LiveEvent::WebFrameReply { handle, frame } => {
                if self
                    .web_surface
                    .document
                    .as_ref()
                    .is_some_and(|document| document.handle == handle)
                {
                    let _ = self.evaluate_web_script(&reply_script(&frame), cx);
                }
            }
            LiveEvent::WebState { handle, frame } => {
                if self
                    .web_surface
                    .document
                    .as_ref()
                    .is_some_and(|document| document.handle == handle)
                {
                    let _ = self.evaluate_web_script(&state_script(&frame), cx);
                }
            }
            LiveEvent::DirectoriesAuthorized { roots } => {
                self.settings_draft.file_search_roots = roots;
                self.set_notice(
                    l10n::search_directories_authorized(self.locale),
                    components::NoticeKind::Success,
                );
            }
            LiveEvent::ApplicationIcon {
                candidate_id,
                bytes,
            } => {
                if let Some(bytes) = bytes {
                    let image = Arc::new(Image::from_bytes(ImageFormat::Png, bytes));
                    if let Some(cache_key) = self
                        .model
                        .rows
                        .iter()
                        .find(|row| row.id == candidate_id)
                        .and_then(|row| row.icon_identity.clone())
                    {
                        self.application_icons.insert(cache_key, image);
                    }
                }
                self.application_icons_requested.remove(&candidate_id);
            }
            LiveEvent::Preview(preview) => {
                self.show_preview(preview, cx);
            }
            LiveEvent::SharedState(_)
            | LiveEvent::ActionResult(_)
            | LiveEvent::Settings(_)
            | LiveEvent::SettingsError { .. }
            | LiveEvent::Ignored
            | LiveEvent::Error(_) => {}
        }
        cx.notify();
    }

    fn activate_selection(&mut self, cx: &mut Context<Self>) {
        if self.screen == Screen::FileSearch && !self.file_search.browsing {
            return;
        }
        if self.model.open_selected_menu() {
            self.notice = None;
            self.resize_current_window(cx);
            cx.notify();
            return;
        }
        if let (Some(live), Some(row)) = (self.live.clone(), self.model.selected_row().cloned()) {
            if row.enabled {
                self.set_notice(
                    l10n::running_action(self.locale, &row.title),
                    components::NoticeKind::Info,
                );
                let receiver = live.execute(self.model.revision, row.id);
                self.poll(receiver, self.live_state.generation, cx);
                self.resize_current_window(cx);
                cx.notify();
                return;
            }
        }
        let message = match self.model.selected_row() {
            Some(row) if row.enabled => l10n::execution_bridge_pending(self.locale, &row.title),
            Some(row) => l10n::action_unavailable_title(self.locale, &row.title),
            None => l10n::no_action_selected(self.locale),
        };
        self.set_notice(message, components::NoticeKind::Warning);
        self.resize_current_window(cx);
        cx.notify();
    }

    fn handle_action_result(&mut self, result: &anywhere_protocol::ActionResult) {
        if self.model.object_actions_active {
            self.model.hide_object_actions();
        }
        self.set_notice(
            action_result_message(result, self.locale),
            action_result_notice_kind(result),
        );
    }

    fn finish_action_result(
        &mut self,
        result: &anywhere_protocol::ActionResult,
        destination_requested: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        self.handle_action_result(result);
        let task_result = result.succeeded() && result.execution_session_id.is_some();
        let open_tasks = task_result && result.warning.is_none() && !destination_requested;
        if open_tasks {
            if let Some(task_id) = result.execution_session_id.clone() {
                cx.emit(LauncherEvent::OpenTasks(task_id));
            }
        }
        let dismiss = if open_tasks {
            true
        } else if task_result && result.warning.is_some() {
            false
        } else {
            result.close_window
        };
        if dismiss {
            self.prepare_hide(cx);
        } else {
            self.resize_current_window(cx);
        }
        cx.notify();
        dismiss
    }

    fn open_action_destination(
        &mut self,
        destination: anywhere_protocol::HostView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match destination {
            anywhere_protocol::HostView::FileSearch => {
                self.capture_parent_workspace(cx);
                self.model.set_query(String::new());
                self.input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.open_screen(Screen::FileSearch, window, cx);
            }
            anywhere_protocol::HostView::WebPlugin { invocation_id } => {
                self.web_surface.invocation_id = Some(invocation_id.clone());
                self.web_surface.document = None;
                self.web_surface.view = None;
                self.web_surface.ready = false;
                self.web_surface.error = None;
                self.web_surface.actions_open = false;
                self.web_surface.action_selected = 0;
                self.open_screen(Screen::WebTool, window, cx);
                if let Some(live) = self.live.clone() {
                    let receiver = live.open_web_document(invocation_id, None);
                    self.poll(receiver, self.live_state.generation, cx);
                }
            }
            anywhere_protocol::HostView::NativeTool { id, input } => match id.as_str() {
                "clipboard.history" => self.open_screen(Screen::Clipboard, window, cx),
                "clipboard.shelf" => cx.emit(LauncherEvent::OpenClipboardShelf),
                "application.history" => {
                    if let Ok(route) = serde_json::from_str::<
                        anywhere_core::features::application_history::model::PageInput,
                    >(&input)
                    {
                        tracing::debug!(
                            source_id = %route.source_id,
                            "application_history_destination_opened"
                        );
                        self.app_history.selected_source = Some(route.source_id);
                        self.app_history.reset_selection();
                    } else {
                        tracing::warn!("application_history_destination_invalid");
                    }
                    self.open_screen(Screen::ApplicationHistory, window, cx);
                }
                _ => {
                    self.web_surface.unsupported_tool = Some((id, input));
                    self.open_screen(Screen::UnsupportedTool, window, cx);
                    cx.notify();
                }
            },
            anywhere_protocol::HostView::PluginDetail { detail } => {
                self.web_surface.plugin_detail = Some(detail);
                self.open_screen(Screen::PluginDetail, window, cx);
                cx.notify();
            }
        }
    }

    fn handle_settings_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_action_picker_open {
            self.close_action_shortcut_picker(cx);
            self.settings_focus_handle.focus(window, cx);
        } else if self.settings_close_pending {
            self.settings_close_pending = false;
            self.settings_exit_intent = None;
            self.notice = None;
            cx.notify();
        } else {
            self.request_close_settings(window, cx);
        }
    }

    fn copy_selected_clipboard(&mut self, cx: &mut Context<Self>) {
        self.copy_selected_clipboard_with_destination(false, cx);
    }

    fn copy_selected_clipboard_and_return(&mut self, cx: &mut Context<Self>) {
        self.copy_selected_clipboard_with_destination(true, cx);
    }

    fn copy_selected_clipboard_with_destination(
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
            Some((item_id, l10n::copied(self.locale))),
            return_to_application,
            ClipboardOperationFailure::Copy,
            cx,
        );
        self.clipboard.loading();
        cx.notify();
    }

    fn preview_selected_clipboard(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(item)) = (self.live.clone(), self.clipboard.selected_item()) {
            let receiver = live.preview_clipboard(item.id);
            self.poll(receiver, self.live_state.generation, cx);
        }
    }

    fn can_toggle_preview_shortcut(&self) -> bool {
        let file_preview = matches!(self.screen, Screen::Launcher | Screen::FileSearch)
            && (self.preview_open || self.can_preview_selected_file());
        let clipboard_preview = matches!(self.screen, Screen::Clipboard | Screen::ClipboardShelf)
            && (self.preview_open || self.clipboard_selection_active());
        file_preview || clipboard_preview
    }

    fn toggle_preview_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
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

    fn confirm_clipboard_delete(&mut self, cx: &mut Context<Self>) {
        // Selection may change when a new snapshot arrives. Authorization is
        // tied to the item displayed by the confirmation, never the new row.
        if let Some(item_id) = self.clipboard.delete_pending {
            if self.select_clipboard_item(item_id) {
                self.delete_selected_clipboard(cx);
            }
        }
    }

    fn delete_selected_clipboard(&mut self, cx: &mut Context<Self>) {
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

    fn select_clipboard_item(&mut self, item_id: i64) -> bool {
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

    fn open_clipboard_actions(&mut self, item_id: i64, cx: &mut Context<Self>) {
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

    fn open_empty_clipboard_actions(&mut self, cx: &mut Context<Self>) {
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

    fn clipboard_refresh_action(screen: Screen) -> usize {
        if screen == Screen::ClipboardShelf {
            4
        } else {
            3
        }
    }

    fn close_clipboard_actions(&mut self, cx: &mut Context<Self>) {
        self.clipboard.actions_open = false;
        self.clipboard.action_target = None;
        self.clipboard.action_selected = 0;
        cx.notify();
    }

    fn move_clipboard_action(&mut self, direction: i8, actions: usize, cx: &mut Context<Self>) {
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

    fn request_delete_clipboard_item(&mut self, item_id: i64, cx: &mut Context<Self>) {
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

    fn execute_clipboard_action(&mut self, action: usize, cx: &mut Context<Self>) {
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

    fn run_clipboard_command(&mut self, receiver: Receiver<LiveEvent>, cx: &mut Context<Self>) {
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

    fn open_selected_history_item(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(item)) = (self.live.clone(), self.app_history.selected_item()) {
            let receiver = live.use_app_history_item(item.id.clone(), false);
            self.poll(receiver, self.live_state.generation, cx);
            self.set_notice(l10n::open(self.locale), components::NoticeKind::Info);
            cx.notify();
        }
    }

    fn copy_selected_history_item(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(item)) = (self.live.clone(), self.app_history.selected_item()) {
            let receiver = live.use_app_history_item(item.id.clone(), true);
            self.poll(receiver, self.live_state.generation, cx);
            self.notice = None;
            cx.notify();
        }
    }

    fn request_app_history_object_actions(&mut self, cx: &mut Context<Self>) {
        let (Some(live), Some(session_id), Some(item)) = (
            self.live.clone(),
            self.live_state.session_id,
            self.app_history.selected_item().cloned(),
        ) else {
            return;
        };
        if !item.can_open {
            return;
        }
        self.notice = None;
        let revision = self.model.revision;
        let request = self
            .app_history
            .begin_object_actions_request(item.id.clone(), revision);
        let receiver =
            live.app_history_object_actions(request, session_id, revision, item.id.clone());
        self.poll(receiver, self.live_state.generation, cx);
        cx.notify();
    }

    fn toggle_empty_app_history_actions(&mut self, cx: &mut Context<Self>) {
        self.app_history.close_object_actions();
        self.app_history.empty_actions_open = !self.app_history.empty_actions_open;
        self.notice = None;
        cx.notify();
    }

    fn toggle_fallback_app_history_actions(&mut self, cx: &mut Context<Self>) {
        self.app_history.close_object_actions();
        self.app_history.empty_actions_open = false;
        self.app_history.fallback_actions_open = !self.app_history.fallback_actions_open;
        self.app_history.fallback_action_selected = 0;
        self.notice = None;
        cx.notify();
    }

    fn execute_fallback_app_history_action(&mut self, action: usize, cx: &mut Context<Self>) {
        self.app_history.fallback_actions_open = false;
        self.app_history.fallback_action_selected = 0;
        match action {
            0 => self.copy_selected_history_item(cx),
            1 => self.request_app_history_search(cx),
            _ => {}
        }
    }

    fn refresh_empty_app_history(&mut self, cx: &mut Context<Self>) {
        self.app_history.empty_actions_open = false;
        self.request_app_history_search(cx);
        cx.notify();
    }

    fn execute_selected_app_history_action(&mut self, cx: &mut Context<Self>) {
        if self.app_history.object_action_selected >= self.app_history.object_actions.len() {
            if self.app_history.object_actions_fallback_copy()
                && self.app_history.object_action_selected == self.app_history.object_actions.len()
            {
                self.copy_selected_history_item(cx);
                return;
            }
            self.request_app_history_search(cx);
            return;
        }
        let Some(action) = self.app_history.selected_object_action().cloned() else {
            return;
        };
        if !action.enabled {
            return;
        }
        let Some(live) = self.live.clone() else {
            return;
        };
        let revision = self.model.revision;
        self.app_history.close_object_actions();
        let receiver = live.execute(revision, action.id);
        self.poll(receiver, self.live_state.generation, cx);
        cx.notify();
    }

    fn open_selected_execution(&mut self, cx: &mut Context<Self>) {
        if let Some(row_id) = self.tasks.selected_row().map(|row| row.id.clone()) {
            self.request_task_detail(row_id, cx);
        }
    }

    fn request_task_detail(&mut self, task_id: String, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            return;
        };
        let request_id = self.tasks.begin_detail_request();
        let receiver = live.execution_detail(task_id.clone());
        self.poll_task_detail(receiver, request_id, task_id, cx);
    }

    fn poll_task_detail(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        request_id: u64,
        task_id: String,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let Some(event) = receiver.recv().await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                if request_id != this.tasks.detail_request
                    || !this
                        .tasks
                        .selected_row()
                        .is_some_and(|row| row.id == task_id)
                {
                    return;
                }
                match event {
                    LiveEvent::ExecutionDetail(detail) => {
                        this.tasks.apply_detail(request_id, detail);
                        if this.tasks.follow_output {
                            this.tasks.request_follow_output();
                        }
                    }
                    LiveEvent::Error(error) => {
                        this.tasks.detail = None;
                        this.set_notice(error, components::NoticeKind::Error);
                    }
                    event => this.apply_auxiliary_event(event, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn stop_selected_execution(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(row)) = (self.live.clone(), self.tasks.selected_row()) {
            let receiver = live.stop_execution(row.id.clone());
            self.poll(receiver, self.live_state.generation, cx);
        }
    }

    fn remove_selected_execution(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(row)) = (self.live.clone(), self.tasks.selected_row()) {
            if row.status.is_active() {
                return;
            }
            let receiver = live.remove_execution(row.id.clone());
            self.poll(receiver, self.live_state.generation, cx);
            cx.notify();
        }
    }

    fn open_tool_options(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tasks.tool_options_open = true;
        self.tasks.tool_option_selected = 0;
        self.tasks.task_actions_open = false;
        self.tasks.task_action_selected = 0;
        self.notice = None;
        window.blur(cx);
        cx.notify();
    }

    fn close_tool_options(&mut self, cx: &mut Context<Self>) {
        self.tasks.tool_options_open = false;
        self.tasks.tool_option_selected = 0;
        cx.notify();
    }

    fn move_tool_option(&mut self, cx: &mut Context<Self>) {
        self.tasks.tool_option_selected = 1 - self.tasks.tool_option_selected;
        cx.notify();
    }

    fn execute_tool_option(&mut self, option: usize, cx: &mut Context<Self>) {
        self.tasks.tool_options_open = false;
        self.tasks.tool_option_selected = 0;
        match option {
            0 => self.start_tasks_refresh(cx),
            1 => cx.emit(LauncherEvent::RestoreLauncher),
            _ => {}
        }
        cx.notify();
    }

    fn open_task_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        window.blur(cx);
        cx.notify();
    }

    fn close_task_actions(&mut self, cx: &mut Context<Self>) {
        self.tasks.task_actions_open = false;
        self.tasks.task_action_selected = 0;
        cx.notify();
    }

    fn task_action_count(status: anywhere_protocol::ExecutionSessionStatus) -> usize {
        if status.is_active() {
            1
        } else {
            2
        }
    }

    fn move_task_action(&mut self, direction: i8, cx: &mut Context<Self>) {
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

    fn execute_task_action(&mut self, action: usize, cx: &mut Context<Self>) {
        let Some(detail) = self.tasks.detail.clone() else {
            self.close_task_actions(cx);
            return;
        };
        self.tasks.task_actions_open = false;
        self.tasks.task_action_selected = 0;
        match action {
            0 => {
                cx.write_to_clipboard(ClipboardItem::new_string(detail.row.command.clone()));
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

    fn start_tasks_refresh(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            return;
        };
        self.cancel_tasks_refresh();
        self.tasks.refresh_generation = self.tasks.refresh_generation.wrapping_add(1);
        let generation = self.tasks.refresh_generation;
        let (cancel, mut cancelled) = oneshot::channel();
        self.tasks_refresh_cancel = Some(cancel);
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
                        || !this.presented
                    {
                        return false;
                    }
                    match event {
                        LiveEvent::Executions(rows) => {
                            this.apply_tasks_rows(rows, cx);
                            this.notice = None;
                            cx.notify();
                        }
                        LiveEvent::Error(error) => {
                            this.set_notice(error, components::NoticeKind::Error)
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

    fn apply_tasks_rows(&mut self, rows: Vec<crate::host::ExecutionRow>, cx: &mut Context<Self>) {
        let had_tasks = !self.tasks.rows.is_empty();
        self.tasks.apply_rows(rows);
        if had_tasks && self.tasks.rows.is_empty() {
            let window_handle = self.window_handle;
            cx.defer(move |cx| {
                let _ = window_handle.update(cx, |_, window, cx| window.blur(cx));
            });
        }
        if let Some(task_id) = self.pending_task_selection.take() {
            if let Some(index) = self.tasks.rows.iter().position(|row| row.id == task_id) {
                if !self.tasks.visible_indices().contains(&index) {
                    self.tasks.set_filter(String::new());
                    let filter_input = self.tasks_filter_input.clone();
                    let window_handle = self.window_handle;
                    cx.defer(move |cx| {
                        let _ = window_handle.update(cx, |_, window, cx| {
                            filter_input.update(cx, |input, cx| input.set_value("", window, cx));
                        });
                    });
                }
                self.tasks.selected = index;
                self.tasks.detail = None;
                self.tasks.detail_request = self.tasks.detail_request.wrapping_add(1);
                self.reveal_result_row(index);
            }
        }
        if !self.tasks.detail_is_current() {
            if let Some(task_id) = self.tasks.selected_row().map(|row| row.id.clone()) {
                self.request_task_detail(task_id, cx);
            }
        }
        self.reveal_result_row(self.tasks.selected);
    }

    fn cancel_tasks_refresh(&mut self) {
        if let Some(cancel) = self.tasks_refresh_cancel.take() {
            let _ = cancel.send(());
        }
    }

    fn toggle_selected_web_package(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(package)) =
            (self.live.clone(), self.web.packages.get(self.web.selected))
        {
            let id = package.id.clone();
            let operation = if package.enabled { "disable" } else { "enable" };
            let receiver = live.change_web_package(id, operation.to_owned());
            self.poll(receiver, self.live_state.generation, cx);
        }
    }

    fn request_preview(&mut self, cx: &mut Context<Self>) {
        if self.screen == Screen::Launcher || self.screen == Screen::FileSearch {
            if let (Some(live), Some(row)) = (self.live.clone(), self.selected_row_for_action()) {
                if row.kind == anywhere_protocol::CandidateKind::File {
                    let receiver = live.preview_file(self.model.revision, row.id.clone());
                    self.poll(receiver, self.live_state.generation, cx);
                }
            }
        }
    }

    fn request_application_icons(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            return;
        };
        let visible_ids = self
            .model
            .rows
            .iter()
            .filter(|row| row.kind == CandidateKind::Application)
            .map(|row| {
                (
                    row.id.clone(),
                    row.icon_identity.clone().unwrap_or_else(|| row.id.clone()),
                )
            })
            .collect::<Vec<_>>();
        for (candidate_id, cache_key) in visible_ids {
            if self.application_icons.contains(&cache_key)
                || self.application_icons_requested.contains_key(&candidate_id)
                || self
                    .application_icons_requested
                    .values()
                    .any(|request| request.cache_key == cache_key)
                || self.application_icons_requested.len() >= MAX_APPLICATION_ICON_REQUESTS
            {
                continue;
            }
            self.application_icon_generation = self.application_icon_generation.wrapping_add(1);
            let ticket = self.application_icon_generation;
            self.application_icons_requested.insert(
                candidate_id.clone(),
                ApplicationIconRequest {
                    ticket,
                    cache_key: cache_key.clone(),
                },
            );
            let receiver = live.application_icon(candidate_id.clone());
            self.poll_application_icon(receiver, candidate_id, cache_key, ticket, cx);
        }
    }

    fn poll_application_icon(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        candidate_id: String,
        cache_key: String,
        ticket: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let event = receiver.recv().await;
            this.update(cx, |this, cx| {
                if this
                    .application_icons_requested
                    .get(&candidate_id)
                    .map(|request| request.ticket)
                    != Some(ticket)
                {
                    return;
                }
                match event {
                    Some(LiveEvent::ApplicationIcon {
                        candidate_id: response_id,
                        bytes,
                    }) if response_id == candidate_id => {
                        this.application_icons_requested.remove(&candidate_id);
                        if let Some(bytes) = bytes {
                            this.application_icons.insert(
                                cache_key,
                                Arc::new(Image::from_bytes(ImageFormat::Png, bytes)),
                            );
                        }
                    }
                    Some(event) => {
                        this.application_icons_requested.remove(&candidate_id);
                        this.apply_auxiliary_event(event, cx);
                    }
                    None => {
                        this.application_icons_requested.remove(&candidate_id);
                    }
                }
                cx.notify();
                this.request_application_icons(cx);
            })
            .ok();
        })
        .detach();
    }

    fn show_preview(&mut self, preview: crate::host::PreviewFile, cx: &mut Context<Self>) {
        self.preview_generation = self.preview_generation.wrapping_add(1);
        let generation = self.preview_generation;
        self.preview_open = false;
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
        cx.notify();
        cx.spawn(async move |this, cx| {
            // AppKit's shared Quick Look panel pumps its event loop while showing.
            // Keep that call outside the mutable GPUI app borrow held by `update`.
            cx.background_executor()
                .timer(std::time::Duration::from_millis(0))
                .await;
            let mut host = loop {
                let reservation = this
                    .update(cx, |this, _cx| {
                        if generation != this.preview_generation {
                            return Ok(PreviewReservation::Stale);
                        }
                        if this.preview_busy {
                            return Ok(PreviewReservation::Busy);
                        }
                        this.preview_busy = true;
                        if this.preview.is_none() {
                            this.preview = Some(PreviewHost::new()?);
                        }
                        let host = this.preview.take().ok_or_else(|| {
                            anyhow::anyhow!("Quick Look host was not initialized")
                        })?;
                        Ok(PreviewReservation::Ready(host))
                    })
                    .and_then(|reservation| reservation);
                match reservation {
                    Ok(PreviewReservation::Ready(host)) => break host,
                    Ok(PreviewReservation::Stale) => return,
                    Ok(PreviewReservation::Busy) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(1))
                            .await;
                    }
                    Err(error) => {
                        let _ = this.update(cx, |this, cx| {
                            this.preview_busy = false;
                            this.set_notice(
                                preview_error_message(this.locale, &error),
                                components::NoticeKind::Error,
                            );
                            cx.notify();
                        });
                        return;
                    }
                }
            };
            let result = host.show(preview.id.clone(), &preview.paths, owner_title);
            tracing::debug!(
                preview_id = %preview.id,
                success = result.is_ok(),
                "quick_look_show_finished"
            );
            let mut reserved_host = Some(host);
            let applied = this
                .update(cx, |this, cx| {
                    if generation != this.preview_generation {
                        return false;
                    }
                    this.preview = reserved_host.take();
                    this.preview_busy = false;
                    match &result {
                        Ok(previous) => {
                            this.preview_open = true;
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
                            this.preview_open = false;
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
                        preview_open = this.preview_open,
                        "quick_look_state_applied"
                    );
                    true
                })
                .unwrap_or(false);
            if !applied {
                let resource_id = reserved_host.as_mut().and_then(|host| host.close());
                let previous = result.ok().flatten();
                let _ = this.update(cx, |this, cx| {
                    this.preview = reserved_host.take();
                    this.preview_busy = false;
                    if let Some(live) = this.live.clone() {
                        if let Some(previous) = previous {
                            let _ = live.release_preview(previous);
                        }
                        if let Some(resource_id) = resource_id {
                            let _ = live.release_preview(resource_id);
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn apply_directories_authorized(
        &mut self,
        roots: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_draft.file_search_roots = roots.clone();
        set_input_value(&self.file_roots_input, roots.join("\n"), window, cx);
        self.settings.error = None;
        self.set_notice(
            l10n::search_directories_authorized(self.locale),
            components::NoticeKind::Success,
        );
        cx.notify();
    }

    fn start_preview_monitor(&mut self, generation: u64, cx: &mut Context<Self>) {
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
                        generation == this.preview_generation
                            && (this.preview_busy || this.preview.is_some())
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

    fn reserve_preview_for_poll(&mut self, generation: u64) -> Option<PreviewHost> {
        if generation != self.preview_generation || self.preview_busy {
            return None;
        }
        let host = self.preview.take()?;
        self.preview_busy = true;
        Some(host)
    }

    fn apply_preview_poll(
        &mut self,
        generation: u64,
        poll: &PreviewPoll,
        host: Option<PreviewHost>,
        resource_id: Option<String>,
        cx: &mut Context<Self>,
    ) -> Option<Option<Entity<InputState>>> {
        if generation != self.preview_generation {
            self.finish_preview_close(host, resource_id, cx);
            return None;
        }
        self.preview = host;
        self.preview_busy = false;
        match poll {
            PreviewPoll::Open => Some(None),
            PreviewPoll::Closed {
                resource_id: active_resource_id,
                application_active,
            } => {
                debug_assert_eq!(resource_id.as_ref(), Some(active_resource_id));
                if let (Some(live), Some(resource_id)) = (self.live.clone(), resource_id.as_ref()) {
                    let _ = live.release_preview(resource_id.clone());
                }
                self.preview_open = false;
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

    fn active_editor_input(&self) -> Option<&Entity<InputState>> {
        match self.screen {
            Screen::FileSearch | Screen::Launcher => &self.input,
            Screen::Clipboard | Screen::ClipboardShelf => &self.clipboard_input,
            Screen::ApplicationHistory => &self.history_input,
            _ => return None,
        }
        .into()
    }

    fn focus_active_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screen == Screen::Launcher {
            self.focus_launcher_input(window, cx);
            return;
        }
        if let Some(input) = self.active_editor_input().cloned() {
            input.update(cx, |input, cx| input.focus(window, cx));
        }
    }

    fn close_preview(&mut self, cx: &mut Context<Self>) -> bool {
        let was_open = self.preview_open;
        self.preview_generation = self.preview_generation.wrapping_add(1);
        let generation = self.preview_generation;
        self.preview_open = false;
        if was_open {
            self.notice = None;
            cx.notify();
            self.refresh_current_window(cx);
        }
        cx.spawn(async move |this, cx| {
            // `hide` and `makeKeyAndOrderFront` may synchronously restore AppKit
            // focus, so perform them after the current GPUI update has ended.
            cx.background_executor()
                .timer(std::time::Duration::from_millis(0))
                .await;
            let host = this
                .update(cx, |this, _cx| this.reserve_preview_for_close(generation))
                .unwrap_or(None);
            if let Some(mut host) = host {
                let resource_id = host.close();
                let _ = this.update(cx, |this, cx| {
                    this.finish_preview_close(Some(host), resource_id, cx);
                });
            }
        })
        .detach();
        was_open
    }

    fn reserve_preview_for_close(&mut self, generation: u64) -> Option<PreviewHost> {
        if generation != self.preview_generation || self.preview_busy {
            return None;
        }
        let host = self.preview.take()?;
        self.preview_busy = true;
        Some(host)
    }

    fn finish_preview_close(
        &mut self,
        host: Option<PreviewHost>,
        resource_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.preview = host;
        self.preview_busy = false;
        if let (Some(live), Some(resource_id)) = (self.live.clone(), resource_id) {
            let _ = live.release_preview(resource_id);
        }
        cx.notify();
        self.refresh_current_window(cx);
    }

    fn reset_file_search_browsing(&mut self, cx: &mut Context<Self>) {
        if self.screen != Screen::FileSearch {
            return;
        }
        self.file_search.browsing = false;
        self.model.selected = 0;
        self.close_preview(cx);
    }

    fn reset_clipboard_browsing(&mut self, cx: &mut Context<Self>) {
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

    fn clipboard_selection_active(&self) -> bool {
        matches!(self.screen, Screen::Clipboard | Screen::ClipboardShelf)
            && self.clipboard.selected_item().is_some()
    }

    fn select_clipboard(&mut self, direction: i8, cx: &mut Context<Self>) {
        if direction < 0 {
            self.clipboard.select_previous();
        } else {
            self.clipboard.select_next();
        }
        self.reveal_result_row(self.clipboard.selected);
        self.notice = None;
        if self.preview_open {
            self.preview_selected_clipboard(cx);
        }
        cx.notify();
        self.refresh_current_window(cx);
    }

    fn select_clipboard_row(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.clipboard.browsing = true;
        self.clipboard.selected = index;
        self.clipboard.copied_item = None;
        self.notice = None;
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

    fn file_search_selection_active(&self) -> bool {
        self.screen == Screen::FileSearch
            && self.file_search.browsing
            && !self.model.object_actions_active
            && self.model.selected_row().is_some()
    }

    fn selected_row_for_action(&self) -> Option<&LauncherRow> {
        if self.screen == Screen::FileSearch
            && !self.file_search.browsing
            && !self.model.object_actions_active
        {
            return None;
        }
        self.model.selected_row()
    }

    fn select_file_search(&mut self, direction: i8, cx: &mut Context<Self>) {
        if self.model.rows.is_empty() {
            self.file_search.browsing = false;
            return;
        }
        if self.file_search.browsing {
            if direction < 0 {
                self.model.select_previous();
            } else {
                if self.model.selected + 1 >= self.model.rows.len() {
                    self.request_more_files(true, cx);
                    return;
                }
                self.model.select_next();
            }
        } else {
            self.file_search.browsing = true;
            self.model.select_first();
        }
        self.reveal_result_row(self.model.selected);
        self.notice = None;
        if self.preview_open {
            self.request_preview(cx);
        }
        cx.notify();
    }

    fn handle_preview_key(
        &mut self,
        event: &gpui_kit::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.handle_preview_keystroke(
            event.keystroke.key.as_str(),
            event.keystroke.modifiers.modified(),
            event.prefer_character_input,
            event.is_held,
            window,
            cx,
        )
    }

    fn handle_preview_keystroke(
        &mut self,
        key: &str,
        modified: bool,
        prefer_character_input: bool,
        repeated: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.preview_open && key == "escape" {
            self.close_preview(cx);
            self.focus_active_editor(window, cx);
            return true;
        }
        if key != "space"
            || prefer_character_input
            || modified
            || !self.active_editor_focused(window, cx)
            || (!self.preview_open && !self.file_search_selection_active())
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

    fn can_preview_selected_file(&self) -> bool {
        matches!(self.screen, Screen::Launcher | Screen::FileSearch)
            && self
                .selected_row_for_action()
                .is_some_and(|row| row.kind == anywhere_protocol::CandidateKind::File)
    }

    fn show_object_actions(&mut self, cx: &mut Context<Self>) {
        if self.screen == Screen::FileSearch && !self.file_search.browsing {
            return;
        }
        let Some(row) = self.model.selected_row().cloned() else {
            return;
        };
        if !matches!(self.screen, Screen::Launcher | Screen::FileSearch) {
            return;
        }
        if !self
            .model
            .can_show_object_actions(&row, self.screen == Screen::FileSearch)
        {
            return;
        }
        self.model.show_object_actions();
        self.notice = None;
        if let Some(live) = self.live.clone() {
            let receiver = live.object_actions(self.model.revision, row.id);
            self.poll_auxiliary_state(receiver, self.live_state.generation, cx);
        }
        cx.notify();
    }

    fn browse_selected_menu(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.model.open_selected_menu() {
            return false;
        }
        self.notice = None;
        self.resize_current_window(cx);
        cx.notify();
        true
    }

    fn poll_auxiliary_state(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let Some(event) = receiver.recv().await else {
                return;
            };
            this.update(cx, |this, cx| {
                if generation != this.live_state.generation {
                    return;
                }
                match event {
                    LiveEvent::SharedState(state) => {
                        let applied = this.apply_shared_state(state, cx);
                        this.request_application_icons(cx);
                        if applied.rows_changed || applied.scopes_changed {
                            this.resize_current_window(cx);
                        }
                    }
                    LiveEvent::Error(error) => {
                        this.model.hide_object_actions();
                        this.set_notice(error, components::NoticeKind::Error);
                    }
                    event => this.apply_auxiliary_event(event, cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn toggle_root_navigation(&mut self, cx: &mut Context<Self>) {
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

    fn escape_launcher_root(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            return;
        }
        self.prepare_hide(cx);
        crate::platform::hide_main_window();
    }

    fn return_to_current_app(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    fn clear_launcher_query(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self._editor_sync_guard.set(true);
        set_input_value(&self.input, "", window, cx);
        let editor_sync_guard = self._editor_sync_guard.clone();
        cx.defer(move |_| editor_sync_guard.set(false));
        self.model.set_query(String::new());
        self.notice = None;
        self.focus_launcher_input(window, cx);
        self.request_live_update(cx);
        self.resize_current_window(cx);
        cx.notify();
    }

    fn move_root_navigation(&mut self, direction: i8, cx: &mut Context<Self>) {
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

    fn choose_root_navigation(&mut self, cx: &mut Context<Self>) {
        if !self.root_navigation_open {
            return;
        }
        self.select_root_mode(self.root_navigation_mode, cx);
    }

    fn select_root_mode(&mut self, input_mode: InputMode, cx: &mut Context<Self>) {
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

    fn render_root_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("launcher-mode-chooser")
            .test_support()
            .h(px(108.))
            .flex_shrink_0()
            .px(px(10.))
            .py(px(8.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(self.root_navigation_row(InputMode::Search, cx))
            .child(self.root_navigation_row(InputMode::Environment, cx))
    }

    fn root_navigation_row(&self, mode: InputMode, cx: &mut Context<Self>) -> impl IntoElement {
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
        div()
            .id(id)
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(label.clone())
            .aria_selected(selected)
            .h(px(44.))
            .flex_shrink_0()
            .px(px(12.))
            .flex()
            .items_center()
            .gap(px(14.))
            .rounded(px(metrics::RESULT_RADIUS))
            .cursor_pointer()
            .when(selected, |element| element.selected_surface())
            .when(!selected, |element| {
                element.hover(|style| style.bg(theme::hover()))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_root_mode(mode, cx);
            }))
            .child(Icon::new(icon).size(px(20.)).text_color(theme::secondary()))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(
                        div()
                            .id(SharedString::from(format!("{id}-title")))
                            .test_support()
                            .truncate()
                            .body_text()
                            .font_weight(FontWeight::MEDIUM)
                            .child(label),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("{id}-detail")))
                            .test_support()
                            .truncate()
                            .caption_text()
                            .text_color(theme::secondary())
                            .child(detail),
                    ),
            )
            .when(active, |element| {
                element.child(
                    Icon::new(IconName::Check)
                        .size(px(15.))
                        .text_color(theme::secondary()),
                )
            })
    }

    fn render_launcher(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let selected_scope = self.model.selected_scope.as_deref();
        let has_scope_bar = self.has_scope_bar();
        let has_panel = self.launcher_has_panel();
        let scope_controls = if !has_scope_bar {
            None
        } else {
            Some(
                div()
                    .id("launcher-scope-scroll")
                    .h(px(metrics::PAGE_TOOLBAR_HEIGHT))
                    .flex_shrink_0()
                    .px(px(metrics::SPACE_LG))
                    .flex()
                    .items_center()
                    .overflow_x_scroll()
                    .track_scroll(&self.scope_scroll)
                    // Empty flexible gutters center a short group but collapse
                    // to zero when the scope buttons overflow. Keeping the
                    // buttons as direct scroll children also makes
                    // `scroll_to_item` able to reveal a late selection.
                    .child(div().min_w(px(0.)).flex_1())
                    .children(self.model.scopes.iter().map(|scope| {
                        self.scope_button(
                            Some(scope.id.clone()),
                            selected_scope == Some(scope.id.as_str()),
                            cx,
                        )
                    }))
                    .child(div().min_w(px(0.)).flex_1())
                    .into_any_element(),
            )
        };
        let mode_label = self.header_mode_label();
        let result_heading = self.result_heading();

        let rows = if self.model.loading {
            div()
                .h(px(64.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(metrics::BODY_SIZE))
                .text_color(theme::secondary())
                .child(l10n::searching(self.locale))
                .into_any_element()
        } else if self.model.rows.is_empty() {
            div()
                .h(px(64.))
                .flex()
                .items_center()
                .justify_center()
                .px(px(metrics::SPACE_XL))
                .child(
                    div()
                        .body_text()
                        .text_color(theme::secondary())
                        .truncate()
                        .child(self.launcher_empty_message()),
                )
                .into_any_element()
        } else {
            let mut list = div()
                .id("launcher-results")
                .flex_1()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .px(px(10.))
                .pb(px(8.))
                .track_scroll(&self.result_scroll)
                .overflow_y_scroll()
                .vertical_scrollbar(&self.result_scroll);
            for (index, row) in self.model.rows.clone().into_iter().enumerate() {
                list = list.child(self.render_row(index, row, self.model.selected == index, cx));
            }
            list.into_any_element()
        };

        let browsing = self.model.object_actions_active || self.model.menu_path().is_some();
        let leading_control = if browsing {
            div()
                .id("launcher-back")
                .test_support()
                .role(gpui_kit::Role::Button)
                .aria_label(l10n::back_to_results(self.locale))
                .cursor_pointer()
                .size(px(36.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(metrics::CONTROL_RADIUS))
                .hover(|style| style.bg(theme::hover()))
                .tooltip(components::text_tooltip(l10n::back_to_results(self.locale)))
                .on_click(cx.listener(|this, _, window, cx| this.go_back(window, cx)))
                .child(
                    Icon::new(IconName::ArrowLeft)
                        .size(px(20.))
                        .text_color(theme::primary()),
                )
                .into_any_element()
        } else if self.input_mode == InputMode::Content {
            div()
                .id("launcher-content-source-icon")
                .test_support()
                .size(px(36.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(self.content_source_icon())
                        .size(px(21.))
                        .text_color(theme::secondary()),
                )
                .into_any_element()
        } else {
            div()
                .id("launcher-mode-toggle")
                .test_support()
                .role(gpui_kit::Role::Button)
                .aria_label(l10n::navigation_modes(self.locale))
                .aria_expanded(self.root_navigation_open)
                .size(px(36.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(3.))
                .rounded(px(metrics::CONTROL_RADIUS))
                .tooltip(components::text_tooltip(l10n::navigation_modes(
                    self.locale,
                )))
                .when(!self.model.loading, |element| {
                    element
                        .cursor_pointer()
                        .hover(|style| style.bg(theme::hover()))
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_root_navigation(cx)))
                })
                .when(self.model.loading, |element| element.opacity(0.45))
                .child(
                    Icon::new(IconName::Search)
                        .size(px(24.))
                        .text_color(theme::primary()),
                )
                .child(
                    Icon::new(if self.root_navigation_open {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    })
                    .size(px(9.))
                    .text_color(theme::secondary()),
                )
                .into_any_element()
        };
        let launcher_editor = if self.input_mode == InputMode::Content {
            Textarea::new(&self.content_input)
                .accessibility_id("launcher-search-input")
                .aria_label(self.desired_search_placeholder())
                .appearance(false)
                .bordered(false)
                .text_size(px(metrics::CONTENT_INPUT_SIZE))
                .p(px(0.))
                .w_full()
                .max_h(px(48.))
                .into_any_element()
        } else {
            Input::new(&self.input)
                .id("launcher-search-input")
                .aria_label(self.desired_search_placeholder())
                .appearance(false)
                .bordered(false)
                .focus_bordered(false)
                .cleanable(false)
                .text_size(px(metrics::PAGE_HEADING_SIZE))
                .line_height(px(metrics::PAGE_HEADING_SIZE * metrics::BODY_LINE_HEIGHT))
                .p(px(0.))
                .w_full()
                .into_any_element()
        };

        let mut root = div()
            .id("launcher-surface")
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("launcher-search-header")
                    .test_support()
                    .h(px(self.launcher_input_height()))
                    .flex_shrink_0()
                    .px(px(metrics::SPACE_XL))
                    .flex()
                    .items_center()
                    .child(leading_control)
                    .child(div().w(px(12.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .justify_center()
                            .min_h(px(0.))
                            .gap(px(if self.input_mode == InputMode::Content {
                                6.
                            } else {
                                2.
                            }))
                            .when_some(mode_label, |element, label| {
                                element.child(
                                    div()
                                        .id(if self.input_mode == InputMode::Environment {
                                            "launcher-current-app-label"
                                        } else {
                                            "launcher-mode-label"
                                        })
                                        .test_support()
                                        .truncate()
                                        .text_size(px(metrics::PAGE_MODE_SIZE))
                                        .line_height(px(
                                            metrics::PAGE_MODE_SIZE * metrics::CAPTION_LINE_HEIGHT
                                        ))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme::secondary())
                                        .child(label),
                                )
                            })
                            .child(
                                div()
                                    .id("launcher-search-input-slot")
                                    .test_support()
                                    .min_w(px(0.))
                                    .min_h(px(0.))
                                    .w_full()
                                    .child(launcher_editor),
                            ),
                    )
                    .child(div().w(px(14.)))
                    .child(if self.model.loading {
                        div()
                            .size(px(16.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Spinner::new().xsmall().color(theme::secondary()))
                            .into_any_element()
                    } else {
                        self.keycap("esc").into_any_element()
                    }),
            );
        if has_panel || self.root_navigation_open {
            root = root.child(div().h(px(metrics::DIVIDER_WIDTH)).bg(theme::divider()));
        }
        if self.root_navigation_open {
            return root
                .child(self.render_root_navigation(cx))
                .into_any_element();
        }
        if let Some(controls) = scope_controls {
            root = root.child(controls);
        }
        if let Some((title, detail)) = result_heading {
            root = root.child(
                div()
                    .id("launcher-result-heading")
                    .test_support()
                    .h(px(24.))
                    .px(px(22.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(11.))
                            .line_height(px(11. * metrics::CAPTION_LINE_HEIGHT))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::secondary())
                            .child(title),
                    )
                    .when_some(detail, |element, detail| {
                        element.child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .text_right()
                                .text_size(px(metrics::CAPTION_SIZE))
                                .text_color(theme::muted())
                                .child(detail),
                        )
                    }),
            );
        }
        if let Some(notice) = self.notice.clone() {
            let notice_height = self.launcher_notice_height();
            root = root.child(
                div()
                    .id("launcher-notice")
                    .test_support()
                    .h(px(notice_height))
                    .flex_shrink_0()
                    .px(px(metrics::SPACE_XL))
                    .py(px(4.))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .id("launcher-notice-scroll")
                            .test_support()
                            .size_full()
                            .track_scroll(&self.notice_scroll)
                            .overflow_y_scroll()
                            .vertical_scrollbar(&self.notice_scroll)
                            .child(components::notice(notice, self.notice_kind)),
                    ),
            );
        }
        if has_panel {
            root = root.child(rows);
        }
        root.into_any_element()
    }

    fn change_web_package_operation(
        &mut self,
        id: String,
        operation: &'static str,
        cx: &mut Context<Self>,
    ) {
        if let Some(live) = self.live.clone() {
            let receiver = live.change_web_package(id, operation.to_owned());
            self.poll(receiver, self.live_state.generation, cx);
        }
    }

    fn ensure_web_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.web_surface.view.is_some() {
            return;
        }
        let (Some(document), Some(live)) = (self.web_surface.document.clone(), self.live.clone())
        else {
            return;
        };
        let policy = match WebDocumentPolicy::new(document.origin.clone(), document.entry.clone()) {
            Ok(policy) => policy,
            Err(error) => {
                self.web_surface.error = Some(l10n::web_document_failed(
                    self.locale,
                    &format!("{error:#}"),
                ));
                return;
            }
        };
        let runtime = live.web_runtime();
        let protocol_policy = policy.clone();
        let protocol_runtime = runtime.clone();
        let ipc_policy = policy.clone();
        let active_binding = Arc::new(Mutex::new(ActiveWebBinding {
            handle: document.handle.clone(),
            origin: document.origin.clone(),
        }));
        let protocol_binding = active_binding.clone();
        let ipc_binding = active_binding.clone();
        let (ipc_sender, ipc_receiver) = tokio::sync::mpsc::channel(WEB_IPC_QUEUE_CAPACITY);
        let entry_url = policy.url();
        let language = match self.locale {
            Locale::En => "en",
            Locale::Zh => "zh",
        };
        let builder = wry::WebViewBuilder::new()
            .with_transparent(true)
            .with_incognito(true)
            .with_devtools(cfg!(debug_assertions))
            .with_autoplay(false)
            .with_initialization_script(bootstrap_script(language))
            .with_navigation_handler(move |candidate| candidate == entry_url)
            .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
            .with_download_started_handler(|_, _| false)
            .with_custom_protocol("anywhere-plugin".to_owned(), move |_, request| {
                let Some(path) = protocol_policy.resource_path(&request.uri().to_string()) else {
                    return plain_response(404, "Not found");
                };
                let handle = protocol_binding
                    .lock()
                    .map(|binding| binding.handle.clone())
                    .unwrap_or_default();
                match protocol_runtime.web_resource(handle.as_str(), &path) {
                    Ok(bytes) => protocol_policy.response(&path, bytes),
                    Err(_) => plain_response(404, "Not found"),
                }
            })
            .with_ipc_handler(move |request| {
                let source = request.uri().to_string();
                let frame = request.body();
                if frame.len() > WEB_FRAME_BYTES {
                    tracing::warn!(
                        source = %source,
                        frame_bytes = frame.len(),
                        "web_ipc_frame_rejected"
                    );
                    return;
                }
                if !ipc_policy.accepts_source(&source) {
                    tracing::warn!(source = %source, "web_ipc_source_rejected");
                    return;
                }
                let Ok(binding) = ipc_binding.lock() else {
                    tracing::warn!("web_ipc_binding_unavailable");
                    return;
                };
                if ipc_sender
                    .try_send(WebIpcRequest {
                        handle: binding.handle.clone(),
                        origin: binding.origin.clone(),
                        frame: frame.clone(),
                    })
                    .is_err()
                {
                    tracing::warn!("web_ipc_queue_full");
                }
            })
            .with_url(policy.url());
        let web_view = match gpui_webview::WebView::build(builder, window, cx) {
            Ok(web_view) => web_view,
            Err(error) => {
                self.web_surface.error =
                    Some(l10n::web_view_start_failed(self.locale, &error.to_string()));
                return;
            }
        };
        if let Err(error) = web_view.set_theme(wry::Theme::Dark) {
            self.web_surface.error = Some(l10n::web_theme_failed(self.locale, &error.to_string()));
        }
        tracing::debug!(
            document = %document.handle,
            origin = %document.origin,
            entry = %document.entry,
            "web_view_created"
        );
        self.web_surface.view = Some(cx.new(|_| web_view));
        self.web_surface.binding = Some(active_binding);
        self.poll_web_ipc(ipc_receiver, cx);
    }

    fn evaluate_web_script(&self, script: &str, cx: &Context<Self>) -> anyhow::Result<()> {
        let web_view = self
            .web_surface
            .view
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Web view is not mounted"))?;
        web_view
            .read(cx)
            .raw()
            .evaluate_script(script)
            .map_err(anyhow::Error::from)
    }

    fn poll_web_ipc(&mut self, mut receiver: Receiver<WebIpcRequest>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            while let Some(request) = receiver.recv().await {
                let reply = this.update(cx, |this, cx| {
                    tracing::debug!(
                        document = %request.handle,
                        origin = %request.origin,
                        frame_bytes = request.frame.len(),
                        current_document = this
                            .web_surface
                            .document
                            .as_ref()
                            .map(|document| document.handle.as_str()),
                        "web_ipc_request_received"
                    );
                    if this
                        .web_surface
                        .document
                        .as_ref()
                        .map(|document| document.handle.as_str())
                        != Some(request.handle.as_str())
                    {
                        return None;
                    }
                    if crate::web_tool::is_host_escape_frame(&request.frame) {
                        tracing::debug!(
                            document = %request.handle,
                            "web_view_escape_requested"
                        );
                        cx.emit(LauncherEvent::WebViewEscapeRequested);
                        return Some(());
                    }
                    let live = this.live.clone()?;
                    let generation = this.live_state.generation;
                    let receiver = live.dispatch_web_frame(
                        request.handle.clone(),
                        request.origin.clone(),
                        true,
                        request.frame.clone(),
                    );
                    this.poll_web_reply(receiver, request.handle, request.frame, generation, cx);
                    Some(())
                });
                if reply.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn poll_web_reply(
        &mut self,
        mut receiver: Receiver<LiveEvent>,
        handle: String,
        request: String,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Some(event) = receiver.recv().await {
                match event {
                    LiveEvent::WebFrameReply {
                        handle: event_handle,
                        frame,
                    } => {
                        let request_method = serde_json::from_str::<serde_json::Value>(&request)
                            .ok()
                            .and_then(|value| {
                                value
                                    .get("method")
                                    .and_then(|method| method.as_str())
                                    .map(str::to_owned)
                            });
                        let reply = serde_json::from_str::<serde_json::Value>(&frame).ok();
                        let reply_error = reply
                            .as_ref()
                            .and_then(|value| value.get("error"))
                            .and_then(|error| error.get("code"))
                            .and_then(|code| code.as_str());
                        let reply_result_kind = reply
                            .as_ref()
                            .and_then(|value| value.get("result"))
                            .map(|result| match result {
                                serde_json::Value::Null => "null",
                                serde_json::Value::Bool(_) => "bool",
                                serde_json::Value::Number(_) => "number",
                                serde_json::Value::String(_) => "string",
                                serde_json::Value::Array(_) => "array",
                                serde_json::Value::Object(_) => "object",
                            })
                            .unwrap_or("missing");
                        tracing::debug!(
                            document = %event_handle,
                            method = request_method.as_deref().unwrap_or("unknown"),
                            reply_bytes = frame.len(),
                            reply_error = reply_error.unwrap_or("none"),
                            reply_result = reply_result_kind,
                            "web_ipc_reply_received"
                        );
                        let _ = this.update(cx, |this, cx| {
                            if generation != this.live_state.generation
                                || event_handle != handle
                                || this
                                    .web_surface
                                    .document
                                    .as_ref()
                                    .map(|document| document.handle.as_str())
                                    != Some(handle.as_str())
                            {
                                return;
                            }
                            if let Err(error) = this.evaluate_web_script(&reply_script(&frame), cx)
                            {
                                tracing::warn!(
                                    %error,
                                    document = %event_handle,
                                    "web_ipc_reply_delivery_failed"
                                );
                                this.web_surface.error =
                                    Some(l10n::web_tool_closed_unexpectedly(this.locale));
                            }
                            let acknowledgement = accepted_web_acknowledgement(&request, &frame);
                            tracing::debug!(
                                document = %event_handle,
                                method = request_method.as_deref().unwrap_or("unknown"),
                                acknowledgement = acknowledgement.as_deref().unwrap_or("none"),
                                "web_ipc_reply_applied"
                            );
                            match acknowledgement.as_deref() {
                                Some("lifecycle.ready") => {
                                    this.web_surface.ready = true;
                                    if this
                                        .evaluate_web_script(
                                            &canvas_script(
                                                !crate::platform::uses_opaque_fallback(),
                                            ),
                                            cx,
                                        )
                                        .is_err()
                                    {
                                        this.web_surface.error =
                                            Some(l10n::web_tool_background_failed(this.locale));
                                    }
                                }
                                Some("state.subscribe")
                                    if this.web_surface.state_handle.as_deref()
                                        != Some(handle.as_str()) =>
                                {
                                    this.web_surface.state_handle = Some(handle.clone());
                                    this.web_surface.state_version = 0;
                                    this.follow_web_state(handle.clone(), 0, generation, cx);
                                }
                                _ => {}
                            }
                            cx.notify();
                        });
                        break;
                    }
                    LiveEvent::Error(error) => {
                        tracing::warn!(
                            %error,
                            document = %handle,
                            "web_ipc_dispatch_failed"
                        );
                        let _ = this.update(cx, |this, cx| {
                            if generation == this.live_state.generation
                                && event_handle_matches(this, &handle)
                            {
                                this.web_surface.error = Some(error);
                                this.web_surface.state_handle = None;
                                cx.notify();
                            }
                        });
                        break;
                    }
                    LiveEvent::Ignored => break,
                    _ => {}
                }
            }
        })
        .detach();
    }

    fn follow_web_state(
        &mut self,
        handle: String,
        after: u64,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(live) = self.live.clone() else {
            return;
        };
        let mut receiver = live.wait_web_state(handle.clone(), after);
        cx.spawn(async move |this, cx| loop {
            match receiver.recv().await {
                Some(LiveEvent::WebState {
                    handle: event_handle,
                    frame,
                }) => {
                    let next = this.update(cx, |this, cx| {
                        if generation != this.live_state.generation
                            || event_handle != handle
                            || !event_handle_matches(this, &handle)
                        {
                            return None;
                        }
                        let version = web_state_version(&frame).unwrap_or(after);
                        if let Err(error) = this.evaluate_web_script(&state_script(&frame), cx) {
                            this.web_surface.error = Some(error.to_string());
                            this.web_surface.state_handle = None;
                            cx.notify();
                            return None;
                        }
                        this.web_surface.state_version = version;
                        cx.notify();
                        Some(version)
                    });
                    match next {
                        Ok(Some(version)) => {
                            let continuation = this.update(cx, |this, cx| {
                                if generation == this.live_state.generation
                                    && event_handle_matches(this, &handle)
                                {
                                    this.follow_web_state(handle.clone(), version, generation, cx);
                                }
                            });
                            if continuation.is_err() {
                                break;
                            }
                        }
                        _ => break,
                    }
                    break;
                }
                Some(LiveEvent::Ignored) => {
                    let restart = this.update(cx, |this, cx| {
                        if generation == this.live_state.generation
                            && event_handle_matches(this, &handle)
                        {
                            this.follow_web_state(handle.clone(), after, generation, cx);
                            true
                        } else {
                            false
                        }
                    });
                    if !matches!(restart, Ok(true)) {
                        break;
                    }
                    break;
                }
                Some(LiveEvent::Error(error)) => {
                    let _ = this.update(cx, |this, cx| {
                        if generation == this.live_state.generation
                            && event_handle_matches(this, &handle)
                        {
                            this.web_surface.error = Some(error);
                            this.web_surface.state_handle = None;
                            cx.notify();
                        }
                    });
                    break;
                }
                Some(_) => continue,
                None => break,
            }
        })
        .detach();
    }

    fn keycap(&self, value: impl Into<SharedString>) -> impl IntoElement {
        components::keycap(value)
    }
}

impl Render for LauncherView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_search_placeholder(window, cx);
        self.sync_launcher_window_after_render(window, cx);
        let content = match self.screen {
            Screen::Launcher => self.render_launcher(cx),
            Screen::FileSearch => self.render_file_search(cx),
            Screen::Clipboard => self.render_clipboard(window, cx),
            Screen::ClipboardShelf => self.render_clipboard_shelf(cx),
            Screen::ApplicationHistory => self.render_app_history(window, cx),
            Screen::Tasks => self.render_tasks(window, cx),
            Screen::WebPackages => self.render_web_packages(cx),
            Screen::WebTool => self.render_web_tool(window, cx),
            Screen::PluginDetail => self.render_plugin_detail(cx),
            Screen::UnsupportedTool => self.render_unsupported_tool(cx),
            Screen::Settings => self.render_settings(cx),
        };
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .rounded(px(LAUNCHER_CORNER_RADIUS))
            .overflow_hidden()
            .bg(if crate::platform::uses_opaque_fallback() {
                theme::fallback_surface()
            } else {
                theme::content_scrim()
            })
            .text_color(theme::primary())
            .font_family(fonts::FONT_UI)
            .font_weight(FontWeight::NORMAL)
            .capture_key_down(
                cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                    if event.prefer_character_input {
                        return;
                    }
                    if event.keystroke.modifiers.platform
                        && !event.keystroke.modifiers.control
                        && !event.keystroke.modifiers.alt
                        && !event.keystroke.modifiers.shift
                        && event.keystroke.key == ","
                        && !this.standalone_shelf
                        && !this.standalone_tasks
                        && !this.preview_open
                    {
                        this.open_settings(window, cx);
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if this.handle_preview_key(event, window, cx) {
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if event.keystroke.modifiers.platform
                        && event.keystroke.key.eq_ignore_ascii_case("y")
                        && this.can_toggle_preview_shortcut()
                    {
                        if !event.is_held {
                            this.toggle_preview_shortcut(window, cx);
                        }
                        window.prevent_default();
                        cx.stop_propagation();
                    } else if event.keystroke.modifiers.platform
                        && event.keystroke.key.eq_ignore_ascii_case("k")
                        && matches!(this.screen, Screen::Launcher | Screen::FileSearch)
                    {
                        this.show_object_actions(cx);
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                }),
            )
            .on_key_down(
                cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                    if this.shortcut_recording.is_some()
                        || (this.screen == Screen::Settings
                            && !this.preview_open
                            && event.keystroke.key.eq_ignore_ascii_case("escape"))
                        || (this.screen == Screen::Settings
                            && event.keystroke.key.eq_ignore_ascii_case("tab"))
                        || (this.preview_open && event.keystroke.key.eq_ignore_ascii_case("escape"))
                    {
                        return;
                    }
                    if event.prefer_character_input && this.shortcut_recording.is_none() {
                        return;
                    }
                    if is_scope_tab_shortcut(event.keystroke.key.as_str(), key_modifiers(event)) {
                        return;
                    }
                    this.handle_key_with_repeat(
                        event.keystroke.key.as_str(),
                        key_modifiers(event),
                        event.is_held,
                        window,
                        cx,
                    );
                }),
            )
            .child(content)
            // The native material owns the window rim and shadow. The GPUI
            // canvas supplies content only; no window-wide stroke or veil.
            .when(self.screen == Screen::Launcher, |element| {
                element.max_h(px(520.))
            })
            .when(
                matches!(
                    self.screen,
                    Screen::FileSearch
                        | Screen::Clipboard
                        | Screen::ClipboardShelf
                        | Screen::ApplicationHistory
                        | Screen::Tasks
                        | Screen::WebPackages
                        | Screen::WebTool
                        | Screen::PluginDetail
                        | Screen::UnsupportedTool
                ),
                |element| element.h_full(),
            )
    }
}

pub fn launcher_window_size() -> gpui_kit::Size<gpui_kit::Pixels> {
    gpui_kit::size(px(LAUNCHER_WIDTH), px(metrics::PAGE_HEADER_HEIGHT))
}

fn event_handle_matches(view: &LauncherView, handle: &str) -> bool {
    view.web_surface
        .document
        .as_ref()
        .is_some_and(|document| document.handle == handle)
}

fn action_result_notice_kind(result: &anywhere_protocol::ActionResult) -> components::NoticeKind {
    if result.error.is_some() {
        components::NoticeKind::Error
    } else if result.warning.is_some() {
        components::NoticeKind::Warning
    } else if matches!(
        result.view,
        Some(anywhere_protocol::HostView::WebPlugin { .. })
    ) {
        components::NoticeKind::Info
    } else if result.succeeded() {
        components::NoticeKind::Success
    } else {
        components::NoticeKind::Error
    }
}

fn accepted_web_acknowledgement(request: &str, response: &str) -> Option<String> {
    let request: serde_json::Value = serde_json::from_str(request).ok()?;
    let response: serde_json::Value = serde_json::from_str(response).ok()?;
    let method = request.get("method")?.as_str()?;
    if !matches!(method, "lifecycle.ready" | "state.subscribe")
        || request.get("requestId")? != response.get("requestId")?
        || response.get("error").is_some()
        || !response.get("result")?.is_object()
    {
        return None;
    }
    if method == "lifecycle.ready" {
        let result = response.get("result")?.as_object()?;
        if result.len() != 1 || !result.contains_key("value") || !result.get("value")?.is_null() {
            return None;
        }
    }
    Some(method.to_owned())
}

fn web_state_version(frame: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(frame)
        .ok()?
        .get("version")?
        .as_u64()
}

enum PreviewReservation {
    Ready(PreviewHost),
    Busy,
    Stale,
}

fn preview_error_message(locale: Locale, error: &anyhow::Error) -> String {
    l10n::preview_unavailable_with(locale, &format!("{error:#}"))
}

fn privacy_pane_for_capture(
    source: ContextSource,
    status: ContextCaptureStatus,
) -> Option<PrivacySettingsPane> {
    if status != ContextCaptureStatus::PermissionDenied {
        return None;
    }
    match source {
        ContextSource::FinderSelection | ContextSource::BrowserCurrentPage => {
            Some(PrivacySettingsPane::Automation)
        }
        ContextSource::SelectedText => Some(PrivacySettingsPane::Accessibility),
        ContextSource::Clipboard => Some(PrivacySettingsPane::Clipboard),
        ContextSource::UserInput
        | ContextSource::ForegroundApplication
        | ContextSource::ApplicationContext => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::WebDocument;
    use crate::launcher::view::navigation::LauncherStreamRecovery;
    use anywhere_protocol::{
        ui::{DetailView, KeyValue},
        FileObjectKind, HostView,
    };
    use gpui_kit::{
        test::TestWindowExt as _, AnyWindowHandle, TestAppContext, VisualTestContext, WindowOptions,
    };

    fn open_view(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<LauncherView>) {
        cx.update(gpui_kit::init);
        let (window, view) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    cx,
                    |window, cx| cx.new(|cx| LauncherView::new(window, cx)),
                )
            })
            .unwrap();
        (window, view)
    }

    fn sync_test_viewport(
        cx: &mut TestAppContext,
        window: AnyWindowHandle,
        size: gpui_kit::Size<gpui_kit::Pixels>,
    ) {
        cx.simulate_window_resize(window, size);
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            assert_eq!(window.viewport_size(), size);
            window.render_frame(cx);
        })
        .unwrap();
    }

    fn sync_test_viewport_to_desired(
        cx: &mut TestAppContext,
        window: AnyWindowHandle,
        view: &Entity<LauncherView>,
    ) {
        let size = cx.update(|cx| view.read(cx).desired_window_size());
        sync_test_viewport(cx, window, size);
    }

    fn render_and_sync_test_viewport(
        cx: &mut TestAppContext,
        window: AnyWindowHandle,
        view: &Entity<LauncherView>,
    ) {
        cx.update_window(window, |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
        let desired = cx.update(|cx| view.read(cx).desired_window_size());
        let bounds = cx
            .update_window(window, |_, window, _| window.bounds().size)
            .unwrap();
        assert_eq!(
            bounds, desired,
            "the deferred native resize must follow the rendered state"
        );
        sync_test_viewport(cx, window, desired);
    }

    fn scoped_state(
        scopes: Vec<anywhere_protocol::ScopeEntry>,
    ) -> anywhere_protocol::LauncherState {
        anywhere_protocol::LauncherState {
            scopes,
            ..Default::default()
        }
    }

    fn current_app_scope() -> anywhere_protocol::ScopeEntry {
        anywhere_protocol::ScopeEntry {
            id: "app.example".into(),
            label: "Editor".into(),
            is_current_app: true,
            action_ids: Vec::new(),
            menu_ids: Vec::new(),
        }
    }

    fn file_row(id: &str, title: &str) -> LauncherRow {
        LauncherRow {
            id: id.to_owned(),
            title: title.to_owned(),
            subtitle: Some(format!("/fixtures/{title}")),
            shortcut: None,
            icon: "file".to_owned(),
            kind: CandidateKind::File,
            enabled: true,
            is_menu: false,
            menu_path: Vec::new(),
            command_id: None,
            file_kind: Some(FileObjectKind::File),
            source_label: Some("Files".to_owned()),
            icon_identity: None,
            execution_context_id: None,
            primary_action: false,
        }
    }

    #[gpui_kit::test]
    fn focusing_an_open_workspace_keeps_the_live_snapshot_stream(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            let expected_generation = view.update(cx, |view, cx| {
                view.presented = true;
                view.screen = Screen::Launcher;
                view.input_mode = InputMode::Environment;
                view.live_state.generation = 41;
                assert!(view.activate_from_entry(InputMode::Environment, false, window, cx));
                view.live_state.generation
            });
            assert_eq!(expected_generation, 41);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn current_app_selects_a_late_scope_once_without_overriding_user_navigation(
        cx: &mut TestAppContext,
    ) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.input_mode = InputMode::Environment;
                view.scope_selection_intent = ScopeSelectionIntent::PendingCurrentApp;
                view.apply_state(scoped_state(Vec::new()), cx);
                assert_eq!(view.model.selected_scope, None);
                assert_eq!(
                    view.scope_selection_intent,
                    ScopeSelectionIntent::PendingCurrentApp
                );

                view.apply_state(scoped_state(vec![current_app_scope()]), cx);
                assert_eq!(view.model.selected_scope.as_deref(), Some("app.example"));
                assert_eq!(
                    view.scope_selection_intent,
                    ScopeSelectionIntent::UserControlled
                );

                view.model.select_scope(None);
                view.scope_selection_intent = ScopeSelectionIntent::PendingCurrentApp;
                view.mark_scope_user_controlled();
                view.apply_state(scoped_state(vec![current_app_scope()]), cx);
                assert_eq!(view.model.selected_scope, None);
            });
        });
    }

    #[gpui_kit::test]
    fn switching_back_to_current_app_uses_an_existing_scope_with_a_query(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.input_mode = InputMode::Search;
                view.model.scopes = vec![
                    crate::launcher::LauncherScope {
                        id: "app.example".into(),
                        label: "Editor".into(),
                        is_current_app: true,
                    },
                    crate::launcher::LauncherScope {
                        id: "files".into(),
                        label: "Files".into(),
                        is_current_app: false,
                    },
                ];
                view.model.set_query("open".into());
                view.model.select_scope(None);
                view.select_root_mode(InputMode::Environment, cx);
                assert_eq!(view.model.selected_scope.as_deref(), Some("app.example"));
                assert_eq!(
                    view.scope_selection_intent,
                    ScopeSelectionIntent::UserControlled
                );
            });
        });
    }

    #[gpui_kit::test]
    fn switching_back_to_current_app_waits_for_a_late_scope_with_a_query(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.input_mode = InputMode::Search;
                view.model.scopes.clear();
                view.model.set_query("open".into());
                view.model.select_scope(None);
                view.select_root_mode(InputMode::Environment, cx);
                assert_eq!(view.model.selected_scope, None);
                assert_eq!(
                    view.scope_selection_intent,
                    ScopeSelectionIntent::PendingCurrentAppNavigation
                );
                assert!(view.model.loading);
                assert!(view.model.rows.is_empty());

                let mut state = scoped_state(vec![current_app_scope()]);
                state.input = "open".into();
                view.apply_state(state, cx);
                assert_eq!(view.model.selected_scope.as_deref(), Some("app.example"));
                assert_eq!(
                    view.scope_selection_intent,
                    ScopeSelectionIntent::UserControlled
                );
            });
        });
    }

    #[gpui_kit::test]
    fn current_app_navigation_finishes_when_no_app_scope_exists(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.input_mode = InputMode::Search;
                view.model.scopes.clear();
                view.model.set_query("open".into());
                view.select_root_mode(InputMode::Environment, cx);
                assert!(view.model.loading);

                let mut state = scoped_state(Vec::new());
                state.input = "open".into();
                state.context_complete = true;
                view.apply_state(state, cx);
                assert!(!view.model.loading);
                assert!(view.model.rows.is_empty());
                assert_eq!(
                    view.scope_selection_intent,
                    ScopeSelectionIntent::UserControlled
                );
            });
        });
    }

    #[gpui_kit::test]
    fn entry_default_current_app_is_still_cancelled_by_a_query(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.input_mode = InputMode::Environment;
                view.scope_selection_intent = ScopeSelectionIntent::PendingCurrentApp;
                view.model.set_query("open".into());
                let mut state = scoped_state(vec![current_app_scope()]);
                state.input = "open".into();
                view.apply_state(state, cx);
                assert_eq!(view.model.selected_scope, None);
                assert_eq!(
                    view.scope_selection_intent,
                    ScopeSelectionIntent::PendingCurrentApp
                );
            });
        });
    }

    #[gpui_kit::test]
    fn programmatic_editor_reset_does_not_cancel_a_pending_current_scope(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.input
                    .update(cx, |input, cx| input.insert("old", window, cx));
                view.input_mode = InputMode::Environment;
                view.scope_selection_intent = ScopeSelectionIntent::PendingCurrentApp;
                view.sync_launcher_editor(String::new(), cx);
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                assert_eq!(
                    view.scope_selection_intent,
                    ScopeSelectionIntent::PendingCurrentApp
                );
                view.apply_state(scoped_state(vec![current_app_scope()]), cx);
                assert_eq!(view.model.selected_scope.as_deref(), Some("app.example"));
            });
        });
    }

    #[gpui_kit::test]
    fn tab_cycles_only_visible_scopes_from_the_focused_editor(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.model.scopes = vec![
                    crate::launcher::LauncherScope {
                        id: "app.example".into(),
                        label: "Editor".into(),
                        is_current_app: true,
                    },
                    crate::launcher::LauncherScope {
                        id: "files".into(),
                        label: "Files".into(),
                        is_current_app: false,
                    },
                ];
                view.model.select_scope(None);
                cx.notify();
            });
            window.render_frame(cx);
            window.click("launcher-search-input", cx);
            assert!(
                view.update(cx, |view, cx| view.active_editor_focused(window, cx)),
                "the launcher editor must own focus before Tab navigation"
            );
            assert!(view.read(cx).has_scope_bar());
            window.press("tab", cx);
            assert_eq!(
                view.read(cx).model.selected_scope.as_deref(),
                Some("app.example")
            );
            window.press("tab", cx);
            assert_eq!(view.read(cx).model.selected_scope.as_deref(), Some("files"));
            window.press("tab", cx);
            assert_eq!(
                view.read(cx).model.selected_scope.as_deref(),
                Some("app.example")
            );
            window.press("shift-tab", cx);
            assert_eq!(view.read(cx).model.selected_scope.as_deref(), Some("files"));
        })
        .unwrap();
    }

    #[test]
    fn scope_tab_shortcuts_reject_command_and_option_modifiers() {
        assert!(is_scope_tab_shortcut("tab", KeyModifiers::default()));
        assert!(is_scope_tab_shortcut(
            "tab",
            KeyModifiers {
                control: true,
                shift: true,
                modified: true,
                ..Default::default()
            }
        ));
        assert!(!is_scope_tab_shortcut(
            "tab",
            KeyModifiers {
                command: true,
                modified: true,
                ..Default::default()
            }
        ));
        assert!(!is_scope_tab_shortcut(
            "tab",
            KeyModifiers {
                alt: true,
                modified: true,
                ..Default::default()
            }
        ));
    }

    #[test]
    fn identical_launcher_sizes_do_not_trigger_native_resizes() {
        let same = gpui_kit::size(px(648.), px(72.));
        assert!(!window_size_changed(same, same));
        assert!(!window_size_changed(
            same,
            gpui_kit::size(px(648.4), px(72.4))
        ));
        assert!(window_size_changed(
            same,
            gpui_kit::size(px(648.), px(128.))
        ));
    }

    #[test]
    fn clipboard_confirmation_rejects_stale_or_loading_submissions() {
        let clear = ClipboardConfirmation::Clear;
        assert!(clipboard_confirmation_is_submittable(
            false,
            Some(clear),
            clear
        ));
        assert!(!clipboard_confirmation_is_submittable(
            true,
            Some(clear),
            clear
        ));
        assert!(!clipboard_confirmation_is_submittable(false, None, clear));
        assert!(!clipboard_confirmation_is_submittable(
            false,
            Some(ClipboardConfirmation::SetRetention(30)),
            ClipboardConfirmation::SetRetention(7),
        ));
    }

    #[test]
    fn tool_pages_consume_unmodified_tab_navigation() {
        for screen in [
            Screen::Clipboard,
            Screen::ClipboardShelf,
            Screen::ApplicationHistory,
            Screen::PluginDetail,
            Screen::UnsupportedTool,
        ] {
            assert!(is_tool_page_tab_shortcut(
                screen,
                "tab",
                KeyModifiers::default()
            ));
            assert!(is_tool_page_tab_shortcut(
                screen,
                "tab",
                KeyModifiers {
                    shift: true,
                    modified: true,
                    ..Default::default()
                }
            ));
            assert!(!is_tool_page_tab_shortcut(
                screen,
                "tab",
                KeyModifiers {
                    control: true,
                    modified: true,
                    ..Default::default()
                }
            ));
        }
        assert!(!is_tool_page_tab_shortcut(
            Screen::Tasks,
            "tab",
            KeyModifiers::default()
        ));
        assert!(!is_tool_page_tab_shortcut(
            Screen::Settings,
            "tab",
            KeyModifiers::default()
        ));
    }

    #[test]
    fn tasks_global_shortcuts_do_not_require_a_focused_control() {
        let command = KeyModifiers {
            command: true,
            modified: true,
            ..Default::default()
        };
        for key in ["k", "l", ".", "r", "f"] {
            assert!(is_tasks_global_shortcut(key, command));
        }
        assert!(is_tasks_global_shortcut("escape", KeyModifiers::default()));
        assert!(is_tasks_global_shortcut(
            "up",
            KeyModifiers {
                alt: true,
                ..command
            }
        ));
        assert!(!is_tasks_global_shortcut("a", command));
        assert!(!is_tasks_global_shortcut("s", KeyModifiers::default()));
        assert!(!is_tasks_global_shortcut(
            "l",
            KeyModifiers {
                control: true,
                modified: true,
                ..Default::default()
            }
        ));
        assert!(should_handle_tasks_shortcut("l", command, true));
        assert!(!should_handle_tasks_shortcut(
            "escape",
            KeyModifiers::default(),
            true
        ));
        assert!(should_handle_tasks_shortcut(
            "escape",
            KeyModifiers::default(),
            false
        ));
    }

    #[gpui_kit::test]
    fn tasks_search_shortcut_only_focuses_the_visible_filter(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let command = KeyModifiers {
            command: true,
            modified: true,
            ..Default::default()
        };
        let row = |id: &str| crate::host::ExecutionRow {
            id: id.to_owned(),
            title: id.to_owned(),
            directory: "/tmp/project".to_owned(),
            command: "true".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Running,
            output_revision: 0,
            exit_code: None,
            started_at_ms: 0,
            finished_at_ms: None,
            output_truncated: false,
            error: None,
        };
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                assert!(view.tasks.rows.is_empty());
                view.handle_key("f", command, window, cx);
                assert!(!view
                    .tasks_filter_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window));

                view.apply_tasks_rows(vec![row("task")], cx);
                view.tasks_filter_input
                    .update(cx, |input, cx| input.set_value("task", window, cx));
                view.handle_key("f", command, window, cx);
                assert!(view
                    .tasks_filter_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window));
                assert_eq!(view.tasks_filter_input.read(cx).selected_range(), 0..4);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn tasks_command_k_opens_the_current_task_actions(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let command = KeyModifiers {
            command: true,
            modified: true,
            ..Default::default()
        };
        let row = crate::host::ExecutionRow {
            id: "task".to_owned(),
            title: "npm Run · task".to_owned(),
            directory: "/tmp/project".to_owned(),
            command: "npm run task".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Succeeded,
            output_revision: 1,
            exit_code: Some(0),
            started_at_ms: 0,
            finished_at_ms: Some(1),
            output_truncated: false,
            error: None,
        };
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                view.apply_tasks_rows(vec![row.clone()], cx);
                view.tasks.detail = Some(crate::host::ExecutionDetail {
                    row,
                    stdout: "ready".to_owned(),
                    stderr: String::new(),
                });

                view.handle_key("k", command, window, cx);
                assert!(view.tasks.task_actions_open);
                assert!(!view.tasks.tool_options_open);
                assert_eq!(view.tasks.task_action_selected, 0);

                view.handle_key_with_repeat("k", command, true, window, cx);
                assert!(
                    view.tasks.task_actions_open,
                    "held command-k must not toggle task Actions closed"
                );
                view.handle_key("down", KeyModifiers::default(), window, cx);
                assert_eq!(view.tasks.task_action_selected, 1);
                view.handle_key("up", KeyModifiers::default(), window, cx);
                assert_eq!(view.tasks.task_action_selected, 0);

                view.handle_key("enter", KeyModifiers::default(), window, cx);
                assert!(!view.tasks.task_actions_open);
                assert_eq!(view.notice, Some(l10n::command_copied(Locale::En)));
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn tasks_command_k_uses_window_options_until_a_detail_is_visible(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let command = KeyModifiers {
            command: true,
            modified: true,
            ..Default::default()
        };
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                view.handle_key("k", command, window, cx);
                assert!(view.tasks.tool_options_open);
                assert!(!view.tasks.task_actions_open);
                view.handle_key_with_repeat("k", command, true, window, cx);
                assert!(
                    view.tasks.tool_options_open,
                    "held command-k must not toggle Tool options closed"
                );
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn tasks_tool_options_expose_the_launcher(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                view.tasks.tool_options_open = true;
                cx.notify();
            });
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        let mut visual = VisualTestContext::from_window(window, cx);
        assert!(visual.debug_bounds("tasks-open-launcher").is_some());
    }

    #[gpui_kit::test]
    fn tasks_tool_options_keyboard_selects_launcher_and_closes_with_escape(
        cx: &mut TestAppContext,
    ) {
        use std::{cell::Cell, rc::Rc};

        let (window, view) = open_view(cx);
        let command = KeyModifiers {
            command: true,
            modified: true,
            ..Default::default()
        };
        let restored = Rc::new(Cell::new(false));
        let observed = restored.clone();
        let _event = cx.update(|cx| {
            cx.subscribe(&view, move |_, event: &LauncherEvent, _| {
                if event == &LauncherEvent::RestoreLauncher {
                    observed.set(true);
                }
            })
        });
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                view.handle_key("k", command, window, cx);
                assert!(view.tasks.tool_options_open);
                assert_eq!(view.tasks.tool_option_selected, 0);

                view.handle_key("down", KeyModifiers::default(), window, cx);
                assert_eq!(view.tasks.tool_option_selected, 1);
                view.handle_key("enter", KeyModifiers::default(), window, cx);
                assert!(!view.tasks.tool_options_open);

                view.handle_key("k", command, window, cx);
                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert!(!view.tasks.tool_options_open);
                assert_eq!(view.screen, Screen::Tasks);
            });
        })
        .unwrap();
        assert!(restored.get());
    }

    #[gpui_kit::test]
    fn repeated_tasks_enter_does_not_repeat_the_tool_option(cx: &mut TestAppContext) {
        use std::{cell::Cell, rc::Rc};

        let (window, view) = open_view(cx);
        let restored = Rc::new(Cell::new(false));
        let observed = restored.clone();
        let _event = cx.update(|cx| {
            cx.subscribe(&view, move |_, event: &LauncherEvent, _| {
                if event == &LauncherEvent::RestoreLauncher {
                    observed.set(true);
                }
            })
        });
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                view.tasks.tool_options_open = true;
                view.tasks.tool_option_selected = 1;

                view.handle_tasks_navigation_key("enter", true, window, cx);
                assert!(view.tasks.tool_options_open);
                assert!(!restored.get());

                view.handle_tasks_navigation_key("enter", false, window, cx);
                assert!(!view.tasks.tool_options_open);
            });
        })
        .unwrap();
        assert!(restored.get());
    }

    #[gpui_kit::test]
    fn clipboard_shelf_search_shortcut_selects_the_existing_query(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let command = KeyModifiers {
            command: true,
            modified: true,
            ..Default::default()
        };
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ClipboardShelf;
                view.standalone_shelf = true;
                view.clipboard_input
                    .update(cx, |input, cx| input.set_value("history", window, cx));

                view.handle_key("f", command, window, cx);

                assert!(view
                    .clipboard_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window));
                assert_eq!(view.clipboard_input.read(cx).selected_range(), 0..7);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn selecting_a_clipboard_row_restores_search_focus(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            window.blur(cx);
            view.update(cx, |view, cx| {
                view.screen = Screen::ClipboardShelf;
                view.standalone_shelf = true;
                view.select_clipboard_row(0, window, cx);
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            assert!(view
                .read(cx)
                .clipboard_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn typing_in_clipboard_search_closes_actions(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ClipboardShelf;
                view.standalone_shelf = true;
                view.clipboard.actions_open = true;
                view.clipboard.action_target = Some(1);
                view.clipboard.action_selected = 3;
                cx.notify();
            });
            window.render_frame(cx);
            window.click("clipboard-shelf-search", cx);
            window.input("history", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            view.read_with(cx, |view, _| {
                assert!(!view.clipboard.actions_open);
                assert_eq!(view.clipboard.action_target, None);
                assert_eq!(view.clipboard.action_selected, 0);
                assert_eq!(view.clipboard.query, "history");
            });
        });
    }

    #[gpui_kit::test]
    fn clipboard_actions_close_before_handling_command_shortcuts(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let command = KeyModifiers {
            command: true,
            modified: true,
            ..Default::default()
        };
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ClipboardShelf;
                view.standalone_shelf = true;
                view.clipboard.actions_open = true;
                view.clipboard.action_target = Some(1);

                view.handle_key("k", command, window, cx);
                assert!(!view.clipboard.actions_open);

                view.clipboard.actions_open = true;
                view.clipboard.action_target = Some(1);
                view.handle_key("f", command, window, cx);
                assert!(!view.clipboard.actions_open);
                assert!(view
                    .clipboard_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window));
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn clipboard_refresh_survives_a_stale_action_target(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let directory = tempfile::tempdir().unwrap();
        let plugins = directory.path().join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        let live = Arc::new(
            LiveLauncher::open_at(
                anywhere_core::application::frontend_runtime::RuntimePaths::new(
                    directory.path().join("data"),
                    plugins,
                ),
            )
            .unwrap(),
        );
        let (window, view) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    cx,
                    move |window, cx| {
                        cx.new(|cx| LauncherView::clipboard_shelf(window, cx, live, Locale::En))
                    },
                )
            })
            .unwrap();

        cx.update_window(window, |_, _, cx| {
            view.update(cx, |view, cx| {
                view.clipboard.actions_open = true;
                view.clipboard.action_target = Some(999);
                view.clipboard.action_selected =
                    LauncherView::clipboard_refresh_action(Screen::ClipboardShelf);
                let generation = view.clipboard.query_generation;

                view.execute_clipboard_action(view.clipboard.action_selected, cx);

                assert_eq!(view.clipboard.query_generation, generation + 1);
                assert!(!view.clipboard.actions_open);
                assert_eq!(view.clipboard.action_target, None);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn held_return_copies_a_shelf_item_once(cx: &mut TestAppContext) {
        use gpui_kit::test::TestEventExt;

        cx.update(gpui_kit::init);
        let directory = tempfile::tempdir().unwrap();
        let plugins = directory.path().join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        let live = Arc::new(
            LiveLauncher::open_at(
                anywhere_core::application::frontend_runtime::RuntimePaths::new(
                    directory.path().join("data"),
                    plugins,
                ),
            )
            .unwrap(),
        );
        let (window, view) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    cx,
                    move |window, cx| {
                        cx.new(|cx| LauncherView::clipboard_shelf(window, cx, live, Locale::En))
                    },
                )
            })
            .unwrap();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, _| {
                view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                    recording: false,
                    recording_enabled: false,
                    available: true,
                    paused_until_ms: None,
                    retention_days: 7,
                    stored_entries: 1,
                    stored_bytes: 4,
                    notice: None,
                    items: vec![crate::host::ClipboardItem {
                        id: 1,
                        summary: "fixture".into(),
                        kind: "text".into(),
                        copied_at_ms: 0,
                        byte_count: 4,
                    }],
                });
                view.clipboard.browsing = true;
                view.clipboard.selected = 0;
            });
            let generation = view.read(cx).clipboard.operation_generation;

            window.key_down("enter", true, cx);
            assert_eq!(view.read(cx).clipboard.operation_generation, generation);

            window.key_down("enter", false, cx);
            assert_eq!(view.read(cx).clipboard.operation_generation, generation + 1);

            window.key_up("enter", cx);
            window.key_down("enter", true, cx);
            assert_eq!(view.read(cx).clipboard.operation_generation, generation + 1);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn held_command_k_does_not_toggle_shelf_actions_twice(cx: &mut TestAppContext) {
        use gpui_kit::test::TestEventExt;

        cx.update(gpui_kit::init);
        let directory = tempfile::tempdir().unwrap();
        let plugins = directory.path().join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        let live = Arc::new(
            LiveLauncher::open_at(
                anywhere_core::application::frontend_runtime::RuntimePaths::new(
                    directory.path().join("data"),
                    plugins,
                ),
            )
            .unwrap(),
        );
        let (window, view) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    cx,
                    move |window, cx| {
                        cx.new(|cx| LauncherView::clipboard_shelf(window, cx, live, Locale::En))
                    },
                )
            })
            .unwrap();

        cx.update_window(window, |_, window, cx| {
            window.key_down("cmd-k", false, cx);
            assert!(view.read(cx).clipboard.actions_open);

            window.key_down("cmd-k", true, cx);
            assert!(view.read(cx).clipboard.actions_open);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn tasks_window_does_not_keep_a_hidden_launcher_editor_focused(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let directory = tempfile::tempdir().unwrap();
        let plugins = directory.path().join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        let live = Arc::new(
            LiveLauncher::open_at(
                anywhere_core::application::frontend_runtime::RuntimePaths::new(
                    directory.path().join("data"),
                    plugins,
                ),
            )
            .unwrap(),
        );
        let (window, view) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    cx,
                    move |window, cx| {
                        cx.new(|cx| LauncherView::tasks_window(window, cx, live, Locale::En))
                    },
                )
            })
            .unwrap();

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                assert!(!view.input.read(cx).focus_handle(cx).is_focused(window));
                assert!(!view
                    .tasks_filter_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window));
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn removing_every_task_blurs_the_hidden_filter(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let command = KeyModifiers {
            command: true,
            modified: true,
            ..Default::default()
        };
        let row = crate::host::ExecutionRow {
            id: "task".to_owned(),
            title: "task".to_owned(),
            directory: "/tmp/project".to_owned(),
            command: "true".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Running,
            output_revision: 0,
            exit_code: None,
            started_at_ms: 0,
            finished_at_ms: None,
            output_truncated: false,
            error: None,
        };
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                view.apply_tasks_rows(vec![row], cx);
                view.handle_key("f", command, window, cx);
                assert!(view
                    .tasks_filter_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window));
                view.apply_tasks_rows(Vec::new(), cx);
            });
        })
        .unwrap();
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                assert!(!view
                    .tasks_filter_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window));
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn tasks_escape_clears_the_filter_and_returns_focus_to_the_list(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let row = crate::host::ExecutionRow {
            id: "task".to_owned(),
            title: "task".to_owned(),
            directory: "/tmp/project".to_owned(),
            command: "true".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Running,
            output_revision: 0,
            exit_code: None,
            started_at_ms: 0,
            finished_at_ms: None,
            output_truncated: false,
            error: None,
        };
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                view.apply_tasks_rows(vec![row], cx);
                view.tasks_filter_input
                    .update(cx, |input, cx| input.set_value("task", window, cx));
                view.tasks.set_filter("task".to_owned());
                view.tasks_filter_input
                    .update(cx, |input, cx| input.focus(window, cx));
                assert!(view
                    .tasks_filter_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window));

                view.handle_key("escape", KeyModifiers::default(), window, cx);

                assert!(view.tasks.filter.is_empty());
                assert_eq!(view.tasks_filter_input.read(cx).value(), "");
                assert!(!view
                    .tasks_filter_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window));
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn tasks_enter_from_the_filter_moves_focus_to_the_list(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let row = crate::host::ExecutionRow {
            id: "task".to_owned(),
            title: "task".to_owned(),
            directory: "/tmp/project".to_owned(),
            command: "true".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Running,
            output_revision: 0,
            exit_code: None,
            started_at_ms: 0,
            finished_at_ms: None,
            output_truncated: false,
            error: None,
        };
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                view.apply_tasks_rows(vec![row], cx);
                view.tasks_filter_input
                    .update(cx, |input, cx| input.focus(window, cx));

                view.handle_key("enter", KeyModifiers::default(), window, cx);

                assert!(!view
                    .tasks_filter_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window));
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn clicking_a_task_row_moves_focus_out_of_the_filter(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let row = crate::host::ExecutionRow {
            id: "task".to_owned(),
            title: "task".to_owned(),
            directory: "/tmp/project".to_owned(),
            command: "true".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Running,
            output_revision: 0,
            exit_code: None,
            started_at_ms: 0,
            finished_at_ms: None,
            output_truncated: false,
            error: None,
        };
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                view.apply_tasks_rows(vec![row], cx);
                view.tasks_filter_input
                    .update(cx, |input, cx| input.focus(window, cx));
                view.select_task_row(0, window, cx);
            });
        })
        .unwrap();
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                assert!(!view
                    .tasks_filter_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window));
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn starting_runtime_wakes_only_the_shell_without_reserving_a_session(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (window, view) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    cx,
                    |window, cx| cx.new(|cx| LauncherView::starting(window, cx, Locale::En)),
                )
            })
            .unwrap();
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                assert!(view.activate_from_entry(InputMode::Environment, true, window, cx));
                assert!(view.presented);
                assert_eq!(view.runtime_state, RuntimeState::Starting);
                assert_eq!(
                    view.notice.as_deref(),
                    Some(l10n::starting_anywhere(Locale::En).as_str())
                );
                assert_eq!(view.live_state.session_id, None);
                assert!(view.live_state.snapshot_gate.is_none());
                view.open_settings(window, cx);
                assert_eq!(view.screen, Screen::Launcher);
            });
        })
        .unwrap();
    }

    #[test]
    fn file_and_action_icons_preserve_the_flutter_vocabulary() {
        assert_eq!(
            file_icon_appearance("Projects", Some(FileObjectKind::Directory)).glyph,
            IconName::Folder
        );
        assert_eq!(
            file_icon_appearance("report.pdf", Some(FileObjectKind::File)).glyph,
            IconName::FileText
        );
        assert_eq!(
            file_icon_appearance("report.pdf", Some(FileObjectKind::File)).stamp,
            Some("PDF")
        );
        assert_eq!(
            file_icon_appearance("notes.md", Some(FileObjectKind::File)).glyph,
            IconName::Type
        );
        assert_eq!(
            file_icon_appearance("budget.xlsx", Some(FileObjectKind::File)).glyph,
            IconName::FileSpreadsheet
        );
        assert_eq!(
            file_icon_appearance("cover.png", Some(FileObjectKind::File)).glyph,
            IconName::Image
        );
        assert_eq!(
            file_icon_appearance("archive.zip", Some(FileObjectKind::File)).glyph,
            IconName::FileArchive
        );
        assert_eq!(
            file_icon_appearance("main.rs", Some(FileObjectKind::File)).glyph,
            IconName::FileCode
        );
        assert_eq!(
            file_icon_appearance("config.json", Some(FileObjectKind::File)).glyph,
            IconName::FileCog
        );
        assert_eq!(
            action_icon_appearance("folder-open", CandidateKind::Action).glyph,
            IconName::FolderOpen
        );
        assert_eq!(
            action_icon_appearance("copy-url", CandidateKind::Action).glyph,
            IconName::Link
        );
        assert_eq!(
            action_icon_appearance("git-diff", CandidateKind::Action).glyph,
            IconName::Diff
        );
        assert_eq!(
            action_icon_appearance("developer-test", CandidateKind::Action).glyph,
            IconName::FlaskConical
        );
    }

    #[test]
    fn permission_failures_open_the_matching_privacy_pane() {
        assert_eq!(
            privacy_pane_for_capture(
                ContextSource::FinderSelection,
                ContextCaptureStatus::PermissionDenied,
            ),
            Some(PrivacySettingsPane::Automation)
        );
        assert_eq!(
            privacy_pane_for_capture(
                ContextSource::BrowserCurrentPage,
                ContextCaptureStatus::PermissionDenied,
            ),
            Some(PrivacySettingsPane::Automation)
        );
        assert_eq!(
            privacy_pane_for_capture(
                ContextSource::SelectedText,
                ContextCaptureStatus::PermissionDenied,
            ),
            Some(PrivacySettingsPane::Accessibility)
        );
        assert_eq!(
            privacy_pane_for_capture(
                ContextSource::Clipboard,
                ContextCaptureStatus::PermissionDenied,
            ),
            Some(PrivacySettingsPane::Clipboard)
        );
        assert_eq!(
            privacy_pane_for_capture(ContextSource::SelectedText, ContextCaptureStatus::TimedOut,),
            None
        );
    }

    #[gpui_kit::test]
    fn settings_expose_privacy_recovery_actions(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.apply_state(
                    anywhere_protocol::LauncherState {
                        context_captures: vec![anywhere_protocol::ContextCapturePresentation {
                            text: None,
                            clipboard_age_ms: None,
                            source: ContextSource::FinderSelection,
                            status: ContextCaptureStatus::PermissionDenied,
                            captured_at_ms: 0,
                        }],
                        ..Default::default()
                    },
                    cx,
                );
                view.settings.selected = SettingsCategory::Context;
                view.screen = Screen::Settings;
                cx.notify();
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window
                .try_find("context-open-settings-FinderSelection")
                .is_some());

            view.update(cx, |view, cx| {
                view.settings.selected = SettingsCategory::Search;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.try_find("open-full-disk-access").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn file_search_uses_the_original_fixed_page_shape(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::FileSearch;
                view.model.query = "report".to_owned();
                view.model.rows = vec![file_row("report", "report.pdf")];
                cx.notify();
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("file-search-back").is_some());
            assert!(window.try_find("file-search-controls").is_some());
            assert!(window.try_find("file-search-input").is_some());
            assert!(window.try_find("launcher-mode-label").is_some());
            view.update(cx, |view, _| {
                assert_eq!(view.file_range_label(), "File Search");
                assert_eq!(view.desired_window_size().height, px(600.));
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn standalone_tasks_and_settings_keep_fixed_headers_separate(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                cx.notify();
            });
        });
        cx.update_window(window, |_, window, cx| window.render_frame(cx))
            .unwrap();
        let mut visual = VisualTestContext::from_window(window, cx);
        let tasks_header = visual
            .debug_bounds("tasks-toolbar")
            .expect("tasks toolbar bounds");
        assert!(
            tasks_header.origin.y >= px(metrics::TITLE_BAR_INSET),
            "the Tasks header must stay below native traffic lights"
        );

        visual.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Settings;
                view.standalone_tasks = false;
                view.settings_draft.launch_at_login = !view.preferences.launch_at_login;
                cx.notify();
            });
            window.render_frame(cx);
        });
        let title = visual
            .debug_bounds("settings-toolbar-title")
            .expect("settings title bounds");
        let actions = visual
            .debug_bounds("settings-toolbar-actions")
            .expect("settings action bounds");
        assert!(
            title.origin.x + title.size.width <= actions.origin.x,
            "the settings title must not overlap Save / Done"
        );
        let dirty = visual
            .debug_bounds("settings-dirty-label")
            .expect("settings dirty label bounds");
        assert!(
            dirty.size.width > px(120.),
            "the dirty status must render its full label instead of the old hard truncation"
        );
        assert!(
            dirty.origin.x + dirty.size.width <= actions.right(),
            "the dirty status must stay inside the toolbar actions"
        );
    }

    #[gpui_kit::test]
    fn content_mode_uses_the_extra_header_height_and_two_line_editor(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.input_mode = InputMode::Content;
                view.model.rows.clear();
                assert_eq!(view.desired_window_size().height, px(184.5));
                assert!(view.content_input.read(cx).is_multi_line());
                cx.notify();
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("launcher-content-source-icon").is_some());
            assert!(window.try_find("launcher-search-input-slot").is_some());
            assert!(window.try_find("launcher-result-heading").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn core_snapshot_inputs_are_reflected_in_the_active_editor(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.apply_state(
                    anywhere_protocol::LauncherState {
                        input: "report".to_owned(),
                        revision: 1,
                        ..Default::default()
                    },
                    cx,
                );
                assert_eq!(view.input.read(cx).value(), "report");

                view.input_mode = InputMode::Content;
                view.apply_state(
                    anywhere_protocol::LauncherState {
                        input: "captured text".to_owned(),
                        revision: 2,
                        ..Default::default()
                    },
                    cx,
                );
                assert_eq!(view.content_input.read(cx).value(), "captured text");
            });
        });
    }

    #[gpui_kit::test]
    fn result_heading_tracks_actions_and_menu_breadcrumbs(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, _| {
                view.model.set_query("git".to_owned());
                assert_eq!(
                    view.result_heading(),
                    None,
                    "ordinary search stays heading-free like the Flutter UI"
                );

                view.model.show_object_actions();
                assert_eq!(view.result_heading(), Some(("Actions".to_owned(), None)));
                view.model.hide_object_actions();

                view.model.rows = vec![LauncherRow {
                    id: "menu-item".to_owned(),
                    title: "Menu item".to_owned(),
                    subtitle: None,
                    shortcut: None,
                    icon: "menu".to_owned(),
                    kind: CandidateKind::Action,
                    enabled: true,
                    is_menu: true,
                    menu_path: vec!["View".to_owned(), "Layout".to_owned()],
                    command_id: None,
                    file_kind: None,
                    source_label: None,
                    icon_identity: None,
                    execution_context_id: None,
                    primary_action: false,
                }];
                assert!(view.model.open_selected_menu());
                assert_eq!(
                    view.result_heading(),
                    Some(("Menus › View › Layout".to_owned(), None))
                );
            });
        });
    }

    #[gpui_kit::test]
    fn command_k_browses_a_menu_when_no_object_actions_exist(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let command = KeyModifiers {
            command: true,
            modified: true,
            ..Default::default()
        };
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Launcher;
                view.model.rows = vec![LauncherRow {
                    id: "menu-item".to_owned(),
                    title: "Menu item".to_owned(),
                    subtitle: None,
                    shortcut: None,
                    icon: "menu".to_owned(),
                    kind: CandidateKind::Action,
                    enabled: true,
                    is_menu: true,
                    menu_path: vec!["View".to_owned()],
                    command_id: None,
                    file_kind: None,
                    source_label: None,
                    icon_identity: None,
                    execution_context_id: None,
                    primary_action: false,
                }];
                view.model.selected = 0;

                view.handle_key("k", command, window, cx);

                assert!(!view.model.object_actions_active);
                assert_eq!(view.model.menu_path(), Some(&["View".to_owned()][..]));
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn launcher_height_starts_compact_and_tracks_rows_and_scope_bar(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, _| {
                assert_eq!(
                    launcher_window_size().height,
                    px(metrics::PAGE_HEADER_HEIGHT)
                );

                view.model.rows.clear();
                view.model.scopes.clear();
                let empty_height = view.desired_window_size().height;
                assert_eq!(empty_height, px(72.));

                view.model.scopes.push(crate::launcher::LauncherScope {
                    id: "current-app".to_owned(),
                    label: "Current App".to_owned(),
                    is_current_app: true,
                });
                assert_eq!(
                    view.desired_window_size().height,
                    empty_height,
                    "a single Current App scope belongs in the header"
                );

                view.model.scopes.push(crate::launcher::LauncherScope {
                    id: "files".to_owned(),
                    label: "Files".to_owned(),
                    is_current_app: false,
                });
                assert_eq!(view.desired_window_size().height, px(108.5));

                view.model.scopes.clear();
                view.model.rows = (0..5)
                    .map(|index| file_row(&format!("row-{index}"), "Result"))
                    .collect();
                assert_eq!(view.desired_window_size().height, px(360.5));

                view.model.rows = (0..7)
                    .map(|index| file_row(&format!("row-{index}"), "Result"))
                    .collect();
                assert_eq!(view.desired_window_size().height, px(472.5));

                view.model.rows = (0..8)
                    .map(|index| file_row(&format!("row-{index}"), "Result"))
                    .collect();
                assert_eq!(view.desired_window_size().height, px(520.));
            });
        });
    }

    #[gpui_kit::test]
    fn launcher_resize_converges_for_loading_empty_and_changing_result_counts(
        cx: &mut TestAppContext,
    ) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.model.set_query("fixture".to_owned());
                view.model.rows = (0..5)
                    .map(|index| file_row(&format!("row-{index}"), "Result"))
                    .collect();
                view.model.loading = false;
                cx.notify();
            });
        });
        render_and_sync_test_viewport(cx, window, &view);
        assert_eq!(
            cx.update_window(window, |_, window, _| window.viewport_size().height)
                .unwrap(),
            px(360.5)
        );

        for rows in [1, 5] {
            cx.update(|cx| {
                view.update(cx, |view, cx| {
                    view.model.rows = (0..rows)
                        .map(|index| file_row(&format!("row-{index}"), "Result"))
                        .collect();
                    view.model.selected = 0;
                    cx.notify();
                });
            });
            render_and_sync_test_viewport(cx, window, &view);
            let expected = if rows == 1 { 136.5 } else { 360.5 };
            assert_eq!(
                cx.update_window(window, |_, window, _| window.viewport_size().height)
                    .unwrap(),
                px(expected)
            );
        }

        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.model.rows.clear();
                view.model.loading = true;
                cx.notify();
            });
        });
        render_and_sync_test_viewport(cx, window, &view);
        assert_eq!(
            cx.update_window(window, |_, window, _| window.viewport_size().height)
                .unwrap(),
            px(136.5)
        );

        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.model.loading = false;
                view.model.set_query(String::new());
                view.model.rows.clear();
                view.clear_notice();
                cx.notify();
            });
        });
        render_and_sync_test_viewport(cx, window, &view);
        assert_eq!(
            cx.update_window(window, |_, window, _| window.viewport_size().height)
                .unwrap(),
            px(72.)
        );
    }

    #[gpui_kit::test]
    fn preview_query_without_a_runtime_resizes_to_its_local_empty_state(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("launcher-search-input", cx);
            window.input("no-preview-match-zzzz", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let (bounds, desired, rows) = cx
            .update_window(window, |_, window, cx| {
                window.render_frame(cx);
                (
                    window.bounds().size,
                    view.read(cx).desired_window_size(),
                    view.read(cx).model.rows.len(),
                )
            })
            .unwrap();
        assert_eq!(rows, 0);
        assert_eq!(
            bounds, desired,
            "preview filtering must trigger the same native size update"
        );
        sync_test_viewport(cx, window, desired);
        assert_eq!(
            cx.update_window(window, |_, window, _| window.viewport_size().height)
                .unwrap(),
            desired.height
        );
    }

    #[gpui_kit::test]
    fn constrained_settings_viewport_fills_the_native_window_and_keeps_content_inside(
        cx: &mut TestAppContext,
    ) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Settings;
                view.settings.selected = SettingsCategory::General;
                cx.notify();
            });
        });
        let constrained = gpui_kit::size(px(648.), px(480.));
        sync_test_viewport(cx, window, constrained);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let surface = window.find("settings-surface").bounds();
            assert_eq!(surface.size.height, constrained.height);
            let content = window.find("settings-content-scroll-general").bounds();
            assert!(content.bottom() <= constrained.height);
            assert!(
                view.read(cx).settings_scrolls[SettingsCategory::General as usize]
                    .max_offset()
                    .y
                    >= px(0.)
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn current_app_moves_from_the_header_to_the_scope_bar(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("launcher-current-app-label").is_some());
            assert!(window.try_find("scope-all").is_none());

            view.update(cx, |view, cx| {
                view.model.scopes = vec![crate::launcher::LauncherScope {
                    id: "current-app".to_owned(),
                    label: "Current App".to_owned(),
                    is_current_app: true,
                }];
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.try_find("launcher-current-app-label").is_some());
            assert!(window.try_find("scope-all").is_none());

            view.update(cx, |view, cx| {
                view.model.scopes.push(crate::launcher::LauncherScope {
                    id: "files".to_owned(),
                    label: "Files".to_owned(),
                    is_current_app: false,
                });
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.try_find("launcher-current-app-label").is_none());
            assert!(window.try_find("scope-all").is_none());
            assert!(window.try_find("scope-files").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn search_mode_chooser_expands_inside_the_window_and_applies_keyboard_choice(
        cx: &mut TestAppContext,
    ) {
        let (window, view) = open_view(cx);
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("launcher-mode-toggle").expanded(), Some(false));
            window.click("launcher-mode-toggle", cx);
        })
        .unwrap();
        cx.run_until_parked();
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("launcher-mode-chooser").is_some());
            assert_eq!(window.find("launcher-mode-toggle").expanded(), Some(true));
            assert_eq!(
                window.find("launcher-navigation-environment").selected(),
                Some(true)
            );
            assert_eq!(
                window.find("launcher-navigation-search").selected(),
                Some(false)
            );
            view.update(cx, |view, _| {
                assert_eq!(view.desired_window_size().height, px(180.5));
            });
            window.press("down", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                window.find("launcher-navigation-search").selected(),
                Some(true)
            );
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("launcher-mode-chooser").is_none());
            view.update(cx, |view, _| {
                assert_eq!(view.input_mode, InputMode::Search);
                assert!(!view.root_navigation_open);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn mode_chooser_resizes_compact_window_and_keeps_both_languages_inside_rows(
        cx: &mut TestAppContext,
    ) {
        let (handle, view) = open_view(cx);
        for locale in [Locale::En, Locale::Zh] {
            cx.update(|cx| {
                view.update(cx, |view, cx| {
                    view.locale = locale;
                    view.root_navigation_open = false;
                    view.model.rows.clear();
                    view.model.scopes.clear();
                    view.model.loading = false;
                    view.clear_notice();
                    cx.notify();
                });
            });
            sync_test_viewport_to_desired(cx, handle, &view);
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                window.click("launcher-mode-toggle", cx);
            })
            .unwrap();
            cx.run_until_parked();
            sync_test_viewport_to_desired(cx, handle, &view);
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                assert_eq!(window.viewport_size().height, px(180.5));
                let header = window.find("launcher-search-header").bounds();
                let chooser = window.find("launcher-mode-chooser").bounds();
                assert!(chooser.top() >= header.bottom());
                assert!(chooser.bottom() <= px(180.5));
                for id in [
                    "launcher-navigation-search",
                    "launcher-navigation-environment",
                ] {
                    let row = window.find(id).bounds();
                    let title = window.find(format!("{id}-title")).bounds();
                    let detail = window.find(format!("{id}-detail")).bounds();
                    assert_eq!(row.size.height, px(44.));
                    assert!(title.top() >= row.top());
                    assert!(detail.top() >= title.bottom());
                    assert!(detail.bottom() <= row.bottom());
                }
            })
            .unwrap();
        }
    }

    #[gpui_kit::test]
    fn long_candidate_text_does_not_compress_rows_or_cover_primary_action(cx: &mut TestAppContext) {
        let (handle, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.model.rows = (0..20)
                    .map(|i| {
                        let mut row =
                            file_row(&format!("long-{i}"), &"长文件名称 with spaces ".repeat(15));
                        row.subtitle = Some("/very/long/目录/path/".repeat(30));
                        row.shortcut = Some("⌃⌥⇧⌘F12".into());
                        row.enabled = true;
                        row
                    })
                    .collect();
                view.model.selected = 0;
                view.model.loading = false;
                view.clear_notice();
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, handle, &view);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let first = window.find("candidate-0").bounds();
            let second = window.find("candidate-1").bounds();
            let title = window.find("candidate-0-title").bounds();
            let subtitle = window.find("candidate-0-subtitle").bounds();
            let action = window.find("candidate-0-primary").bounds();
            assert_eq!(first.size.height, px(LAUNCHER_ROW_HEIGHT));
            assert_eq!(second.size.height, px(LAUNCHER_ROW_HEIGHT));
            assert!(first.bottom() <= second.top());
            assert!(title.right() <= action.left());
            assert!(subtitle.right() <= action.left());
            assert!(title.bottom() <= subtitle.top());
            assert!(subtitle.bottom() <= first.bottom());
        })
        .unwrap();
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::FileSearch;
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, handle, &view);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let header = window.find("file-search-header").bounds();
            let filters = window.find("file-search-controls").bounds();
            assert_eq!(header.size.height, px(metrics::PAGE_HEADER_HEIGHT));
            assert_eq!(filters.size.height, px(metrics::PAGE_TOOLBAR_HEIGHT));
            assert!(filters.top() >= header.bottom());
            assert_eq!(
                window.find("candidate-0").bounds().size.height,
                px(LAUNCHER_ROW_HEIGHT)
            );
            assert!(window.find("candidate-0").bounds().top() >= filters.bottom());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn file_search_selection_reveals_rows_in_a_long_tracked_list(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::FileSearch;
                view.model.rows = (0..20)
                    .map(|index| file_row(&format!("file-{index}"), &format!("file-{index}.txt")))
                    .collect();
                view.model.selected = 0;
                view.file_search.browsing = true;
                view.model.loading = false;
                view.notice = None;
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let initial_maximum = view.read(cx).file_search_scroll.max_offset().y;
            assert!(
                initial_maximum > px(0.),
                "a long result list must expose a real scroll range"
            );
            view.update(cx, |view, cx| {
                for _ in 0..19 {
                    view.select_file_search(1, cx);
                }
                assert_eq!(view.model.selected, 19);
            });
            window.render_frame(cx);
            let scroll = &view.read(cx).file_search_scroll;
            assert!(
                scroll.offset().y < px(0.),
                "revealing the selected row must scroll the tracked list"
            );
            let selected = window.find("candidate-19").bounds();
            assert!(selected.bottom() <= scroll.bounds().bottom());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn tasks_output_follow_tracks_the_shared_scroll_handle(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let row = crate::host::ExecutionRow {
            id: "streaming".to_owned(),
            title: "Streaming output".to_owned(),
            directory: "/tmp/fixture".to_owned(),
            command: "printf output".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Running,
            output_revision: 1,
            exit_code: None,
            started_at_ms: 0,
            finished_at_ms: None,
            output_truncated: false,
            error: None,
        };
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                view.tasks.apply_rows(vec![row.clone()]);
                view.tasks.detail = Some(crate::host::ExecutionDetail {
                    row,
                    stdout: (0..200)
                        .map(|index| format!("output line {index}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    stderr: String::new(),
                });
                view.tasks.request_follow_output();
                view.notice = None;
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.simulate_next_frame(cx);
            window.render_frame(cx);
            window.simulate_next_frame(cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.update_window(window, |_, _, cx| {
            let scroll = &view.read(cx).tasks_output_scroll;
            let maximum = scroll.max_offset().y;
            assert!(
                maximum > px(0.),
                "long task output must expose a real scroll range"
            );
            assert_eq!(scroll.offset().y, -maximum);
            assert!(view.read(cx).tasks.follow_output);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn settings_shortcut_closes_mode_chooser_and_ignores_queued_launcher_resize(
        cx: &mut TestAppContext,
    ) {
        let (handle, view) = open_view(cx);
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click("launcher-mode-toggle", cx);
            window.press("cmd-,", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(view.read(cx).screen, Screen::Settings);
            assert!(!view.read(cx).root_navigation_open);
            assert_eq!(window.bounds().size.height, px(SETTINGS_HEIGHT));
            assert!(window.try_find("settings-surface").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn selected_file_row_exposes_the_original_preview_affordance(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::FileSearch;
                view.file_search.browsing = true;
                view.model.rows = vec![file_row("file-a", "a.txt")];
                view.model.selected = 0;
                cx.notify();
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("candidate-0-preview").is_some());
            assert!(window.try_find("candidate-0-actions").is_none());
            assert!(window.try_find("candidate-0-primary").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn menu_navigation_replaces_the_mode_control_with_back(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.model.rows = vec![LauncherRow {
                    id: "menu-item".to_owned(),
                    title: "Menu item".to_owned(),
                    subtitle: None,
                    shortcut: None,
                    icon: "menu".to_owned(),
                    kind: CandidateKind::Action,
                    enabled: true,
                    is_menu: true,
                    menu_path: Vec::new(),
                    command_id: None,
                    file_kind: None,
                    source_label: None,
                    icon_identity: None,
                    execution_context_id: None,
                    primary_action: false,
                }];
                assert!(view.model.open_selected_menu());
                cx.notify();
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("launcher-back").is_some());
            assert!(window.try_find("launcher-mode-toggle").is_none());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn only_the_transient_main_launcher_dismisses_on_focus_loss(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, _| {
                assert!(view.dismiss_on_focus_loss());

                view.screen = Screen::FileSearch;
                assert!(view.dismiss_on_focus_loss());

                view.preview_open = true;
                assert!(!view.dismiss_on_focus_loss());
                view.preview_open = false;
                view.preview_busy = true;
                assert!(!view.dismiss_on_focus_loss());
                view.preview_busy = false;

                view.screen = Screen::Settings;
                assert!(!view.dismiss_on_focus_loss());

                view.screen = Screen::WebPackages;
                assert!(!view.dismiss_on_focus_loss());

                view.screen = Screen::Launcher;
                view.standalone_shelf = true;
                assert!(!view.dismiss_on_focus_loss());
            });
        });
    }

    #[gpui_kit::test]
    fn reopening_content_focuses_the_visible_editor(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Launcher;
                view.input_mode = InputMode::Content;
                view.input.update(cx, |input, cx| input.focus(window, cx));
                assert!(!view.active_editor_focused(window, cx));
                view.refocus_active_editor(window, cx);
                assert!(view
                    .content_input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window));
                assert!(view.active_editor_focused(window, cx));
                view.input_mode = InputMode::Search;
                view.refocus_active_editor(window, cx);
                assert!(view.input.read(cx).focus_handle(cx).is_focused(window));
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn clipboard_confirmation_blocks_shortcuts_and_keeps_its_target(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ClipboardShelf;
                view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                    recording: false,
                    recording_enabled: false,
                    available: true,
                    paused_until_ms: None,
                    retention_days: 7,
                    stored_entries: 2,
                    stored_bytes: 2,
                    notice: None,
                    items: (1..=2)
                        .map(|id| crate::host::ClipboardItem {
                            id,
                            summary: id.to_string(),
                            kind: "text".into(),
                            copied_at_ms: 0,
                            byte_count: 1,
                        })
                        .collect(),
                });
                view.request_delete_clipboard_item(1, cx);
                view.handle_key(
                    "down",
                    KeyModifiers {
                        command: true,
                        modified: true,
                        ..Default::default()
                    },
                    window,
                    cx,
                );
                assert_eq!(view.clipboard.selected_item().unwrap().id, 1);
                view.clipboard.selected = 1; // A refreshed snapshot changes selection.
                view.confirm_clipboard_delete(cx);
                assert_eq!(view.clipboard.selected_item().unwrap().id, 1);
                view.clipboard.status.as_mut().unwrap().items.remove(0);
                view.clipboard.selected = 0;
                view.confirm_clipboard_delete(cx); // Missing target must not authorize item 2.
                assert_eq!(view.clipboard.operation_generation, 0);
                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert!(view.clipboard.delete_pending.is_none());
                view.clipboard.delete_pending = Some(1);
                view.screen = Screen::Launcher;
                view.root_navigation_open = true;
                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert!(!view.root_navigation_open);
                assert_eq!(view.clipboard.delete_pending, Some(1));
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn hiding_the_shelf_invalidates_pending_clipboard_operations(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.clipboard.operation_generation = 7;
                view.prepare_hide(cx);
                assert_eq!(view.clipboard.operation_generation, 8);
            });
        });
    }

    #[gpui_kit::test]
    fn invalid_context_binding_keeps_recording(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Settings;
                for key in ["1", "c"] {
                    view.shortcut_recording = Some(ShortcutSlot::ContextShortcut);
                    view.handle_key(
                        key,
                        KeyModifiers {
                            command: true,
                            modified: true,
                            ..Default::default()
                        },
                        window,
                        cx,
                    );
                    assert_eq!(view.shortcut_recording, Some(ShortcutSlot::ContextShortcut));
                    assert!(view.notice.is_some());
                }
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn modified_navigation_preserves_history_selection(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ApplicationHistory;
                view.app_history.items = (0..3)
                    .map(|index| crate::host::AppHistoryItem {
                        id: index.to_string(),
                        object_key: index.to_string(),
                        source_id: "code".into(),
                        title: "Project".into(),
                        value: "/tmp/project".into(),
                        kind: "directory",
                        can_open: true,
                    })
                    .collect();
                for modifiers in [
                    KeyModifiers {
                        shift: true,
                        modified: true,
                        ..Default::default()
                    },
                    KeyModifiers {
                        alt: true,
                        modified: true,
                        ..Default::default()
                    },
                    KeyModifiers {
                        command: true,
                        modified: true,
                        ..Default::default()
                    },
                    KeyModifiers {
                        control: true,
                        modified: true,
                        ..Default::default()
                    },
                ] {
                    view.handle_key("down", modifiers, window, cx);
                    assert_eq!(view.app_history.selected, 0);
                }
                view.handle_key("down", KeyModifiers::default(), window, cx);
                assert_eq!(view.app_history.selected, 1);
                view.handle_key("pageup", KeyModifiers::default(), window, cx);
                assert_eq!(view.app_history.selected, 0);
                view.app_history.selected = 2;
                view.handle_key("pagedown", KeyModifiers::default(), window, cx);
                assert_eq!(view.app_history.selected, 2);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn application_history_escape_clears_the_query_before_leaving(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ApplicationHistory;
                view.app_history.set_query("project".to_owned());
                set_input_value(&view.history_input, "project", window, cx);
                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert_eq!(view.screen, Screen::ApplicationHistory);
                assert_eq!(view.history_input.read(cx).value(), "");
                assert!(view.app_history.query.is_empty());
                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert_eq!(view.screen, Screen::Launcher);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn application_history_escape_control_uses_the_same_query_chain(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ApplicationHistory;
                view.app_history.set_query("project".to_owned());
                set_input_value(&view.history_input, "project", window, cx);
                cx.notify();
            });
        })
        .unwrap();
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("page-escape", cx);
        })
        .unwrap();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.screen, Screen::ApplicationHistory);
            assert_eq!(view.history_input.read(cx).value(), "");
            assert!(view.app_history.query.is_empty());
        });
    }

    #[gpui_kit::test]
    fn opening_application_history_starts_with_a_fresh_query_and_rows(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.app_history.set_query("old query".to_owned());
                view.app_history.items = vec![crate::host::AppHistoryItem {
                    id: "old".to_owned(),
                    object_key: "old".to_owned(),
                    source_id: "old-source".to_owned(),
                    title: "Old source".to_owned(),
                    value: "/tmp/old".to_owned(),
                    kind: "file",
                    can_open: true,
                }];
                set_input_value(&view.history_input, "old query", window, cx);
                view.screen = Screen::Launcher;
                view.open_screen(Screen::ApplicationHistory, window, cx);

                assert_eq!(view.screen, Screen::ApplicationHistory);
                assert_eq!(view.history_input.read(cx).value(), "");
                assert!(view.app_history.query.is_empty());
                assert!(view.app_history.items.is_empty());
                assert!(view.app_history.loading);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn application_history_does_not_bind_command_k_to_actions(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let command = KeyModifiers {
            command: true,
            modified: true,
            ..Default::default()
        };
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ApplicationHistory;
                view.app_history.items = vec![crate::host::AppHistoryItem {
                    id: "remote".to_owned(),
                    object_key: "remote:item".to_owned(),
                    source_id: "browser".to_owned(),
                    title: "Remote".to_owned(),
                    value: "https://example.com".to_owned(),
                    kind: "remote",
                    can_open: false,
                }];

                view.handle_key("k", command, window, cx);
                assert!(!view.app_history.fallback_actions_open);
                assert!(!view.app_history.object_actions_active);

                view.app_history.items.clear();
                view.handle_key("k", command, window, cx);
                assert!(!view.app_history.empty_actions_open);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn application_history_does_not_bind_command_r_to_refresh(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let command = KeyModifiers {
            command: true,
            modified: true,
            ..Default::default()
        };
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ApplicationHistory;
                view.app_history.session_generation = 7;
                view.app_history.query_generation = 11;

                view.handle_key("r", command, window, cx);

                assert_eq!(view.app_history.session_generation, 7);
                assert_eq!(view.app_history.query_generation, 11);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn application_history_renders_and_dismisses_object_actions(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, _| {
                view.screen = Screen::ApplicationHistory;
                view.app_history.apply(crate::host::AppHistorySnapshot {
                    source_id: "code".to_owned(),
                    items: vec![crate::host::AppHistoryItem {
                        id: "item".to_owned(),
                        object_key: "code:item".to_owned(),
                        source_id: "code".to_owned(),
                        title: "Project".to_owned(),
                        value: "/tmp/project".to_owned(),
                        kind: "directory",
                        can_open: true,
                    }],
                    failed_sources: vec![],
                    notice: None,
                });
                let request = view
                    .app_history
                    .begin_object_actions_request("item".to_owned(), view.model.revision);
                assert!(view.app_history.apply_object_actions(
                    request,
                    "item",
                    view.model.revision,
                    vec![crate::host::AppHistoryObjectAction {
                        id: "copy-path".to_owned(),
                        title: "Copy Path".to_owned(),
                        localized_titles: Default::default(),
                        enabled: true,
                    }],
                ));
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        let viewport = cx
            .update_window(window, |_, window, cx| {
                window.render_frame(cx);
                assert!(window.try_find("history-object-action-0").is_some());
                assert!(window.try_find("history-object-action-refresh").is_some());
                window.viewport_size()
            })
            .unwrap();
        let mut visual = VisualTestContext::from_window(window, cx);
        let panel = visual
            .debug_bounds("history-object-actions")
            .expect("history object actions panel bounds");
        assert_eq!(panel.size.width, px(300.));
        assert!(
            panel.left() >= px(0.) && panel.right() >= viewport.width - px(16.),
            "panel={panel:?} viewport={viewport:?}"
        );
        assert!(
            panel.bottom() <= viewport.height,
            "panel={panel:?} viewport={viewport:?}"
        );
        visual.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.handle_key(
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
            });
            window.render_frame(cx);
            assert!(window.try_find("history-object-action-0").is_none());
        });
    }

    #[gpui_kit::test]
    fn application_history_object_actions_keep_copy_and_refresh_fallbacks(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, _| {
                view.screen = Screen::ApplicationHistory;
                view.app_history.apply(crate::host::AppHistorySnapshot {
                    source_id: "code".to_owned(),
                    items: vec![crate::host::AppHistoryItem {
                        id: "item".to_owned(),
                        object_key: "code:item".to_owned(),
                        source_id: "code".to_owned(),
                        title: "Project".to_owned(),
                        value: "/tmp/project".to_owned(),
                        kind: "directory",
                        can_open: true,
                    }],
                    failed_sources: vec![],
                    notice: None,
                });
                let request = view
                    .app_history
                    .begin_object_actions_request("item".to_owned(), view.model.revision);
                assert!(view.app_history.apply_object_actions(
                    request,
                    "item",
                    view.model.revision,
                    Vec::new(),
                ));
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("history-object-action-copy").is_some());
            assert!(window.try_find("history-object-action-refresh").is_some());
            view.update(cx, |view, cx| {
                view.handle_key("down", KeyModifiers::default(), window, cx);
                assert_eq!(view.app_history.object_action_selected, 1);
                view.handle_key("down", KeyModifiers::default(), window, cx);
                assert_eq!(view.app_history.object_action_selected, 0);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn empty_application_history_exposes_refresh_actions(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ApplicationHistory;
                view.app_history.items.clear();
                view.app_history.empty_actions_open = false;
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("app-history-empty-actions").is_some());
            window.click("app-history-empty-actions", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("app-history-empty-refresh").is_some());
            view.update(cx, |view, cx| {
                view.history_input
                    .update(cx, |input, cx| input.focus(window, cx));
            });
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(!view.read(cx).app_history.empty_actions_open);
        });
    }

    #[gpui_kit::test]
    fn non_openable_application_history_exposes_copy_and_refresh(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ApplicationHistory;
                view.app_history.items = vec![crate::host::AppHistoryItem {
                    id: "remote".to_owned(),
                    object_key: "remote:item".to_owned(),
                    source_id: "browser".to_owned(),
                    title: "Remote".to_owned(),
                    value: "https://example.com".to_owned(),
                    kind: "remote",
                    can_open: false,
                }];
                view.app_history.selected = 0;
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("history-actions-remote", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window
                .try_find("app-history-fallback-actions-panel")
                .is_some());
            assert!(window.try_find("app-history-fallback-action-0").is_some());
            assert!(window.try_find("app-history-fallback-action-1").is_some());
            view.update(cx, |view, cx| {
                view.handle_key("down", KeyModifiers::default(), window, cx);
                assert_eq!(view.app_history.fallback_action_selected, 1);
                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert!(!view.app_history.fallback_actions_open);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn clipboard_actions_panel_stays_inside_the_page(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let viewport = cx
            .update_window(window, |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.screen = Screen::Clipboard;
                    view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                        recording: true,
                        recording_enabled: true,
                        available: true,
                        paused_until_ms: None,
                        retention_days: 7,
                        stored_entries: 1,
                        stored_bytes: 4,
                        notice: None,
                        items: vec![crate::host::ClipboardItem {
                            id: 1,
                            summary: "fixture".into(),
                            kind: "text".into(),
                            copied_at_ms: 0,
                            byte_count: 4,
                        }],
                    });
                    view.clipboard.browsing = true;
                    view.clipboard.actions_open = true;
                    view.clipboard.action_target = Some(1);
                    cx.notify();
                });
                window.render_frame(cx);
                assert!(window.try_find("clipboard-action-row-0").is_some());
                window.viewport_size()
            })
            .unwrap();
        let mut visual = VisualTestContext::from_window(window, cx);
        let panel = visual
            .debug_bounds("clipboard-actions")
            .expect("clipboard actions panel bounds");
        assert_eq!(panel.size.width, px(300.));
        assert!(
            panel.left() >= px(0.) && panel.right() >= viewport.width - px(16.),
            "panel={panel:?} viewport={viewport:?}"
        );
        assert!(
            panel.bottom() <= viewport.height,
            "panel={panel:?} viewport={viewport:?}"
        );
    }

    #[gpui_kit::test]
    fn empty_clipboard_pages_expose_refresh_without_a_target(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Clipboard;
                view.clipboard.status = None;
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("history-empty-actions").is_some());
        })
        .unwrap();
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.open_empty_clipboard_actions(cx);
                assert!(view.clipboard.actions_open);
                assert_eq!(view.clipboard.action_target, None);
                assert_eq!(view.clipboard.action_selected, 3);
                view.move_clipboard_action(-1, 4, cx);
                assert_eq!(view.clipboard.action_selected, 3);
                view.move_clipboard_action(1, 4, cx);
                assert_eq!(view.clipboard.action_selected, 3);
                view.execute_clipboard_action(3, cx);
                assert!(!view.clipboard.actions_open);

                view.screen = Screen::ClipboardShelf;
                view.standalone_shelf = true;
                view.open_empty_clipboard_actions(cx);
                assert_eq!(view.clipboard.action_selected, 4);
                view.move_clipboard_action(-1, 5, cx);
                assert_eq!(view.clipboard.action_selected, 4);
                view.move_clipboard_action(1, 5, cx);
                assert_eq!(view.clipboard.action_selected, 4);
                view.execute_clipboard_action(4, cx);
                assert!(!view.clipboard.actions_open);
                assert_eq!(view.clipboard.action_selected, 0);
            });
        });
    }

    #[gpui_kit::test]
    fn tasks_tool_options_stay_inside_the_page(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let viewport = cx
            .update_window(window, |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.screen = Screen::Tasks;
                    view.standalone_tasks = true;
                    view.tasks.tool_options_open = true;
                    cx.notify();
                });
                window.render_frame(cx);
                assert!(
                    view.read(cx).tasks.tool_options_open,
                    "the render must preserve the open tool options state"
                );
                window.viewport_size()
            })
            .unwrap();
        let mut visual = VisualTestContext::from_window(window, cx);
        assert!(
            visual.debug_bounds("tasks-toolbar").is_some(),
            "the Tasks toolbar must render"
        );
        let panel = visual
            .debug_bounds("tasks-tool-options-panel")
            .expect("tasks tool options panel bounds");
        assert_eq!(panel.size.width, px(210.));
        assert!(
            panel.left() >= px(0.) && panel.right() >= viewport.width - px(22.),
            "panel={panel:?} viewport={viewport:?}"
        );
        assert!(
            panel.top() >= px(metrics::TITLE_BAR_INSET) && panel.bottom() <= viewport.height,
            "panel={panel:?} viewport={viewport:?}"
        );
    }

    #[gpui_kit::test]
    fn tasks_actions_stay_inside_the_page(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let row = crate::host::ExecutionRow {
            id: "task".to_owned(),
            title: "npm Run · task".to_owned(),
            directory: "/tmp/project".to_owned(),
            command: "npm run task".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Succeeded,
            output_revision: 1,
            exit_code: Some(0),
            started_at_ms: 0,
            finished_at_ms: Some(1),
            output_truncated: false,
            error: None,
        };
        let viewport = cx
            .update_window(window, |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.screen = Screen::Tasks;
                    view.standalone_tasks = true;
                    view.tasks.apply_rows(vec![row.clone()]);
                    view.tasks.detail = Some(crate::host::ExecutionDetail {
                        row,
                        stdout: "ready".to_owned(),
                        stderr: String::new(),
                    });
                    view.tasks.task_actions_open = true;
                    cx.notify();
                });
                window.render_frame(cx);
                assert!(window.try_find("task-more-actions").is_some());
                window.viewport_size()
            })
            .unwrap();
        let mut visual = VisualTestContext::from_window(window, cx);
        let panel = visual
            .debug_bounds("tasks-actions")
            .expect("tasks actions panel bounds");
        assert_eq!(panel.size.width, px(210.));
        assert!(
            panel.left() >= px(0.) && panel.right() >= viewport.width - px(22.),
            "panel={panel:?} viewport={viewport:?}"
        );
        assert!(
            panel.top() >= px(metrics::TITLE_BAR_INSET) && panel.bottom() <= viewport.height,
            "panel={panel:?} viewport={viewport:?}"
        );
    }

    #[gpui_kit::test]
    fn search_updates_rows_and_keyboard_selection_in_the_real_input(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("launcher-search-input", cx);
            window.input("Git", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            let view = view.read(cx);
            assert_eq!(view.model.query, "Git");
            assert_eq!(view.model.rows.len(), 1);
            assert_eq!(view.model.selected_row().unwrap().id, "git-status");
            assert_eq!(
                window.find("candidate-0").label(),
                Some("Git Status"),
                "candidate rows expose their semantic label"
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn file_search_requires_navigation_before_actions_and_space_preview(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::FileSearch;
                view.model.rows = vec![
                    file_row("file-a", "a.txt"),
                    file_row("file-b", "b.txt"),
                    file_row("file-c", "c.txt"),
                ];
                view.model.selected = 0;
                view.file_search.browsing = false;

                assert!(view.selected_row_for_action().is_none());
                assert!(!view.file_search_selection_active());
                assert!(!view.can_preview_selected_file());
                view.activate_selection(cx);
                view.show_object_actions(cx);
                assert!(view.notice.is_none());
                assert!(!view.model.object_actions_active);

                view.select_file_search(1, cx);
                assert!(view.file_search.browsing);
                assert_eq!(view.model.selected, 0);
                assert_eq!(
                    view.selected_row_for_action().map(|row| row.id.as_str()),
                    Some("file-a")
                );
                assert!(view.can_preview_selected_file());

                view.reset_file_search_browsing(cx);
                assert!(!view.file_search.browsing);
                view.select_file_search(-1, cx);
                assert_eq!(view.model.selected, 0);
            });
        });
    }

    #[gpui_kit::test]
    fn file_search_space_only_previews_after_navigation_and_escape_closes_preview(
        cx: &mut TestAppContext,
    ) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, _| {
                view.screen = Screen::FileSearch;
                view.model.rows = vec![file_row("file-a", "a.txt")];
                view.model.selected = 0;
                view.file_search.browsing = false;
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("file-search-input", cx);
            window.input("guide", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.input.read(cx).value().as_ref(), "guide");
            assert_eq!(view.model.query, "guide");
        });

        cx.update_window(window, |_, window, cx| {
            window.press("space", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(
                view.input.read(cx).value().as_ref(),
                "guide ",
                "space remains query text before the file list is browsed"
            );
            assert_eq!(view.model.query, "guide ");
        });

        cx.update(|cx| {
            view.update(cx, |view, _| {
                view.file_search.browsing = true;
                view.model.rows = vec![file_row("file-a", "a.txt")];
                view.model.selected = 0;
            });
            assert!(view.read(cx).file_search_selection_active());
        });
        cx.update_window(window, |_, window, cx| {
            window.press("space", cx);
        })
        .unwrap();
        cx.update(|cx| {
            assert_eq!(
                view.read(cx).input.read(cx).value().as_ref(),
                "guide ",
                "space preview must not be inserted into the query"
            );
            assert_eq!(view.read(cx).model.query, "guide ");
        });

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.input
                    .update(cx, |input, cx| input.insert(" ", window, cx));
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(
                view.input.read(cx).value().as_ref(),
                "guide ",
                "an IME-delivered space must be rolled back before previewing"
            );
            assert_eq!(view.model.query, "guide ");
            assert!(view.file_search.browsing);
        });

        cx.update(|cx| {
            view.update(cx, |view, _| view.preview_open = true);
        });
        cx.update_window(window, |_, window, cx| {
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(!view.preview_open);
            assert_eq!(view.screen, Screen::FileSearch);
            assert_eq!(
                view.input.read(cx).value().as_ref(),
                "guide ",
                "escape closes preview before changing the query"
            );
        });

        cx.update(|cx| {
            view.update(cx, |view, _| view.file_search.browsing = true);
        });
        cx.update_window(window, |_, window, cx| {
            window.input("x", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(!view.file_search.browsing);
            assert_eq!(view.model.selected, 0);
            assert_eq!(view.model.query, "guide x");
        });
    }

    #[gpui_kit::test]
    fn leaving_settings_keeps_resumes_or_restarts_the_launcher_stream(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, _| {
                view.live_state.session_id = Some(7);
                view.retained_workspace = true;
                view.hidden_at = Some(Instant::now());
                assert_eq!(
                    view.settings_launcher_recovery(),
                    LauncherStreamRecovery::Resume
                );

                let (cancel, _cancelled) = oneshot::channel();
                view.live_state.cancel = Some(cancel);
                assert_eq!(
                    view.settings_launcher_recovery(),
                    LauncherStreamRecovery::KeepActive
                );

                view.live_state.cancel = None;
                view.hidden_at = Some(
                    Instant::now()
                        - Duration::from_millis(
                            anywhere_protocol::policy::LAUNCHER_RETENTION_MS + 1,
                        ),
                );
                assert_eq!(
                    view.settings_launcher_recovery(),
                    LauncherStreamRecovery::Restart
                );

                view.hidden_at = None;
                view.retained_workspace = false;
                view.live_state.session_id = None;
                assert_eq!(
                    view.settings_launcher_recovery(),
                    LauncherStreamRecovery::Restart
                );
            });
        });
    }

    #[gpui_kit::test]
    fn settings_edits_are_dirty_until_saved_or_discarded(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            view.update(cx, |view, cx| view.open_settings(window, cx));
            window.render_frame(cx);
            assert!(window.try_find("settings-surface").is_some());
            window.click("settings-category-general", cx);
            window.render_frame(cx);
            let login = window.find("open-at-login");
            assert_eq!(login.checked(), Some(false));
            window.click("open-at-login", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                assert!(view.settings_draft.launch_at_login);
                assert!(!view.preferences.launch_at_login);
                assert!(view.settings_dirty(cx));
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("open-at-login").checked(), Some(true));
            window.click("save-settings", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                assert!(view.preferences.launch_at_login);
                assert!(!view.settings_dirty(cx));
            });
        });

        cx.update_window(window, |_, window, cx| {
            window.click("open-at-login", cx);
            window.render_frame(cx);
            window.click("done-settings", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(view.settings_close_pending);
            assert_eq!(view.screen, Screen::Settings);
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("discard-settings", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.screen, Screen::Launcher);
            assert!(view.preferences.launch_at_login);
            assert!(view.settings_draft.launch_at_login);
        });
    }

    #[gpui_kit::test]
    fn settings_pointer_navigation_does_not_leave_general_keyboard_focused(
        cx: &mut TestAppContext,
    ) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            view.update(cx, |view, cx| view.open_settings(window, cx));
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(view.read(cx).settings_focus_handle.is_focused(window));
            window.click("settings-category-shortcuts", cx);
            window.render_frame(cx);
            assert_eq!(view.read(cx).settings.selected, SettingsCategory::Shortcuts);
            assert!(view.read(cx).settings_focus_handle.is_focused(window));
            assert_ne!(
                window.find("settings-category-general").focused(),
                Some(true)
            );
            view.update(cx, |view, cx| view.focus_settings_navigation(window, cx));
            assert!(view.read(cx).settings_focus_handle.is_focused(window));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn repeated_space_does_not_toggle_an_open_file_preview(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, _| {
                view.screen = Screen::FileSearch;
                view.model.rows = vec![file_row("file-a", "a.txt")];
                view.model.selected = 0;
                view.file_search.browsing = true;
                view.preview_open = true;
            });
        });
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                assert!(view.handle_preview_keystroke("space", false, false, true, window, cx));
                assert!(view.preview_open, "auto-repeat must not close the preview");
                assert!(view.handle_preview_keystroke("space", false, false, false, window, cx));
                assert!(!view.preview_open, "a new key-down closes the preview");
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn command_y_toggles_file_preview_once_and_ignores_repeat(cx: &mut TestAppContext) {
        use gpui_kit::test::TestEventExt;

        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, _| {
                view.screen = Screen::FileSearch;
                view.model.rows = vec![file_row("file-a", "a.txt")];
                view.model.selected = 0;
                view.file_search.browsing = true;
                view.preview_open = true;
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.key_down("cmd-y", false, cx);
        })
        .unwrap();
        cx.update(|cx| {
            assert!(!view.read(cx).preview_open);
            view.update(cx, |view, _| {
                view.preview_open = true;
                crate::platform::set_key_event_is_repeat_for_test(true);
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.key_down("cmd-y", true, cx);
        })
        .unwrap();
        cx.update(|cx| {
            crate::platform::set_key_event_is_repeat_for_test(false);
            assert!(
                view.read(cx).preview_open,
                "held command-y must not toggle the preview twice"
            );
        });
    }

    #[gpui_kit::test]
    fn command_y_closes_a_clipboard_preview_and_command_l_restores_search(cx: &mut TestAppContext) {
        use gpui_kit::test::TestEventExt;

        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, _| {
                view.screen = Screen::Clipboard;
                view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                    recording: false,
                    recording_enabled: false,
                    available: true,
                    paused_until_ms: None,
                    retention_days: 7,
                    stored_entries: 1,
                    stored_bytes: 4,
                    notice: None,
                    items: vec![crate::host::ClipboardItem {
                        id: 1,
                        summary: "fixture".into(),
                        kind: "text".into(),
                        copied_at_ms: 0,
                        byte_count: 4,
                    }],
                });
                view.clipboard.browsing = true;
                view.clipboard.selected = 0;
                view.preview_open = true;
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.key_down("cmd-y", false, cx);
        })
        .unwrap();
        cx.update(|cx| {
            assert!(!view.read(cx).preview_open);
            view.update(cx, |view, _| {
                view.screen = Screen::FileSearch;
                view.model.rows = vec![file_row("file-a", "a.txt")];
                view.model.selected = 0;
                view.file_search.browsing = true;
                view.preview_open = true;
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.key_down("cmd-l", false, cx);
        })
        .unwrap();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(!view.preview_open);
            assert_eq!(view.screen, Screen::FileSearch);
        });
    }

    #[gpui_kit::test]
    fn space_and_enter_activate_a_focused_page_button(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, _| {
                view.screen = Screen::Clipboard;
                view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                    recording: false,
                    recording_enabled: false,
                    available: true,
                    paused_until_ms: None,
                    retention_days: 7,
                    stored_entries: 1,
                    stored_bytes: 4,
                    notice: None,
                    items: vec![crate::host::ClipboardItem {
                        id: 1,
                        summary: "fixture".into(),
                        kind: "text".into(),
                        copied_at_ms: 0,
                        byte_count: 4,
                    }],
                });
                view.clipboard.browsing = true;
                view.clipboard.selected = 0;
                view.preview_open = true;
            });
        });
        let mut focused = false;
        for _ in 0..16 {
            cx.update_window(window, |_, window, cx| {
                window.render_frame(cx);
                window.focus_next(cx);
            })
            .unwrap();
            focused = cx
                .update_window(window, |_, window, cx| {
                    window.render_frame(cx);
                    window.find("page-back").focused() == Some(true)
                })
                .unwrap();
            if focused {
                break;
            }
        }
        assert!(focused, "the page back button must be reachable by Tab");
        cx.update_window(window, |_, window, cx| {
            window.press("space", cx);
        })
        .unwrap();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(
                view.screen,
                Screen::Launcher,
                "Space must activate the focused Back button"
            );
        });

        cx.update(|cx| {
            view.update(cx, |view, _| {
                view.screen = Screen::Clipboard;
                view.clipboard.browsing = true;
                view.clipboard.selected = 0;
                view.preview_open = true;
            });
        });
        let mut focused = false;
        for _ in 0..16 {
            cx.update_window(window, |_, window, cx| {
                window.render_frame(cx);
                window.focus_next(cx);
            })
            .unwrap();
            focused = cx
                .update_window(window, |_, window, cx| {
                    window.render_frame(cx);
                    window.find("page-back").focused() == Some(true)
                })
                .unwrap();
            if focused {
                break;
            }
        }
        assert!(focused, "the page back button must be reachable by Tab");
        cx.update_window(window, |_, window, cx| {
            window.press("enter", cx);
        })
        .unwrap();
        cx.update(|cx| {
            assert_eq!(
                view.read(cx).screen,
                Screen::Launcher,
                "Enter must activate the focused Back button"
            );
        });
    }

    #[gpui_kit::test]
    fn file_search_space_preview_requires_the_editor_to_own_focus(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::FileSearch;
                view.model.rows = vec![file_row("file-a", "a.txt")];
                view.model.selected = 0;
                view.file_search.browsing = true;
                view.preview_open = true;

                window.blur(cx);
                assert!(!view.active_editor_focused(window, cx));
                assert!(!view.handle_preview_keystroke("space", false, false, false, window, cx));
                assert!(
                    view.preview_open,
                    "Space must not preview for an unfocused editor"
                );

                view.input.update(cx, |input, cx| input.focus(window, cx));
                assert!(view.active_editor_focused(window, cx));
                assert!(view.handle_preview_keystroke("space", false, false, false, window, cx));
                assert!(!view.preview_open);
            });
        })
        .unwrap();
        cx.update(|cx| {
            assert_eq!(
                view.read(cx).screen,
                Screen::FileSearch,
                "preview key ownership must not change the current page"
            );
        });
    }

    #[gpui_kit::test]
    fn editing_navigation_closes_the_current_preview(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Clipboard;
                view.clipboard.browsing = true;
                view.preview_open = true;
                view.handle_key("left", KeyModifiers::default(), window, cx);
                assert!(!view.preview_open);
                assert!(!view.clipboard.browsing);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn repeated_space_edit_does_not_toggle_a_preview(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::FileSearch;
                view.model.rows = vec![file_row("file-a", "a.txt")];
                view.model.selected = 0;
                view.file_search.browsing = true;
                view.preview_open = true;
                crate::platform::set_key_event_is_repeat_for_test(true);
                view.input
                    .update(cx, |input, cx| input.insert(" ", window, cx));
                assert!(view.preview_open);
                crate::platform::set_key_event_is_repeat_for_test(false);

                view.screen = Screen::Clipboard;
                view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                    recording: false,
                    recording_enabled: false,
                    available: true,
                    paused_until_ms: None,
                    retention_days: 7,
                    stored_entries: 1,
                    stored_bytes: 4,
                    notice: None,
                    items: vec![crate::host::ClipboardItem {
                        id: 1,
                        summary: "fixture".into(),
                        kind: "text".into(),
                        copied_at_ms: 0,
                        byte_count: 4,
                    }],
                });
                view.clipboard.browsing = true;
                view.clipboard.selected = 0;
                view.preview_open = true;
                crate::platform::set_key_event_is_repeat_for_test(true);
                view.clipboard_input
                    .update(cx, |input, cx| input.insert(" ", window, cx));
                assert!(view.preview_open);
                crate::platform::set_key_event_is_repeat_for_test(false);
            });
        })
        .unwrap();
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn settings_shortcut_recorders_are_reachable_and_activated_from_the_keyboard(
        cx: &mut TestAppContext,
    ) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            view.update(cx, |view, cx| view.open_settings(window, cx));
            window.render_frame(cx);
            window.click("settings-category-shortcuts", cx);
            window.render_frame(cx);
        })
        .unwrap();

        let mut focused = false;
        for _ in 0..64 {
            cx.update_window(window, |_, window, cx| {
                window.press("tab", cx);
            })
            .unwrap();
            cx.run_until_parked();
            focused = cx
                .update_window(window, |_, window, cx| {
                    window.render_frame(cx);
                    window.find("current-shortcut-record").focused() == Some(true)
                })
                .unwrap();
            if focused {
                break;
            }
        }
        assert!(focused, "the shortcut recorder must be in the Tab order");
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.press("space", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                view.read(cx).shortcut_recording,
                Some(ShortcutSlot::CurrentApp)
            );
        });
    }

    #[gpui_kit::test]
    fn settings_add_action_shortcut_opens_a_picker_and_saves_the_recorded_scope(
        cx: &mut TestAppContext,
    ) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            view.update(cx, |view, cx| {
                view.shortcut_catalog = vec![ShortcutCatalogAction {
                    action_id: "developer.terminal.open".to_owned(),
                    title: "Open terminal".to_owned(),
                    localized_titles: Default::default(),
                    context_types: vec!["core.file".to_owned()],
                }];
                view.open_settings(window, cx);
            });
            window.render_frame(cx);
            window.click("settings-category-shortcuts", cx);
            window.render_frame(cx);
            assert!(window.try_find("settings-action-shortcut-search").is_none());
            window.click("settings-add-action-shortcut", cx);
            window.render_frame(cx);
            assert!(window.try_find("settings-action-picker").is_some());
            window.click("settings-action-choice-developer.terminal.open", cx);
            window.render_frame(cx);
            window.click("settings-action-picker-record", cx);
            view.update(cx, |view, cx| {
                view.handle_key(
                    "k",
                    KeyModifiers {
                        command: false,
                        control: true,
                        shift: true,
                        alt: true,
                        modified: true,
                    },
                    window,
                    cx,
                );
            });
            window.render_frame(cx);
            window.click("settings-action-picker-save", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(!view.settings_action_picker_open);
            let binding = view
                .settings_draft
                .action_shortcuts
                .iter()
                .find(|binding| binding.action_id == "developer.terminal.open")
                .expect("the recorded action shortcut is saved to the draft");
            assert_eq!(binding.shortcut, "control+alt+shift+KeyK");
            assert!(!binding.global);
        });
    }

    #[gpui_kit::test]
    fn settings_use_localized_engine_names_and_full_width_path_fields(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.locale = Locale::Zh;
                view.model.set_locale(Locale::Zh);
                view.settings_draft.file_search_engine = "system_index".to_owned();
                view.settings.selected = SettingsCategory::Search;
                view.screen = Screen::Settings;
                cx.notify();
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                window.find("cycle-file-engine").label(),
                Some(l10n::search_engine_system_index(Locale::Zh).as_str())
            );

            view.update(cx, |view, cx| {
                view.settings.selected = SettingsCategory::ApplicationHistory;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(
                window
                    .find("settings-field-history-custom-path")
                    .bounds()
                    .size
                    .width
                    > px(300.0)
            );
            assert!(window.find("history-custom-path").bounds().size.width > px(300.0));
            assert!(
                window
                    .find("settings-field-app-history-custom-adapters-row")
                    .bounds()
                    .size
                    .width
                    > px(300.0)
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn settings_history_filter_matches_identity_and_enabled_state(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.settings.selected = SettingsCategory::ApplicationHistory;
                view.screen = Screen::Settings;
                view.app_history.sources = vec![
                    crate::host::AppHistorySource {
                        id: "code".to_owned(),
                        title: "Code".to_owned(),
                        file_hint: "~/Library/Application Support/Code/state.vscdb".to_owned(),
                        action_enabled: true,
                        path: None,
                        mode:
                            anywhere_core::features::application_history::model::SourceMode::Automatic,
                    },
                    crate::host::AppHistorySource {
                        id: "browser".to_owned(),
                        title: "Browser".to_owned(),
                        file_hint: "~/Library/Application Support/Browser/History".to_owned(),
                        action_enabled: false,
                        path: None,
                        mode:
                            anywhere_core::features::application_history::model::SourceMode::Automatic,
                    },
                ];
                cx.notify();
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("history-action-code").is_some());
            assert!(window.try_find("history-action-browser").is_some());

            window.click("history-filter-enabled", cx);
            window.render_frame(cx);
            assert!(window.try_find("history-action-code").is_some());
            assert!(window.try_find("history-action-browser").is_none());

            window.click("history-filter-all", cx);
            window.click("history-settings-filter", cx);
            window.input("browser", cx);
            window.render_frame(cx);
            assert!(window.try_find("history-action-browser").is_some());
            assert!(window.try_find("history-action-code").is_none());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn settings_sources_load_without_starting_an_application_history_query(
        cx: &mut TestAppContext,
    ) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.settings.selected = SettingsCategory::ApplicationHistory;
                view.screen = Screen::Settings;
                view.app_history.loading = false;
                view.apply_auxiliary_event(
                    LiveEvent::AppHistorySources {
                        request: 1,
                        session_id: Some(7),
                        sources: vec![crate::host::AppHistorySource {
                            id: "code".to_owned(),
                            title: "Code".to_owned(),
                            file_hint: "~/Library/Application Support/Code/state.vscdb"
                                .to_owned(),
                            action_enabled: true,
                            path: None,
                            mode: anywhere_core::features::application_history::model::SourceMode::Automatic,
                        }],
                    },
                    cx,
                );
                assert_eq!(view.app_history.sources.len(), 1);
                assert!(!view.app_history.loading);
                cx.notify();
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("history-action-code").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn capture_and_excluded_apps_belong_to_their_settings_pages(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Settings;
                view.settings.selected = SettingsCategory::Context;
                cx.notify();
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("capture-clipboard").is_none());
            assert!(window.try_find("excluded-apps").is_none());
            view.update(cx, |view, cx| {
                view.settings.selected = SettingsCategory::Clipboard;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.try_find("capture-clipboard").is_some());
            view.update(cx, |view, cx| {
                view.settings.selected = SettingsCategory::Search;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.try_find("excluded-apps").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn settings_switch_starts_the_new_category_at_the_top(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            view.update(cx, |view, cx| view.open_settings(window, cx));
            window.render_frame(cx);
        })
        .unwrap();

        cx.update(|cx| {
            view.update(cx, |view, _| {
                view.settings_scrolls[SettingsCategory::Shortcuts as usize]
                    .set_offset(gpui_kit::point(px(0.), px(-240.)));
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.click("settings-category-search", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.settings.selected, SettingsCategory::Search);
            assert_eq!(
                view.settings_scrolls[SettingsCategory::Search as usize].offset(),
                gpui_kit::point(px(0.), px(0.))
            );
        });
    }

    #[gpui_kit::test]
    fn settings_escape_runs_before_a_focused_text_control(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            view.update(cx, |view, cx| {
                view.shortcut_catalog = vec![ShortcutCatalogAction {
                    action_id: "developer.terminal.open".to_owned(),
                    title: "Open terminal".to_owned(),
                    localized_titles: Default::default(),
                    context_types: vec!["core.file".to_owned()],
                }];
                view.open_settings(window, cx);
            });
            window.render_frame(cx);
            window.click("settings-category-shortcuts", cx);
            window.render_frame(cx);
            window.click("settings-add-action-shortcut", cx);
            window.render_frame(cx);
            assert!(window.try_find("settings-action-picker").is_some());
            window.click("settings-action-shortcut-search", cx);
        })
        .unwrap();

        cx.update_window(window, |_, window, cx| {
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.screen, Screen::Settings);
            assert!(!view.settings_action_picker_open);
        });
        cx.update_window(window, |_, window, cx| {
            window.click("done-settings", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(view.read(cx).screen, Screen::Launcher);
        });

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.open_settings(window, cx));
            window.render_frame(cx);
            window.click("settings-category-general", cx);
            window.click("open-at-login", cx);
            window.render_frame(cx);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.screen, Screen::Settings);
            assert!(view.settings_close_pending);
        });

        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.screen, Screen::Settings);
            assert!(!view.settings_close_pending);
        });
    }

    #[gpui_kit::test]
    fn leaving_settings_restores_the_launcher_editor_focus(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            view.update(cx, |view, cx| view.open_settings(window, cx));
            window.render_frame(cx);
            window.click("done-settings", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(view.read(cx).screen, Screen::Launcher);
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let focused = view.update(cx, |view, cx| view.active_editor_focused(window, cx));
            assert!(
                focused,
                "Done must restore the Launcher editor focus before returning"
            );
            window.press("x", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(view.read(cx).input.read(cx).value().as_ref(), "x");
        });
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.open_settings(window, cx));
        })
        .unwrap();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(view.read(cx).screen, Screen::Launcher);
            let focused = view.update(cx, |view, cx| view.active_editor_focused(window, cx));
            assert!(
                focused,
                "Escape must restore the Launcher editor focus before returning"
            );
            window.press("y", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(view.read(cx).input.read(cx).value().as_ref(), "xy");
        });
    }

    #[gpui_kit::test]
    fn launcher_hotkey_closes_clean_settings_and_keeps_dirty_settings_visible(
        cx: &mut TestAppContext,
    ) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let shown = view.update(cx, |view, cx| {
                view.presented = true;
                view.screen = Screen::Settings;
                view.activate_from_hotkey(InputMode::Search, window, cx)
            });
            assert!(shown);
            assert_eq!(view.read(cx).screen, Screen::Launcher);

            view.update(cx, |view, cx| {
                view.presented = true;
                view.screen = Screen::Settings;
                view.settings_draft.launch_at_login = !view.preferences.launch_at_login;
                assert!(!view.activate_from_hotkey(InputMode::Search, window, cx));
            });
            let view = view.read(cx);
            assert_eq!(view.screen, Screen::Settings);
            assert!(view.settings_close_pending);
            assert!(view.notice.is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn web_packages_escape_returns_to_settings(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            view.update(cx, |view, cx| {
                view.screen = Screen::Settings;
                view.open_screen(Screen::WebPackages, window, cx);
                assert_eq!(view.screen, Screen::WebPackages);
                assert!(view.is_settings_surface());
                assert!(!view.dismiss_on_focus_loss());
                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert_eq!(view.screen, Screen::Settings);
                assert!(view.is_settings_surface());
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn launcher_escape_restores_the_flutter_root_navigation_chain(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.presented = true;
                view.input_mode = InputMode::Environment;
                view.model.set_query(String::new());
                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert!(view.presented);
                assert_eq!(view.input_mode, InputMode::Search);

                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert!(!view.presented);

                view.presented = true;
                view.input_mode = InputMode::Content;
                view.model.set_query("captured".to_owned());
                set_textarea_value(&view.content_input, "captured", window, cx);
                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert!(view.presented);
                assert_eq!(view.input_mode, InputMode::Environment);
                assert_eq!(view.model.query, "");
                assert_eq!(view.input.read(cx).value().as_ref(), "");
                assert_eq!(view.content_input.read(cx).value().as_ref(), "");

                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert!(view.presented);
                assert_eq!(view.input_mode, InputMode::Search);
                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert!(!view.presented);

                view.presented = true;
                view.input_mode = InputMode::Environment;
                view.model.set_query("git".to_owned());
                set_input_value(&view.input, "git", window, cx);
                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert!(view.presented);
                assert_eq!(view.input_mode, InputMode::Environment);
                assert_eq!(view.model.query, "");
                assert_eq!(view.input.read(cx).value().as_ref(), "");

                view.model.rows = vec![LauncherRow {
                    id: "menu-item".to_owned(),
                    title: "Menu item".to_owned(),
                    subtitle: None,
                    shortcut: None,
                    icon: "menu".to_owned(),
                    kind: CandidateKind::Action,
                    enabled: true,
                    is_menu: true,
                    menu_path: Vec::new(),
                    command_id: None,
                    file_kind: None,
                    source_label: None,
                    icon_identity: None,
                    execution_context_id: None,
                    primary_action: false,
                }];
                assert!(view.model.open_selected_menu());
                view.model.set_query("export".to_owned());
                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert_eq!(view.model.query, "");
                assert_eq!(view.model.menu_path(), Some(&[][..]));
                view.handle_key("escape", KeyModifiers::default(), window, cx);
                assert!(view.model.menu_path().is_none());
                assert!(view.presented);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn returning_from_a_tool_restores_the_parent_workspace(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.model.set_query("git".to_owned());
                view.model.selected_scope = None;
                view.model.selected = 0;
                view.capture_parent_workspace(cx);
                view.screen = Screen::UnsupportedTool;
                view.model.set_query("tool query".to_owned());
                view.go_back(window, cx);
                assert_eq!(view.screen, Screen::Launcher);
                assert_eq!(view.model.query, "git");
                assert_eq!(
                    view.model.selection_id(),
                    Some("git-status"),
                    "selection is restored by stable ID"
                );
            });
        })
        .unwrap();
    }

    #[test]
    fn shortcut_recorder_normalizes_product_hotkeys() {
        assert_eq!(
            normalized_shortcut("k", false, true, true, true),
            Some("control+alt+shift+KeyK".to_owned())
        );
        assert_eq!(
            normalized_shortcut("space", true, false, true, false),
            Some("alt+meta+Space".to_owned())
        );
        assert_eq!(normalized_shortcut("k", false, false, false, true), None);
    }

    #[gpui_kit::test]
    fn context_shortcut_builder_keeps_routes_and_records_one_chord(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Settings;
                let action = ShortcutCatalogAction {
                    action_id: "developer.terminal.open".to_owned(),
                    title: "Open terminal".to_owned(),
                    localized_titles: Default::default(),
                    context_types: vec!["core.file".to_owned()],
                };
                view.open_context_shortcut_draft(None, window, cx);
                set_input_value(&view.context_shortcut_name_input, "Open here", window, cx);
                view.add_context_shortcut_action(&action, cx);
                view.shortcut_recording = Some(ShortcutSlot::ContextShortcut);
                view.handle_key(
                    "k",
                    KeyModifiers {
                        command: false,
                        control: true,
                        shift: true,
                        alt: true,
                        modified: true,
                    },
                    window,
                    cx,
                );
                assert_eq!(
                    view.context_shortcut_draft
                        .as_ref()
                        .and_then(|draft| draft.shortcut.as_deref()),
                    Some("control+alt+shift+KeyK")
                );
                view.commit_context_shortcut_draft(cx).unwrap();
                let binding = view
                    .settings_draft
                    .action_shortcuts
                    .iter()
                    .find(|binding| binding.action_id.starts_with("intent:"))
                    .unwrap();
                assert_eq!(binding.title, "Open here");
                assert_eq!(binding.routes.len(), 1);
                assert_eq!(binding.routes[0].context_type.as_deref(), Some("core.file"));
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn native_destinations_open_typed_surfaces(cx: &mut TestAppContext) {
        use std::{cell::Cell, rc::Rc};

        let (window, view) = open_view(cx);
        let shelf_requested = Rc::new(Cell::new(false));
        let observed = shelf_requested.clone();
        let _shelf_event = cx.update(|cx| {
            cx.subscribe(&view, move |_, event: &LauncherEvent, _| {
                if event == &LauncherEvent::OpenClipboardShelf {
                    observed.set(true);
                }
            })
        });
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.open_action_destination(
                    HostView::PluginDetail {
                        detail: DetailView {
                            schema_version: 1,
                            title: "Repository".to_owned(),
                            body: Some("Read-only details".to_owned()),
                            rows: vec![KeyValue {
                                label: "Branch".to_owned(),
                                value: "main".to_owned(),
                            }],
                        },
                    },
                    window,
                    cx,
                );
            });
        })
        .unwrap();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.screen, Screen::PluginDetail);
            assert_eq!(
                view.web_surface
                    .plugin_detail
                    .as_ref()
                    .map(|detail| detail.title.as_str()),
                Some("Repository")
            );
        });

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.go_back(window, cx);
                view.open_action_destination(
                    HostView::NativeTool {
                        id: "clipboard.shelf".to_owned(),
                        input: String::new(),
                    },
                    window,
                    cx,
                );
            });
        })
        .unwrap();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.screen, Screen::Launcher);
            assert!(shelf_requested.get());
        });

        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.go_back(window, cx);
                view.open_action_destination(
                    HostView::NativeTool {
                        id: "unknown.tool".to_owned(),
                        input: "{\"value\":1}".to_owned(),
                    },
                    window,
                    cx,
                );
            });
        })
        .unwrap();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.screen, Screen::UnsupportedTool);
            assert_eq!(
                view.web_surface
                    .unsupported_tool
                    .as_ref()
                    .map(|(id, _)| id.as_str()),
                Some("unknown.tool")
            );
        });
    }

    #[gpui_kit::test]
    fn web_teardown_clears_every_document_owned_field(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::WebTool;
                view.web_surface.document = Some(WebDocument {
                    handle: "handle-a".to_owned(),
                    origin: "anywhere-plugin://0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.plugin.local".to_owned(),
                    entry: "index.html".to_owned(),
                    title: "Tool".to_owned(),
                });
                view.web_surface.ready = true;
                view.web_surface.error = Some("old error".to_owned());
                view.web_surface.invocation_id = Some("invocation".to_owned());
                view.web_surface.actions_open = true;
                view.web_surface.action_selected = 1;
                view.web_surface.state_handle = Some("state".to_owned());
                view.web_surface.state_version = 9;
                assert!(event_handle_matches(view, "handle-a"));
                assert!(!event_handle_matches(view, "handle-b"));
                view.close_web_surface(cx);
                assert!(view.web_surface.document.is_none());
                assert!(!view.web_surface.ready);
                assert!(view.web_surface.error.is_none());
                assert!(view.web_surface.invocation_id.is_none());
                assert!(!view.web_surface.actions_open);
                assert_eq!(view.web_surface.action_selected, 0);
                assert!(view.web_surface.state_handle.is_none());
                assert_eq!(view.web_surface.state_version, 0);
                assert!(!event_handle_matches(view, "handle-a"));
            });
        });
    }

    #[gpui_kit::test]
    fn web_package_review_actions_stay_inside_the_window(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, _window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::WebPackages;
                view.web_surface.review = Some(crate::host::WebPackageReview {
                    token: "review-token".into(),
                    id: "dev.example.text-pad".into(),
                    name: "Text Pad".into(),
                    version: "1.1.1".into(),
                    digest: "0123456789abcdef".into(),
                    permissions: vec!["Read Launcher search text".into()],
                    updating: false,
                });
                cx.notify();
            });
        })
        .unwrap();
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let approve = window.find("approve-web-package").bounds();
            let viewport = window.viewport_size();
            assert!(approve.left() >= px(0.));
            assert!(approve.top() >= px(0.));
            assert!(approve.bottom() <= viewport.height);
            assert!(
                approve.right() <= viewport.width,
                "approve={approve:?} viewport={viewport:?}"
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn task_results_open_tasks_before_dismissing_launcher(cx: &mut TestAppContext) {
        use std::{cell::Cell, rc::Rc};

        let (_window, view) = open_view(cx);
        let opened = Rc::new(Cell::new(false));
        let observed = opened.clone();
        let _event = cx.update(|cx| {
            cx.subscribe(&view, move |_, event: &LauncherEvent, _| {
                if matches!(event, LauncherEvent::OpenTasks(task_id) if task_id == "task") {
                    observed.set(true);
                }
            })
        });
        let task_result = || anywhere_protocol::ActionResult {
            session_id: 1,
            revision: 1,
            status: anywhere_protocol::ActionStatus::Succeeded,
            error: None,
            warning: None,
            output_text: None,
            view: None,
            close_window: true,
            execution_session_id: Some("task".to_owned()),
        };
        let (generation_before, dismissed, generation_after) = cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.presented = true;
                let generation = view.live_state.generation;
                let dismissed = view.finish_action_result(&task_result(), false, cx);
                let generation_after = view.live_state.generation;
                (generation, dismissed, generation_after)
            })
        });
        assert!(dismissed);
        assert_eq!(generation_after, generation_before.wrapping_add(1));
        assert!(opened.get());

        opened.set(false);
        let (generation_before, dismissed, still_presented, generation_after_warning) =
            cx.update(|cx| {
                view.update(cx, |view, cx| {
                    view.presented = true;
                    let generation = view.live_state.generation;
                    let mut warning = task_result();
                    warning.warning = Some("usage unavailable".to_owned());
                    let dismissed = view.finish_action_result(&warning, false, cx);
                    (
                        generation,
                        dismissed,
                        view.presented,
                        view.live_state.generation,
                    )
                })
            });
        assert!(!dismissed);
        assert!(still_presented);
        assert_eq!(generation_after_warning, generation_before);
        assert!(!opened.get());
    }

    #[gpui_kit::test]
    fn selecting_a_started_task_applies_to_the_next_tasks_snapshot(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        let row = |id: &str| crate::host::ExecutionRow {
            id: id.to_owned(),
            title: id.to_owned(),
            directory: "/tmp/project".to_owned(),
            command: "true".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Running,
            output_revision: 0,
            exit_code: None,
            started_at_ms: 0,
            finished_at_ms: None,
            output_truncated: false,
            error: None,
        };
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.tasks.set_filter("old-task".to_owned());
                view.select_execution("new-task".to_owned(), cx);
                view.apply_tasks_rows(vec![row("old-task"), row("new-task")], cx);
                assert!(view.tasks.filter.is_empty());
                assert_eq!(
                    view.tasks.selected_row().map(|row| row.id.as_str()),
                    Some("new-task")
                );
                assert!(view.pending_task_selection.is_none());
                assert!(view.tasks.detail.is_none());

                view.select_execution("old-task".to_owned(), cx);
                assert_eq!(
                    view.tasks.selected_row().map(|row| row.id.as_str()),
                    Some("old-task")
                );
            });
        });
    }

    #[gpui_kit::test]
    fn task_navigation_at_a_boundary_keeps_the_current_detail(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let row = |id: &str| crate::host::ExecutionRow {
            id: id.to_owned(),
            title: id.to_owned(),
            directory: "/tmp/project".to_owned(),
            command: "true".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Running,
            output_revision: 0,
            exit_code: None,
            started_at_ms: 0,
            finished_at_ms: None,
            output_truncated: false,
            error: None,
        };
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.tasks.apply_rows(vec![row("first"), row("last")]);
                view.tasks.selected = 1;
                view.tasks.detail = Some(crate::host::ExecutionDetail {
                    row: row("last"),
                    stdout: "ready".to_owned(),
                    stderr: String::new(),
                });
                view.tasks.detail_request = 7;

                view.handle_key("down", KeyModifiers::default(), window, cx);
                assert_eq!(view.tasks.selected, 1);
                assert!(view.tasks.detail.is_some());
                assert_eq!(view.tasks.detail_request, 7);

                view.handle_key("up", KeyModifiers::default(), window, cx);
                assert_eq!(view.tasks.selected, 0);
                assert!(view.tasks.detail.is_none());
                assert_eq!(view.tasks.detail_request, 8);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn web_tool_actions_expose_reload_and_back(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::WebTool;
                view.web_surface.invocation_id = Some("invocation".to_owned());
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("web-tool-actions").is_some());
            window.click("web-tool-actions", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("web-tool-actions-panel").is_some());
            view.update(cx, |view, cx| {
                assert!(window.try_find("web-tool-action-0").is_some());
                assert!(window.try_find("web-tool-action-1").is_some());
                view.web_surface.action_selected = 0;
                view.handle_key("enter", KeyModifiers::default(), window, cx);
                assert_eq!(view.screen, Screen::WebTool);
                assert!(!view.web_surface.actions_open);
                assert_eq!(
                    view.web_surface.invocation_id.as_deref(),
                    Some("invocation")
                );

                view.toggle_web_actions(cx);
                view.handle_key("down", KeyModifiers::default(), window, cx);
                assert_eq!(view.web_surface.action_selected, 1);
                view.handle_key("enter", KeyModifiers::default(), window, cx);
                assert_eq!(view.screen, Screen::Launcher);
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn platform_escape_leaves_web_tool_only(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::WebTool;
                view.notice = None;
                assert!(view.handle_web_view_escape_from_platform(window, cx));
                assert_eq!(view.screen, Screen::Launcher);
                assert!(!view.handle_web_view_escape_from_platform(window, cx));
            });
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn preview_completion_restores_the_editor_for_the_current_screen(cx: &mut TestAppContext) {
        let (_window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, _| {
                assert!(view
                    .active_editor_input()
                    .is_some_and(|input| input == &view.input));
                view.screen = Screen::FileSearch;
                assert!(view
                    .active_editor_input()
                    .is_some_and(|input| input == &view.input));
                view.screen = Screen::ClipboardShelf;
                assert!(view
                    .active_editor_input()
                    .is_some_and(|input| input == &view.clipboard_input));
                view.screen = Screen::Tasks;
                assert!(view.active_editor_input().is_none());
            });
        });
    }

    #[gpui_kit::test]
    fn settings_close_prompt_blocks_pointer_and_traps_focus(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Settings;
                view.settings.selected = SettingsCategory::General;
                view.settings_draft.launch_at_login = !view.preferences.launch_at_login;
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            view.update(cx, |view, cx| {
                view.settings_focus_handle.focus(window, cx);
                assert!(!view.request_settings_exit(SettingsExitIntent::Close, window, cx));
            });
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(cx
            .update_window(window, |_, window, cx| {
                view.read(cx).settings_keep_editing_focus.is_focused(window)
            })
            .unwrap());

        let before = cx.update(|cx| view.read(cx).settings_draft.launch_at_login);
        let switch_center = cx
            .update_window(window, |_, window, _| {
                window.find("open-at-login").bounds().center()
            })
            .unwrap();
        let mut visual = VisualTestContext::from_window(window, cx);
        visual.simulate_click(switch_center, gpui_kit::Modifiers::default());
        visual.run_until_parked();
        assert_eq!(
            visual.update(|_, cx| view.read(cx).settings_draft.launch_at_login),
            before,
            "the scrim must block pointer interaction with the underlying switch"
        );

        visual.update(|window, cx| window.press("tab", cx));
        visual.run_until_parked();
        visual.update(|window, cx| {
            let view = view.read(cx);
            assert!(
                view.settings_discard_focus.is_focused(window)
                    || view.settings_save_focus.is_focused(window)
            );
            assert!(!view.settings_focus_handle.is_focused(window));
        });

        visual.update(|window, cx| window.press("x", cx));
        visual.run_until_parked();
        visual.update(|_, cx| {
            let view = view.read(cx);
            assert!(view.settings_close_pending);
            assert_eq!(view.settings_draft.launch_at_login, before);
        });

        visual.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.settings_keep_editing_focus.focus(window, cx);
                cx.notify();
            });
        });
        visual.run_until_parked();
        visual.update(|window, cx| window.press("space", cx));
        visual.run_until_parked();
        visual.update(|window, cx| {
            let view = view.read(cx);
            assert!(!view.settings_close_pending);
            assert_eq!(view.screen, Screen::Settings);
            assert_eq!(view.settings_draft.launch_at_login, before);
            assert!(view.settings_focus_handle.is_focused(window));
        });
    }

    #[gpui_kit::test]
    fn launcher_notice_uses_bounded_height_and_never_overlaps_rows(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.model.set_query("report".to_owned());
                view.model.rows = vec![file_row("one", "Report.pdf")];
                view.set_notice(
                    "A long error detail\n".repeat(32),
                    components::NoticeKind::Error,
                );
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        let mut visual = VisualTestContext::from_window(window, cx);
        visual.update(|window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("launcher-search-header").is_some());
            assert!(window.try_find("launcher-notice").is_some());
            assert!(window.try_find("candidate-0").is_some());
            assert!(window.try_find("launcher-notice-scroll").is_some());
        });
        let header = visual.update(|window, _| window.find("launcher-search-header").bounds());
        let notice = visual.update(|window, _| window.find("launcher-notice").bounds());
        let result = visual.update(|window, _| window.find("candidate-0").bounds());
        assert_eq!(notice.size.height, px(metrics::NOTICE_MAX_HEIGHT));
        assert!(notice.top() >= header.bottom());
        assert!(result.top() >= notice.bottom());
    }

    #[gpui_kit::test]
    fn notice_changes_converge_to_the_latest_window_size(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.model.set_query("action".to_owned());
                view.model.rows = vec![file_row("disabled", "Unavailable action")];
                view.model.rows[0].enabled = false;
                view.model.selected = 0;
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            view.update(cx, |view, cx| {
                view.activate_selection(cx);
                assert_eq!(
                    view.desired_window_size().height,
                    px(180.5),
                    "the one-row failure state must include the notice"
                );
            });
        })
        .unwrap();
        sync_test_viewport_to_desired(cx, window, &view);
        let viewport = cx
            .update_window(window, |_, window, _| window.viewport_size())
            .unwrap();
        assert_eq!(viewport.height, px(180.5));
        let mut visual = VisualTestContext::from_window(window, cx);
        visual.update(|window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("candidate-0").is_some());
        });
        let result = visual.update(|window, _| window.find("candidate-0").bounds());
        assert!(result.bottom() <= viewport.height);
    }

    #[gpui_kit::test]
    fn scope_bar_centers_short_groups_and_keeps_overflow_reachable(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.model.scopes = vec![
                    crate::launcher::LauncherScope {
                        id: "one".to_owned(),
                        label: "One".to_owned(),
                        is_current_app: false,
                    },
                    crate::launcher::LauncherScope {
                        id: "two".to_owned(),
                        label: "Two".to_owned(),
                        is_current_app: false,
                    },
                ];
                view.model.set_query("x".to_owned());
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        let mut visual = VisualTestContext::from_window(window, cx);
        visual.update(|window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("scope-one").is_some());
            assert!(window.try_find("scope-two").is_some());
        });
        let one = visual.update(|window, _| window.find("scope-one").bounds());
        let two = visual.update(|window, _| window.find("scope-two").bounds());
        let group_center = (one.left() + two.right()) / 2.0;
        let viewport_center = visual.update(|window, _| window.viewport_size().width) / 2.0;
        assert!((group_center - viewport_center).abs() <= px(2.));
    }

    #[gpui_kit::test]
    fn scope_bar_overflow_starts_at_the_leading_edge_and_reveals_a_late_selection(
        cx: &mut TestAppContext,
    ) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.model.scopes = (0..8)
                    .map(|index| crate::launcher::LauncherScope {
                        id: format!("scope-{index}"),
                        label: format!("A very long scope label {index}"),
                        is_current_app: false,
                    })
                    .collect();
                view.model.set_query("x".to_owned());
                view.model.selected_scope = Some("scope-7".to_owned());
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let maximum = view.read(cx).scope_scroll.max_offset().x;
            assert!(
                maximum > px(0.),
                "an overflowing scope group must expose horizontal scrolling"
            );
            let first = window.find("scope-scope-0").bounds();
            assert!(first.left() >= px(0.));
            assert!(
                first.left() < window.viewport_size().width,
                "the first scope must remain at the reachable leading edge"
            );
            view.read(cx).reveal_selected_scope();
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            let last = window.find("scope-scope-7").bounds();
            let offset = view.read(cx).scope_scroll.offset().x;
            assert!(
                last.left() >= px(0.) && last.right() <= window.viewport_size().width,
                "the selected late scope must be revealed inside the viewport: \
                 last={last:?}, offset={offset:?}, viewport={:?}",
                window.viewport_size()
            );
            assert!(offset < px(0.));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn shelf_header_respects_traffic_lights_and_escape_shares_keyboard_route(
        cx: &mut TestAppContext,
    ) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ClipboardShelf;
                view.standalone_shelf = true;
                view.clipboard.query = "fixture".to_owned();
                cx.notify();
            });
        });
        sync_test_viewport(cx, window, gpui_kit::size(px(360.), px(360.)));
        let mut visual = VisualTestContext::from_window(window, cx);
        visual.update(|window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("shelf-state-icon").is_some());
        });
        let icon = visual.update(|window, _| window.find("shelf-state-icon").bounds());
        assert!(icon.top() >= px(metrics::TITLE_BAR_INSET));
        visual.update(|window, cx| {
            window.click("shelf-escape", cx);
        });
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.screen, Screen::ClipboardShelf);
            assert!(view.clipboard.query.is_empty());
        });

        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.clipboard.actions_open = true;
                cx.notify();
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("shelf-escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(!view.clipboard.actions_open);
            assert_eq!(view.screen, Screen::ClipboardShelf);
        });

        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.clipboard.delete_pending = Some(42);
                cx.notify();
            });
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("shelf-escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(view.clipboard.delete_pending.is_none());
            assert_eq!(view.screen, Screen::ClipboardShelf);
        });
    }

    #[gpui_kit::test]
    fn unsupported_tool_content_scrolls_to_long_input(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::UnsupportedTool;
                view.web_surface.unsupported_tool = Some((
                    "tool".to_owned(),
                    (0..60)
                        .map(|i| format!("line {i}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ));
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        let mut visual = VisualTestContext::from_window(window, cx);
        let maximum = visual.update(|_, cx| view.read(cx).unsupported_tool_scroll.max_offset().y);
        assert!(maximum > px(0.));
        visual.update(|_, cx| {
            view.read(cx)
                .unsupported_tool_scroll
                .set_offset(gpui_kit::point(px(0.), -maximum));
        });
        visual.update(|window, cx| window.render_frame(cx));
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("unsupported-tool-input-text").is_some());
        });
        let input = visual.update(|window, _| window.find("unsupported-tool-input-text").bounds());
        let viewport_height = visual.update(|window, _| window.viewport_size().height);
        assert!(input.bottom() <= viewport_height);
    }

    #[gpui_kit::test]
    fn discarding_language_draft_restores_the_saved_locale(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Settings;
                view.settings.selected = SettingsCategory::General;
                view.preferences.ui_language = UiLanguage::Zh;
                view.settings_draft.ui_language = UiLanguage::Zh;
                view.locale = Locale::Zh;
                view.model.set_locale(Locale::Zh);
                view.settings_draft.ui_language = UiLanguage::En;
                view.locale = Locale::En;
                view.model.set_locale(Locale::En);
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| view.discard_settings(window, cx));
        })
        .unwrap();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.locale, Locale::Zh);
            assert_eq!(view.settings_draft.ui_language, UiLanguage::Zh);
            assert_eq!(view.screen, Screen::Launcher);
        });
    }

    #[gpui_kit::test]
    fn clipboard_settings_expose_real_choices_and_confirm_destructive_changes(
        cx: &mut TestAppContext,
    ) {
        let (window, view) = open_view(cx);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Settings;
                view.settings.selected = SettingsCategory::Clipboard;
                view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                    recording: true,
                    recording_enabled: true,
                    available: true,
                    paused_until_ms: None,
                    retention_days: 30,
                    stored_entries: 0,
                    stored_bytes: 0,
                    notice: None,
                    items: Vec::new(),
                });
                cx.notify();
            });
        });
        sync_test_viewport_to_desired(cx, window, &view);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("clipboard-retention", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("clipboard-retention-90").is_some());
            window.click("clipboard-retention-1", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                view.read(cx).clipboard_confirmation,
                Some(ClipboardConfirmation::SetRetention(1))
            );
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("clipboard-confirm-prompt").is_some());
            window.click("clipboard-confirm-cancel", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(view.clipboard_confirmation.is_none());
            assert!(!view.clipboard.loading);
        });

        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("clipboard-pause", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            for id in [
                "clipboard-resume",
                "clipboard-pause-5",
                "clipboard-pause-15",
                "clipboard-pause-60",
            ] {
                assert!(window.try_find(id).is_some(), "missing pause choice {id}");
            }
            window.click("clipboard-resume", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(view.clipboard_settings_choice.is_none());
            assert!(view.clipboard_confirmation.is_none());
        });
    }

    #[gpui_kit::test]
    fn closed_clipboard_status_channel_clears_loading_and_reports_an_error(
        cx: &mut TestAppContext,
    ) {
        let (_window, view) = open_view(cx);
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        drop(sender);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Settings;
                view.clipboard.query_generation = 7;
                view.clipboard.loading = true;
                view.poll_clipboard_search(receiver, 7, cx);
            });
        });
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(!view.clipboard.loading);
            assert_eq!(view.notice_kind, components::NoticeKind::Error);
            assert_eq!(
                view.notice.as_deref(),
                Some(l10n::clipboard_settings_failed(Locale::En).as_str())
            );
        });
    }

    #[gpui_kit::test]
    fn task_notices_keep_success_and_help_semantics(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        let row = crate::host::ExecutionRow {
            id: "task".to_owned(),
            title: "npm Run · task".to_owned(),
            directory: "/tmp/project".to_owned(),
            command: "npm run task".to_owned(),
            status: anywhere_protocol::ExecutionSessionStatus::Succeeded,
            output_revision: 1,
            exit_code: Some(0),
            started_at_ms: 0,
            finished_at_ms: Some(1),
            output_truncated: false,
            error: None,
        };
        cx.update_window(window, |_, _, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.standalone_tasks = true;
                view.apply_tasks_rows(vec![row.clone()], cx);
                view.tasks.detail = Some(crate::host::ExecutionDetail {
                    row,
                    stdout: "ready".to_owned(),
                    stderr: String::new(),
                });
            });
        })
        .unwrap();
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.execute_task_action(0, cx);
                assert_eq!(view.notice_kind, components::NoticeKind::Success);
                view.set_notice(
                    l10n::task_keyboard_help(Locale::En, "⌘F", "⌘."),
                    components::NoticeKind::Info,
                );
                assert_eq!(view.notice_kind, components::NoticeKind::Info);
            });
        });
    }
}
