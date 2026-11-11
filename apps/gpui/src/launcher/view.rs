use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        input::{Input, InputEditsEvent, InputEvent, InputState},
        scroll::ScrollableElement as _,
        switch::Switch,
        Disableable as _,
    },
    div, img,
    prelude::*,
    px, AnyElement, AnyWindowHandle, Context, Div, Entity, FontWeight, Image, ImageFormat,
    IntoElement, ObjectFit, Render, SharedString, Styled, Subscription, TestSupportExt as _,
    Window,
};

use crate::{
    host::{
        HostPreferences, HotKeyController, LiveEvent, LiveLauncher, UiLanguage, WebDocument,
        WebPackageReview,
    },
    l10n::{self, Locale},
    launcher::{
        AppHistoryPageModel, ClipboardPageModel, FileSearchPageModel, TasksPageModel, WebPageModel,
    },
    platform::{PreviewHost, PreviewPoll},
    settings::{SettingsCategory, SettingsModel},
    ui::{
        design_tokens::{fonts, metrics},
        theme, LAUNCHER_CORNER_RADIUS, LAUNCHER_ROW_HEIGHT, LAUNCHER_WIDTH, SETTINGS_HEIGHT,
    },
    web_tool::{
        bootstrap_script, plain_response, reply_script, state_script, visibility_script,
        WebDocumentPolicy,
    },
};

use super::{LauncherModel, LauncherRow, SnapshotGate};
use anywhere_protocol::{CandidateKind, InputMode};

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc::UnboundedReceiver as Receiver;

const WEB_FRAME_BYTES: usize = 64 * 1024;

