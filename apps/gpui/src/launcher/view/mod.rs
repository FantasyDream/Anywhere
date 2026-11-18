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
        registered_preferences, ActionShortcutBinding, ActionShortcutRoute, HostPreferences,
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
        visibility_script, web_theme_script, WebDocumentPolicy,
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
#[path = "controllers/clipboard.rs"]
mod clipboard_controller;
#[path = "controllers/file_search.rs"]
mod file_search_controller;
#[path = "controllers/history.rs"]
mod history_controller;
#[path = "controllers/icons.rs"]
mod icons_controller;
mod input;
mod live_state;
mod navigation;
#[path = "controllers/navigation.rs"]
mod navigation_controller;
mod page_shell;
mod pages;
#[path = "controllers/preview.rs"]
mod preview_controller;
mod settings;
#[path = "settings/controller.rs"]
mod settings_controller;
#[path = "controllers/streams.rs"]
mod streams_controller;
#[path = "controllers/tasks.rs"]
mod tasks_controller;
#[path = "controllers/web.rs"]
mod web_controller;
mod web_surface;

use live_state::LiveState;
use web_surface::{ActiveWebBinding, WebIpcRequest, WebSurfaceState};

#[derive(Default)]
struct ToolListScrolls {
    file_search: gpui_kit::UniformListScrollHandle,
    clipboard: gpui_kit::UniformListScrollHandle,
    clipboard_shelf: gpui_kit::UniformListScrollHandle,
    history: gpui_kit::UniformListScrollHandle,
    tasks: gpui_kit::UniformListScrollHandle,
    tasks_output: gpui_kit::UniformListScrollHandle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewKind {
    Main,
    ClipboardShelf,
    Tasks,
}

const WEB_FRAME_BYTES: usize = 64 * 1024;
const WEB_IPC_QUEUE_CAPACITY: usize = 32;
const MAX_APPLICATION_ICON_REQUESTS: usize = 4;

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

fn effective_window_size(
    natural: gpui_kit::Size<gpui_kit::Pixels>,
    available: gpui_kit::Size<gpui_kit::Pixels>,
) -> gpui_kit::Size<gpui_kit::Pixels> {
    let (width, height) = crate::platform::constrained_window_extent(
        available.width.as_f32(),
        available.height.as_f32(),
        natural.width.as_f32(),
        natural.height.as_f32(),
        metrics::SPACE_LG,
    );
    gpui_kit::size(px(width), px(height))
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
    object_action_generation: u64,
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
    settings_action_picker_focus: FocusHandle,
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
    tool_list_scrolls: ToolListScrolls,
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
    preview_request_generation: u64,
    preview_open: bool,
    live: Option<Arc<LiveLauncher>>,
    hotkeys: Option<Arc<Mutex<HotKeyController>>>,
    hotkey_error: Option<String>,
    standalone_shelf: bool,
    standalone_tasks: bool,
    runtime_state: RuntimeState,
    live_state: LiveState,
    presented: bool,
    retained_workspace: bool,
    hidden_at: Option<Instant>,
    retention_task: Option<gpui_kit::Task<()>>,
    resize_pending: bool,
    clipboard_search_task: Option<gpui_kit::Task<()>>,
    preview_selection_task: Option<gpui_kit::Task<()>>,
    window_handle: AnyWindowHandle,
    _editor_sync_guard: Rc<Cell<bool>>,
    _input_subscription: Subscription,
    _key_interceptor: Subscription,
    _window_appearance_subscription: Subscription,
    _window_display_subscription: Subscription,
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
            ViewKind::Main,
        );
        view.focus_launcher_input(window, cx);
        view.resize_window_for_screen(window, cx);
        view
    }

    pub fn with_model(
        window: &mut Window,
        cx: &mut Context<Self>,
        model: LauncherModel,
        locale: Locale,
    ) -> Self {
        let view = Self::build(window, cx, model, locale, None, None, ViewKind::Main);
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
            ViewKind::Main,
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
            ViewKind::Main,
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
            ViewKind::Main,
        );
        view.focus_launcher_input(window, cx);
        view
    }

    pub(crate) fn attach_runtime(
        &mut self,
        live: Arc<LiveLauncher>,
        hotkeys: Option<Arc<Mutex<HotKeyController>>>,
        hotkey_error: Option<String>,
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
        self.locale = locale;
        self.model.set_locale(locale);
        self.refresh_editor_placeholders(window, cx);
        self.preferences = preferences.clone();
        self.settings_draft = preferences;
        self.settings = settings;
        self.input_placeholder = l10n::search_apps_actions(locale);
        self.hotkey_error = hotkey_error;
        self.notice = settings_error.or_else(|| self.hotkey_error_message());
        self.notice_kind = components::NoticeKind::Error;
        self.live = Some(live);
        self.hotkeys = hotkeys;
        self.runtime_state = RuntimeState::Ready;
        cx.notify();
    }

    pub(crate) fn global_hotkey_action(
        &self,
        event: global_hotkey::GlobalHotKeyEvent,
    ) -> Option<crate::host::HotKeyAction> {
        self.hotkeys.as_ref()?.lock().ok()?.action_for_event(event)
    }

    fn hotkey_error_message(&self) -> Option<String> {
        self.hotkey_error
            .as_ref()
            .map(|error| l10n::global_shortcut_registration_failed(self.locale, error))
    }

    fn registered_preferences(&self) -> HostPreferences {
        let controller = self
            .hotkeys
            .as_ref()
            .and_then(|hotkeys| hotkeys.lock().ok());
        registered_preferences(&self.preferences, controller.as_deref())
    }

    fn build(
        window: &mut Window,
        cx: &mut Context<Self>,
        model: LauncherModel,
        locale: Locale,
        live: Option<Arc<LiveLauncher>>,
        hotkeys: Option<Arc<Mutex<HotKeyController>>>,
        kind: ViewKind,
    ) -> Self {
        let standalone_shelf = kind == ViewKind::ClipboardShelf;
        let standalone_tasks = kind == ViewKind::Tasks;
        let auxiliary = kind != ViewKind::Main;
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
        let clipboard_input = if auxiliary {
            input.clone()
        } else {
            cx.new(|cx| InputState::new(window, cx).placeholder(l10n::search_history(locale)))
        };
        let history_input = if auxiliary {
            input.clone()
        } else {
            cx.new(|cx| InputState::new(window, cx).placeholder(l10n::app_history_search(locale)))
        };
        let tasks_filter_input = if auxiliary {
            input.clone()
        } else {
            cx.new(|cx| InputState::new(window, cx).placeholder(l10n::find_atask(locale)))
        };
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
                if this.standalone_shelf || this.standalone_tasks {
                    return;
                }
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
                    if this.model.object_actions_active {
                        this.model.hide_object_actions();
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
                if !matches!(this.screen, Screen::Clipboard | Screen::ClipboardShelf) {
                    return;
                }
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
                    this.schedule_clipboard_search(cx);
                    cx.notify();
                }
            },
        );
        let history_subscription = cx.subscribe(
            &history_input,
            |this: &mut Self, _, event: &InputEvent, cx| {
                if this.screen != Screen::ApplicationHistory {
                    return;
                }
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
                if this.screen != Screen::Tasks {
                    return;
                }
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let filter = this.tasks_filter_input.read(cx).value().to_string();
                if filter == this.tasks.filter {
                    return;
                }
                this.pending_task_selection = None;
                this.tasks.set_filter(filter);
                this.refresh_selected_task_detail(cx);
                cx.notify();
            },
        );
        let action_shortcut_input = if auxiliary {
            input.clone()
        } else {
            cx.new(|cx| InputState::new(window, cx).placeholder(l10n::find_an_action(locale)))
        };
        let context_shortcut_name_input = if auxiliary {
            input.clone()
        } else {
            cx.new(|cx| InputState::new(window, cx).placeholder(l10n::e_gopen_terminal(locale)))
        };
        let action_shortcut_subscription = cx.subscribe(
            &action_shortcut_input,
            move |_: &mut Self, _, event: &InputEvent, cx| {
                if !auxiliary && matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
        let context_shortcut_name_subscription = cx.subscribe(
            &context_shortcut_name_input,
            move |_: &mut Self, _, event: &InputEvent, cx| {
                if !auxiliary && matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
        let excluded_apps_input = if auxiliary {
            input.clone()
        } else {
            cx.new(|cx| {
                InputState::new(window, cx).default_value(preferences.excluded_apps.join("\n"))
            })
        };
        let file_roots_input = if auxiliary {
            input.clone()
        } else {
            cx.new(|cx| {
                InputState::new(window, cx).default_value(preferences.file_search_roots.join("\n"))
            })
        };
        let file_exclusions_input = if auxiliary {
            input.clone()
        } else {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(preferences.file_search_excluded_names.join("\n"))
            })
        };
        let history_path_input = if auxiliary {
            input.clone()
        } else {
            cx.new(|cx| InputState::new(window, cx))
        };
        let history_settings_filter_input = if auxiliary {
            input.clone()
        } else {
            cx.new(|cx| {
                InputState::new(window, cx).placeholder(l10n::app_history_filter_sources(locale))
            })
        };
        let history_settings_filter_subscription = cx.subscribe(
            &history_settings_filter_input,
            move |_: &mut Self, _, event: &InputEvent, cx| {
                if !auxiliary && matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );
        let current_shortcut_input = if auxiliary {
            input.clone()
        } else {
            cx.new(|cx| {
                InputState::new(window, cx).default_value(preferences.current_app_shortcut.clone())
            })
        };
        let universal_shortcut_input = if auxiliary {
            input.clone()
        } else {
            cx.new(|cx| {
                InputState::new(window, cx).default_value(preferences.universal_shortcut.clone())
            })
        };
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
                    if this.settings_modal_active() {
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
                            this.handle_settings_modal_escape(window, cx);
                            window.prevent_default();
                            cx.stop_propagation();
                            return;
                        }
                        if this.settings_action_picker_open {
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
                            }
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
        let window_appearance_subscription = {
            let this = cx.weak_entity();
            window.observe_window_appearance(move |window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.window_appearance_changed(window, cx);
                });
            })
        };
        let window_display_subscription = {
            let this = cx.weak_entity();
            window.observe_window_display(move |window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.resize_current_window(cx);
                    cx.notify();
                    window.refresh();
                });
            })
        };
        let tool_list_scrolls = ToolListScrolls::default();
        let view = Self {
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
            tasks_output_scroll: tool_list_scrolls
                .tasks_output
                .0
                .borrow()
                .base_handle
                .clone(),
            tasks_refresh_cancel: None,
            web: WebPageModel::default(),
            web_surface: WebSurfaceState::default(),
            application_icons: ApplicationIconCache::default(),
            application_icons_requested: HashMap::new(),
            application_icon_generation: 0,
            object_action_generation: 0,
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
            settings_action_picker_focus: cx.focus_handle(),
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
            file_search_scroll: tool_list_scrolls.file_search.0.borrow().base_handle.clone(),
            clipboard_scroll: tool_list_scrolls.clipboard.0.borrow().base_handle.clone(),
            clipboard_shelf_scroll: tool_list_scrolls
                .clipboard_shelf
                .0
                .borrow()
                .base_handle
                .clone(),
            history_scroll: tool_list_scrolls.history.0.borrow().base_handle.clone(),
            tasks_scroll: tool_list_scrolls.tasks.0.borrow().base_handle.clone(),
            web_packages_scroll: ScrollHandle::new(),
            tool_list_scrolls: ToolListScrolls {
                file_search: tool_list_scrolls.file_search.clone(),
                clipboard: tool_list_scrolls.clipboard.clone(),
                clipboard_shelf: tool_list_scrolls.clipboard_shelf.clone(),
                history: tool_list_scrolls.history.clone(),
                tasks: tool_list_scrolls.tasks.clone(),
                tasks_output: tool_list_scrolls.tasks_output.clone(),
            },
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
            preview_request_generation: 0,
            preview_open: false,
            live,
            hotkey_error: hotkeys.as_ref().and_then(|hotkeys| {
                hotkeys
                    .lock()
                    .ok()
                    .and_then(|hotkeys| hotkeys.registration_error().map(str::to_owned))
            }),
            hotkeys,
            standalone_shelf,
            standalone_tasks,
            runtime_state,
            live_state: LiveState::default(),
            presented: false,
            retained_workspace: false,
            hidden_at: None,
            retention_task: None,
            resize_pending: false,
            clipboard_search_task: None,
            preview_selection_task: None,
            window_handle: window.window_handle(),
            _editor_sync_guard: editor_sync_guard,
            _input_subscription,
            _key_interceptor: key_interceptor,
            _window_appearance_subscription: window_appearance_subscription,
            _window_display_subscription: window_display_subscription,
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
        };
        view
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
        let mut applied = self.model.apply_shared(state);
        if self.apply_pending_current_app_scope(cx) {
            applied.rows_changed = true;
        }
        if applied.input_changed {
            self.sync_launcher_editor(self.model.query.clone(), cx);
        }
        if applied.rows_changed || applied.scopes_changed {
            self.reveal_selected_scope();
        }
        self.notice = None;
        self.try_start_pending_action_shortcut(cx);
        self.resume_web_surface_if_needed(cx);
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

    fn toggle_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let leaving_settings = self.is_settings_surface();
        self.screen = if self.screen == Screen::Launcher {
            Screen::Settings
        } else {
            Screen::Launcher
        };
        self.notice = None;
        self.resize_window_for_screen(window, cx);
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
            Screen::WebTool => {
                return gpui_kit::size(
                    px(crate::ui::TASKS_WINDOW_WIDTH),
                    px(crate::ui::TASKS_WINDOW_HEIGHT),
                );
            }
            Screen::WebPackages | Screen::UnsupportedTool => {
                return gpui_kit::size(px(LAUNCHER_WIDTH), px(480.));
            }
            Screen::ClipboardShelf => {
                return gpui_kit::size(px(480.), px(520.));
            }
            Screen::Tasks => {
                return gpui_kit::size(
                    px(crate::ui::TASKS_WINDOW_WIDTH),
                    px(crate::ui::TASKS_WINDOW_HEIGHT),
                );
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

    fn effective_window_size(
        &self,
        window: &Window,
        cx: &gpui_kit::App,
    ) -> gpui_kit::Size<gpui_kit::Pixels> {
        let natural = self.desired_window_size();
        let available = window
            .display(cx)
            .map(|display| display.visible_bounds().size)
            .unwrap_or(natural);
        effective_window_size(natural, available)
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

    fn sync_launcher_window_after_render(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.resize_pending || !self.should_sync_launcher_size() {
            return;
        }
        let size = self.effective_window_size(window, cx);
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
        let tool_scroll = match self.screen {
            Screen::FileSearch => Some(&self.tool_list_scrolls.file_search),
            Screen::Clipboard => Some(&self.tool_list_scrolls.clipboard),
            Screen::ClipboardShelf => Some(&self.tool_list_scrolls.clipboard_shelf),
            Screen::ApplicationHistory => Some(&self.tool_list_scrolls.history),
            Screen::Tasks => Some(&self.tool_list_scrolls.tasks),
            _ => None,
        };
        if let Some(scroll) = tool_scroll {
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
            scroll.scroll_to_item(index, gpui_kit::ScrollStrategy::Nearest);
        } else {
            let scroll = match self.screen {
                Screen::Launcher => &self.result_scroll,
                Screen::WebPackages => &self.web_packages_scroll,
                _ => return,
            };
            scroll.scroll_to_item(index);
        }
    }

    pub(super) fn refresh_editor_placeholders(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let locale = self.locale;
        self.input_placeholder = self.desired_search_placeholder();
        let search_placeholder = self.input_placeholder.clone();
        self.input.update(cx, |input, cx| {
            input.set_placeholder(search_placeholder, window, cx)
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

    pub(crate) fn update_tool_locale(
        &mut self,
        locale: Locale,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.locale = locale;
        self.model.set_locale(locale);
        if self.standalone_tasks {
            self.tasks_filter_input.update(cx, |input, cx| {
                input.set_placeholder(l10n::find_atask(locale), window, cx)
            });
        } else {
            self.clipboard_input.update(cx, |input, cx| {
                input.set_placeholder(l10n::search_clipboard(locale), window, cx)
            });
        }
        cx.notify();
    }

    pub fn platform_appearance_changed(&self, cx: &mut Context<Self>) {
        let Some(handle) = self
            .web_surface
            .document
            .as_ref()
            .map(|document| document.handle.clone())
        else {
            cx.notify();
            return;
        };
        if let Some(web_view) = self.web_surface.view.as_ref() {
            let web_theme = if theme::glass_surface_is_light() {
                wry::Theme::Light
            } else {
                wry::Theme::Dark
            };
            if let Err(error) = web_view.read(cx).set_theme(web_theme) {
                tracing::warn!(%error, "web_theme_update_failed");
            }
        }
        if event_handle_matches(self, &handle) {
            let _ = self
                .evaluate_web_script(&canvas_script(!crate::platform::uses_opaque_fallback()), cx);
            let _ =
                self.evaluate_web_script(&web_theme_script(theme::glass_surface_is_light()), cx);
        }
        cx.notify();
    }

    fn window_appearance_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = if self.standalone_shelf {
            crate::CLIPBOARD_SHELF_TITLE
        } else if self.standalone_tasks {
            crate::TASKS_WINDOW_TITLE
        } else {
            crate::PRODUCT_NAME
        };
        crate::host::defer_native_window_update(
            cx.weak_entity(),
            window.window_handle(),
            cx,
            move || crate::platform::refresh_window_material(title),
            |this, window, cx, refreshed| {
                if refreshed {
                    crate::app::install_component_theme(cx);
                }
                cx.set_reduce_motion(crate::platform::uses_reduce_motion());
                this.platform_appearance_changed(cx);
                this.resize_current_window(cx);
                window.refresh();
            },
        );
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

    fn should_sync_launcher_size(&self) -> bool {
        let waiting_for_results = self.screen == Screen::Launcher
            && self.live_state.awaiting_query_layout
            && !self.root_navigation_open
            && !self.model.object_actions_active
            && self.model.menu_path().is_none();
        (self.presented || self.runtime_state == RuntimeState::Preview)
            && matches!(self.screen, Screen::Launcher | Screen::FileSearch)
            && !self.standalone_shelf
            && !self.standalone_tasks
            && !waiting_for_results
    }

    fn resize_current_window(&mut self, cx: &mut Context<Self>) {
        if self.resize_pending || !self.should_sync_launcher_size() {
            return;
        }
        self.resize_pending = true;
        // Coalesce changes before the window borrow ends and use the latest layout.
        let view = cx.weak_entity();
        let window_handle = self.window_handle;
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = view.update(cx, |this, cx| {
                    this.resize_pending = false;
                    if this.should_sync_launcher_size() {
                        this.resize_window_for_screen(window, cx);
                    }
                });
            });
        });
    }

    fn resize_window_for_screen(&self, window: &mut Window, cx: &mut Context<Self>) {
        let size = self.effective_window_size(window, cx);
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
        self.file_search.cancel_page();
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
        self.live_state.awaiting_query_layout = self.screen == Screen::Launcher
            && self.live_state.snapshot_gate.is_some()
            && self.model.query_differs_from_snapshot();
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
                self.close_web_surface(cx);
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
                    self.poll_web_document(receiver, cx);
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
        }
    }

    fn handle_settings_escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.handle_settings_modal_escape(window, cx) {
            return;
        }
        if self.settings_action_picker_open {
            self.close_action_shortcut_picker(cx);
            self.settings_focus_handle.focus(window, cx);
        } else {
            self.request_close_settings(window, cx);
        }
    }

    fn settings_modal_active(&self) -> bool {
        self.screen == Screen::Settings
            && (self.settings_close_pending
                || self.clipboard_confirmation.is_some()
                || self.settings_action_picker_open)
    }

    fn handle_settings_modal_escape(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.settings_modal_active() {
            return false;
        }
        self.shortcut_recording = None;
        if self.clipboard_confirmation.is_some() {
            self.cancel_clipboard_confirmation(window, cx);
        } else if self.settings_action_picker_open {
            self.close_action_shortcut_picker(cx);
            self.settings_focus_handle.focus(window, cx);
        } else {
            self.cancel_settings_close_prompt(window, cx);
        }
        true
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

    fn reset_file_search_browsing(&mut self, cx: &mut Context<Self>) {
        self.file_search.advance_from = None;
        if self.screen != Screen::FileSearch {
            return;
        }
        self.file_search.browsing = false;
        self.model.selected = 0;
        self.close_preview(cx);
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
        let previous_selection = self.model.selected;
        if self.file_search.browsing {
            if direction < 0 {
                self.model.select_previous();
            } else {
                if self.model.selected + 1 >= self.model.rows.len()
                    && self
                        .model
                        .file_search_progress()
                        .is_some_and(|progress| progress.next_cursor.is_some())
                {
                    self.request_more_files(true, cx);
                    return;
                }
                self.model.select_next();
            }
        } else {
            self.file_search.browsing = true;
            self.model.select_first();
        }
        if self.model.selected != previous_selection {
            self.file_search.advance_from = None;
        }
        self.reveal_result_row(self.model.selected);
        self.notice = None;
        if self.preview_open {
            self.schedule_selected_preview(cx);
        }
        cx.notify();
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
                .overflow_y_scroll();
            for (index, row) in self.model.rows.clone().into_iter().enumerate() {
                list = list.child(self.render_row(index, row, self.model.selected == index, cx));
            }
            list.vertical_scrollbar(&self.result_scroll)
                .into_any_element()
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
                    .h(px(self.launcher_input_height() - metrics::SPACE_MD))
                    .flex_shrink_0()
                    .mx(px(metrics::SPACE_SM))
                    .my(px(metrics::SPACE_MD / 2.))
                    .px(px(metrics::SPACE_XL - metrics::SPACE_SM))
                    .flex()
                    .items_center()
                    .rounded(px(metrics::RESULT_RADIUS))
                    .bg(theme::result_surface(false))
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
                    .child(if self.model.loading && self.presented {
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
                            .child(components::notice(notice, self.notice_kind))
                            .vertical_scrollbar(&self.notice_scroll),
                    ),
            );
        }
        if has_panel {
            root = root.child(rows);
        }
        root.into_any_element()
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
                theme::window()
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

fn accepted_web_acknowledgement_values(
    request: &serde_json::Value,
    response: &serde_json::Value,
) -> Option<String> {
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum PreviewTarget {
    File { candidate_id: String, revision: u32 },
    Clipboard(i64),
}

#[derive(Clone, Debug)]
struct PreviewRequest {
    generation: u64,
    session_generation: u64,
    screen: Screen,
    target: PreviewTarget,
}

enum PreviewReservation {
    Ready(PreviewHost, u64),
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
mod tests;
