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
    Focusable as _, FontWeight, IntoElement, MouseButton, ObjectFit, Render, ScrollHandle,
    SharedString, Styled, Subscription, TestSupportExt as _, Window,
};

use crate::{
    host::{
        registered_preferences, ActionShortcutBinding, ActionShortcutRoute, HostPreferences,
        HotKeyController, LiveEvent, LiveLauncher, ShortcutCatalogAction, UiLanguage, UiTheme,
    },
    l10n::{self, Locale},
    launcher::{
        AppHistoryPageModel, ClipboardKindFilter, ClipboardPageModel, FileSearchPageModel,
        TasksPageModel, WebPageModel,
    },
    platform::{open_privacy_settings, PreviewHost, PreviewPoll, PrivacySettingsPane},
    settings::{SettingsCategory, SettingsModel},
    ui::{
        design_tokens::{fonts, metrics},
        styles::AppStyle as _,
        theme, LAUNCHER_ROW_HEIGHT, LAUNCHER_WIDTH, SETTINGS_HEIGHT,
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
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc::Receiver, oneshot};

mod actions;
#[path = "controllers/activation_prefill.rs"]
mod activation_prefill_controller;
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
mod state;
#[path = "controllers/streams.rs"]
mod streams_controller;
#[path = "controllers/tasks.rs"]
mod tasks_controller;
mod tool_window;
#[path = "controllers/web.rs"]
mod web_controller;
mod web_surface;

pub(crate) use actions::bind_keys;
use live_state::LiveState;
use state::{
    ActivationPrefill, HudToastState, IconLoader, PreviewState, ScrollHandles, SettingsEditor,
    ShortcutEditor, WindowPresence,
};
use web_surface::{ActiveWebBinding, WebIpcRequest, WebSurfaceState};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewKind {
    Main,
    ToolWindow,
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
    Dismiss,
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

impl KeyModifiers {
    fn primary(self) -> bool {
        crate::platform::primary_modifier_only(self.command, self.control)
    }

    fn primary_only(self) -> bool {
        self.primary() && !self.alt && !self.shift
    }
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
    web: WebPageModel,
    web_surface: WebSurfaceState,
    icons: IconLoader,
    object_action_generation: u64,
    preferences: HostPreferences,
    shortcuts: ShortcutEditor,
    pending_shortcut_execution: Option<ActionShortcutBinding>,
    clipboard_confirmation: Option<ClipboardConfirmation>,
    parent_workspace: Option<ParentWorkspace>,
    input_mode: InputMode,
    scope_selection_intent: ScopeSelectionIntent,
    input_placeholder: String,
    root_navigation_open: bool,
    root_navigation_mode: InputMode,
    scrolls: ScrollHandles,
    settings_editor: SettingsEditor,
    screen: Screen,
    locale: Locale,
    notice: Option<String>,
    notice_kind: components::NoticeKind,
    hud_toast: HudToastState,
    preview: PreviewState,
    live: Option<Arc<LiveLauncher>>,
    hotkeys: Option<HotKeyController>,
    hotkey_error: Option<String>,
    standalone_shelf: bool,
    standalone_tasks: bool,
    standalone_tool: bool,
    tool_windows: Vec<tool_window::OpenToolWindow>,
    runtime_state: RuntimeState,
    live_state: LiveState,
    activation_prefill: ActivationPrefill,
    presence: WindowPresence,
    clipboard_search_task: Option<gpui_kit::Task<()>>,
    window_handle: AnyWindowHandle,
    surface_focus_handle: FocusHandle,
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
        hotkeys: HotKeyController,
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
        hotkeys: Option<HotKeyController>,
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
            &self.settings_editor.excluded_apps_input,
            preferences.excluded_apps.join("\n"),
            window,
            cx,
        );
        set_input_value(
            &self.settings_editor.file_roots_input,
            preferences.file_search_roots.join("\n"),
            window,
            cx,
        );
        set_input_value(
            &self.settings_editor.file_exclusions_input,
            preferences.file_search_excluded_names.join("\n"),
            window,
            cx,
        );
        set_input_value(
            &self.shortcuts.current_input,
            preferences.current_app_shortcut.clone(),
            window,
            cx,
        );
        set_input_value(
            &self.shortcuts.universal_input,
            preferences.universal_shortcut.clone(),
            window,
            cx,
        );
        self.locale = locale;
        self.model.set_locale(locale);
        self.refresh_editor_placeholders(window, cx);
        self.preferences = preferences.clone();
        self.settings_editor.draft = preferences;
        self.settings_editor.model = settings;
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
        self.hotkeys.as_ref()?.action_for_event(event)
    }

    fn hotkey_error_message(&self) -> Option<String> {
        self.hotkey_error
            .as_ref()
            .map(|error| l10n::global_shortcut_registration_failed(self.locale, error))
    }

    fn registered_preferences(&self) -> HostPreferences {
        registered_preferences(&self.preferences, self.hotkeys.as_ref())
    }

    fn build(
        window: &mut Window,
        cx: &mut Context<Self>,
        model: LauncherModel,
        locale: Locale,
        live: Option<Arc<LiveLauncher>>,
        hotkeys: Option<HotKeyController>,
        kind: ViewKind,
    ) -> Self {
        actions::bind_keys(cx);
        let standalone_shelf = kind == ViewKind::ClipboardShelf;
        let standalone_tasks = kind == ViewKind::Tasks;
        let auxiliary = standalone_tasks || standalone_shelf;
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
        let (shortcuts, mut page_input_subscriptions) = ShortcutEditor::new(
            auxiliary || kind == ViewKind::ToolWindow,
            &input,
            &preferences,
            locale,
            window,
            cx,
        );
        let (settings_editor, history_filter_subscription) = SettingsEditor::new(
            auxiliary || kind == ViewKind::ToolWindow,
            &input,
            settings,
            &preferences,
            locale,
            window,
            cx,
        );
        let (_input_subscription, mut editor_subscriptions) = Self::build_editor_subscriptions(
            &input,
            &content_input,
            &clipboard_input,
            &history_input,
            &tasks_filter_input,
            &editor_sync_guard,
            cx,
        );
        page_input_subscriptions.append(&mut editor_subscriptions);
        page_input_subscriptions.push(history_filter_subscription);
        let key_interceptor = Self::build_key_interceptor(cx);
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
            web: WebPageModel::default(),
            web_surface: WebSurfaceState::default(),
            icons: IconLoader::default(),
            object_action_generation: 0,
            preferences,
            shortcuts,
            pending_shortcut_execution: None,
            clipboard_confirmation: None,
            parent_workspace: None,
            input_mode: InputMode::Environment,
            scope_selection_intent: ScopeSelectionIntent::PendingCurrentApp,
            input_placeholder: l10n::search_apps_actions(locale),
            root_navigation_open: false,
            root_navigation_mode: InputMode::Environment,
            scrolls: ScrollHandles::default(),
            settings_editor,
            screen: Screen::Launcher,
            locale,
            notice: search_access_error,
            notice_kind: components::NoticeKind::Error,
            hud_toast: HudToastState::default(),
            preview: PreviewState::default(),
            live,
            hotkey_error: hotkeys
                .as_ref()
                .and_then(HotKeyController::registration_error)
                .map(str::to_owned),
            hotkeys,
            standalone_shelf,
            standalone_tasks,
            standalone_tool: kind == ViewKind::ToolWindow,
            tool_windows: Vec::new(),
            runtime_state,
            live_state: LiveState::default(),
            activation_prefill: ActivationPrefill::default(),
            presence: WindowPresence::default(),
            clipboard_search_task: None,
            window_handle: window.window_handle(),
            surface_focus_handle: cx.focus_handle(),
            _editor_sync_guard: editor_sync_guard,
            _input_subscription,
            _key_interceptor: key_interceptor,
            _window_appearance_subscription: window_appearance_subscription,
            _window_display_subscription: window_display_subscription,
            _page_input_subscriptions: page_input_subscriptions,
        }
    }

    fn build_editor_subscriptions(
        input: &Entity<InputState>,
        content_input: &Entity<TextareaState>,
        clipboard_input: &Entity<InputState>,
        history_input: &Entity<InputState>,
        tasks_filter_input: &Entity<InputState>,
        editor_sync_guard: &Rc<Cell<bool>>,
        cx: &mut Context<Self>,
    ) -> (Subscription, Vec<Subscription>) {
        let input_sync_guard = editor_sync_guard.clone();
        let input_edits_subscription =
            cx.subscribe(input, |this: &mut Self, _, event: &InputEditsEvent, cx| {
                if this.screen != Screen::FileSearch
                    || (!this.preview.open && !this.file_search_selection_active())
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
                if this.preview.open {
                    this.close_preview(cx);
                } else {
                    this.request_preview(cx);
                }
            });
        let input_subscription =
            cx.subscribe(input, move |this: &mut Self, _, event: &InputEvent, cx| {
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
            content_input,
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
            clipboard_input,
            |this: &mut Self, _, event: &InputEditsEvent, cx| {
                if !matches!(this.screen, Screen::Clipboard | Screen::ClipboardShelf)
                    || (!this.preview.open && !this.clipboard_selection_active())
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
                if this.preview.open {
                    this.close_preview(cx);
                } else {
                    this.preview_selected_clipboard(cx);
                }
            },
        );
        let clipboard_subscription = cx.subscribe(
            clipboard_input,
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
            history_input,
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
            tasks_filter_input,
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
                this.tasks.pending_selection = None;
                this.tasks.set_filter(filter);
                this.refresh_selected_task_detail(cx);
                cx.notify();
            },
        );
        (
            input_subscription,
            vec![
                clipboard_subscription,
                history_subscription,
                input_edits_subscription,
                clipboard_edits_subscription,
                content_subscription,
                tasks_filter_subscription,
            ],
        )
    }

    fn build_key_interceptor(cx: &mut Context<Self>) -> Subscription {
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
                this.intercept_window_keystroke(event, modifiers, repeated, window, cx);
            });
        })
    }

    fn intercept_window_keystroke(
        &mut self,
        event: &gpui_kit::KeystrokeEvent,
        modifiers: KeyModifiers,
        repeated: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.window_handle != window.window_handle() {
            return;
        }
        if self.settings_modal_active() {
            self.intercept_settings_modal_keystroke(event, modifiers, repeated, window, cx);
            return;
        }
        if self.shortcuts.recording.is_some() {
            self.handle_key_with_repeat(
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
        if self.screen == Screen::Launcher
            && self.input_mode == InputMode::Content
            && !modifiers.modified
            && matches!(
                event.keystroke.key.as_str(),
                "up" | "down" | "enter" | "return"
            )
            && self.active_editor_focused(window, cx)
        {
            self.handle_key_with_repeat(
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
        ) {
            return;
        }
        let input = input::KeyInput {
            key: event.keystroke.key.as_str(),
            modifiers,
            shortcut: normalized_shortcut(
                event.keystroke.key.as_str(),
                modifiers.command,
                modifiers.control,
                modifiers.alt,
                modifiers.shift,
            ),
            repeated,
        };
        if self.filter_repeated_key(&input) == input::KeyFlow::Handled
            || self.handle_overlay_keys(&input, window, cx) == input::KeyFlow::Handled
            || self.handle_user_shortcut_key(&input, cx) == input::KeyFlow::Handled
        {
            window.prevent_default();
            cx.stop_propagation();
            return;
        }
        if self.screen == Screen::Settings
            && !self.preview.open
            && event.keystroke.key.eq_ignore_ascii_case("escape")
        {
            self.handle_key_with_repeat(
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
        if self.screen == Screen::Settings
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
        if is_tool_page_tab_shortcut(self.screen, event.keystroke.key.as_str(), modifiers) {
            window.prevent_default();
            cx.stop_propagation();
            return;
        }
        if self.handle_preview_key_overlay(&input, window, cx) == input::KeyFlow::Handled
            || (input.modifiers.primary_only()
                && input.key.eq_ignore_ascii_case("y")
                && !self.root_navigation_open
                && self.toggle_preview_shortcut(window, cx))
        {
            window.prevent_default();
            cx.stop_propagation();
            return;
        }
        if self.handle_preview_keystroke(
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
        if matches!(
            event.input_preference,
            gpui_kit::InputPreference::CharacterInput
        ) || !is_scope_tab_shortcut(event.keystroke.key.as_str(), modifiers)
            || !self.active_editor_focused(window, cx)
            || !(self.screen == Screen::FileSearch
                || (self.screen == Screen::Launcher
                    && self.has_scope_bar()
                    && !self.model.object_actions_active
                    && self.model.menu_path().is_none()))
        {
            return;
        }
        self.handle_key_with_repeat(
            event.keystroke.key.as_str(),
            modifiers,
            repeated,
            window,
            cx,
        );
        window.prevent_default();
        cx.stop_propagation();
    }

    fn intercept_settings_modal_keystroke(
        &mut self,
        event: &gpui_kit::KeystrokeEvent,
        modifiers: KeyModifiers,
        repeated: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if is_scope_tab_shortcut(event.keystroke.key.as_str(), modifiers) {
            self.cycle_settings_modal_focus(event.keystroke.modifiers.shift, window, cx);
            window.prevent_default();
            cx.stop_propagation();
            return;
        }
        if !modifiers.modified && event.keystroke.key.eq_ignore_ascii_case("escape") {
            self.handle_settings_modal_escape(window, cx);
            window.prevent_default();
            cx.stop_propagation();
            return;
        }
        if self.settings_editor.action_picker_open {
            if self.shortcuts.recording.is_some() {
                self.handle_key_with_repeat(
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
                &self.settings_editor.keep_editing_focus,
                &self.settings_editor.discard_focus,
                &self.settings_editor.save_focus,
                &self.settings_editor.confirm_cancel_focus,
                &self.settings_editor.confirm_accept_focus,
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
        let previous_index = self.model.selected;
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
        if self.screen == Screen::Launcher
            && self.model.selected != previous_index
            && self.model.selection_id().is_some()
        {
            self.reveal_result_row(self.model.selected);
        }
        self.notice = None;
        self.try_start_pending_action_shortcut(cx);
        self.maybe_request_activation_prefill(cx);
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
                self.model.clear_rows_until_snapshot();
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
        self.model.clear_rows_until_snapshot();
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
            self.model.clear_rows_until_snapshot();
            return;
        };
        self.scope_selection_intent = ScopeSelectionIntent::UserControlled;
        self.model.select_scope(Some(current_app.id));
    }

    fn mark_scope_user_controlled(&mut self) {
        self.scope_selection_intent = ScopeSelectionIntent::UserControlled;
        self.cancel_activation_prefill();
    }

    fn mark_query_user_controlled(&mut self) {
        self.cancel_activation_prefill();
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

    pub(super) fn close_settings_to_background(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.prepare_hide(cx);
        crate::platform::hide_main_window();
        self.screen = Screen::Launcher;
        self.notice = None;
        self.settings_editor.dismiss_on_done = false;
        self.resize_window_for_screen(window, cx);
        crate::platform::configure_main_window_after_update_then(PRODUCT_NAME, || {});
        cx.notify();
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

    pub(super) fn show_hud_toast(
        &mut self,
        message: impl Into<String>,
        icon: IconName,
        kind: components::NoticeKind,
        cx: &mut Context<Self>,
    ) {
        self.hud_toast.generation = self.hud_toast.generation.wrapping_add(1);
        let generation = self.hud_toast.generation;
        self.hud_toast.message = Some(message.into());
        self.hud_toast.icon = Some(icon);
        self.hud_toast.kind = kind;
        self.hud_toast.task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1600))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.hud_toast.generation == generation {
                    this.hud_toast.message = None;
                    this.hud_toast.icon = None;
                    this.hud_toast.task = None;
                    cx.notify();
                }
            });
        }));
    }

    fn sync_launcher_window_after_render(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.presence.resize_pending || !self.should_sync_launcher_size() {
            return;
        }
        if self.standalone_tool {
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
            self.scrolls
                .scope
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
        self.scrolls
            .file_filter
            .scroll_to_item(self.selected_file_filter_index());
    }

    fn reveal_result_row(&self, index: usize) {
        let tool_scroll = match self.screen {
            Screen::Launcher => Some(&self.scrolls.tool_lists.launcher),
            Screen::FileSearch => Some(&self.scrolls.tool_lists.file_search),
            Screen::Clipboard => Some(&self.scrolls.tool_lists.clipboard),
            Screen::ClipboardShelf => Some(&self.scrolls.tool_lists.clipboard_shelf),
            Screen::ApplicationHistory => Some(&self.scrolls.tool_lists.history),
            Screen::Tasks => Some(&self.scrolls.tool_lists.tasks),
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
                Screen::WebPackages => &self.scrolls.web_packages,
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
        self.settings_editor
            .history_filter_input
            .update(cx, |input, cx| {
                input.set_placeholder(l10n::app_history_filter_sources(locale), window, cx)
            });
        self.shortcuts.action_input.update(cx, |input, cx| {
            input.set_placeholder(l10n::find_an_action(locale), window, cx)
        });
        self.shortcuts.context_name_input.update(cx, |input, cx| {
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
        self.refresh_editor_placeholders(window, cx);
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
            let web_theme = if theme::surface_is_light() {
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
            let _ = self.evaluate_web_script(
                &web_theme_script(theme::palette(), theme::fixed(), theme::surface_is_light()),
                cx,
            );
        }
        cx.notify();
    }

    fn window_appearance_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.standalone_tool {
            crate::host::defer_native_window_update(
                cx.weak_entity(),
                window.window_handle(),
                cx,
                crate::platform::refresh_tool_window_materials,
                |this, window, cx, refreshed| {
                    if refreshed {
                        crate::app::install_component_theme(cx);
                    }
                    cx.set_reduce_motion(crate::platform::uses_reduce_motion());
                    this.platform_appearance_changed(cx);
                    window.refresh();
                },
            );
            return;
        }
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
        (self.presence.presented || self.runtime_state == RuntimeState::Preview)
            && matches!(self.screen, Screen::Launcher | Screen::FileSearch)
            && !self.standalone_tool
            && !self.standalone_shelf
            && !self.standalone_tasks
            && !waiting_for_results
    }

    fn resize_current_window(&mut self, cx: &mut Context<Self>) {
        if self.presence.resize_pending || !self.should_sync_launcher_size() {
            return;
        }
        self.presence.resize_pending = true;
        // Coalesce changes before the window borrow ends and use the latest layout.
        let view = cx.weak_entity();
        let window_handle = self.window_handle;
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = view.update(cx, |this, cx| {
                    this.presence.resize_pending = false;
                    if this.should_sync_launcher_size() {
                        this.resize_window_for_screen(window, cx);
                    }
                });
            });
        });
    }

    fn resize_window_for_screen(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.standalone_tool {
            return;
        }
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
        self.cancel_activation_prefill();
        if self.screen == Screen::FileSearch && !self.file_search.browsing {
            return;
        }
        if self.model.open_selected_menu() {
            self.notice = None;
            self.resize_current_window(cx);
            cx.notify();
            return;
        }
        if let (Some(live), Some(row)) = (
            self.live.clone(),
            self.model.selected_row().map(std::borrow::Cow::into_owned),
        ) {
            if row.enabled {
                self.set_notice(
                    l10n::running_action(self.locale, &row.title),
                    components::NoticeKind::Info,
                );
                let receiver = live.execute(self.model.revision, row.id.to_string());
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
        let dismiss = dismiss && !self.standalone_tool;
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
        if self.settings_editor.action_picker_open {
            self.close_action_shortcut_picker(cx);
            self.settings_editor.focus_handle.focus(window, cx);
        } else {
            self.request_close_settings(window, cx);
        }
    }

    fn settings_modal_active(&self) -> bool {
        self.is_settings_surface()
            && (self.settings_editor.close_pending
                || self.clipboard_confirmation.is_some()
                || self.settings_editor.action_picker_open)
    }

    fn handle_settings_modal_escape(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.settings_modal_active() {
            return false;
        }
        self.shortcuts.recording = None;
        if self.clipboard_confirmation.is_some() {
            self.cancel_clipboard_confirmation(window, cx);
        } else if self.settings_editor.action_picker_open {
            self.close_action_shortcut_picker(cx);
            self.settings_editor.focus_handle.focus(window, cx);
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
            && self.model.selection_id().is_some()
    }

    fn selected_row_for_action(&self) -> Option<std::borrow::Cow<'_, LauncherRow>> {
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
        if self.preview.open {
            self.schedule_selected_preview(cx);
        }
        cx.notify();
    }

    fn show_object_actions(&mut self, cx: &mut Context<Self>) {
        if self.screen == Screen::FileSearch && !self.file_search.browsing {
            return;
        }
        let Some(row) = self.model.selected_row().map(std::borrow::Cow::into_owned) else {
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
            let receiver = live.object_actions(self.model.revision, row.id.to_string());
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
        let has_scope_bar = self.has_scope_bar();
        let has_panel = self.launcher_has_panel();
        let scope_controls = has_scope_bar.then(|| self.render_launcher_scope_bar(cx));
        let result_heading = self.result_heading();
        let rows = self.render_launcher_rows(cx);
        let search_header = self.render_launcher_search_header(cx);

        let mut root = div()
            .id("launcher-surface")
            .key_context("Launcher")
            .track_focus(&self.surface_focus_handle)
            .on_action(cx.listener(Self::on_action_toggle_preview))
            .on_action(cx.listener(Self::on_action_close_preview_to_editor))
            .on_action(cx.listener(Self::on_action_toggle_object_actions))
            .on_action(cx.listener(Self::on_action_toggle_root_navigation))
            .size_full()
            .flex()
            .flex_col()
            .child(search_header);
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
                            .track_scroll(&self.scrolls.notice)
                            .overflow_y_scroll()
                            .child(components::notice(notice, self.notice_kind))
                            .vertical_scrollbar(&self.scrolls.notice),
                    ),
            );
        }
        if has_panel {
            root = root.child(rows);
        }
        root.into_any_element()
    }

    fn render_launcher_scope_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let selected_scope = self.model.selected_scope.as_deref();
        div()
            .id("launcher-scope-scroll")
            .h(px(metrics::PAGE_TOOLBAR_HEIGHT))
            .flex_shrink_0()
            .px(px(metrics::SPACE_LG))
            .flex()
            .items_center()
            .overflow_x_scroll()
            .track_scroll(&self.scrolls.scope)
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
            .into_any_element()
    }

    fn render_launcher_rows(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if self.model.loading {
            div()
                .h(px(64.))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(metrics::SPACE_SM))
                .text_size(px(metrics::BODY_SIZE))
                .text_color(theme::secondary())
                .child(Spinner::new().xsmall().color(theme::accent()))
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
            div()
                .flex_1()
                .min_h(px(0.))
                .relative()
                .child(
                    gpui_kit::uniform_list(
                        "launcher-results",
                        self.model.rows.len(),
                        cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                            range
                                .map(|index| {
                                    this.render_row(index, this.model.selected == index, cx)
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .size_full()
                    .px(px(metrics::ROW_PADDING_X))
                    .py(px(metrics::SPACE_XS))
                    .track_scroll(&self.scrolls.tool_lists.launcher),
                )
                .vertical_scrollbar(&self.scrolls.launcher_base())
                .into_any_element()
        }
    }

    fn render_launcher_search_header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let mode_label = self.header_mode_label();
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
            .child(if self.model.loading && self.presence.presented {
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
            })
            .into_any_element()
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
            Screen::Clipboard => self.render_clipboard(cx),
            Screen::ClipboardShelf => self.render_clipboard_shelf(cx),
            Screen::ApplicationHistory => self.render_app_history(cx),
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
            .key_context("Anywhere")
            .on_action(cx.listener(Self::on_action_open_settings))
            .on_action(cx.listener(Self::on_action_quit_app))
            .rounded(crate::platform::window_content_corner_radius())
            .overflow_hidden()
            .bg(if crate::platform::window_uses_opaque_background(window) {
                theme::fallback_surface()
            } else {
                theme::window()
            })
            .text_color(theme::primary())
            .font_family(fonts::FONT_UI)
            .font_weight(FontWeight::NORMAL)
            .on_key_down(
                cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                    if this.shortcuts.recording.is_some()
                        || (this.screen == Screen::Settings
                            && !this.preview.open
                            && event.keystroke.key.eq_ignore_ascii_case("escape"))
                        || (this.screen == Screen::Settings
                            && event.keystroke.key.eq_ignore_ascii_case("tab"))
                        || (this.preview.open && event.keystroke.key.eq_ignore_ascii_case("escape"))
                    {
                        return;
                    }
                    if event.prefer_character_input && this.shortcuts.recording.is_none() {
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
            .when(
                self.is_settings_surface() && self.settings_editor.close_pending,
                |root| root.child(self.render_settings_close_prompt(cx)),
            )
            .when_some(self.hud_toast.message.clone(), |element, message| {
                let icon = self.hud_toast.icon.unwrap_or(IconName::Check);
                let tint = match self.hud_toast.kind {
                    components::NoticeKind::Success => theme::success(),
                    components::NoticeKind::Warning => theme::warning(),
                    components::NoticeKind::Error => theme::danger(),
                    components::NoticeKind::Info => theme::accent(),
                };
                element.child(
                    div()
                        .absolute()
                        .bottom(px(metrics::SPACE_XL))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(components::hud_toast(message, icon, tint)),
                )
            })
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