struct WebIpcRequest {
    handle: String,
    origin: String,
    frame: String,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LauncherEvent {
    OpenClipboardShelf,
}

impl gpui_kit::EventEmitter<LauncherEvent> for LauncherView {}

/// The first GPUI vertical slice. It owns only visual state and delegates all
/// matching, ranking and authorization to a `LauncherState` snapshot.
pub struct LauncherView {
    input: Entity<InputState>,
    clipboard_input: Entity<InputState>,
    history_input: Entity<InputState>,
    model: LauncherModel,
    file_search: FileSearchPageModel,
    clipboard: ClipboardPageModel,
    app_history: AppHistoryPageModel,
    tasks: TasksPageModel,
    web: WebPageModel,
    web_review: Option<WebPackageReview>,
    web_document: Option<WebDocument>,
    web_view: Option<Entity<gpui_webview::WebView>>,
    web_ready: bool,
    web_error: Option<String>,
    web_state_handle: Option<String>,
    web_state_version: u64,
    plugin_detail: Option<anywhere_protocol::ui::DetailView>,
    unsupported_tool: Option<(String, String)>,
    application_icons: HashMap<String, Option<Arc<Image>>>,
    application_icons_requested: HashSet<String>,
    preferences: HostPreferences,
    settings_draft: HostPreferences,
    excluded_apps_input: Entity<InputState>,
    file_roots_input: Entity<InputState>,
    file_exclusions_input: Entity<InputState>,
    history_path_input: Entity<InputState>,
    current_shortcut_input: Entity<InputState>,
    universal_shortcut_input: Entity<InputState>,
    settings_close_pending: bool,
    input_mode: InputMode,
    settings: SettingsModel,
    screen: Screen,
    locale: Locale,
    notice: Option<String>,
    preview: Option<PreviewHost>,
    preview_busy: bool,
    preview_generation: u64,
    preview_open: bool,
    file_search_space_edit: Option<(String, usize)>,
    clipboard_space_edit: Option<(String, usize)>,
    live: Option<Arc<LiveLauncher>>,
    hotkeys: Option<Arc<Mutex<HotKeyController>>>,
    standalone_shelf: bool,
    file_search_browsing: bool,
    request_revision: u32,
    stream_generation: u64,
    snapshot_gate: Option<SnapshotGate>,
    window_handle: AnyWindowHandle,
    _input_subscription: Subscription,
    _page_input_subscriptions: Vec<Subscription>,
}

impl LauncherView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::build(
            window,
            cx,
            LauncherModel::preview(Locale::En),
            Locale::En,
            None,
            None,
            false,
        )
    }

    pub fn with_model(
        window: &mut Window,
        cx: &mut Context<Self>,
        model: LauncherModel,
        locale: Locale,
    ) -> Self {
        Self::build(window, cx, model, locale, None, None, false)
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
        Self::build(
            window,
            cx,
            LauncherModel::loading(locale),
            locale,
            Some(live),
            None,
            false,
        )
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
        Self::build(
            window,
            cx,
            LauncherModel::loading(locale),
            locale,
            Some(live),
            Some(hotkeys),
            false,
        )
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
        window.resize(view.desired_window_size());
        view
    }

    pub fn initialize_clipboard_shelf(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            return;
        };
        tracing::debug!("clipboard_shelf_initial_snapshot_requested");
        self.clipboard.loading();
        let receiver = live.clipboard_snapshot(self.clipboard.query.clone());
        self.poll(receiver, self.stream_generation, cx);
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
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(match locale {
                Locale::En => "Search apps and actions",
                Locale::Zh => "搜索应用和操作",
            })
        });
        let clipboard_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(match locale {
                Locale::En => "Search clipboard history",
                Locale::Zh => "搜索剪贴板历史",
            })
        });
        let history_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(match locale {
                Locale::En => "Search application history",
                Locale::Zh => "搜索应用历史",
            })
        });
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
                this.file_search_space_edit = Some((restored_value, cursor));
                if this.preview_open {
                    this.close_preview(cx);
                } else {
                    this.request_preview(cx);
                }
            });
        let _input_subscription =
            cx.subscribe(&input, |this: &mut Self, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some((restored_value, cursor)) = this.file_search_space_edit.take() {
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
                    this.model
                        .set_query(this.input.read(cx).value().to_string());
                    this.notice = this
                        .model
                        .selected_row()
                        .filter(|row| !row.enabled)
                        .map(|_| {
                            if this.locale == Locale::Zh {
                                "此操作暂时不可用".to_owned()
                            } else {
                                "This action is unavailable right now".to_owned()
                            }
                        });
                    this.request_live_update(cx);
                    cx.notify();
                }
            });
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
                this.clipboard_space_edit = Some((restored_value, cursor));
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
                    if let Some((restored_value, cursor)) = this.clipboard_space_edit.take() {
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
                    this.app_history.query = this.history_input.read(cx).value().to_string();
                    this.request_app_history_search(cx);
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
        let current_shortcut_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(preferences.current_app_shortcut.clone())
        });
        let universal_shortcut_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(preferences.universal_shortcut.clone())
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        let snapshot_gate = live
            .as_ref()
            .map(|live| SnapshotGate::new(live.session_id()));
        let mut view = Self {
            input,
            clipboard_input,
            history_input,
            model,
            file_search: FileSearchPageModel::default(),
            clipboard: ClipboardPageModel::default(),
            app_history: AppHistoryPageModel::default(),
            tasks: TasksPageModel::default(),
            web: WebPageModel::default(),
            web_review: None,
            web_document: None,
            web_view: None,
            web_ready: false,
            web_error: None,
            web_state_handle: None,
            web_state_version: 0,
            plugin_detail: None,
            unsupported_tool: None,
            application_icons: HashMap::new(),
            application_icons_requested: HashSet::new(),
            settings_draft: preferences.clone(),
            preferences,
            excluded_apps_input,
            file_roots_input,
            file_exclusions_input,
            history_path_input,
            current_shortcut_input,
            universal_shortcut_input,
            settings_close_pending: false,
            input_mode: InputMode::Environment,
            settings,
            screen: Screen::Launcher,
            locale,
            notice: search_access_error,
            preview: None,
            preview_busy: false,
            preview_generation: 0,
            preview_open: false,
            file_search_space_edit: None,
            clipboard_space_edit: None,
            live,
            hotkeys,
            standalone_shelf,
            file_search_browsing: false,
            request_revision: 0,
            stream_generation: 0,
            snapshot_gate,
            window_handle: window.window_handle(),
            _input_subscription,
            _page_input_subscriptions: vec![
                clipboard_subscription,
                history_subscription,
                input_edits_subscription,
                clipboard_edits_subscription,
            ],
        };
        if !standalone_shelf {
            if let Some(live) = view.live.clone() {
                view.begin_stream(live.activate(InputMode::Environment), cx);
            }
        }
        view
    }

    pub fn apply_state(&mut self, state: anywhere_protocol::LauncherState) {
        self.model.apply(state);
        self.notice = None;
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

    pub fn activate_from_hotkey(
        &mut self,
        input_mode: InputMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.screen = Screen::Launcher;
        self.input_mode = input_mode;
        self.model.hide_object_actions();
        self.model.selected_scope = None;
        self.notice = None;
        window.resize(self.desired_window_size());
        self.input.update(cx, |input, cx| input.focus(window, cx));
        if let Some(live) = self.live.clone() {
            self.begin_stream(live.activate(input_mode), cx);
        }
        cx.notify();
    }

    pub fn prepare_hide(&mut self, cx: &mut Context<Self>) {
        self.close_preview(cx);
        self.close_web_surface(cx);
    }

    fn close_web_surface(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = self.web_view.as_ref() {
            let _ = view
                .read(cx)
                .raw()
                .evaluate_script(&visibility_script(false));
        }
        if let (Some(live), Some(document)) = (self.live.clone(), self.web_document.as_ref()) {
            let _ = live.close_web_document(document.handle.clone());
        }
        self.web_document = None;
        self.web_view = None;
        self.web_ready = false;
        self.web_error = None;
        self.web_state_handle = None;
        self.web_state_version = 0;
    }

    fn toggle_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.screen = if self.screen == Screen::Launcher {
            Screen::Settings
        } else {
            Screen::Launcher
        };
        self.notice = None;
        window.resize(self.desired_window_size());
        cx.notify();
    }

    fn desired_window_size(&self) -> gpui_kit::Size<gpui_kit::Pixels> {
        match self.screen {
            Screen::Settings => {
                return gpui_kit::size(px(LAUNCHER_WIDTH), px(SETTINGS_HEIGHT));
            }
            Screen::Clipboard
            | Screen::ApplicationHistory
            | Screen::WebPackages
            | Screen::WebTool
            | Screen::PluginDetail
            | Screen::UnsupportedTool => {
                return gpui_kit::size(px(LAUNCHER_WIDTH), px(560.));
            }
            Screen::ClipboardShelf => {
                return gpui_kit::size(px(480.), px(520.));
            }
            Screen::Tasks => {
                return gpui_kit::size(px(crate::ui::TASKS_WINDOW_WIDTH.min(900.)), px(560.));
            }
            Screen::Launcher | Screen::FileSearch => {}
        }
        let rows_height = if self.model.loading || self.model.rows.is_empty() {
            64.0
        } else {
            self.model.rows.len() as f32 * LAUNCHER_ROW_HEIGHT + 8.0
        };
        let scope_height = if self.model.scopes.is_empty() {
            0.0
        } else {
            metrics::PAGE_TOOLBAR_HEIGHT + metrics::DIVIDER_WIDTH
        };
        let context_height = if self.screen == Screen::Launcher
            && (!self.model.context_summary().is_empty() || !self.model.search_sources().is_empty())
        {
            metrics::PAGE_TOOLBAR_HEIGHT + metrics::DIVIDER_WIDTH
        } else {
            0.0
        };
        let preview_height = if self.model.is_preview() { 42.0 } else { 0.0 };
        let notice_height = if self.notice.is_some() { 44.0 } else { 0.0 };
        let height = (metrics::PAGE_HEADER_HEIGHT
            + scope_height
            + context_height
            + metrics::DIVIDER_WIDTH
            + rows_height
            + preview_height
            + notice_height)
            .clamp(metrics::PAGE_HEADER_HEIGHT, 520.0);
        gpui_kit::size(px(LAUNCHER_WIDTH), px(height))
    }

    fn resize_current_window(&self, cx: &mut Context<Self>) {
        if !matches!(self.screen, Screen::Launcher | Screen::FileSearch) {
            return;
        }
        let size = self.desired_window_size();
        let _ = cx.update_window(self.window_handle, |_, window, _| window.resize(size));
    }

    fn refresh_current_window(&self, cx: &mut Context<Self>) {
        let entity_id = cx.entity_id();
        let _ = cx.update_window(self.window_handle, move |_, window, _| {
            window.on_next_frame(move |_, cx| cx.notify(entity_id));
            window.refresh();
        });
    }

    fn preferences_from_editors(&self, cx: &Context<Self>) -> HostPreferences {
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

    fn settings_dirty(&self, cx: &Context<Self>) -> bool {
        self.preferences_from_editors(cx) != self.preferences
    }

    fn save_settings(&mut self, cx: &mut Context<Self>) {
        let preferences = self.preferences_from_editors(cx);
        if let Err(error) = preferences.validate() {
            self.notice = Some(match self.locale {
                Locale::En => format!("Settings could not be saved: {error:#}"),
                Locale::Zh => format!("设置无法保存：{error:#}"),
            });
            cx.notify();
            return;
        }
        self.settings_draft = preferences.clone();
        self.settings.saving = true;
        self.save_preferences(preferences, cx);
        cx.notify();
    }

    fn save_preferences(&mut self, preferences: HostPreferences, cx: &mut Context<Self>) {
        if let Some(live) = self.live.clone() {
            let receiver = live.set_preferences(preferences);
            self.poll(receiver, self.stream_generation, cx);
        } else {
            self.preferences = preferences;
            self.settings.saving = false;
        }
    }

    fn request_close_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_dirty(cx) {
            self.settings_close_pending = true;
            self.notice = Some(l10n::save_before_close(self.locale));
            cx.notify();
            return;
        }
        self.settings_close_pending = false;
        self.toggle_screen(window, cx);
    }

    fn discard_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        self.notice = None;
        self.toggle_screen(window, cx);
    }

    fn request_live_update(&mut self, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            return;
        };
        if self.model.object_actions_active {
            return;
        }
        self.request_revision = self
            .request_revision
            .max(self.model.revision)
            .saturating_add(1);
        let revision = self.request_revision;
        if let Some(gate) = &mut self.snapshot_gate {
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

    fn request_clipboard_search(&mut self, cx: &mut Context<Self>) {
        if let Some(live) = self.live.clone() {
            self.clipboard.loading();
            let receiver = live.clipboard_snapshot(self.clipboard.query.clone());
            self.poll(receiver, self.stream_generation, cx);
        }
    }

    fn request_app_history_search(&mut self, cx: &mut Context<Self>) {
        let (Some(live), Some(session_id), Some(source_id)) = (
            self.live.clone(),
            self.app_history.session_id,
            self.app_history.selected_source.clone(),
        ) else {
            return;
        };
        self.app_history.loading = true;
        let receiver =
            live.search_app_history(session_id, self.app_history.query.clone(), source_id);
        self.poll(receiver, self.stream_generation, cx);
    }

    fn open_screen(&mut self, screen: Screen, window: &mut Window, cx: &mut Context<Self>) {
        if self.screen == screen {
            return;
        }
        if self.screen == Screen::FileSearch {
            self.reset_file_search_browsing(cx);
        }
        self.screen = screen;
        self.notice = None;
        self.model.hide_object_actions();
        let receiver = match (screen, self.live.clone()) {
            (Screen::Clipboard | Screen::ClipboardShelf, Some(live)) => {
                Some(live.clipboard_snapshot(self.clipboard.query.clone()))
            }
            (Screen::ApplicationHistory, Some(live)) => Some(live.open_app_history_session()),
            (Screen::Tasks, Some(live)) => Some(live.execution_rows()),
            (Screen::WebPackages, Some(live)) => Some(live.web_packages()),
            _ => None,
        };
        if let Some(receiver) = receiver {
            self.poll(receiver, self.stream_generation, cx);
        }
        if screen == Screen::FileSearch {
            self.file_search_browsing = false;
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
        window.resize(self.desired_window_size());
        cx.notify();
    }

    fn go_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.standalone_shelf && self.screen == Screen::ClipboardShelf {
            self.prepare_hide(cx);
            window.remove_window();
            return;
        }
        if self.screen == Screen::Launcher {
            if self.model.object_actions_active {
                self.model.hide_object_actions();
                cx.notify();
            } else if self.model.menu_path().is_some() {
                self.model.back_menu();
                cx.notify();
            } else if !self.model.query.is_empty() {
                self.input
                    .update(cx, |input, cx| input.set_value("", window, cx));
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
        self.plugin_detail = None;
        self.unsupported_tool = None;
        self.screen = Screen::Launcher;
        self.notice = None;
        self.input.update(cx, |input, cx| input.focus(window, cx));
        window.resize(self.desired_window_size());
        cx.notify();
    }

    fn begin_stream(&mut self, receiver: Receiver<LiveEvent>, cx: &mut Context<Self>) {
        self.stream_generation = self.stream_generation.wrapping_add(1);
        let generation = self.stream_generation;
        self.poll(receiver, generation, cx);
    }

    fn poll(&mut self, mut receiver: Receiver<LiveEvent>, generation: u64, cx: &mut Context<Self>) {
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
                Some(LiveEvent::State(state)) => {
                    let accepted = this
                        .update(cx, |this, cx| {
                            if generation != this.stream_generation {
                                return None;
                            }
                            let accepted = this
                                .snapshot_gate
                                .as_mut()
                                .is_some_and(|gate| gate.accept_request(&state));
                            if accepted {
                                this.apply_state(state);
                                this.request_application_icons(cx);
                                this.resize_current_window(cx);
                                cx.notify();
                                this.live.as_ref().map(|live| {
                                    live.follow_snapshots(
                                        this.snapshot_gate.as_ref().map_or(0, SnapshotGate::cursor),
                                    )
                                })
                            } else {
                                None
                            }
                        })
                        .ok()
                        .flatten();
                    if let Some(follow) = accepted {
                        this.update(cx, |this, cx| {
                            if generation == this.stream_generation {
                                this.poll_follow(follow, generation, cx);
                            }
                        })
                        .ok();
                    }
                }
                Some(LiveEvent::Ignored) => {}
                Some(LiveEvent::Settings(snapshot)) => {
                    this.update(cx, |this, cx| {
                        if generation == this.stream_generation {
                            this.settings.apply_snapshot(snapshot);
                            this.notice = None;
                            cx.notify();
                        }
                    })
                    .ok();
                }
                Some(LiveEvent::SettingsError { message, snapshot }) => {
                    this.update(cx, |this, cx| {
                        if generation == this.stream_generation {
                            if let Some(snapshot) = snapshot {
                                this.settings.apply_snapshot(snapshot);
                            }
                            this.settings.saving = false;
                            this.settings.error = Some(message.clone());
                            this.notice = Some(message);
                            cx.notify();
                        }
                    })
                    .ok();
                }
                Some(LiveEvent::DirectoriesAuthorized { roots }) => {
                    let _ = cx.update_window(window_handle, |_, window, cx| {
                        let _ = this.update(cx, |this, cx| {
                            if generation == this.stream_generation {
                                this.apply_directories_authorized(roots, window, cx);
                            }
                        });
                    });
                }
                Some(LiveEvent::ActionResult(result)) => {
                    let close_window = result.close_window;
                    let destination = result.view.clone();
                    let open_tasks = result.execution_session_id.is_some();
                    this.update(cx, |this, cx| {
                        this.handle_action_result(&result);
                        cx.notify();
                        if close_window {
                            this.prepare_hide(cx);
                        }
                    })
                    .ok();
                    if close_window {
                        crate::platform::hide_main_window();
                    }
                    if let Some(destination) = destination {
                        let _ = cx.update_window(window_handle, |_, window, cx| {
                            let _ = this.update(cx, |this, cx| {
                                this.open_action_destination(destination, window, cx)
                            });
                        });
                    } else if open_tasks {
                        let _ = cx.update_window(window_handle, |_, window, cx| {
                            let _ = this
                                .update(cx, |this, cx| this.open_screen(Screen::Tasks, window, cx));
                        });
                    }
                }
                Some(LiveEvent::Error(error)) => {
                    this.update(cx, |this, cx| {
                        this.model.loading = false;
                        this.settings.saving = false;
                        this.settings.error = Some(error.clone());
                        this.notice = Some(error);
                        cx.notify();
                    })
                    .ok();
                }
                Some(event) => {
                    if let Err(error) = this.update(cx, |this, cx| {
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
        cx: &mut Context<Self>,
    ) {
        let window_handle = self.window_handle;
        cx.spawn(async move |this, cx| loop {
            match receiver.recv().await {
                Some(LiveEvent::State(state)) => {
                    let should_continue = this
                        .update(cx, |this, cx| {
                            if generation != this.stream_generation {
                                return false;
                            }
                            let accepted = this
                                .snapshot_gate
                                .as_mut()
                                .is_some_and(|gate| gate.accept_update(&state));
                            if accepted {
                                this.apply_state(state);
                                this.request_application_icons(cx);
                                this.resize_current_window(cx);
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
                        if generation == this.stream_generation {
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
                        if generation == this.stream_generation {
                            if let Some(snapshot) = snapshot {
                                this.settings.apply_snapshot(snapshot);
                            }
                            this.settings.saving = false;
                            this.settings.error = Some(message.clone());
                            this.notice = Some(message);
                            cx.notify();
                        }
                    })
                    .ok();
                    break;
                }
                Some(LiveEvent::DirectoriesAuthorized { roots }) => {
                    let _ = cx.update_window(window_handle, |_, window, cx| {
                        let _ = this.update(cx, |this, cx| {
                            if generation == this.stream_generation {
                                this.apply_directories_authorized(roots, window, cx);
                            }
                        });
                    });
                    break;
                }
                Some(LiveEvent::ActionResult(result)) => {
                    let close_window = result.close_window;
                    let destination = result.view.clone();
                    let open_tasks = result.execution_session_id.is_some();
                    this.update(cx, |this, cx| {
                        if generation == this.stream_generation {
                            this.handle_action_result(&result);
                            cx.notify();
                            if close_window {
                                this.prepare_hide(cx);
                            }
                        }
                    })
                    .ok();
                    if close_window {
                        crate::platform::hide_main_window();
                    }
                    if let Some(destination) = destination {
                        let _ = cx.update_window(window_handle, |_, window, cx| {
                            let _ = this.update(cx, |this, cx| {
                                if generation == this.stream_generation {
                                    this.open_action_destination(destination, window, cx);
                                }
                            });
                        });
                    } else if open_tasks {
                        let _ = cx.update_window(window_handle, |_, window, cx| {
                            let _ = this.update(cx, |this, cx| {
                                if generation == this.stream_generation {
                                    this.open_screen(Screen::Tasks, window, cx);
                                }
                            });
                        });
                    }
                    break;
                }
                Some(LiveEvent::Error(error)) => {
                    this.update(cx, |this, cx| {
                        if generation == this.stream_generation {
                            this.model.loading = false;
                            this.settings.saving = false;
                            this.settings.error = Some(error.clone());
                            this.notice = Some(error);
                            cx.notify();
                        }
                    })
                    .ok();
                    break;
                }
                Some(event) => {
                    tracing::debug!("launcher_live_event_received");
                    this.update(cx, |this, cx| {
                        if generation == this.stream_generation {
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
            } => {
                let next_locale = locale_for_language(preferences.ui_language);
                if self.locale != next_locale {
                    self.locale = next_locale;
                    self.model.set_locale(self.locale);
                }
                let hotkey_error = self.hotkeys.as_ref().and_then(|hotkeys| {
                    hotkeys
                        .lock()
                        .map_err(|_| "Global hotkey state is unavailable".to_owned())
                        .and_then(|mut hotkeys| {
                            hotkeys
                                .register(&preferences)
                                .map_err(|error| format!("{error:#}"))
                        })
                        .err()
                });
                self.preferences = preferences.clone();
                self.settings_draft = preferences;
                self.settings.saving = false;
                self.settings.error = host_error.clone().or(hotkey_error.clone());
                self.settings_close_pending = false;
                self.notice = host_error.or(hotkey_error);
            }
            LiveEvent::Clipboard(snapshot) => {
                self.clipboard.apply(snapshot);
                self.notice = None;
            }
            LiveEvent::AppHistorySources {
                session_id,
                sources,
            } => {
                self.app_history.apply_sources(session_id, sources);
                self.request_app_history_search(cx);
            }
            LiveEvent::AppHistory(snapshot) => {
                self.app_history.apply(snapshot);
                self.notice = None;
            }
            LiveEvent::Executions(rows) => {
                self.tasks.apply_rows(rows);
                self.notice = None;
            }
            LiveEvent::ExecutionDetail(detail) => {
                self.tasks.apply_detail(detail);
                self.notice = None;
            }
            LiveEvent::WebPackages(packages) => {
                self.web.apply(packages);
                self.web_review = None;
                self.notice = None;
            }
            LiveEvent::WebReview(review) => {
                self.web_review = Some(review);
                self.notice = None;
            }
            LiveEvent::WebDocument(document) => {
                if let (Some(previous), Some(live)) =
                    (self.web_document.as_ref(), self.live.clone())
                {
                    if previous.handle != document.handle {
                        let receiver = live.close_web_document(previous.handle.clone());
                        self.poll(receiver, self.stream_generation, cx);
                    }
                }
                if self.web_view.is_some() {
                    let _ = self.web_view.as_ref().map(|view| {
                        view.read(cx)
                            .raw()
                            .evaluate_script(&visibility_script(false))
                    });
                }
                self.web_view = None;
                self.web_ready = false;
                self.web_error = None;
                self.web_state_handle = None;
                self.web_state_version = 0;
                self.web_document = Some(document);
                self.notice = None;
            }
            LiveEvent::WebFrameReply { handle, frame } => {
                if self
                    .web_document
                    .as_ref()
                    .is_some_and(|document| document.handle == handle)
                {
                    let _ = self.evaluate_web_script(&reply_script(&frame), cx);
                }
            }
            LiveEvent::WebState { handle, frame } => {
                if self
                    .web_document
                    .as_ref()
                    .is_some_and(|document| document.handle == handle)
                {
                    let _ = self.evaluate_web_script(&state_script(&frame), cx);
                }
            }
            LiveEvent::DirectoriesAuthorized { roots } => {
                self.settings_draft.file_search_roots = roots;
                self.notice = Some(match self.locale {
                    Locale::En => "Search directories authorized.".to_owned(),
                    Locale::Zh => "搜索目录已授权。".to_owned(),
                });
            }
            LiveEvent::ApplicationIcon {
                candidate_id,
                bytes,
            } => {
                let visible =
                    self.model.rows.iter().any(|row| {
                        row.id == candidate_id && row.kind == CandidateKind::Application
                    });
                if visible {
                    let image =
                        bytes.map(|bytes| Arc::new(Image::from_bytes(ImageFormat::Png, bytes)));
                    self.application_icons.insert(candidate_id.clone(), image);
                }
                self.application_icons_requested.remove(&candidate_id);
            }
            LiveEvent::Preview(preview) => {
                self.show_preview(preview, cx);
            }
            LiveEvent::State(_)
            | LiveEvent::ActionResult(_)
            | LiveEvent::Settings(_)
            | LiveEvent::SettingsError { .. }
            | LiveEvent::Ignored
            | LiveEvent::Error(_) => {}
        }
        cx.notify();
    }

    fn activate_selection(&mut self, cx: &mut Context<Self>) {
        if self.screen == Screen::FileSearch && !self.file_search_browsing {
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
                self.notice = Some(match self.locale {
                    Locale::En => format!("Running “{}”…", row.title),
                    Locale::Zh => format!("正在执行“{}”…", row.title),
                });
                let receiver = live.execute(self.model.revision, row.id);
                self.poll(receiver, self.stream_generation, cx);
                cx.notify();
                return;
            }
        }
        let message = match self.model.selected_row() {
            Some(row) if row.enabled => match self.locale {
                Locale::En => format!("Execution bridge pending for “{}”", row.title),
                Locale::Zh => format!("“{}”的执行桥接尚待接入", row.title),
            },
            Some(row) => match self.locale {
                Locale::En => format!("“{}” is unavailable right now", row.title),
                Locale::Zh => format!("“{}”暂时不可用", row.title),
            },
            None => match self.locale {
                Locale::En => "No action is selected".to_owned(),
                Locale::Zh => "当前没有选中的操作".to_owned(),
            },
        };
        self.notice = Some(message);
        cx.notify();
    }

    fn handle_action_result(&mut self, result: &anywhere_protocol::ActionResult) {
        if self.model.object_actions_active {
            self.model.hide_object_actions();
        }
        self.notice = Some(action_result_message(result, self.locale));
    }

    fn open_action_destination(
        &mut self,
        destination: anywhere_protocol::HostView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match destination {
            anywhere_protocol::HostView::FileSearch => {
                self.model.set_query(String::new());
                self.input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.open_screen(Screen::FileSearch, window, cx);
            }
            anywhere_protocol::HostView::WebPlugin { invocation_id } => {
                self.web_document = None;
                self.web_view = None;
                self.web_ready = false;
                self.web_error = None;
                self.open_screen(Screen::WebTool, window, cx);
                if let Some(live) = self.live.clone() {
                    let receiver = live.open_web_document(invocation_id, None);
                    self.poll(receiver, self.stream_generation, cx);
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
                        self.app_history.selected_source = Some(route.source_id);
                    }
                    self.open_screen(Screen::ApplicationHistory, window, cx);
                }
                _ => {
                    self.unsupported_tool = Some((id, input));
                    self.open_screen(Screen::UnsupportedTool, window, cx);
                    cx.notify();
                }
            },
            anywhere_protocol::HostView::PluginDetail { detail } => {
                self.plugin_detail = Some(detail);
                self.open_screen(Screen::PluginDetail, window, cx);
                cx.notify();
            }
        }
    }

    fn handle_key(
        &mut self,
        key: &str,
        command: bool,
        modified: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.preview_open && key == "escape" {
            self.close_preview(cx);
            self.focus_active_editor(window, cx);
            return;
        }
        if command && key.eq_ignore_ascii_case("y") {
            self.request_preview(cx);
            return;
        }
        if command && key.eq_ignore_ascii_case("k") {
            self.show_object_actions(cx);
            return;
        }
        if command && key.eq_ignore_ascii_case("s") && self.screen == Screen::Settings {
            self.save_settings(cx);
            return;
        }
        if command && matches!(key, "enter" | "return") {
            self.toggle_root_mode(cx);
            return;
        }
        match (self.screen, key) {
            (Screen::Settings, "escape") => {
                if self.settings_close_pending {
                    self.settings_close_pending = false;
                    self.notice = None;
                    cx.notify();
                } else {
                    self.request_close_settings(window, cx);
                }
            }
            (Screen::Launcher, "down") if !modified => {
                self.model.select_next();
                self.notice = None;
                cx.notify();
            }
            (Screen::Launcher, "up") if !modified => {
                self.model.select_previous();
                self.notice = None;
                cx.notify();
            }
            (Screen::FileSearch, "down") if !modified => self.select_file_search(1, cx),
            (Screen::FileSearch, "up") if !modified => self.select_file_search(-1, cx),
            (Screen::Launcher, "tab") => {
                self.model.cycle_scope();
                self.request_live_update(cx);
                self.notice = None;
                cx.notify();
            }
            (Screen::FileSearch, "tab") => {
                self.file_search.next_filter();
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
                if self.model.object_actions_active
                    || self.model.menu_path().is_some()
                    || !self.model.query.is_empty()
                {
                    self.go_back(window, cx);
                } else {
                    self.prepare_hide(cx);
                    crate::platform::hide_main_window();
                }
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
            (Screen::Clipboard | Screen::ClipboardShelf, "space") => {
                self.preview_selected_clipboard(cx)
            }
            (Screen::Clipboard | Screen::ClipboardShelf, "delete" | "backspace") => {
                self.delete_selected_clipboard(cx)
            }
            (Screen::Clipboard | Screen::ClipboardShelf, "escape") => self.go_back(window, cx),
            (Screen::ApplicationHistory, "down") => {
                self.app_history.select_next();
                cx.notify();
            }
            (Screen::ApplicationHistory, "up") => {
                self.app_history.select_previous();
                cx.notify();
            }
            (Screen::ApplicationHistory, "enter" | "return") => self.open_selected_history_item(cx),
            (Screen::ApplicationHistory, "escape") => self.go_back(window, cx),
            (Screen::Tasks, "down") => {
                if !self.tasks.rows.is_empty() {
                    self.tasks.selected = (self.tasks.selected + 1) % self.tasks.rows.len();
                    self.tasks.detail = None;
                    cx.notify();
                }
            }
            (Screen::Tasks, "up") => {
                if !self.tasks.rows.is_empty() {
                    self.tasks.selected = if self.tasks.selected == 0 {
                        self.tasks.rows.len() - 1
                    } else {
                        self.tasks.selected - 1
                    };
                    self.tasks.detail = None;
                    cx.notify();
                }
            }
            (Screen::Tasks, "enter" | "return") => self.open_selected_execution(cx),
            (Screen::Tasks, "s") => self.stop_selected_execution(cx),
            (Screen::Tasks, "escape") => self.go_back(window, cx),
            (Screen::WebPackages, "enter" | "return") => self.toggle_selected_web_package(cx),
            (Screen::WebPackages, "escape") => self.go_back(window, cx),
            (Screen::WebTool, "escape") => self.go_back(window, cx),
            (Screen::PluginDetail | Screen::UnsupportedTool, "escape") => self.go_back(window, cx),
            _ => {}
        }
    }

    fn copy_selected_clipboard(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(item)) = (self.live.clone(), self.clipboard.selected_item()) {
            let receiver = live.copy_clipboard(item.id);
            self.poll(receiver, self.stream_generation, cx);
            self.notice = Some(l10n::copied(self.locale));
            cx.notify();
        }
    }

    fn preview_selected_clipboard(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(item)) = (self.live.clone(), self.clipboard.selected_item()) {
            let receiver = live.preview_clipboard(item.id);
            self.poll(receiver, self.stream_generation, cx);
        }
    }

    fn delete_selected_clipboard(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(item)) = (self.live.clone(), self.clipboard.selected_item()) {
            let receiver = live.delete_clipboard(item.id);
            self.poll(receiver, self.stream_generation, cx);
        }
    }

    fn open_selected_history_item(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(item)) = (self.live.clone(), self.app_history.selected_item()) {
            let receiver = live.use_app_history_item(item.id.clone(), false);
            self.poll(receiver, self.stream_generation, cx);
            self.notice = Some(l10n::open(self.locale));
            cx.notify();
        }
    }

    fn open_selected_execution(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(row)) = (self.live.clone(), self.tasks.selected_row()) {
            let receiver = live.execution_detail(row.id.clone());
            self.poll(receiver, self.stream_generation, cx);
        }
    }

    fn stop_selected_execution(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(row)) = (self.live.clone(), self.tasks.selected_row()) {
            let receiver = live.stop_execution(row.id.clone());
            self.poll(receiver, self.stream_generation, cx);
        }
    }

    fn toggle_selected_web_package(&mut self, cx: &mut Context<Self>) {
        if let (Some(live), Some(package)) =
            (self.live.clone(), self.web.packages.get(self.web.selected))
        {
            let id = package.id.clone();
            let operation = if package.enabled { "disable" } else { "enable" };
            let receiver = live.change_web_package(id, operation.to_owned());
            self.poll(receiver, self.stream_generation, cx);
        }
    }

    fn request_preview(&mut self, cx: &mut Context<Self>) {
        if self.screen == Screen::Launcher || self.screen == Screen::FileSearch {
            if let (Some(live), Some(row)) = (self.live.clone(), self.selected_row_for_action()) {
                if row.kind == anywhere_protocol::CandidateKind::File {
                    let receiver = live.preview_file(self.model.revision, row.id.clone());
                    self.poll(receiver, self.stream_generation, cx);
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
            .map(|row| row.id.clone())
            .collect::<Vec<_>>();
        for candidate_id in visible_ids {
            if self.application_icons.contains_key(&candidate_id)
                || !self
                    .application_icons_requested
                    .insert(candidate_id.clone())
            {
                continue;
            }
            let receiver = live.application_icon(candidate_id);
            self.poll_application_icon(receiver, self.stream_generation, cx);
        }
    }

    fn poll_application_icon(
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
                if generation == this.stream_generation {
                    this.apply_auxiliary_event(event, cx);
                    cx.notify();
                }
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
                        Ok(PreviewReservation::Ready(
                            this.preview
                                .take()
                                .expect("Quick Look host was initialized"),
                        ))
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
                            this.notice = Some(preview_error_message(this.locale, &error));
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
                            this.notice = Some(match this.locale {
                                Locale::En => format!("Previewing {}", preview.title),
                                Locale::Zh => format!("正在预览 {}", preview.title),
                            });
                            this.start_preview_monitor(generation, cx);
                        }
                        Err(error) => {
                            this.preview_open = false;
                            this.notice = Some(preview_error_message(this.locale, error));
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
        self.notice = Some(match self.locale {
            Locale::En => "Search directories authorized.".to_owned(),
            Locale::Zh => "搜索目录已授权。".to_owned(),
        });
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
        self.file_search_browsing = false;
        self.model.selected = 0;
        self.close_preview(cx);
    }

    fn reset_clipboard_browsing(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.screen, Screen::Clipboard | Screen::ClipboardShelf) {
            return;
        }
        self.clipboard.reset_selection();
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
        self.notice = None;
        if self.preview_open {
            self.preview_selected_clipboard(cx);
        }
        cx.notify();
        self.refresh_current_window(cx);
    }

    fn file_search_selection_active(&self) -> bool {
        self.screen == Screen::FileSearch
            && self.file_search_browsing
            && !self.model.object_actions_active
            && self.model.selected_row().is_some()
    }

    fn selected_row_for_action(&self) -> Option<&LauncherRow> {
        if self.screen == Screen::FileSearch
            && !self.file_search_browsing
            && !self.model.object_actions_active
        {
            return None;
        }
        self.model.selected_row()
    }

    fn select_file_search(&mut self, direction: i8, cx: &mut Context<Self>) {
        if self.model.rows.is_empty() {
            self.file_search_browsing = false;
            return;
        }
        if self.file_search_browsing {
            if direction < 0 {
                self.model.select_previous();
            } else {
                self.model.select_next();
            }
        } else {
            self.file_search_browsing = true;
            if direction < 0 {
                self.model.select_last();
            } else {
                self.model.select_first();
            }
        }
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
        let key = event.keystroke.key.as_str();
        if self.preview_open && key == "escape" {
            self.close_preview(cx);
            self.focus_active_editor(window, cx);
            return true;
        }
        if key != "space"
            || event.prefer_character_input
            || event.keystroke.modifiers.modified()
            || (!self.preview_open && !self.file_search_selection_active())
        {
            return false;
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
        if self.screen == Screen::FileSearch && !self.file_search_browsing {
            return;
        }
        let Some(row) = self.model.selected_row().cloned() else {
            return;
        };
        if !matches!(self.screen, Screen::Launcher | Screen::FileSearch) {
            return;
        }
        self.model.show_object_actions();
        self.notice = None;
        if let Some(live) = self.live.clone() {
            let receiver = live.object_actions(self.model.revision, row.id);
            self.poll_auxiliary_state(receiver, self.stream_generation, cx);
        }
        cx.notify();
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
                if generation != this.stream_generation {
                    return;
                }
                match event {
                    LiveEvent::State(state) => {
                        this.apply_state(state);
                        this.request_application_icons(cx);
                        this.resize_current_window(cx);
                    }
                    LiveEvent::Error(error) => {
                        this.model.hide_object_actions();
                        this.notice = Some(error);
                    }
                    event => this.apply_auxiliary_event(event, cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn toggle_root_mode(&mut self, cx: &mut Context<Self>) {
        if self.screen != Screen::Launcher || self.model.object_actions_active {
            return;
        }
        self.input_mode = match self.input_mode {
            InputMode::Environment => InputMode::Search,
            _ => InputMode::Environment,
        };
        self.model.selected_scope = None;
        self.request_live_update(cx);
        self.notice = Some(match self.locale {
            Locale::En => match self.input_mode {
                InputMode::Search => "Universal Search".to_owned(),
                _ => "Current App".to_owned(),
            },
            Locale::Zh => match self.input_mode {
                InputMode::Search => "全局搜索".to_owned(),
                _ => "当前应用".to_owned(),
            },
        });
        cx.notify();
    }

    fn render_launcher(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let selected_scope = self.model.selected_scope.as_deref();
        let scope_controls = if self.model.scopes.is_empty() {
            None
        } else {
            Some(
                div()
                    .h(px(metrics::PAGE_TOOLBAR_HEIGHT))
                    .px(px(metrics::SPACE_MD))
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_XS))
                    .child(self.scope_button(None, selected_scope.is_none(), cx))
                    .children(self.model.scopes.iter().map(|scope| {
                        self.scope_button(
                            Some(scope.id.clone()),
                            selected_scope == Some(scope.id.as_str()),
                            cx,
                        )
                    })),
            )
        };
        let activity_strip = self.launcher_activity_strip();

        let rows = if self.model.loading {
            div()
                .h(px(64.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(metrics::BODY_SIZE))
                .text_color(theme::secondary())
                .child(match self.locale {
                    Locale::En => "Building the current context…",
                    Locale::Zh => "正在构建当前上下文…",
                })
        } else if self.model.rows.is_empty() {
            div()
                .h(px(64.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(metrics::SPACE_XS))
                .text_color(theme::secondary())
                .child(
                    div()
                        .text_size(px(metrics::HEADING_SIZE))
                        .font_weight(FontWeight::MEDIUM)
                        .child(match self.locale {
                            Locale::En => "No matching actions",
                            Locale::Zh => "没有匹配的操作",
                        }),
                )
                .child(
                    div()
                        .text_size(px(metrics::CAPTION_SIZE))
                        .text_color(theme::muted())
                        .child(match self.locale {
                            Locale::En => "Try another search or scope",
                            Locale::Zh => "尝试其他搜索词或范围",
                        }),
                )
        } else {
            let mut list = div().flex().flex_col().gap(px(2.0)).p(px(6.0));
            for (index, row) in self.model.rows.clone().into_iter().enumerate() {
                list = list.child(self.render_row(index, row, self.model.selected == index, cx));
            }
            list
        };

        let mut root = div()
            .id("launcher-surface")
            .w_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(metrics::PAGE_HEADER_HEIGHT))
                    .px(px(metrics::SPACE_LG))
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_MD))
                    .child(
                        div()
                            .size(px(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(10.))
                            .bg(theme::field())
                            .text_color(theme::accent())
                            .text_size(px(20.))
                            .child("⌕"),
                    )
                    .child(
                        div().flex_1().min_w(px(0.)).child(
                            Input::new(&self.input)
                                .id("launcher-search-input")
                                .aria_label(match self.locale {
                                    Locale::En => "Search apps and actions",
                                    Locale::Zh => "搜索应用和操作",
                                })
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .cleanable(true)
                                .text_size(px(metrics::PAGE_HEADING_SIZE))
                                .w_full(),
                        ),
                    )
                    .child(self.keycap("esc"))
                    .child(
                        Button::new("settings")
                            .ghost()
                            .label("⚙")
                            .accessibility_label(match self.locale {
                                Locale::En => "Settings",
                                Locale::Zh => "设置",
                            })
                            .on_click(
                                cx.listener(|this, _, window, cx| this.toggle_screen(window, cx)),
                            ),
                    ),
            );
        if let Some(controls) = scope_controls {
            root = root
                .child(
                    div()
                        .h(px(0.5))
                        .mx(px(metrics::SPACE_MD))
                        .bg(theme::divider()),
                )
                .child(controls);
        }
        if let Some(activity_strip) = activity_strip {
            root = root
                .child(
                    div()
                        .h(px(0.5))
                        .mx(px(metrics::SPACE_MD))
                        .bg(theme::divider()),
                )
                .child(activity_strip);
        }
        if self.model.is_preview() {
            root = root.child(
                div()
                    .mx(px(metrics::SPACE_MD))
                    .mb(px(metrics::SPACE_SM))
                    .px(px(metrics::SPACE_MD))
                    .py(px(metrics::SPACE_SM))
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .bg(theme::field())
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::muted())
                    .child(match self.locale {
                        Locale::En => {
                            "UI migration preview · the shared Rust runtime is not attached in this window"
                        }
                        Locale::Zh => "UI 迁移预览 · 此窗口尚未接入共享 Rust 运行时",
                    }),
            );
        }
        root = root
            .child(div().h(px(0.5)).bg(theme::divider()))
            .child(rows);
        if let Some(notice) = &self.notice {
            root = root.child(
                div()
                    .m(px(metrics::SPACE_SM))
                    .px(px(metrics::SPACE_MD))
                    .py(px(metrics::SPACE_SM))
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .bg(theme::field())
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::secondary())
                    .child(notice.clone()),
            );
        }
        root.into_any_element()
    }

    fn launcher_activity_strip(&self) -> Option<AnyElement> {
        let contexts = self.model.context_summary();
        let sources = self.model.search_sources();
        if contexts.is_empty() && sources.is_empty() {
            return None;
        }
        let context_chips = contexts.iter().take(3).map(|context| {
            div()
                .id(SharedString::from(format!("context-{}", context.id)))
                .role(gpui_kit::Role::ListItem)
                .aria_label(format!("{} {}", context.label, context.detail))
                .max_w(px(210.))
                .px(px(metrics::SPACE_SM))
                .py(px(metrics::SPACE_XS))
                .flex()
                .items_center()
                .gap(px(metrics::SPACE_XS))
                .rounded(px(metrics::CONTROL_RADIUS))
                .bg(theme::field())
                .child(
                    div()
                        .text_size(px(metrics::CAPTION_SIZE))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::primary())
                        .child(context.label.clone()),
                )
                .child(
                    div()
                        .min_w(px(0.))
                        .truncate()
                        .text_size(px(10.5))
                        .text_color(theme::muted())
                        .child(context.detail.clone()),
                )
        });
        let source_chips = sources.iter().take(4).map(|source| {
            let label = source.id.rsplit('.').next().unwrap_or(&source.id);
            div()
                .id(SharedString::from(format!("search-source-{}", source.id)))
                .role(gpui_kit::Role::Status)
                .aria_label(format!(
                    "{label}: {}",
                    search_source_label(source.state, self.locale)
                ))
                .px(px(metrics::SPACE_SM))
                .py(px(metrics::SPACE_XS))
                .rounded(px(metrics::CONTROL_RADIUS))
                .border(px(0.5))
                .border_color(theme::border())
                .text_size(px(10.5))
                .text_color(match source.state {
                    anywhere_protocol::SearchSourceState::Unavailable => theme::warning(),
                    anywhere_protocol::SearchSourceState::Partial => theme::warning(),
                    _ => theme::secondary(),
                })
                .child(format!(
                    "{label} · {}",
                    search_source_label(source.state, self.locale)
                ))
        });
        Some(
            div()
                .id("launcher-context-summary")
                .h(px(metrics::PAGE_TOOLBAR_HEIGHT))
                .px(px(metrics::SPACE_MD))
                .flex()
                .items_center()
                .gap(px(metrics::SPACE_XS))
                .overflow_hidden()
                .children(context_chips)
                .children(source_chips)
                .into_any_element(),
        )
    }

    fn render_file_search(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let progress = self.model.file_search_progress();
        let title = match self.locale {
            Locale::En => "File Search",
            Locale::Zh => "文件搜索",
        };
        let subtitle = progress
            .map(|progress| {
                if progress.finished {
                    format!(
                        "{} matches · {} entries scanned",
                        progress.confirmed_matches, progress.scanned_entries
                    )
                } else {
                    format!("{} matches · scanning…", progress.confirmed_matches)
                }
            })
            .unwrap_or_else(|| match self.locale {
                Locale::En => "Search by name inside authorized folders".to_owned(),
                Locale::Zh => "在已授权目录内按名称搜索".to_owned(),
            });
        let filter_label = self.file_search.label(self.locale);
        let rows = if self.model.loading {
            self.empty_page_state(
                match self.locale {
                    Locale::En => "Searching files…",
                    Locale::Zh => "正在搜索文件…",
                },
                None,
            )
        } else if self.model.rows.is_empty() {
            self.empty_page_state(
                if progress.is_some_and(|progress| !progress.finished) {
                    match self.locale {
                        Locale::En => "Searching files…",
                        Locale::Zh => "正在搜索文件…",
                    }
                } else {
                    match self.locale {
                        Locale::En => "No matching files",
                        Locale::Zh => "没有匹配的文件",
                    }
                },
                progress
                    .filter(|progress| progress.inaccessible_count > 0)
                    .map(|progress| match self.locale {
                        Locale::En => format!(
                            "{} locations could not be read; check Search settings.",
                            progress.inaccessible_count
                        ),
                        Locale::Zh => format!(
                            "有 {} 个位置无法读取，请检查搜索设置。",
                            progress.inaccessible_count
                        ),
                    }),
            )
        } else {
            let mut list = div()
                .id("file-search-list")
                .flex()
                .flex_col()
                .gap(px(2.0))
                .p(px(6.0));
            for (index, row) in self.model.rows.clone().into_iter().enumerate() {
                list = list.child(self.render_row(
                    index,
                    row,
                    self.file_search_browsing && self.model.selected == index,
                    cx,
                ));
            }
            if progress
                .and_then(|progress| progress.next_cursor.as_ref())
                .is_some()
            {
                list = list.child(
                    div()
                        .p(px(metrics::SPACE_SM))
                        .flex()
                        .justify_center()
                        .child(
                            Button::new("load-more-files")
                                .secondary()
                                .label(match self.locale {
                                    Locale::En => "Load more",
                                    Locale::Zh => "加载更多",
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live.load_more_files(this.model.revision);
                                        this.poll(receiver, this.stream_generation, cx);
                                    }
                                })),
                        ),
                );
            }
            list.into_any_element()
        };
        self.render_page_shell(
            SharedString::from(title),
            SharedString::from(subtitle),
            Some((&self.input, "file-search-input", filter_label)),
            rows,
            cx,
        )
        .into_any_element()
    }

    fn render_clipboard(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let status = self.clipboard.status.as_ref();
        let subtitle = status
            .map(|status| {
                let state = if !status.available {
                    l10n::unavailable(self.locale)
                } else if !status.recording_enabled {
                    l10n::disabled(self.locale)
                } else if status.paused_until_ms.is_some() {
                    l10n::paused(self.locale)
                } else {
                    l10n::clipboard_history_setting(self.locale)
                };
                format!(
                    "{} · {} · {}",
                    state, status.stored_entries, status.retention_days
                )
            })
            .unwrap_or_else(|| l10n::clipboard(self.locale));
        let content = if self.clipboard.items().is_empty() {
            self.empty_page_state(
                if self.clipboard.loading {
                    l10n::loading(self.locale)
                } else if self.clipboard.query.trim().is_empty() {
                    l10n::no_saved_entries(self.locale)
                } else {
                    l10n::no_matching_entries(self.locale)
                },
                status.map(|status| {
                    l10n::history_storage_usage(
                        self.locale,
                        i64::from(status.stored_entries),
                        &format!("{:.1}", f64::from(status.stored_bytes) / 1_048_576.0),
                    )
                }),
            )
        } else {
            let mut list = div()
                .id("clipboard-history-list")
                .flex()
                .flex_col()
                .gap(px(2.0))
                .p(px(6.0));
            for (index, item) in self.clipboard.items().to_vec().into_iter().enumerate() {
                let selected = self.clipboard.browsing && self.clipboard.selected == index;
                let item_id = item.id;
                list = list.child(
                    div()
                        .id(SharedString::from(format!("clipboard-item-{item_id}")))
                        .role(gpui_kit::Role::ListBoxOption)
                        .aria_label(item.summary.clone())
                        .aria_selected(selected)
                        .h(px(LAUNCHER_ROW_HEIGHT - 4.0))
                        .px(px(metrics::SPACE_MD))
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_MD))
                        .rounded(px(metrics::RESULT_RADIUS))
                        .when(selected, |element| {
                            element
                                .bg(theme::selected())
                                .border(px(metrics::FOCUS_BORDER_WIDTH))
                                .border_color(theme::border())
                        })
                        .when(!selected, |element| {
                            element.hover(|style| style.bg(theme::hover()))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.clipboard.browsing = true;
                            this.clipboard.selected = index;
                            this.notice = None;
                            cx.notify();
                            this.refresh_current_window(cx);
                        }))
                        .child(
                            div()
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(8.))
                                .bg(theme::field())
                                .text_color(theme::accent())
                                .child(if item.kind.contains("image") {
                                    "▧"
                                } else if item.kind.contains("file") {
                                    "▰"
                                } else {
                                    "T"
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(px(metrics::BODY_SIZE))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme::primary())
                                        .child(item.summary),
                                )
                                .child(
                                    div()
                                        .text_size(px(metrics::CAPTION_SIZE))
                                        .text_color(theme::secondary())
                                        .child(format!("{} · {} B", item.kind, item.byte_count)),
                                ),
                        )
                        .child(if selected {
                            self.keycap("↵")
                        } else {
                            self.keycap("space")
                        }),
                );
            }
            list.into_any_element()
        };
        let shell = self.render_page_shell(
            SharedString::from(l10n::clipboard_history(self.locale)),
            SharedString::from(subtitle),
            Some((
                &self.clipboard_input,
                "clipboard-search",
                l10n::search_history(self.locale),
            )),
            content,
            cx,
        );
        shell
            .child(
                div()
                    .px(px(metrics::SPACE_MD))
                    .py(px(metrics::SPACE_SM))
                    .border_t(px(0.5))
                    .border_color(theme::divider())
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_SM))
                    .child(
                        Button::new("pause-clipboard")
                            .secondary()
                            .label(match self.locale {
                                Locale::En => "Pause 15 min",
                                Locale::Zh => "暂停 15 分钟",
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(live) = this.live.clone() {
                                    let receiver = live.set_clipboard_pause(15);
                                    this.poll(receiver, this.stream_generation, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("clear-clipboard")
                            .secondary()
                            .label(l10n::clear_history(self.locale))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(live) = this.live.clone() {
                                    let receiver = live.clear_clipboard();
                                    this.poll(receiver, this.stream_generation, cx);
                                }
                            })),
                    )
                    .child(div().flex_1())
                    .child(self.keycap("⌘K")),
            )
            .into_any_element()
    }

    fn render_clipboard_shelf(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let status = self.clipboard.status.clone();
        let state_label = status
            .as_ref()
            .map(|status| {
                if !status.available {
                    l10n::unavailable(self.locale)
                } else if !status.recording_enabled {
                    l10n::disabled(self.locale)
                } else if status.paused_until_ms.is_some() {
                    l10n::paused(self.locale)
                } else {
                    l10n::clipboard_always_on_top(self.locale)
                }
            })
            .unwrap_or_else(|| l10n::clipboard_always_on_top(self.locale));
        let content = if self.clipboard.items().is_empty() {
            self.empty_page_state(
                if self.clipboard.loading {
                    l10n::loading(self.locale)
                } else if self.clipboard.query.trim().is_empty() {
                    l10n::no_saved_entries(self.locale)
                } else {
                    l10n::no_matching_entries(self.locale)
                },
                Some(l10n::shelf_select_hint(self.locale)),
            )
        } else {
            let mut list = div()
                .id("clipboard-shelf-list")
                .flex_1()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .p(px(6.0))
                .overflow_y_scrollbar();
            for (index, item) in self.clipboard.items().to_vec().into_iter().enumerate() {
                let selected = self.clipboard.browsing && self.clipboard.selected == index;
                let item_id = item.id;
                list = list.child(
                    div()
                        .id(SharedString::from(format!(
                            "clipboard-shelf-item-{item_id}"
                        )))
                        .role(gpui_kit::Role::ListBoxOption)
                        .aria_label(item.summary.clone())
                        .aria_selected(selected)
                        .h(px(46.))
                        .px(px(metrics::SPACE_SM))
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_SM))
                        .rounded(px(metrics::RESULT_RADIUS))
                        .when(selected, |element| {
                            element
                                .bg(theme::selected())
                                .border(px(metrics::FOCUS_BORDER_WIDTH))
                                .border_color(theme::border())
                        })
                        .when(!selected, |element| {
                            element.hover(|style| style.bg(theme::hover()))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.clipboard.browsing = true;
                            this.clipboard.selected = index;
                            this.notice = None;
                            cx.notify();
                            this.refresh_current_window(cx);
                        }))
                        .child(
                            div()
                                .size(px(26.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(7.))
                                .bg(theme::field())
                                .text_color(theme::accent())
                                .child(if item.kind.contains("image") {
                                    "▧"
                                } else if item.kind.contains("file") {
                                    "▰"
                                } else {
                                    "T"
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .text_size(px(metrics::BODY_SIZE))
                                .text_color(theme::primary())
                                .child(item.summary),
                        )
                        .when(selected, |element| element.child(self.keycap("↵"))),
                );
            }
            list.into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(metrics::SPACE_MD))
                    .pt(px(metrics::SPACE_MD))
                    .pb(px(metrics::SPACE_SM))
                    .flex()
                    .flex_col()
                    .gap(px(metrics::SPACE_XS))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(metrics::SPACE_SM))
                            .child(
                                div()
                                    .text_color(theme::accent())
                                    .text_size(px(16.))
                                    .child("⌖"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(metrics::PAGE_HEADING_SIZE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::primary())
                                    .child(l10n::clipboard_shelf(self.locale)),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(theme::muted())
                                    .child(state_label),
                            )
                            .child(self.keycap("esc")),
                    )
                    .child(
                        div()
                            .h(px(34.))
                            .px(px(metrics::SPACE_SM))
                            .flex()
                            .items_center()
                            .rounded(px(metrics::CONTROL_RADIUS))
                            .bg(theme::field())
                            .child("⌕")
                            .child(
                                div().flex_1().min_w(px(0.)).child(
                                    Input::new(&self.clipboard_input)
                                        .id("clipboard-shelf-search")
                                        .aria_label(l10n::search_clipboard(self.locale))
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false)
                                        .cleanable(true)
                                        .text_size(px(metrics::BODY_SIZE))
                                        .w_full(),
                                ),
                            ),
                    ),
            )
            .child(div().h(px(0.5)).bg(theme::divider()))
            .child(content)
            .child(
                div()
                    .px(px(metrics::SPACE_MD))
                    .py(px(metrics::SPACE_SM))
                    .border_t(px(0.5))
                    .border_color(theme::divider())
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_XS))
                    .child(
                        Button::new("shelf-copy")
                            .secondary()
                            .label(l10n::copy(self.locale))
                            .disabled(self.clipboard.selected_item().is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.copy_selected_clipboard(cx);
                            })),
                    )
                    .child(
                        Button::new("shelf-preview")
                            .ghost()
                            .label(l10n::preview(self.locale))
                            .disabled(self.clipboard.selected_item().is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.preview_selected_clipboard(cx);
                            })),
                    )
                    .child(
                        Button::new("shelf-delete")
                            .ghost()
                            .label(l10n::delete_entry(self.locale))
                            .disabled(self.clipboard.selected_item().is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.delete_selected_clipboard(cx);
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("shelf-refresh")
                            .ghost()
                            .label(l10n::refresh(self.locale))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request_clipboard_search(cx);
                            })),
                    ),
            )
            .when_some(self.notice.clone(), |element, notice| {
                element.child(
                    div()
                        .mx(px(metrics::SPACE_MD))
                        .mb(px(metrics::SPACE_SM))
                        .px(px(metrics::SPACE_SM))
                        .py(px(metrics::SPACE_XS))
                        .rounded(px(metrics::CONTROL_RADIUS))
                        .bg(theme::field())
                        .text_size(px(metrics::CAPTION_SIZE))
                        .text_color(theme::secondary())
                        .child(notice),
                )
            })
            .into_any_element()
    }

    fn render_plugin_detail(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(detail) = self.plugin_detail.clone() else {
            return self.render_launcher(cx);
        };
        let has_body = detail
            .body
            .as_deref()
            .is_some_and(|body| !body.trim().is_empty());
        let has_rows = !detail.rows.is_empty();
        let subtitle = match self.locale {
            Locale::En => "Read-only details provided by the action",
            Locale::Zh => "由操作提供的只读详情",
        };
        let mut content = div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_MD))
            .p(px(metrics::SPACE_LG))
            .overflow_y_scrollbar();
        for row in detail.rows {
            content = content.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(metrics::SPACE_LG))
                    .child(
                        div()
                            .w(px(140.))
                            .flex_shrink_0()
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::muted())
                            .child(row.label),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_size(px(metrics::BODY_SIZE))
                            .text_color(theme::primary())
                            .child(row.value),
                    ),
            );
        }
        if let Some(body) = detail.body.filter(|body| !body.trim().is_empty()) {
            content = content.child(div().h(px(0.5)).bg(theme::divider())).child(
                div()
                    .text_size(px(metrics::BODY_SIZE))
                    .line_height(px(24.))
                    .text_color(theme::secondary())
                    .child(body),
            );
        }
        if !has_rows && !has_body {
            content =
                content.child(self.empty_page_state(l10n::no_matching_entries(self.locale), None));
        }
        self.render_page_shell(
            SharedString::from(detail.title),
            SharedString::from(subtitle),
            None,
            content.into_any_element(),
            cx,
        )
        .into_any_element()
    }

    fn render_unsupported_tool(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (id, input) = self
            .unsupported_tool
            .clone()
            .unwrap_or_else(|| ("unknown".to_owned(), String::new()));
        let title = match self.locale {
            Locale::En => "Unsupported tool",
            Locale::Zh => "不支持的工具",
        };
        let description = match self.locale {
            Locale::En => {
                "This action requested a native tool that is not registered in the GPUI host."
            }
            Locale::Zh => "此操作请求了 GPUI 宿主中未注册的原生工具。",
        };
        let mut content = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_MD))
            .p(px(metrics::SPACE_LG))
            .child(
                div()
                    .text_size(px(metrics::BODY_SIZE))
                    .text_color(theme::secondary())
                    .child(description),
            )
            .child(
                div()
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .bg(theme::field())
                    .p(px(metrics::SPACE_MD))
                    .flex()
                    .flex_col()
                    .gap(px(metrics::SPACE_SM))
                    .child(
                        div()
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::muted())
                            .child("Tool ID"),
                    )
                    .child(
                        div()
                            .font_family(fonts::FONT_MONO)
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::primary())
                            .child(id),
                    ),
            );
        if !input.is_empty() {
            content = content.child(
                div()
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .bg(theme::field())
                    .p(px(metrics::SPACE_MD))
                    .flex()
                    .flex_col()
                    .gap(px(metrics::SPACE_SM))
                    .child(
                        div()
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::muted())
                            .child("Input"),
                    )
                    .child(
                        div()
                            .font_family(fonts::FONT_MONO)
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::primary())
                            .child(input),
                    ),
            );
        }
        self.render_page_shell(
            SharedString::from(title),
            SharedString::from(match self.locale {
                Locale::En => "Native destination unavailable",
                Locale::Zh => "原生目标不可用",
            }),
            None,
            content.into_any_element(),
            cx,
        )
        .into_any_element()
    }

    fn render_app_history(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let sources = self.app_history.sources.clone();
        let source_bar = div()
            .min_h(px(metrics::PAGE_TOOLBAR_HEIGHT + 8.0))
            .px(px(metrics::SPACE_MD))
            .py(px(metrics::SPACE_XS))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(metrics::SPACE_XS))
            .border_b(px(0.5))
            .border_color(theme::divider())
            .children(sources.into_iter().map(|source| {
                let selected = self.app_history.selected_source.as_deref() == Some(&source.id);
                let id = source.id;
                let label = source.title;
                Button::new(SharedString::from(format!("history-source-{id}")))
                    .when(selected, |button| button.secondary())
                    .when(!selected, |button| button.ghost())
                    .label(label)
                    .disabled(!source.action_enabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.app_history.selected_source = Some(id.clone());
                        this.request_app_history_search(cx);
                        cx.notify();
                    }))
            }));
        let content = if self.app_history.items.is_empty() {
            self.empty_page_state(
                if self.app_history.loading {
                    l10n::loading(self.locale)
                } else {
                    l10n::no_matching_entries(self.locale)
                },
                self.app_history.notice.clone(),
            )
        } else {
            let mut list = div()
                .id("application-history-list")
                .flex()
                .flex_col()
                .gap(px(2.0))
                .p(px(6.0));
            for (index, item) in self.app_history.items.clone().into_iter().enumerate() {
                let selected = self.app_history.selected == index;
                list = list.child(
                    div()
                        .id(SharedString::from(format!("history-item-{index}")))
                        .role(gpui_kit::Role::ListBoxOption)
                        .aria_label(item.title.clone())
                        .aria_selected(selected)
                        .h(px(LAUNCHER_ROW_HEIGHT - 4.0))
                        .px(px(metrics::SPACE_MD))
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_MD))
                        .rounded(px(metrics::RESULT_RADIUS))
                        .when(selected, |element| {
                            element
                                .bg(theme::selected())
                                .border(px(metrics::FOCUS_BORDER_WIDTH))
                                .border_color(theme::border())
                        })
                        .when(!selected, |element| {
                            element.hover(|style| style.bg(theme::hover()))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.app_history.selected = index;
                            this.notice = None;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .size(px(30.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(8.))
                                .bg(theme::field())
                                .text_color(theme::accent())
                                .child(match item.kind {
                                    "directory" => "▰",
                                    "workspace" => "◇",
                                    "remote" => "↗",
                                    _ => "▤",
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(px(metrics::BODY_SIZE))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme::primary())
                                        .child(item.title),
                                )
                                .child(
                                    div()
                                        .text_size(px(metrics::CAPTION_SIZE))
                                        .text_color(theme::secondary())
                                        .child(item.value),
                                ),
                        )
                        .when(selected, |element| element.child(self.keycap("↵"))),
                );
            }
            list.into_any_element()
        };
        self.render_page_shell(
            SharedString::from(l10n::app_history(self.locale)),
            SharedString::from(format!(
                "{} · {}",
                self.app_history.sources.len(),
                l10n::app_history_sources(self.locale)
            )),
            Some((
                &self.history_input,
                "application-history-search",
                l10n::app_history_search(self.locale),
            )),
            div()
                .flex()
                .flex_col()
                .child(source_bar)
                .child(content)
                .into_any_element(),
            cx,
        )
        .into_any_element()
    }

    fn render_tasks(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let body = if self.tasks.rows.is_empty() {
            self.empty_page_state(l10n::no_matching_tasks(self.locale), None)
        } else {
            let mut list = div()
                .id("execution-list")
                .w(px(360.))
                .h_full()
                .flex()
                .flex_col()
                .border_r(px(0.5))
                .border_color(theme::divider())
                .overflow_y_scrollbar();
            for (index, row) in self.tasks.rows.clone().into_iter().enumerate() {
                let selected = self.tasks.selected == index;
                let row_id = row.id.clone();
                let status = match row.status {
                    anywhere_protocol::ExecutionSessionStatus::Running => {
                        l10n::running(self.locale)
                    }
                    anywhere_protocol::ExecutionSessionStatus::Stopping => {
                        l10n::stopping(self.locale)
                    }
                    anywhere_protocol::ExecutionSessionStatus::Succeeded => l10n::done(self.locale),
                    anywhere_protocol::ExecutionSessionStatus::Failed => l10n::failed(self.locale),
                    anywhere_protocol::ExecutionSessionStatus::Stopped => {
                        l10n::stopped(self.locale)
                    }
                };
                list = list.child(
                    div()
                        .id(SharedString::from(format!("execution-row-{index}")))
                        .aria_label(row.title.clone())
                        .aria_selected(selected)
                        .p(px(metrics::SPACE_MD))
                        .flex()
                        .flex_col()
                        .gap(px(metrics::SPACE_XS))
                        .border_b(px(0.5))
                        .border_color(theme::divider())
                        .when(selected, |element| element.bg(theme::selected()))
                        .when(!selected, |element| {
                            element.hover(|style| style.bg(theme::hover()))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.tasks.selected = index;
                            this.tasks.detail = None;
                            if let Some(live) = this.live.clone() {
                                let receiver = live.execution_detail(row_id.clone());
                                this.poll(receiver, this.stream_generation, cx);
                            }
                            cx.notify();
                        }))
                        .child(
                            div()
                                .text_size(px(metrics::BODY_SIZE))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::primary())
                                .child(row.title),
                        )
                        .child(
                            div()
                                .text_size(px(metrics::CAPTION_SIZE))
                                .text_color(theme::secondary())
                                .child(format!("{status} · {}", row.directory)),
                        ),
                );
            }
            let detail = self
                .tasks
                .detail
                .as_ref()
                .map(|detail| {
                    let mut output = div().flex().flex_col().gap(px(metrics::SPACE_XS));
                    for line in detail.stdout.lines().chain(detail.stderr.lines()).take(400) {
                        output = output.child(
                            div()
                                .font_family(fonts::FONT_MONO)
                                .text_size(px(metrics::PREVIEW_CODE_SIZE))
                                .text_color(theme::secondary())
                                .child(line.to_owned()),
                        );
                    }
                    if detail.stdout.is_empty() && detail.stderr.is_empty() {
                        output = output.child(
                            div()
                                .text_size(px(metrics::BODY_SIZE))
                                .text_color(theme::muted())
                                .child(l10n::loading(self.locale)),
                        );
                    }
                    output
                })
                .unwrap_or_else(|| {
                    div()
                        .text_size(px(metrics::BODY_SIZE))
                        .text_color(theme::muted())
                        .child(match self.locale {
                            Locale::En => "Select a task to inspect its output",
                            Locale::Zh => "选择任务以查看输出",
                        })
                });
            div()
                .flex()
                .h_full()
                .child(list)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .h_full()
                        .p(px(metrics::SPACE_LG))
                        .flex()
                        .flex_col()
                        .gap(px(metrics::SPACE_MD))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px(metrics::HEADING_SIZE))
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(
                                            self.tasks
                                                .detail
                                                .as_ref()
                                                .map(|detail| detail.row.title.clone())
                                                .or_else(|| {
                                                    self.tasks
                                                        .selected_row()
                                                        .map(|row| row.title.clone())
                                                })
                                                .unwrap_or_default(),
                                        ),
                                )
                                .child(
                                    Button::new("stop-execution")
                                        .secondary()
                                        .label(l10n::stop(self.locale))
                                        .disabled(
                                            !self
                                                .tasks
                                                .selected_row()
                                                .is_some_and(|row| row.status.is_active()),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.stop_selected_execution(cx)
                                        })),
                                ),
                        )
                        .child(div().h(px(0.5)).bg(theme::divider()))
                        .child(
                            div()
                                .flex_1()
                                .min_h(px(0.))
                                .overflow_y_scrollbar()
                                .child(detail),
                        ),
                )
                .into_any_element()
        };
        self.render_page_shell(
            SharedString::from(l10n::running_tasks(self.locale)),
            SharedString::from(format!("{} tasks", self.tasks.rows.len())),
            None,
            body,
            cx,
        )
        .into_any_element()
    }

    fn render_web_packages(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let content = if self.web.packages.is_empty() {
            self.empty_page_state(
                match self.locale {
                    Locale::En => "No Web tools installed",
                    Locale::Zh => "尚未安装 Web 工具",
                },
                None,
            )
        } else {
            let mut list = div()
                .id("web-package-list")
                .flex()
                .flex_col()
                .gap(px(2.0))
                .p(px(6.));
            for (index, package) in self.web.packages.clone().into_iter().enumerate() {
                let selected = self.web.selected == index;
                let package_id = package.id.clone();
                list = list.child(
                    div()
                        .id(SharedString::from(format!("web-package-{index}")))
                        .aria_label(package.name.clone())
                        .aria_selected(selected)
                        .min_h(px(64.))
                        .px(px(metrics::SPACE_MD))
                        .py(px(metrics::SPACE_SM))
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_MD))
                        .rounded(px(metrics::RESULT_RADIUS))
                        .when(selected, |element| {
                            element
                                .bg(theme::selected())
                                .border(px(metrics::FOCUS_BORDER_WIDTH))
                                .border_color(theme::border())
                        })
                        .when(!selected, |element| {
                            element.hover(|style| style.bg(theme::hover()))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.web.selected = index;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .size(px(34.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(9.))
                                .bg(theme::field())
                                .text_color(theme::accent())
                                .child("◇"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(px(metrics::BODY_SIZE))
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(package.name),
                                )
                                .child(
                                    div()
                                        .text_size(px(metrics::CAPTION_SIZE))
                                        .text_color(theme::secondary())
                                        .child(format!(
                                            "{} {} · {} permissions",
                                            package.version,
                                            package.entry,
                                            package.permission_count
                                        )),
                                ),
                        )
                        .child(
                            Button::new(SharedString::from(format!("web-toggle-{package_id}")))
                                .secondary()
                                .label(if package.enabled {
                                    match self.locale {
                                        Locale::En => "Disable",
                                        Locale::Zh => "停用",
                                    }
                                } else {
                                    match self.locale {
                                        Locale::En => "Enable",
                                        Locale::Zh => "启用",
                                    }
                                })
                                .disabled(!package.available)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.change_web_package_operation(
                                        package_id.clone(),
                                        if package.enabled { "disable" } else { "enable" },
                                        cx,
                                    )
                                })),
                        ),
                );
            }
            list.into_any_element()
        };
        let selected = self.web.packages.get(self.web.selected).cloned();
        let review = self.web_review.clone().map(|review| {
            let approve_token = review.token.clone();
            let cancel_token = review.token.clone();
            div()
                .m(px(metrics::SPACE_MD))
                .p(px(metrics::SPACE_LG))
                .flex()
                .flex_col()
                .gap(px(metrics::SPACE_MD))
                .rounded(px(metrics::CARD_RADIUS))
                .bg(theme::field())
                .border(px(0.7))
                .border_color(theme::warning())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(px(metrics::HEADING_SIZE))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(format!("{} {}", review.name, review.version)),
                                )
                                .child(
                                    div()
                                        .text_size(px(metrics::CAPTION_SIZE))
                                        .text_color(theme::secondary())
                                        .child(format!(
                                            "{} · {}",
                                            review.id,
                                            if review.updating {
                                                match self.locale {
                                                    Locale::En => "Update",
                                                    Locale::Zh => "更新",
                                                }
                                            } else {
                                                match self.locale {
                                                    Locale::En => "New package",
                                                    Locale::Zh => "新安装包",
                                                }
                                            }
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .px(px(metrics::SPACE_SM))
                                .py(px(2.))
                                .rounded(px(5.0))
                                .bg(theme::glass_surface())
                                .font_family(fonts::FONT_MONO)
                                .text_size(px(metrics::CAPTION_SIZE))
                                .text_color(theme::muted())
                                .child(review.digest.chars().take(16).collect::<String>()),
                        ),
                )
                .child(
                    div()
                        .text_size(px(metrics::CAPTION_SIZE))
                        .text_color(theme::secondary())
                        .child(if review.permissions.is_empty() {
                            match self.locale {
                                Locale::En => "No capabilities requested",
                                Locale::Zh => "未请求任何能力",
                            }
                            .to_owned()
                        } else {
                            format!(
                                "{}: {}",
                                match self.locale {
                                    Locale::En => "Permissions",
                                    Locale::Zh => "权限",
                                },
                                review.permissions.join(", ")
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap(px(metrics::SPACE_SM))
                        .child(
                            Button::new("cancel-web-review")
                                .ghost()
                                .label(match self.locale {
                                    Locale::En => "Cancel",
                                    Locale::Zh => "取消",
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.web_review = None;
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live.cancel_web_review(cancel_token.clone());
                                        this.poll(receiver, this.stream_generation, cx);
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("approve-web-package")
                                .primary()
                                .label(match self.locale {
                                    Locale::En => "Approve and install",
                                    Locale::Zh => "批准并安装",
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(live) = this.live.clone() {
                                        let receiver =
                                            live.approve_web_package(approve_token.clone());
                                        this.poll(receiver, this.stream_generation, cx);
                                    }
                                })),
                        ),
                )
        });
        let footer = div()
            .px(px(metrics::SPACE_MD))
            .py(px(metrics::SPACE_SM))
            .border_t(px(0.5))
            .border_color(theme::divider())
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_SM))
            .child(
                Button::new("import-web-package")
                    .secondary()
                    .label(match self.locale {
                        Locale::En => "Import package…",
                        Locale::Zh => "导入安装包…",
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        let dialog = rfd::AsyncFileDialog::new()
                            .set_title(match this.locale {
                                Locale::En => "Choose a Web tool package",
                                Locale::Zh => "选择 Web 工具安装包",
                            })
                            .set_parent(window);
                        cx.spawn(async move |this, cx| {
                            let Some(folder) = dialog.pick_folder().await else {
                                return;
                            };
                            this.update(cx, |this, cx| {
                                if let Some(live) = this.live.clone() {
                                    let receiver = live
                                        .inspect_web_package(folder.path().display().to_string());
                                    this.poll(receiver, this.stream_generation, cx);
                                }
                            })
                            .ok();
                        })
                        .detach();
                    })),
            )
            .child(div().flex_1())
            .children(selected.map(|package| {
                let id = package.id;
                let rollback_id = id.clone();
                let uninstall_id = id;
                div()
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_SM))
                    .child(
                        Button::new("web-rollback")
                            .secondary()
                            .label(match self.locale {
                                Locale::En => "Rollback",
                                Locale::Zh => "回滚",
                            })
                            .disabled(!package.rollback)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_web_package_operation(
                                    rollback_id.clone(),
                                    "rollback",
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Button::new("web-uninstall")
                            .secondary()
                            .label(l10n::remove(self.locale))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_web_package_operation(
                                    uninstall_id.clone(),
                                    "uninstall",
                                    cx,
                                )
                            })),
                    )
            }));
        let mut page = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .child(content);
        if let Some(review) = review {
            page = page.child(review);
        }
        self.render_page_shell(
            SharedString::from(match self.locale {
                Locale::En => "Web Tools",
                Locale::Zh => "Web 工具",
            }),
            SharedString::from(match self.locale {
                Locale::En => "Installed packages and permissions",
                Locale::Zh => "已安装包与权限",
            }),
            None,
            page.child(footer).into_any_element(),
            cx,
        )
        .into_any_element()
    }

    fn change_web_package_operation(
        &mut self,
        id: String,
        operation: &'static str,
        cx: &mut Context<Self>,
    ) {
        if let Some(live) = self.live.clone() {
            let receiver = live.change_web_package(id, operation.to_owned());
            self.poll(receiver, self.stream_generation, cx);
        }
    }

    fn ensure_web_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.web_view.is_some() {
            return;
        }
        let (Some(document), Some(live)) = (self.web_document.clone(), self.live.clone()) else {
            return;
        };
        let policy = match WebDocumentPolicy::new(document.origin.clone(), document.entry.clone()) {
            Ok(policy) => policy,
            Err(error) => {
                self.web_error = Some(format!("Invalid Web document: {error:#}"));
                return;
            }
        };
        let runtime = live.web_runtime();
        let protocol_policy = policy.clone();
        let protocol_runtime = runtime.clone();
        let ipc_policy = policy.clone();
        let ipc_handle = document.handle.clone();
        let protocol_handle = document.handle.clone();
        let ipc_origin = document.origin.clone();
        let (ipc_sender, ipc_receiver) = tokio::sync::mpsc::unbounded_channel();
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
                match protocol_runtime.web_resource(protocol_handle.as_str(), &path) {
                    Ok(bytes) => protocol_policy.response(&path, bytes),
                    Err(_) => plain_response(404, "Not found"),
                }
            })
            .with_ipc_handler(move |request| {
                let source = request.uri().to_string();
                let frame = request.body();
                if frame.len() > WEB_FRAME_BYTES || !ipc_policy.accepts_source(&source) {
                    return;
                }
                let _ = ipc_sender.send(WebIpcRequest {
                    handle: ipc_handle.clone(),
                    origin: ipc_origin.clone(),
                    frame: frame.clone(),
                });
            })
            .with_url(policy.url());
        let web_view = match gpui_webview::WebView::build(builder, window, cx) {
            Ok(web_view) => web_view,
            Err(error) => {
                self.web_error = Some(format!("WebKit could not start: {error}"));
                return;
            }
        };
        if let Err(error) = web_view.set_theme(wry::Theme::Dark) {
            self.web_error = Some(format!("WebKit theme failed: {error}"));
        }
        self.web_view = Some(cx.new(|_| web_view));
        self.poll_web_ipc(ipc_receiver, document.handle, self.stream_generation, cx);
    }

    fn evaluate_web_script(&self, script: &str, cx: &Context<Self>) -> anyhow::Result<()> {
        let web_view = self
            .web_view
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Web view is not mounted"))?;
        web_view
            .read(cx)
            .raw()
            .evaluate_script(script)
            .map_err(anyhow::Error::from)
    }

    fn poll_web_ipc(
        &mut self,
        mut receiver: Receiver<WebIpcRequest>,
        handle: String,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Some(request) = receiver.recv().await {
                let reply = this.update(cx, |this, cx| {
                    if generation != this.stream_generation
                        || request.handle != handle
                        || this
                            .web_document
                            .as_ref()
                            .map(|document| document.handle.as_str())
                            != Some(handle.as_str())
                    {
                        return None;
                    }
                    let live = this.live.clone()?;
                    let receiver = live.dispatch_web_frame(
                        request.handle,
                        request.origin,
                        true,
                        request.frame.clone(),
                    );
                    this.poll_web_reply(receiver, handle.clone(), request.frame, generation, cx);
                    Some(())
                });
                if !matches!(reply, Ok(Some(()))) {
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
                        let _ = this.update(cx, |this, cx| {
                            if generation != this.stream_generation
                                || event_handle != handle
                                || this
                                    .web_document
                                    .as_ref()
                                    .map(|document| document.handle.as_str())
                                    != Some(handle.as_str())
                            {
                                return;
                            }
                            if this.evaluate_web_script(&reply_script(&frame), cx).is_err() {
                                this.web_error = Some(match this.locale {
                                    Locale::En => "The Web tool closed unexpectedly".to_owned(),
                                    Locale::Zh => "Web 工具意外关闭".to_owned(),
                                });
                            }
                            match accepted_web_acknowledgement(&request, &frame).as_deref() {
                                Some("lifecycle.ready") => this.web_ready = true,
                                Some("state.subscribe")
                                    if this.web_state_handle.as_deref()
                                        != Some(handle.as_str()) =>
                                {
                                    this.web_state_handle = Some(handle.clone());
                                    this.web_state_version = 0;
                                    this.follow_web_state(handle.clone(), 0, generation, cx);
                                }
                                _ => {}
                            }
                            cx.notify();
                        });
                        break;
                    }
                    LiveEvent::Error(error) => {
                        let _ = this.update(cx, |this, cx| {
                            if generation == this.stream_generation
                                && event_handle_matches(this, &handle)
                            {
                                this.web_error = Some(error);
                                this.web_state_handle = None;
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
                        if generation != this.stream_generation
                            || event_handle != handle
                            || !event_handle_matches(this, &handle)
                        {
                            return None;
                        }
                        let version = web_state_version(&frame).unwrap_or(after);
                        if let Err(error) = this.evaluate_web_script(&state_script(&frame), cx) {
                            this.web_error = Some(error.to_string());
                            this.web_state_handle = None;
                            cx.notify();
                            return None;
                        }
                        this.web_state_version = version;
                        cx.notify();
                        Some(version)
                    });
                    match next {
                        Ok(Some(version)) => {
                            let continuation = this.update(cx, |this, cx| {
                                if generation == this.stream_generation
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
                        if generation == this.stream_generation
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
                        if generation == this.stream_generation
                            && event_handle_matches(this, &handle)
                        {
                            this.web_error = Some(error);
                            this.web_state_handle = None;
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

    fn render_web_tool(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.ensure_web_view(window, cx);
        let (title, subtitle) = if let Some(document) = &self.web_document {
            (document.title.clone(), document.origin.clone())
        } else {
            (
                match self.locale {
                    Locale::En => "Web Tool".to_owned(),
                    Locale::Zh => "Web 工具".to_owned(),
                },
                match self.locale {
                    Locale::En => "Preparing the plugin document…".to_owned(),
                    Locale::Zh => "正在准备插件文档…".to_owned(),
                },
            )
        };
        let body = if let Some(error) = &self.web_error {
            self.empty_page_state(
                error.clone(),
                Some(match self.locale {
                    Locale::En => "Return to the launcher and try again".to_owned(),
                    Locale::Zh => "返回启动器后重试".to_owned(),
                }),
            )
        } else if let Some(web_view) = &self.web_view {
            div()
                .relative()
                .flex_1()
                .min_h(px(0.))
                .child(web_view.clone())
                .when(!self.web_ready, |element| {
                    element.child(
                        div()
                            .absolute()
                            .right(px(metrics::SPACE_MD))
                            .bottom(px(metrics::SPACE_MD))
                            .px(px(metrics::SPACE_SM))
                            .py(px(metrics::SPACE_XS))
                            .rounded(px(metrics::CONTROL_RADIUS))
                            .bg(theme::field())
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::secondary())
                            .child(match self.locale {
                                Locale::En => "Starting Web tool…",
                                Locale::Zh => "正在启动 Web 工具…",
                            }),
                    )
                })
                .into_any_element()
        } else {
            self.empty_page_state(
                match self.locale {
                    Locale::En => "Waiting for the Host document",
                    Locale::Zh => "正在等待宿主文档",
                },
                Some(if self.locale == Locale::En {
                    "Plugin code never runs without an approved package and Host-issued document."
                        .to_owned()
                } else {
                    "插件只有在包已批准且宿主签发文档后才会运行。".to_owned()
                }),
            )
        };
        self.render_page_shell(
            SharedString::from(title),
            SharedString::from(subtitle),
            None,
            body,
            cx,
        )
        .into_any_element()
    }

    fn empty_page_state(
        &self,
        title: impl Into<SharedString>,
        detail: Option<String>,
    ) -> AnyElement {
        div()
            .flex_1()
            .min_h(px(160.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(metrics::SPACE_SM))
            .p(px(metrics::SPACE_XXL))
            .text_size(px(metrics::BODY_SIZE))
            .text_color(theme::secondary())
            .child(
                div()
                    .text_size(px(28.))
                    .text_color(theme::accent())
                    .child("⌕"),
            )
            .child(title.into())
            .when_some(detail, |element, detail| {
                element.child(
                    div()
                        .text_size(px(metrics::CAPTION_SIZE))
                        .text_color(theme::muted())
                        .child(detail),
                )
            })
            .into_any_element()
    }

    fn render_page_shell(
        &self,
        title: SharedString,
        subtitle: SharedString,
        search: Option<(&Entity<InputState>, &'static str, String)>,
        content: AnyElement,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .min_h(px(metrics::PAGE_HEADER_HEIGHT))
                    .px(px(metrics::SPACE_LG))
                    .py(px(metrics::SPACE_SM))
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_MD))
                    .child(
                        Button::new("page-back")
                            .ghost()
                            .label("‹")
                            .accessibility_label(l10n::back(self.locale))
                            .on_click(cx.listener(|this, _, window, cx| this.go_back(window, cx))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(metrics::PAGE_TITLE_GAP))
                            .child(
                                div()
                                    .text_size(px(metrics::PAGE_HEADING_SIZE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::primary())
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_size(px(metrics::CAPTION_SIZE))
                                    .text_color(theme::muted())
                                    .child(subtitle),
                            ),
                    )
                    .children(search.map(|(input, id, placeholder)| {
                        div()
                            .w(px(250.))
                            .flex()
                            .items_center()
                            .gap(px(metrics::SPACE_SM))
                            .child(
                                div().flex_1().min_w(px(0.)).child(
                                    Input::new(input)
                                        .id(SharedString::from(id))
                                        .aria_label(placeholder.clone())
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false)
                                        .cleanable(true)
                                        .text_size(px(metrics::BODY_SIZE))
                                        .w_full(),
                                ),
                            )
                    }))
                    .child(self.keycap("esc")),
            )
            .child(div().h(px(0.5)).bg(theme::divider()))
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .flex_col()
                    .child(content),
            )
            .when_some(self.notice.clone(), |element, notice| {
                element.child(
                    div()
                        .mx(px(metrics::SPACE_MD))
                        .mb(px(metrics::SPACE_SM))
                        .px(px(metrics::SPACE_MD))
                        .py(px(metrics::SPACE_SM))
                        .rounded(px(metrics::CONTROL_RADIUS))
                        .bg(theme::field())
                        .text_size(px(metrics::CAPTION_SIZE))
                        .text_color(theme::secondary())
                        .child(notice),
                )
            })
    }

    fn scope_button(
        &self,
        scope: Option<String>,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = scope
            .as_deref()
            .and_then(|id| self.model.scopes.iter().find(|item| item.id == id))
            .map(|item| item.label.clone())
            .unwrap_or_else(|| match self.locale {
                Locale::En => "All".to_owned(),
                Locale::Zh => "全部".to_owned(),
            });
        Button::new(if selected {
            SharedString::from(format!("scope-selected-{label}"))
        } else {
            SharedString::from(format!("scope-{label}"))
        })
        .secondary()
        .label(label)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.model.select_scope(scope.clone());
            this.request_live_update(cx);
            this.notice = None;
            cx.notify();
        }))
    }

    fn render_row(
        &self,
        index: usize,
        row: LauncherRow,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let enabled = row.enabled;
        let is_menu = row.is_menu;
        let is_file = row.kind == anywhere_protocol::CandidateKind::File;
        let icon = self.row_icon(&row);
        let subtitle = row.subtitle.clone();
        let title = row.title.clone();
        let accessibility = row.title.clone();
        let shortcut = row.shortcut.clone();
        div()
            .id(SharedString::from(format!("candidate-{index}")))
            .role(gpui_kit::Role::ListBoxOption)
            .aria_label(accessibility)
            .aria_selected(selected)
            .test_support()
            .h(px(LAUNCHER_ROW_HEIGHT - 4.0))
            .px(px(metrics::SPACE_SM + 2.0))
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_MD))
            .rounded(px(metrics::RESULT_RADIUS))
            .when(selected, |element| {
                element
                    .bg(theme::selected())
                    .border(px(metrics::FOCUS_BORDER_WIDTH))
                    .border_color(theme::border())
            })
            .when(!selected, |element| {
                element.hover(|style| style.bg(theme::hover()))
            })
            .when(!enabled, |element| {
                element.opacity(metrics::DISABLED_OPACITY)
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.screen == Screen::FileSearch {
                    this.file_search_browsing = true;
                }
                this.model.select_row(index);
                if is_menu {
                    this.model.open_selected_menu();
                    this.resize_current_window(cx);
                } else if this.preview_open && is_file {
                    this.request_preview(cx);
                }
                this.notice = None;
                cx.notify();
            }))
            .child(
                div()
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(9.))
                    .bg(theme::field())
                    .text_color(if enabled {
                        theme::accent()
                    } else {
                        theme::muted()
                    })
                    .text_size(px(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(icon),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(metrics::HEADING_SIZE))
                            .font_weight(if selected {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::MEDIUM
                            })
                            .text_color(theme::primary())
                            .child(title),
                    )
                    .when_some(subtitle, |element, subtitle| {
                        element.child(
                            div()
                                .text_size(px(11.5))
                                .text_color(if enabled {
                                    theme::secondary()
                                } else {
                                    theme::warning()
                                })
                                .child(subtitle),
                        )
                    }),
            )
            .when_some(shortcut, |element, shortcut| {
                element.child(self.keycap(shortcut))
            })
            .when(selected && enabled, |element| {
                element.child(
                    div()
                        .px(px(metrics::SPACE_XS + 2.0))
                        .py(px(2.))
                        .rounded(px(5.))
                        .bg(theme::field())
                        .text_size(px(10.5))
                        .text_color(theme::secondary())
                        .child("↵"),
                )
            })
    }

    fn row_icon(&self, row: &LauncherRow) -> AnyElement {
        if row.kind == CandidateKind::Application {
            if let Some(Some(image)) = self.application_icons.get(&row.id) {
                return img(image.clone())
                    .size(px(22.))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element();
            }
        }
        div().child(icon_sigil(&row.icon)).into_any_element()
    }

    fn keycap(&self, value: impl Into<SharedString>) -> impl IntoElement {
        div()
            .min_w(px(metrics::KEYCAP_MIN_WIDTH))
            .px(px(metrics::KEYCAP_PADDING_X))
            .py(px(metrics::KEYCAP_PADDING_Y))
            .rounded(px(metrics::KEYCAP_RADIUS))
            .border(px(0.5))
            .border_color(theme::border())
            .text_size(px(metrics::KEYCAP_FONT_SIZE))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme::secondary())
            .child(value.into())
    }

    fn render_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let categories = self.settings.selected.label(self.locale).to_owned();
        let dirty = self.settings_dirty(cx);
        let detail = match self.settings.selected {
            SettingsCategory::General => self.general_settings(cx),
            SettingsCategory::Shortcuts => self.shortcut_settings(cx),
            SettingsCategory::Context => self.context_settings(cx),
            SettingsCategory::Search => self.search_settings(cx),
            SettingsCategory::Clipboard => self.clipboard_settings(cx),
            SettingsCategory::ApplicationHistory => self.application_history_settings(cx),
            SettingsCategory::Plugins => self.feature_settings(cx),
            SettingsCategory::Language => self.language_settings(cx),
        };
        let close_prompt = self.settings_close_pending.then(|| {
            div()
                .p(px(metrics::SPACE_MD))
                .flex()
                .items_center()
                .gap(px(metrics::SPACE_SM))
                .rounded(px(metrics::CARD_RADIUS))
                .bg(theme::field())
                .border(px(0.5))
                .border_color(theme::warning())
                .child(
                    div()
                        .flex_1()
                        .text_size(px(metrics::CAPTION_SIZE))
                        .text_color(theme::secondary())
                        .child(l10n::settings_drafts(self.locale)),
                )
                .child(
                    Button::new("keep-editing")
                        .ghost()
                        .label(l10n::keep_editing(self.locale))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings_close_pending = false;
                            this.notice = None;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("discard-settings")
                        .secondary()
                        .label(l10n::close_without_saving(self.locale))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.discard_settings(window, cx)),
                        ),
                )
                .child(
                    Button::new("save-settings-confirm")
                        .primary()
                        .label(l10n::save_changes(self.locale))
                        .on_click(cx.listener(|this, _, _, cx| this.save_settings(cx))),
                )
        });
        div()
            .id("settings-surface")
            .size_full()
            .flex()
            .test_support()
            .child(
                div()
                    .w(px(184.))
                    .h_full()
                    .p(px(metrics::SPACE_MD))
                    .flex()
                    .flex_col()
                    .gap(px(metrics::SPACE_XS))
                    .bg(theme::sidebar())
                    .child(
                        div()
                            .h(px(48.))
                            .px(px(metrics::SPACE_SM))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(metrics::HEADING_SIZE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::primary())
                                    .child(l10n::settings(self.locale)),
                            )
                            .child(
                                Button::new("close-settings")
                                    .ghost()
                                    .label("×")
                                    .accessibility_label(match self.locale {
                                        Locale::En => "Close settings",
                                        Locale::Zh => "关闭设置",
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.request_close_settings(window, cx)
                                    })),
                            ),
                    )
                    .children(SettingsCategory::ALL.into_iter().map(|category| {
                        let selected = category == self.settings.selected;
                        div()
                            .id(SharedString::from(format!(
                                "settings-category-{}",
                                category.label(self.locale)
                            )))
                            .role(gpui_kit::Role::ListItem)
                            .aria_label(category.label(self.locale))
                            .aria_selected(selected)
                            .h(px(38.))
                            .px(px(metrics::SPACE_MD))
                            .flex()
                            .items_center()
                            .rounded(px(metrics::CONTROL_RADIUS))
                            .when(selected, |element| element.bg(theme::sidebar_selection()))
                            .when(!selected, |element| {
                                element.hover(|style| style.bg(theme::hover()))
                            })
                            .text_size(px(metrics::BODY_SIZE))
                            .text_color(if selected {
                                theme::primary()
                            } else {
                                theme::secondary()
                            })
                            .child(category.label(self.locale))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.settings.selected = category;
                                cx.notify();
                            }))
                    })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .p(px(metrics::SPACE_XL))
                    .flex()
                    .flex_col()
                    .gap(px(metrics::SPACE_LG))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(metrics::PAGE_HEADING_SIZE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::primary())
                                    .child(categories),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(metrics::SPACE_SM))
                                    .when(self.settings.saving || dirty, |element| {
                                        element.child(
                                            div()
                                                .text_size(px(metrics::CAPTION_SIZE))
                                                .text_color(if dirty {
                                                    theme::warning()
                                                } else {
                                                    theme::muted()
                                                })
                                                .child(if self.settings.saving {
                                                    l10n::saving(self.locale)
                                                } else {
                                                    l10n::changes_not_saved(self.locale)
                                                }),
                                        )
                                    })
                                    .child(
                                        Button::new("save-settings")
                                            .secondary()
                                            .label(l10n::save_changes(self.locale))
                                            .disabled(!dirty || self.settings.saving)
                                            .on_click(
                                                cx.listener(|this, _, _, cx| {
                                                    this.save_settings(cx)
                                                }),
                                            ),
                                    )
                                    .child(
                                        Button::new("done-settings")
                                            .primary()
                                            .label(l10n::done(self.locale))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.request_close_settings(window, cx)
                                            })),
                                    ),
                            ),
                    )
                    .child(div().h(px(0.5)).bg(theme::divider()))
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(0.))
                            .overflow_y_scrollbar()
                            .p(px(metrics::SPACE_LG))
                            .flex()
                            .flex_col()
                            .gap(px(metrics::SPACE_MD))
                            .when_some(self.settings.error.clone(), |element, error| {
                                element.child(
                                    div()
                                        .p(px(metrics::SPACE_MD))
                                        .rounded(px(metrics::CONTROL_RADIUS))
                                        .bg(theme::field())
                                        .text_size(px(metrics::CAPTION_SIZE))
                                        .text_color(theme::danger())
                                        .child(error),
                                )
                            })
                            .when_some(close_prompt, |element, prompt| element.child(prompt))
                            .child(detail),
                    ),
            )
            .into_any_element()
    }

    fn general_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let data_directory = self
            .live
            .as_ref()
            .map(|live| live.data_directory().display().to_string())
            .unwrap_or_else(|| "~/Library/Application Support/Anywhere".to_owned());
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .child(
                self.settings_row(
                    l10n::open_at_login(self.locale),
                    l10n::open_at_login_description(self.locale),
                    Switch::new("open-at-login")
                        .checked(self.settings_draft.launch_at_login)
                        .accessibility_label(l10n::open_at_login(self.locale))
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_draft.launch_at_login = *checked;
                            cx.notify();
                        })),
                ),
            )
            .child(self.settings_row(
                match self.locale {
                    Locale::En => "Anywhere",
                    Locale::Zh => "Anywhere",
                },
                match self.locale {
                    Locale::En => "Native GPUI product · GNU make not required",
                    Locale::Zh => "原生 GPUI 产品 · 最低 macOS 15",
                },
                self.keycap("0.14"),
            ))
            .child(
                self.settings_row(
                    match self.locale {
                        Locale::En => "Data directory",
                        Locale::Zh => "数据目录",
                    },
                    data_directory,
                    div()
                        .text_size(px(metrics::CAPTION_SIZE))
                        .text_color(theme::secondary())
                        .child(l10n::location(self.locale)),
                ),
            )
    }

    fn shortcut_settings(&mut self, _cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .child(self.settings_input_row(
                l10n::current_app(self.locale),
                l10n::navigation_modes(self.locale),
                &self.current_shortcut_input,
                "current-shortcut",
                "meta+alt+Space",
            ))
            .child(self.settings_input_row(
                l10n::universal_search(self.locale),
                l10n::search_apps_actions(self.locale),
                &self.universal_shortcut_input,
                "universal-shortcut",
                "meta+Space",
            ))
            .child(self.settings_row(
                l10n::global_search(self.locale),
                l10n::shortcut_help(self.locale),
                self.keycap("⌘↩"),
            ))
    }

    fn context_settings(&mut self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .child(
                self.settings_row(
                    l10n::finder_selection(self.locale),
                    l10n::selected_files_description(self.locale),
                    Switch::new("capture-finder")
                        .checked(self.settings_draft.capture_finder_selection)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_draft.capture_finder_selection = *checked;
                            cx.notify();
                        })),
                ),
            )
            .child(
                self.settings_row(
                    l10n::selected_text(self.locale),
                    l10n::selected_text_description(self.locale),
                    Switch::new("capture-selected-text")
                        .checked(self.settings_draft.capture_selected_text)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_draft.capture_selected_text = *checked;
                            cx.notify();
                        })),
                ),
            )
            .child(
                self.settings_row(
                    l10n::browser_current_page(self.locale),
                    l10n::context_description(self.locale),
                    Switch::new("capture-browser")
                        .checked(self.settings_draft.capture_browser_current_page)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_draft.capture_browser_current_page = *checked;
                            cx.notify();
                        })),
                ),
            )
            .child(
                self.settings_row(
                    l10n::clipboard_text(self.locale),
                    l10n::context_description(self.locale),
                    Switch::new("capture-clipboard")
                        .checked(self.settings_draft.capture_clipboard)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_draft.capture_clipboard = *checked;
                            cx.notify();
                        })),
                ),
            )
            .child(self.settings_input_row(
                match self.locale {
                    Locale::En => "Excluded applications",
                    Locale::Zh => "排除的应用",
                },
                match self.locale {
                    Locale::En => "Bundle identifiers separated by commas",
                    Locale::Zh => "使用逗号分隔 Bundle ID",
                },
                &self.excluded_apps_input,
                "excluded-apps",
                "com.example.Editor, com.example.Terminal",
            ))
    }

    fn search_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let engine = match self.settings_draft.file_search_engine.as_str() {
            "ripgrep" => "ripgrep",
            "combined" => "combined",
            _ => "system",
        };
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .child(
                self.settings_row(
                    l10n::file_search(self.locale),
                    l10n::search_files_folders(self.locale),
                    Switch::new("file-search-enabled")
                        .checked(self.settings_draft.file_search_enabled)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_draft.file_search_enabled = *checked;
                            cx.notify();
                        })),
                ),
            )
            .child(
                self.settings_row(
                    match self.locale {
                        Locale::En => "Search engine",
                        Locale::Zh => "搜索引擎",
                    },
                    match self.locale {
                        Locale::En => "System Index, Ripgrep, or Combined",
                        Locale::Zh => "系统索引、Ripgrep 或组合模式",
                    },
                    Button::new("cycle-file-engine")
                        .secondary()
                        .label(engine)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings_draft.file_search_engine =
                                match this.settings_draft.file_search_engine.as_str() {
                                    "system_index" | "systemIndex" => "ripgrep".to_owned(),
                                    "ripgrep" => "combined".to_owned(),
                                    _ => "system_index".to_owned(),
                                };
                            cx.notify();
                        })),
                ),
            )
            .child(
                self.settings_row(
                    match self.locale {
                        Locale::En => "Include hidden files",
                        Locale::Zh => "包含隐藏文件",
                    },
                    l10n::find_files_folders(self.locale),
                    Switch::new("file-search-hidden")
                        .checked(self.settings_draft.file_search_include_hidden)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_draft.file_search_include_hidden = *checked;
                            cx.notify();
                        })),
                ),
            )
            .child(
                self.settings_row(
                    match self.locale {
                        Locale::En => "Directory access",
                        Locale::Zh => "目录访问权限",
                    },
                    l10n::file_search_access_message(self.locale),
                    Button::new("authorize-search-directories")
                        .secondary()
                        .label(l10n::authorize_saved_directories(self.locale))
                        .on_click(cx.listener(|this, _, window, cx| {
                            let existing = lines(this.file_roots_input.read(cx).value().as_ref());
                            let mut dialog = rfd::AsyncFileDialog::new()
                                .set_title(match this.locale {
                                    Locale::En => "Authorize search directories",
                                    Locale::Zh => "授权搜索目录",
                                })
                                .set_parent(window);
                            if let Some(first) = existing.first() {
                                dialog = dialog.set_directory(first);
                            }
                            let window_handle = this.window_handle;
                            cx.spawn(async move |this, cx| {
                                let Some(selected) = dialog.pick_folders().await else {
                                    return;
                                };
                                let selected = selected
                                    .into_iter()
                                    .map(|path| path.path().display().to_string())
                                    .collect::<Vec<_>>();
                                let roots = unique_trimmed(
                                    existing
                                        .iter()
                                        .map(String::as_str)
                                        .chain(selected.iter().map(String::as_str)),
                                );
                                this.update(cx, |this, cx| {
                                    if let Some(live) = this.live.clone() {
                                        this.notice = Some(match this.locale {
                                            Locale::En => "Preparing directory access…".to_owned(),
                                            Locale::Zh => "正在准备目录访问权限…".to_owned(),
                                        });
                                        let receiver =
                                            live.authorize_search_directories(selected, roots);
                                        this.poll(receiver, this.stream_generation, cx);
                                    } else {
                                        this.settings_draft.file_search_roots = roots.clone();
                                        let input = this.file_roots_input.clone();
                                        let value = roots.join("\n");
                                        let _ = cx.update_window(window_handle, |_, window, cx| {
                                            set_input_value(&input, value, window, cx);
                                        });
                                    }
                                    cx.notify();
                                })
                                .ok();
                            })
                            .detach();
                        })),
                ),
            )
            .child(self.settings_input_row(
                match self.locale {
                    Locale::En => "Authorized directories",
                    Locale::Zh => "授权目录",
                },
                match self.locale {
                    Locale::En => "Absolute paths separated by commas or new lines",
                    Locale::Zh => "使用逗号或换行分隔绝对路径",
                },
                &self.file_roots_input,
                "file-search-roots",
                "/Users/me/Documents, /Users/me/Projects",
            ))
            .child(self.settings_input_row(
                match self.locale {
                    Locale::En => "Excluded directory names",
                    Locale::Zh => "排除的目录名",
                },
                l10n::search(self.locale),
                &self.file_exclusions_input,
                "file-search-exclusions",
                "node_modules, target, .git",
            ))
    }

    fn clipboard_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let status = self.clipboard.status.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .child(
                self.settings_row(
                    l10n::record_clipboard_history(self.locale),
                    l10n::save_future_copies_while_anywhere_is_running(self.locale),
                    Switch::new("clipboard-recording")
                        .checked(self.settings.clipboard_recording)
                        .accessibility_label(l10n::clipboard(self.locale))
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings.clipboard_recording = *checked;
                            if let Some(live) = this.live.clone() {
                                this.settings.saving = true;
                                this.settings.error = None;
                                let receiver = live.set_clipboard_recording(*checked);
                                this.poll(receiver, this.stream_generation, cx);
                            }
                            cx.notify();
                        })),
                ),
            )
            .child(
                self.settings_row(
                    l10n::history_storage(self.locale),
                    status
                        .as_ref()
                        .map(|status| {
                            l10n::history_storage_usage(
                                self.locale,
                                i64::from(status.stored_entries),
                                &format!("{:.1}", f64::from(status.stored_bytes) / 1_048_576.0),
                            )
                        })
                        .unwrap_or_else(|| l10n::clipboard_history(self.locale)),
                    div()
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_SM))
                        .child(
                            Button::new("clipboard-retention")
                                .secondary()
                                .label(l10n::retention_days(
                                    self.locale,
                                    i64::from(
                                        status
                                            .as_ref()
                                            .map(|status| status.retention_days)
                                            .unwrap_or(30),
                                    ),
                                ))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live.set_clipboard_retention(30);
                                        this.poll(receiver, this.stream_generation, cx);
                                    }
                                })),
                        )
                        .child(
                            Button::new("clipboard-pause")
                                .secondary()
                                .label(l10n::paused(self.locale))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live.set_clipboard_pause(15);
                                        this.poll(receiver, this.stream_generation, cx);
                                    }
                                })),
                        )
                        .child(
                            Button::new("clipboard-clear-settings")
                                .secondary()
                                .label(l10n::clear_history(self.locale))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live.clear_clipboard();
                                        this.poll(receiver, this.stream_generation, cx);
                                    }
                                })),
                        ),
                ),
            )
    }

    fn application_history_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let sources = self.app_history.sources.clone();
        let mut rows = div().flex().flex_col().gap(px(metrics::SPACE_SM));
        for source in sources {
            let id = source.id.clone();
            let configure_id = id.clone();
            let enabled_id = id.clone();
            let automatic_id = id.clone();
            let custom_id = id.clone();
            let disabled_id = id;
            let mode_label = match source.mode {
                anywhere_core::features::application_history::model::SourceMode::Automatic => {
                    l10n::app_history_automatic(self.locale)
                }
                anywhere_core::features::application_history::model::SourceMode::Custom => {
                    l10n::app_history_custom(self.locale)
                }
                anywhere_core::features::application_history::model::SourceMode::Disabled => {
                    l10n::app_history_source_off(self.locale)
                }
            };
            let path = source
                .path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| source.file_hint.clone());
            rows = rows.child(
                div()
                    .p(px(metrics::SPACE_MD))
                    .flex()
                    .flex_col()
                    .gap(px(metrics::SPACE_SM))
                    .rounded(px(metrics::CARD_RADIUS))
                    .bg(theme::field())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(metrics::SPACE_MD))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_size(px(metrics::BODY_SIZE))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(source.title),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(metrics::CAPTION_SIZE))
                                            .text_color(theme::secondary())
                                            .child(format!("{mode_label} · {path}")),
                                    ),
                            )
                            .child(
                                Switch::new(SharedString::from(format!(
                                    "history-action-{enabled_id}"
                                )))
                                .checked(source.action_enabled)
                                .on_change(cx.listener(move |this, checked, _, cx| {
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live
                                            .set_app_history_action_enabled(
                                                enabled_id.clone(),
                                                *checked,
                                            );
                                        this.poll(receiver, this.stream_generation, cx);
                                    }
                                })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(metrics::SPACE_XS))
                            .child(
                                Button::new(SharedString::from(format!(
                                    "history-auto-{automatic_id}"
                                )))
                                .secondary()
                                .label(l10n::app_history_automatic(self.locale))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live.configure_app_history(
                                            automatic_id.clone(),
                                            String::new(),
                                            anywhere_core::features::application_history::model::SourceMode::Automatic,
                                        );
                                        this.poll(receiver, this.stream_generation, cx);
                                    }
                                })),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "history-custom-{custom_id}"
                                )))
                                .secondary()
                                .label(l10n::app_history_custom(self.locale))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let path = this
                                        .history_path_input
                                        .read(cx)
                                        .value()
                                        .trim()
                                        .to_owned();
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live.configure_app_history(
                                            custom_id.clone(),
                                            path,
                                            anywhere_core::features::application_history::model::SourceMode::Custom,
                                        );
                                        this.poll(receiver, this.stream_generation, cx);
                                    }
                                })),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "history-off-{disabled_id}"
                                )))
                                .secondary()
                                .label(l10n::app_history_source_off(self.locale))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live.configure_app_history(
                                            disabled_id.clone(),
                                            String::new(),
                                            anywhere_core::features::application_history::model::SourceMode::Disabled,
                                        );
                                        this.poll(receiver, this.stream_generation, cx);
                                    }
                                })),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!(
                            "history-select-{configure_id}"
                        )))
                        .ghost()
                        .label(l10n::app_history_custom(self.locale))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.app_history.selected_source = Some(configure_id.clone());
                            let value = this
                                .app_history
                                .sources
                                .iter()
                                .find(|source| source.id == configure_id)
                                .and_then(|source| source.path.clone())
                                .map(|path| path.display().to_string())
                                .unwrap_or_default();
                            set_input_value(&this.history_path_input, value, window, cx);
                        })),
                    ),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_MD))
            .child(
                div()
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::secondary())
                    .child(l10n::app_history_source_description(self.locale)),
            )
            .child(self.settings_input_row(
                l10n::app_history_source_path(self.locale),
                l10n::app_history_custom_adapters_help(self.locale),
                &self.history_path_input,
                "history-custom-path",
                "~/Library/Application Support/...",
            ))
            .child(rows)
    }

    fn feature_settings(&mut self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .children(
                self.settings
                    .features
                    .clone()
                    .into_iter()
                    .filter(|feature| feature.key != "clipboard")
                    .map(|feature| {
                        let key = feature.key.clone();
                        let switch_key = format!("feature-{}", feature.key);
                        let accessibility = feature.name.clone();
                        self.settings_row(
                            feature.name,
                            match self.locale {
                                Locale::En => "Enable this built-in feature",
                                Locale::Zh => "启用此内置功能",
                            },
                            Switch::new(switch_key)
                                .checked(feature.enabled)
                                .accessibility_label(accessibility)
                                .on_change(cx.listener(move |this, checked, _, cx| {
                                    this.settings.set_feature_enabled(&key, *checked);
                                    if let Some(live) = this.live.clone() {
                                        this.settings.saving = true;
                                        this.settings.error = None;
                                        let receiver = live.change_feature(key.clone(), *checked);
                                        this.poll(receiver, this.stream_generation, cx);
                                    }
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                self.settings_row(
                    match self.locale {
                        Locale::En => "Web tools",
                        Locale::Zh => "Web 工具",
                    },
                    match self.locale {
                        Locale::En => {
                            "Install, review permissions, update, roll back, or remove packages"
                        }
                        Locale::Zh => "安装、审查权限、更新、回滚或移除安装包",
                    },
                    Button::new("manage-web-packages")
                        .secondary()
                        .label(match self.locale {
                            Locale::En => "Manage…",
                            Locale::Zh => "管理…",
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_screen(Screen::WebPackages, window, cx);
                        })),
                ),
            )
    }

    fn language_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let options = [
            (UiLanguage::System, l10n::language_system(self.locale)),
            (UiLanguage::Zh, l10n::language_chinese(self.locale)),
            (UiLanguage::En, l10n::language_english(self.locale)),
        ];
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_SM))
            .child(
                div()
                    .text_size(px(metrics::BODY_SIZE))
                    .text_color(theme::secondary())
                    .child(l10n::language_description(self.locale)),
            )
            .children(options.into_iter().map(|(language, label)| {
                let selected = self.settings_draft.ui_language == language;
                div()
                    .id(SharedString::from(format!("language-{label}")))
                    .aria_label(label.clone())
                    .aria_selected(selected)
                    .h(px(44.))
                    .px(px(metrics::SPACE_MD))
                    .flex()
                    .items_center()
                    .justify_between()
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .when(selected, |element| element.bg(theme::selected()))
                    .when(!selected, |element| {
                        element.hover(|style| style.bg(theme::hover()))
                    })
                    .child(
                        div()
                            .text_size(px(metrics::BODY_SIZE))
                            .text_color(theme::primary())
                            .child(label),
                    )
                    .when(selected, |element| {
                        element.child(
                            div()
                                .text_color(theme::accent())
                                .text_size(px(metrics::HEADING_SIZE))
                                .child("✓"),
                        )
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.settings_draft.ui_language = language;
                        let next = locale_for_language(language);
                        if this.locale != next {
                            this.locale = next;
                            this.model.set_locale(next);
                        }
                        set_input_value(
                            &this.input,
                            this.input.read(cx).value().to_string(),
                            window,
                            cx,
                        );
                        cx.notify();
                    }))
            }))
    }

    fn settings_input_row(
        &self,
        title: impl Into<SharedString>,
        subtitle: impl Into<SharedString>,
        input: &Entity<InputState>,
        id: &'static str,
        placeholder: &'static str,
    ) -> Div {
        self.settings_row(
            title,
            subtitle,
            div().w(px(250.)).child(
                Input::new(input)
                    .id(id)
                    .appearance(true)
                    .text_size(px(metrics::BODY_SIZE))
                    .aria_label(placeholder),
            ),
        )
    }

    fn settings_row(
        &self,
        title: impl Into<SharedString>,
        subtitle: impl Into<SharedString>,
        control: impl IntoElement,
    ) -> Div {
        div()
            .min_h(px(60.))
            .px(px(metrics::SPACE_MD))
            .py(px(metrics::SPACE_SM))
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_LG))
            .rounded(px(metrics::CARD_RADIUS))
            .bg(theme::field())
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(metrics::BODY_SIZE))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::primary())
                            .child(title.into()),
                    )
                    .child(
                        div()
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::secondary())
                            .child(subtitle.into()),
                    ),
            )
            .child(control.into_any_element())
    }
}

impl Render for LauncherView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.screen {
            Screen::Launcher => self.render_launcher(cx),
            Screen::FileSearch => self.render_file_search(cx),
            Screen::Clipboard => self.render_clipboard(cx),
            Screen::ClipboardShelf => self.render_clipboard_shelf(cx),
            Screen::ApplicationHistory => self.render_app_history(cx),
            Screen::Tasks => self.render_tasks(cx),
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
            .bg(theme::glass_surface())
            .border(px(0.7))
            .border_color(theme::border())
            .text_color(theme::primary())
            .font_family(fonts::FONT_UI)
            .font_weight(FontWeight::NORMAL)
            .capture_key_down(
                cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                    if this.handle_preview_key(event, window, cx) {
                        window.prevent_default();
                        cx.stop_propagation();
                        return;
                    }
                    if event.keystroke.modifiers.platform
                        && event.keystroke.key.eq_ignore_ascii_case("y")
                        && this.can_preview_selected_file()
                    {
                        this.request_preview(cx);
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
                    if event.prefer_character_input {
                        return;
                    }
                    this.handle_key(
                        event.keystroke.key.as_str(),
                        event.keystroke.modifiers.platform,
                        event.keystroke.modifiers.modified(),
                        window,
                        cx,
                    );
                }),
            )
            .child(content)
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
            .when(self.screen == Screen::Settings, |element| {
                element.h(px(SETTINGS_HEIGHT))
            })
    }
}

pub fn launcher_window_size() -> gpui_kit::Size<gpui_kit::Pixels> {
    gpui_kit::size(px(LAUNCHER_WIDTH), px(520.))
}

fn locale_for_language(language: UiLanguage) -> Locale {
    match language {
        UiLanguage::En => Locale::En,
        UiLanguage::Zh => Locale::Zh,
        UiLanguage::System => sys_locale::get_locale()
            .and_then(|identifier| {
                identifier
                    .split(['-', '_'])
                    .next()
                    .map(str::to_ascii_lowercase)
            })
            .map(|language| {
                if language == "zh" {
                    Locale::Zh
                } else {
                    Locale::En
                }
            })
            .unwrap_or(Locale::En),
    }
}

fn lines(value: &str) -> Vec<String> {
    unique_trimmed(value.split('\n'))
}

fn excluded_app_ids(value: &str) -> Vec<String> {
    unique_trimmed(value.split(['\n', ',']))
}

fn unique_trimmed<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        let value = value.trim();
        if !value.is_empty() && !result.iter().any(|existing| existing == value) {
            result.push(value.to_owned());
        }
    }
    result
}

fn set_input_value<C: gpui_kit::AppContext>(
    input: &Entity<InputState>,
    value: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut C,
) {
    let value = value.into();
    input.update(cx, |input, cx| input.set_value(value, window, cx));
}

fn icon_sigil(icon: &str) -> &'static str {
    let icon = icon.to_ascii_lowercase();
    if icon.contains("folder") || icon.contains("directory") {
        "▰"
    } else if icon.contains("git") {
        "⌘"
    } else if icon.contains("search") {
        "⌕"
    } else if icon.contains("menu") {
        "≡"
    } else if icon.contains("clipboard") {
        "▤"
    } else if icon.contains("application") || icon.contains("app") {
        "◆"
    } else {
        "•"
    }
}

fn search_source_label(
    state: anywhere_protocol::SearchSourceState,
    locale: Locale,
) -> &'static str {
    match state {
        anywhere_protocol::SearchSourceState::Loading => match locale {
            Locale::En => "loading",
            Locale::Zh => "加载中",
        },
        anywhere_protocol::SearchSourceState::Ready => match locale {
            Locale::En => "ready",
            Locale::Zh => "就绪",
        },
        anywhere_protocol::SearchSourceState::Unavailable => match locale {
            Locale::En => "unavailable",
            Locale::Zh => "不可用",
        },
        anywhere_protocol::SearchSourceState::Partial => match locale {
            Locale::En => "partial",
            Locale::Zh => "部分结果",
        },
    }
}

fn event_handle_matches(view: &LauncherView, handle: &str) -> bool {
    view.web_document
        .as_ref()
        .is_some_and(|document| document.handle == handle)
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

fn action_result_message(result: &anywhere_protocol::ActionResult, locale: Locale) -> String {
    if let Some(error) = &result.error {
        return match locale {
            Locale::En => format!("Action failed: {error}"),
            Locale::Zh => format!("操作失败：{error}"),
        };
    }
    if let Some(warning) = &result.warning {
        return match locale {
            Locale::En => format!("Action completed with a warning: {warning}"),
            Locale::Zh => format!("操作已完成，但出现警告：{warning}"),
        };
    }
    if matches!(
        result.view,
        Some(anywhere_protocol::HostView::WebPlugin { .. })
    ) {
        return match locale {
            Locale::En => "Opening the Web tool…".to_owned(),
            Locale::Zh => "正在打开 Web 工具…".to_owned(),
        };
    }
    if let Some(output) = result
        .output_text
        .as_deref()
        .map(str::trim)
        .filter(|output| !output.is_empty())
    {
        return output.chars().take(240).collect();
    }
    match locale {
        Locale::En => "Action completed".to_owned(),
        Locale::Zh => "操作已完成".to_owned(),
    }
}

fn plain_space_insertion(event: &InputEditsEvent, previous: &str) -> Option<(String, usize)> {
    let (_, after) = event.edits().last()?;
    let after = after.to_string();
    if after.len() != previous.len().checked_add(1)? {
        return None;
    }
    let cursor = after
        .bytes()
        .zip(previous.bytes())
        .position(|(after, previous)| after != previous)
        .unwrap_or(previous.len());
    if after.as_bytes().get(cursor) != Some(&b' ')
        || after.as_bytes().get(cursor + 1..) != previous.as_bytes().get(cursor..)
    {
        return None;
    }
    Some((previous.to_owned(), cursor))
}

enum PreviewReservation {
    Ready(PreviewHost),
    Busy,
    Stale,
}

fn preview_error_message(locale: Locale, error: &anyhow::Error) -> String {
    match locale {
        Locale::En => format!("Preview is unavailable: {error:#}"),
        Locale::Zh => format!("无法预览：{error:#}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anywhere_protocol::{
        ui::{DetailView, KeyValue},
        HostView,
    };
    use gpui_kit::{test::TestWindowExt as _, AnyWindowHandle, TestAppContext, WindowOptions};

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
        }
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
                view.file_search_browsing = false;

                assert!(view.selected_row_for_action().is_none());
                assert!(!view.file_search_selection_active());
                assert!(!view.can_preview_selected_file());
                view.activate_selection(cx);
                view.show_object_actions(cx);
                assert!(view.notice.is_none());
                assert!(!view.model.object_actions_active);

                view.select_file_search(1, cx);
                assert!(view.file_search_browsing);
                assert_eq!(view.model.selected, 0);
                assert_eq!(
                    view.selected_row_for_action().map(|row| row.id.as_str()),
                    Some("file-a")
                );
                assert!(view.can_preview_selected_file());

                view.reset_file_search_browsing(cx);
                assert!(!view.file_search_browsing);
                view.select_file_search(-1, cx);
                assert_eq!(view.model.selected, 2);
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
                view.file_search_browsing = false;
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
                view.file_search_browsing = true;
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
            assert!(view.file_search_browsing);
        });

        cx.update(|cx| {
            view.update(cx, |view, _| view.preview_open = true);
        });
        cx.update_window(window, |_, window, cx| {
            window.press("escape", cx);
        })
        .unwrap();
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
            view.update(cx, |view, _| view.file_search_browsing = true);
        });
        cx.update_window(window, |_, window, cx| {
            window.input("x", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(!view.file_search_browsing);
            assert_eq!(view.model.selected, 0);
            assert_eq!(view.model.query, "guide x");
        });
    }

    #[gpui_kit::test]
    fn settings_edits_are_dirty_until_saved_or_discarded(cx: &mut TestAppContext) {
        let (window, view) = open_view(cx);
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            window.click("settings", cx);
            assert!(window.try_find("settings-surface").is_some());
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
            window.click("close-settings", cx);
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
                view.plugin_detail
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
                view.unsupported_tool.as_ref().map(|(id, _)| id.as_str()),
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
                view.web_document = Some(WebDocument {
                    handle: "handle-a".to_owned(),
                    origin: "anywhere-plugin://0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.plugin.local".to_owned(),
                    entry: "index.html".to_owned(),
                    title: "Tool".to_owned(),
                });
                view.web_ready = true;
                view.web_error = Some("old error".to_owned());
                view.web_state_handle = Some("state".to_owned());
                view.web_state_version = 9;
                assert!(event_handle_matches(view, "handle-a"));
                assert!(!event_handle_matches(view, "handle-b"));
                view.close_web_surface(cx);
                assert!(view.web_document.is_none());
                assert!(!view.web_ready);
                assert!(view.web_error.is_none());
                assert!(view.web_state_handle.is_none());
                assert_eq!(view.web_state_version, 0);
                assert!(!event_handle_matches(view, "handle-a"));
            });
        });
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
}
