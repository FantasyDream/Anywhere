use gpui_kit::{
    point, px, size, ActivationPolicy, AnyWindowHandle, App as GpuiApp, AppContext as _, Bounds,
    Context, Entity, Render, Subscription, Window, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions,
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use crate::{
    host::{HotKeyAction, HotKeyController, LiveLauncher},
    launcher::{
        launcher_window_size, locale_for_language, ClipboardShelfView, LauncherEvent, LauncherView,
        ToolWindowView,
    },
    ui::{
        design_tokens::{fonts, metrics},
        theme,
    },
    BUNDLE_ID, CLIPBOARD_SHELF_TITLE, PRODUCT_NAME, TASKS_WINDOW_TITLE,
};

struct ProductShell {
    launcher: Entity<LauncherView>,
    live: Option<Arc<LiveLauncher>>,
    shelf_window: Option<AnyWindowHandle>,
    tasks_window: Option<AnyWindowHandle>,
    tasks_launcher: Option<Entity<LauncherView>>,
    _status_item: Option<crate::platform::StatusItemHost>,
    _launcher_events: Option<Subscription>,
    _window_closed: Option<Subscription>,
    #[cfg(windows)]
    _window_activation: Option<Subscription>,
}

enum RuntimeStartup {
    Ready(Arc<LiveLauncher>),
    Failed(String),
}

impl ProductShell {
    fn refresh_appearance(&self, cx: &mut Context<Self>) {
        crate::platform::refresh_all_window_materials();
        cx.set_reduce_motion(crate::platform::uses_reduce_motion());
        install_component_theme(cx);
        self.launcher
            .update(cx, |launcher, cx| launcher.platform_appearance_changed(cx));
        for (handle, tool) in self
            .launcher
            .read(cx)
            .tool_window_handles()
            .collect::<Vec<_>>()
        {
            let _ = handle.update(cx, |_, _, cx| {
                let _ = tool.update(cx, |tool, cx| tool.platform_appearance_changed(cx));
            });
        }
        cx.refresh_windows();
    }

    fn hide_launcher(&mut self, cx: &mut Context<Self>) {
        self.launcher
            .update(cx, |launcher, cx| launcher.prepare_hide(cx));
        crate::platform::hide_main_window();
    }

    fn open_clipboard_shelf(&mut self, cx: &mut Context<Self>) {
        crate::platform::remember_clipboard_shelf_destination();
        if self.shelf_window.is_some() {
            if crate::platform::activate_window_after_update(CLIPBOARD_SHELF_TITLE) {
                return;
            }
            self.shelf_window = None;
        }
        let Some(live) = self.live.clone() else {
            return;
        };
        let locale = self.launcher.read(cx).locale();
        let opened = gpui_kit::open_window(shelf_window_options(cx), cx, |window, cx| {
            crate::platform::bind_window_identity(window, CLIPBOARD_SHELF_TITLE);
            let launcher =
                cx.new(|cx| LauncherView::clipboard_shelf(window, cx, live.clone(), locale));
            cx.new(|cx| ClipboardShelfView::new(launcher, window, cx))
        });
        match opened {
            Ok((window, shell)) => {
                crate::platform::configure_clipboard_shelf_window_after_update(
                    CLIPBOARD_SHELF_TITLE,
                );
                crate::platform::activate_window_after_update(CLIPBOARD_SHELF_TITLE);
                let launcher = shell.read(cx).launcher.clone();
                let _ = window.update(cx, |_, window, _| {
                    window.on_next_frame(move |_, cx| {
                        launcher.update(cx, |launcher, cx| launcher.initialize_clipboard_shelf(cx));
                    });
                });
                self.shelf_window = Some(window);
            }
            Err(error) => tracing::error!(%error, "clipboard_shelf_open_failed"),
        }
    }

    fn open_tasks_window(
        &mut self,
        selected_task_id: Option<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.tasks_window.is_some() {
            if let (Some(task_id), Some(launcher)) =
                (selected_task_id.clone(), self.tasks_launcher.clone())
            {
                launcher.update(cx, |launcher, cx| launcher.select_execution(task_id, cx));
            }
            if crate::platform::window_exists(TASKS_WINDOW_TITLE) {
                return true;
            }
            self.tasks_window = None;
            self.tasks_launcher = None;
        }
        let Some(live) = self.live.clone() else {
            return false;
        };
        let locale = self.launcher.read(cx).locale();
        let main_launcher = self.launcher.downgrade();
        let opened = gpui_kit::open_window(tasks_window_options(cx), cx, |window, cx| {
            crate::platform::bind_window_identity(window, TASKS_WINDOW_TITLE);
            let launcher =
                cx.new(|cx| LauncherView::tasks_window(window, cx, live.clone(), locale));
            cx.new(|cx| ToolWindowView::new(launcher, main_launcher, window, cx))
        });
        match opened {
            Ok((window, shell)) => {
                crate::platform::configure_tasks_window_after_update(TASKS_WINDOW_TITLE);
                let launcher = shell.read(cx).launcher.clone();
                self.tasks_launcher = Some(launcher.clone());
                let _ = window.update(cx, move |_, window, _| {
                    window.on_next_frame(move |_, cx| {
                        launcher.update(cx, |launcher, cx| {
                            launcher.initialize_tasks_window(selected_task_id, cx)
                        });
                    });
                });
                self.tasks_window = Some(window);
                true
            }
            Err(error) => {
                tracing::error!(%error, "tasks_window_open_failed");
                false
            }
        }
    }
}

impl Render for ProductShell {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        let launcher = self.launcher.downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let should_hide = launcher
                .update(cx, |launcher, cx| {
                    launcher.request_main_window_close(window, cx)
                })
                .unwrap_or(true);
            if should_hide {
                crate::platform::hide_main_window();
            }
            false
        });
        self.launcher.clone()
    }
}

fn configure_component_theme(theme: &mut gpui_kit::component::Theme) {
    theme.font_family = fonts::FONT_UI.into();
    theme.font_size = px(metrics::BODY_SIZE);
    theme.mono_font_family = fonts::FONT_MONO.into();
    theme.mono_font_size = px(metrics::PREVIEW_CODE_SIZE);
    theme.radius = px(metrics::CONTROL_RADIUS);
    theme.window_radius = px(crate::ui::LAUNCHER_CORNER_RADIUS);
    // GPUI Kit's Root paints this token underneath the product view. A clear
    // LauncherView alone cannot expose native glass through an opaque Root.
    theme.colors.background = theme::transparent();
    theme.tokens.background = theme::transparent().into();
    theme.colors.foreground = theme::primary();
    theme.colors.secondary_foreground = theme::secondary();
    theme.colors.muted_foreground = theme::muted();
    theme.colors.border = theme::border();
    theme.colors.popover = theme::action_menu_surface();
    theme.colors.popover_foreground = theme::primary();
    theme.colors.input = theme::control_border();
    theme.colors.ring = theme::selection_outline();
    theme.colors.caret = theme::accent();
    theme.colors.selection = theme::text_selection();
    theme.tokens.input = theme::control_border().into();
    theme.tokens.selection = theme::text_selection().into();
    theme.colors.primary = theme::accent_fill();
    theme.colors.primary_foreground = theme::on_dark_primary();
    theme.colors.primary_hover = theme::accent_fill().opacity(0.88);
    theme.colors.primary_active = theme::accent_fill().opacity(0.76);
    theme.tokens.primary = theme::accent_fill().into();
    theme.tokens.primary_hover = theme::accent_fill().opacity(0.88).into();
    theme.tokens.primary_active = theme::accent_fill().opacity(0.76).into();
    theme.colors.button_primary = theme::accent_fill();
    theme.colors.button_primary_foreground = theme::on_dark_primary();
    theme.colors.button_primary_hover = theme::accent_fill().opacity(0.88);
    theme.colors.button_primary_active = theme::accent_fill().opacity(0.76);
    theme.tokens.button_primary = theme::accent_fill().into();
    theme.tokens.button_primary_hover = theme::accent_fill().opacity(0.88).into();
    theme.tokens.button_primary_active = theme::accent_fill().opacity(0.76).into();
    theme.colors.button_danger = theme::danger().opacity(0.14);
    theme.colors.button_danger_foreground = theme::danger();
    theme.colors.button_danger_hover = theme::danger().opacity(0.22);
    theme.colors.button_danger_active = theme::danger().opacity(0.30);
    theme.tokens.button_danger = theme::danger().opacity(0.14).into();
    theme.tokens.button_danger_hover = theme::danger().opacity(0.22).into();
    theme.tokens.button_danger_active = theme::danger().opacity(0.30).into();
    theme.colors.button_secondary = theme::transparent();
    theme.colors.button_secondary_foreground = theme::primary();
    theme.colors.button_secondary_hover = theme::hover();
    theme.colors.button_secondary_active = theme::button_selection();
    theme.tokens.button_secondary = theme::transparent().into();
    theme.tokens.button_secondary_hover = theme::hover().into();
    theme.tokens.button_secondary_active = theme::button_selection().into();
    theme.colors.button = theme::transparent();
    theme.colors.button_foreground = theme::primary();
    theme.colors.button_hover = theme::hover();
    theme.colors.button_active = theme::button_selection();
    theme.tokens.button = theme::transparent().into();
    theme.tokens.button_hover = theme::hover().into();
    theme.tokens.button_active = theme::button_selection().into();
    theme.colors.list = theme::transparent();
    theme.colors.list_hover = theme::hover();
    theme.colors.list_active = theme::selected();
    theme.colors.list_active_border = theme::selection_outline();
    theme.tokens.list = theme::transparent().into();
    theme.tokens.list_hover = theme::hover().into();
    theme.tokens.list_active = theme::selected().into();
    theme.colors.switch = theme::control_surface();
    theme.colors.switch_thumb = theme::on_dark_primary();
    theme.colors.scrollbar_thumb = theme::control_border();
    theme.colors.scrollbar_thumb_hover = theme::secondary();
    theme.tokens.switch = theme::control_surface().into();
    theme.tokens.switch_thumb = theme::on_dark_primary().into();
    theme.tokens.scrollbar_thumb = theme::control_border().into();
    theme.tokens.scrollbar_thumb_hover = theme::secondary().into();
}

pub(crate) fn install_component_theme(cx: &mut GpuiApp) {
    gpui_kit::component::Theme::update(cx, |theme| {
        theme.mode = if theme::surface_is_light() {
            gpui_kit::component::ThemeMode::Light
        } else {
            gpui_kit::component::ThemeMode::Dark
        };
    });
    gpui_kit::component::Theme::update(cx, configure_component_theme);
}

pub fn run(cx: &mut GpuiApp) {
    gpui_kit::init(cx);
    #[cfg(windows)]
    crate::platform::windows::init(cx);
    crate::launcher::bind_keys(cx);
    crate::platform::prepare_window_material();
    install_component_theme(cx);
    cx.set_reduce_motion(crate::platform::uses_reduce_motion());
    cx.set_activation_policy(app_activation_policy());
    let preview = std::env::var_os("ANYWHERE_PREVIEW").is_some();
    let startup_receiver = (!preview).then(start_live_runtime);
    let (status_actions, status_receiver) =
        tokio::sync::mpsc::channel::<crate::platform::StatusItemAction>(64);
    let options = main_window_options(cx);
    let opened = gpui_kit::open_window(options, cx, |window, cx| {
        crate::platform::bind_window_identity(window, PRODUCT_NAME);
        let launcher = cx.new(|cx| {
            if preview {
                LauncherView::new(window, cx)
            } else {
                LauncherView::starting(window, cx, crate::l10n::Locale::En)
            }
        });
        cx.new(|_cx| ProductShell {
            launcher,
            live: None,
            shelf_window: None,
            tasks_window: None,
            tasks_launcher: None,
            _status_item: None,
            _launcher_events: None,
            _window_closed: None,
            #[cfg(windows)]
            _window_activation: Some(_cx.observe_window_activation(window, |_, window, _| {
                crate::platform::windows::report_activation(window.is_window_active());
            })),
        })
    });
    let (window_handle, shell) = match opened {
        Ok(opened) => opened,
        Err(error) => {
            tracing::error!(%error, "launcher_window_open_failed");
            return;
        }
    };
    crate::host::defer_native_window_update(
        shell.downgrade(),
        window_handle,
        cx,
        || crate::platform::configure_main_window(PRODUCT_NAME),
        |_, _, _, configured| {
            if !configured {
                tracing::warn!("launcher_window_configuration_failed");
            }
        },
    );
    if preview {
        tracing::warn!("ui_preview_uses_fixture_data_without_live_runtime");
        crate::platform::show_main_window_after_launch();
    }
    let launcher_window = window_handle;
    install_status_and_shell_events(launcher_window, &shell, status_actions, cx);
    if let Some(startup_receiver) = startup_receiver {
        spawn_runtime_startup(startup_receiver, window_handle, shell.clone(), cx);
    }
    spawn_status_action_loop(status_receiver, window_handle, shell.clone(), cx);
    spawn_platform_event_loop(window_handle, shell, preview, cx);
}

fn install_status_and_shell_events(
    launcher_window: AnyWindowHandle,
    shell: &gpui_kit::Entity<ProductShell>,
    status_actions: tokio::sync::mpsc::Sender<crate::platform::StatusItemAction>,
    cx: &mut GpuiApp,
) {
    let locale = shell.read(cx).launcher.read(cx).locale();
    let preferences =
        crate::host::registered_preferences(&crate::host::HostPreferences::default(), None);
    tracing::debug!(
        locale = ?locale,
        current_shortcut = %preferences.current_app_shortcut,
        universal_shortcut = %preferences.universal_shortcut,
        "status_menu_labels_loading"
    );
    let status_item = crate::platform::install_status_item(
        status_actions,
        &crate::platform::StatusMenuLabels::new(locale, &preferences),
    );
    if status_item.is_none() {
        tracing::warn!("status_item_unavailable");
    }
    #[cfg(windows)]
    if !crate::platform::windows::has_tray_icon() {
        crate::platform::show_main_window();
    }
    shell.update(cx, |shell, cx| {
        shell._status_item = status_item;
        let weak = cx.weak_entity();
        shell._window_closed = Some(cx.on_window_closed(move |cx, id| {
            let _ = weak.update(cx, |shell, _| {
                if shell
                    .shelf_window
                    .is_some_and(|handle| handle.window_id() == id)
                {
                    shell.shelf_window = None;
                }
                if shell
                    .tasks_window
                    .is_some_and(|handle| handle.window_id() == id)
                {
                    shell.tasks_window = None;
                    shell.tasks_launcher = None;
                }
            });
        }));
        shell._launcher_events = Some(cx.subscribe(
            &shell.launcher,
            move |shell: &mut ProductShell, _, event: &LauncherEvent, cx| {
                match event {
                    LauncherEvent::RestoreLauncher => shell
                        .launcher
                        .update(cx, |view, cx| view.restore_from_tool(cx)),
                    LauncherEvent::OpenClipboardShelf => {
                        cx.spawn(async move |shell, cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(0))
                                .await;
                            let _ = shell.update(cx, |shell, cx| {
                                shell.open_clipboard_shelf(cx);
                            });
                        })
                        .detach();
                    }
                    LauncherEvent::OpenTasks(task_id) => {
                        let task_id = task_id.clone();
                        cx.spawn(async move |shell, cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(0))
                                .await;
                            let opened = shell
                                .update(cx, |shell, cx| shell.open_tasks_window(Some(task_id), cx))
                                .unwrap_or(false);
                            if opened {
                                crate::platform::activate_window_after_update(TASKS_WINDOW_TITLE);
                            }
                        })
                        .detach();
                    }
                    LauncherEvent::WebViewEscapeRequested => {
                        let _ = launcher_window.update(cx, |_, window, cx| {
                            let _ = shell.launcher.update(cx, |launcher, cx| {
                                launcher.handle_web_view_escape_from_platform(window, cx)
                            });
                        });
                    }
                    LauncherEvent::QuitRequested => cx.quit(),
                    LauncherEvent::PreferencesApplied {
                        locale,
                        preferences,
                    } => {
                        if crate::platform::set_application_appearance(
                            preferences.ui_theme.appearance(),
                        ) {
                            shell.refresh_appearance(cx);
                        }
                        for handle in [shell.shelf_window, shell.tasks_window]
                            .into_iter()
                            .flatten()
                        {
                            let locale = *locale;
                            let _ = handle.update(cx, |view, window, cx| {
                                if let Ok(shelf) = view.clone().downcast::<ClipboardShelfView>() {
                                    shelf.read(cx).launcher.clone().update(cx, |view, cx| {
                                        view.update_tool_locale(locale, window, cx)
                                    });
                                } else if let Ok(tasks) = view.downcast::<ToolWindowView>() {
                                    tasks.read(cx).launcher.clone().update(cx, |view, cx| {
                                        view.update_tool_locale(locale, window, cx)
                                    });
                                }
                                window.refresh();
                            });
                        }
                        for (handle, tool) in shell
                            .launcher
                            .read(cx)
                            .tool_window_handles()
                            .collect::<Vec<_>>()
                        {
                            let locale = *locale;
                            let _ = handle.update(cx, |_, window, cx| {
                                let _ = tool.update(cx, |tool, cx| {
                                    tool.update_tool_locale(locale, window, cx);
                                    tool.platform_appearance_changed(cx);
                                });
                                window.refresh();
                            });
                        }
                        if let Some(status_item) = shell._status_item.as_ref() {
                            status_item.update_labels(&crate::platform::StatusMenuLabels::new(
                                *locale,
                                preferences,
                            ));
                        }
                    }
                }
            },
        ));
    });
}

fn spawn_runtime_startup(
    mut startup_receiver: tokio::sync::mpsc::Receiver<RuntimeStartup>,
    startup_window: AnyWindowHandle,
    startup_shell: gpui_kit::Entity<ProductShell>,
    cx: &mut GpuiApp,
) {
    cx.spawn(async move |cx| {
        let Some(startup) = startup_receiver.recv().await else {
            let error = "Anywhere runtime startup worker stopped unexpectedly".to_owned();
            tracing::error!("{error}");
            let _ = startup_window.update(cx, |_, _, cx| {
                startup_shell.update(cx, |shell, cx| {
                    shell.launcher.update(cx, |launcher, cx| {
                        launcher.set_startup_error(error.clone(), cx)
                    });
                });
            });
            return;
        };
        match startup {
            RuntimeStartup::Ready(live) => {
                let preferences = match live.preferences() {
                    Ok(preferences) => preferences,
                    Err(error) => {
                        tracing::error!(%error, "startup_preferences_unavailable");
                        let message = format!("Anywhere preferences are unavailable: {error:#}");
                        let _ = startup_window.update(cx, |_, _, cx| {
                            startup_shell.update(cx, |shell, cx| {
                                shell.launcher.update(cx, |launcher, cx| {
                                    launcher.set_startup_error(message.clone(), cx)
                                });
                            });
                        });
                        return;
                    }
                };
                let (hotkeys, hotkey_error) = match HotKeyController::new(&preferences) {
                    Ok(controller) => {
                        let error = controller.registration_error().map(str::to_owned);
                        (Some(controller), error)
                    }
                    Err(error) => {
                        tracing::error!(%error, "global_hotkey_manager_unavailable");
                        (None, Some(format!("{error:#}")))
                    }
                };
                let locale = locale_for_language(preferences.ui_language);
                let effective = crate::host::registered_preferences(&preferences, hotkeys.as_ref());
                let labels = crate::platform::StatusMenuLabels::new(locale, &effective);
                let attached = startup_window
                    .update(cx, |_, window, cx| {
                        startup_shell.update(cx, |shell, cx| {
                            shell.live = Some(live.clone());
                            shell.launcher.update(cx, |launcher, cx| {
                                launcher.attach_runtime(live, hotkeys, hotkey_error, window, cx);
                                #[cfg(windows)]
                                if window.is_window_active() {
                                    launcher.activate_from_menu(
                                        anywhere_protocol::InputMode::Search,
                                        window,
                                        cx,
                                    );
                                }
                            });
                            if crate::platform::set_application_appearance(
                                preferences.ui_theme.appearance(),
                            ) {
                                shell.refresh_appearance(cx);
                            }
                            if let Some(status_item) = shell._status_item.as_ref() {
                                status_item.update_labels(&labels);
                            }
                        })
                    })
                    .is_ok();
                if attached {
                    install_global_hotkeys(cx, startup_window, &startup_shell);
                }
            }
            RuntimeStartup::Failed(error) => {
                tracing::error!(%error, "shared_runtime_unavailable");
                let message = format!("Anywhere could not open its local runtime: {error}");
                let _ = startup_window.update(cx, |_, _, cx| {
                    startup_shell.update(cx, |shell, cx| {
                        shell.launcher.update(cx, |launcher, cx| {
                            launcher.set_startup_error(message.clone(), cx)
                        });
                    });
                });
            }
        }
    })
    .detach();
}

fn spawn_status_action_loop(
    mut status_receiver: tokio::sync::mpsc::Receiver<crate::platform::StatusItemAction>,
    status_window: AnyWindowHandle,
    status_shell: gpui_kit::Entity<ProductShell>,
    cx: &mut GpuiApp,
) {
    cx.spawn(async move |cx| {
        while let Some(action) = status_receiver.recv().await {
            match action {
                crate::platform::StatusItemAction::ShowCurrentApp
                | crate::platform::StatusItemAction::ShowUniversal => {
                    let mode = match action {
                        crate::platform::StatusItemAction::ShowCurrentApp => {
                            anywhere_protocol::InputMode::Environment
                        }
                        crate::platform::StatusItemAction::ShowUniversal => {
                            anywhere_protocol::InputMode::Search
                        }
                        _ => unreachable!(),
                    };
                    let should_show = status_window
                        .update(cx, |_, window, cx| {
                            status_shell.update(cx, |shell, cx| {
                                shell.launcher.update(cx, |launcher, cx| {
                                    launcher.activate_from_menu(mode, window, cx)
                                })
                            })
                        })
                        .unwrap_or(false);
                    if !should_show {
                        continue;
                    }
                    // The status callback still runs while AppKit owns the menu
                    // tracking loop. Let that menu close before activating an
                    // Accessory app, or macOS rejects the activation request.
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(80))
                        .await;
                    if !crate::platform::configure_main_window(PRODUCT_NAME) {
                        tracing::warn!("launcher_window_configuration_failed");
                    }
                    crate::platform::show_main_window();
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(40))
                        .await;
                    let _ = status_window.update(cx, |_, window, cx| {
                        status_shell.update(cx, |shell, cx| {
                            shell.launcher.update(cx, |launcher, cx| {
                                launcher.refocus_active_editor(window, cx)
                            });
                        });
                    });
                }
                crate::platform::StatusItemAction::OpenTasks => {
                    let opened =
                        status_shell.update(cx, |shell, cx| shell.open_tasks_window(None, cx));
                    if !opened {
                        continue;
                    }
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(80))
                        .await;
                    if !crate::platform::activate_window(TASKS_WINDOW_TITLE) {
                        tracing::warn!("tasks_window_activation_failed");
                    }
                }
                crate::platform::StatusItemAction::OpenSettings => {
                    let opened = status_window
                        .update(cx, |_, window, cx| {
                            status_shell.update(cx, |shell, cx| {
                                shell
                                    .launcher
                                    .update(cx, |launcher, cx| launcher.open_settings(window, cx))
                            })
                        })
                        .unwrap_or(false);
                    if !opened {
                        continue;
                    }
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(80))
                        .await;
                    if !crate::platform::configure_settings_window(PRODUCT_NAME) {
                        tracing::warn!("settings_window_configuration_failed");
                    }
                    crate::platform::show_main_window();
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(40))
                        .await;
                    let _ = status_window.update(cx, |_, window, cx| {
                        status_shell.update(cx, |shell, cx| {
                            shell.launcher.update(cx, |launcher, cx| {
                                launcher.focus_settings_navigation(window, cx)
                            });
                        });
                    });
                }
                crate::platform::StatusItemAction::Quit => {
                    let _ = status_window.update(cx, |_, window, cx| {
                        status_shell.update(cx, |shell, cx| {
                            shell.launcher.update(cx, |launcher, cx| {
                                launcher.request_application_quit(window, cx)
                            });
                        });
                    });
                }
            }
        }
    })
    .detach();
}

fn spawn_platform_event_loop(
    focus_window: AnyWindowHandle,
    focus_shell: gpui_kit::Entity<ProductShell>,
    preview: bool,
    cx: &mut GpuiApp,
) {
    let mut platform_events = crate::platform::install_platform_observers();
    let focus_epoch = Arc::new(AtomicU64::new(0));
    cx.spawn(async move |cx| {
        while let Some(event) = platform_events.recv().await {
            match event {
                crate::platform::PlatformEvent::LauncherLostKey
                | crate::platform::PlatformEvent::ApplicationLostActive => {
                    tracing::debug!(event = ?event, "launcher_focus_loss_grace_started");
                    let grace = focus_epoch.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
                    let window = focus_window;
                    let shell = focus_shell.clone();
                    let epoch = focus_epoch.clone();
                    cx.spawn(async move |cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(220))
                            .await;
                        if epoch.load(Ordering::Acquire) != grace {
                            return;
                        }
                        let dismissible = window
                            .update(cx, |_, _, cx| {
                                shell.read(cx).launcher.read(cx).dismiss_on_focus_loss()
                            })
                            .unwrap_or(false);
                        tracing::debug!(
                            dismissible,
                            should_dismiss =
                                crate::platform::launcher_should_dismiss_for_focus_loss(),
                            "launcher_focus_loss_grace_elapsed"
                        );
                        if !preview
                            && dismissible
                            && crate::platform::launcher_should_dismiss_for_focus_loss()
                        {
                            let _ = window.update(cx, |_, _, cx| {
                                shell.update(cx, |shell, cx| shell.hide_launcher(cx));
                            });
                        }
                    })
                    .detach();
                }
                crate::platform::PlatformEvent::LauncherBecameKey
                | crate::platform::PlatformEvent::ApplicationBecameActive => {
                    tracing::debug!(event = ?event, "launcher_focus_gained");
                    focus_epoch.fetch_add(1, Ordering::AcqRel);
                }
                crate::platform::PlatformEvent::SettingsEscape => {
                    let handled = focus_window
                        .update(cx, |_, window, cx| {
                            focus_shell.update(cx, |shell, cx| {
                                shell.launcher.update(cx, |launcher, cx| {
                                    launcher.handle_settings_escape_from_platform(window, cx)
                                })
                            })
                        })
                        .unwrap_or(false);
                    tracing::debug!(handled, "settings_escape_dispatched");
                }
                crate::platform::PlatformEvent::TasksKey { key, repeated } => {
                    let key_name = match key {
                        crate::platform::TasksNavigationKey::Escape => "escape",
                        crate::platform::TasksNavigationKey::Up => "up",
                        crate::platform::TasksNavigationKey::Down => "down",
                        crate::platform::TasksNavigationKey::Enter => "enter",
                    };
                    let tasks = focus_shell.update(cx, |shell, _| {
                        shell.tasks_window.zip(shell.tasks_launcher.clone())
                    });
                    let handled = tasks.is_some_and(|(window, launcher)| {
                        window
                            .update(cx, |_, window, cx| {
                                launcher.update(cx, |launcher, cx| {
                                    launcher
                                        .handle_tasks_navigation_key(key_name, repeated, window, cx)
                                })
                            })
                            .is_ok()
                    });
                    tracing::debug!(?key, handled, "tasks_navigation_key_dispatched");
                }
                crate::platform::PlatformEvent::AccessibilityDisplayChanged => {
                    focus_shell.update(cx, |shell, cx| shell.refresh_appearance(cx));
                }
            }
        }
    })
    .detach();
}

fn start_live_runtime() -> tokio::sync::mpsc::Receiver<RuntimeStartup> {
    start_runtime_worker(|| match LiveLauncher::open() {
        Ok(live) => RuntimeStartup::Ready(Arc::new(live)),
        Err(error) => RuntimeStartup::Failed(format!("{error:#}")),
    })
}

fn start_runtime_worker(
    operation: impl FnOnce() -> RuntimeStartup + Send + 'static,
) -> tokio::sync::mpsc::Receiver<RuntimeStartup> {
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    let worker_sender = sender.clone();
    let spawned = std::thread::Builder::new()
        .name("anywhere-startup".to_owned())
        .spawn(move || {
            let _ = worker_sender.blocking_send(operation());
        });
    if let Err(error) = spawned {
        let _ = sender.try_send(RuntimeStartup::Failed(format!(
            "could not create the runtime startup worker: {error}"
        )));
    }
    receiver
}

fn install_global_hotkeys(
    cx: &mut gpui_kit::AsyncApp,
    window_handle: AnyWindowHandle,
    shell: &Entity<ProductShell>,
) {
    let (event_sender, mut event_receiver) = tokio::sync::mpsc::channel(64);
    global_hotkey::GlobalHotKeyEvent::set_event_handler(Some(move |event| {
        if event_sender.try_send(event).is_err() {
            tracing::warn!("global_hotkey_event_queue_full");
        }
    }));
    let hotkey_shell = shell.clone();
    cx.spawn(async move |cx| {
        while let Some(event) = event_receiver.recv().await {
            tracing::debug!(
                hotkey_event_id = event.id(),
                hotkey_event_state = ?event.state(),
                "global_hotkey_event_received"
            );
            // Read the current controller so a retry can recover even if the
            // native manager could not be created during startup.
            let action = window_handle
                .update(cx, |_, _, cx| {
                    hotkey_shell
                        .read(cx)
                        .launcher
                        .read(cx)
                        .global_hotkey_action(event)
                })
                .ok()
                .flatten();
            let Some(action) = action else {
                continue;
            };
            let hotkey_started = std::time::Instant::now();
            let should_show = window_handle
                .update(cx, |_, window, cx| {
                    hotkey_shell.update(cx, |shell, cx| {
                        shell.launcher.update(cx, |launcher, cx| match action {
                            HotKeyAction::CurrentApp => launcher.activate_from_hotkey(
                                anywhere_protocol::InputMode::Environment,
                                window,
                                cx,
                            ),
                            HotKeyAction::Universal => launcher.activate_from_hotkey(
                                anywhere_protocol::InputMode::Search,
                                window,
                                cx,
                            ),
                            HotKeyAction::ExecuteAction(binding) => {
                                launcher.activate_action_shortcut(binding, window, cx)
                            }
                        })
                    })
                })
                .unwrap_or(false);
            if !should_show {
                continue;
            }
            if !crate::platform::configure_main_window(PRODUCT_NAME) {
                tracing::warn!("launcher_window_configuration_failed");
            }
            crate::platform::show_main_window();
            cx.background_executor()
                .timer(std::time::Duration::from_millis(40))
                .await;
            let _ = window_handle.update(cx, |_, window, cx| {
                hotkey_shell.update(cx, |shell, cx| {
                    shell.launcher.update(cx, |launcher, cx| {
                        launcher.refocus_active_editor(window, cx)
                    });
                });
            });
            tracing::debug!(
                hotkey_to_visible_ms = hotkey_started.elapsed().as_secs_f64() * 1000.0,
                "hotkey_to_visible_ms"
            );
        }
    })
    .detach();
}

fn main_window_options(cx: &mut GpuiApp) -> WindowOptions {
    let window_size = launcher_window_size();
    #[cfg(windows)]
    let show = !crate::platform::windows::background_launch()
        || std::env::var_os("ANYWHERE_PREVIEW").is_some();
    #[cfg(not(windows))]
    let show = false;
    WindowOptions {
        window_bounds: Some(launcher_window_bounds(window_size, cx)),
        titlebar: Some(gpui_kit::TitlebarOptions {
            title: Some(PRODUCT_NAME.into()),
            appears_transparent: !cfg!(windows),
            traffic_light_position: None,
        }),
        focus: show,
        show,
        // PopUp creates the native macOS NSPanel with the nonactivating style
        // from the outset. The launcher overrides its level to floating in the
        // platform bridge after launch.
        kind: if cfg!(windows) {
            WindowKind::Normal
        } else {
            WindowKind::PopUp
        },
        is_resizable: false,
        is_minimizable: cfg!(windows),
        window_background: product_window_background(),
        app_id: Some(BUNDLE_ID.to_owned()),
        ..Default::default()
    }
}

fn launcher_window_bounds(
    window_size: gpui_kit::Size<gpui_kit::Pixels>,
    cx: &GpuiApp,
) -> WindowBounds {
    let window_size = constrained_window_size(window_size, cx);
    let Some(display) = cx.primary_display() else {
        return WindowBounds::centered(window_size, cx);
    };
    let visible = display.visible_bounds();
    // gpui-fast-macos 0.1.14 treats WindowBounds Y as the window's bottom
    // edge when creating the native frame. Keep a constant 20% gap below the
    // usable screen top so later resizes do not move that top edge.
    let max_x = visible.origin.x + (visible.size.width - window_size.width).max(px(0.));
    let origin = point(
        (visible.origin.x + (visible.size.width - window_size.width) * 0.5)
            .clamp(visible.origin.x, max_x),
        (visible.origin.y + visible.size.height - visible.size.height * 0.2 - window_size.height)
            .max(visible.origin.y),
    );
    WindowBounds::Windowed(Bounds::new(origin, window_size))
}

fn constrained_window_size(
    requested: gpui_kit::Size<gpui_kit::Pixels>,
    cx: &GpuiApp,
) -> gpui_kit::Size<gpui_kit::Pixels> {
    let Some(display) = cx.primary_display() else {
        return requested;
    };
    let visible = display.visible_bounds();
    let (width, height) = crate::platform::constrained_window_extent(
        visible.size.width.as_f32(),
        visible.size.height.as_f32(),
        requested.width.as_f32(),
        requested.height.as_f32(),
        metrics::SPACE_LG,
    );
    size(px(width), px(height))
}

fn app_activation_policy() -> ActivationPolicy {
    ActivationPolicy::Accessory
}

fn product_window_background() -> WindowBackgroundAppearance {
    if cfg!(windows) {
        WindowBackgroundAppearance::Opaque
    } else {
        WindowBackgroundAppearance::Transparent
    }
}

fn shelf_window_options(cx: &mut GpuiApp) -> WindowOptions {
    let requested = size(px(480.), px(520.));
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(
            constrained_window_size(requested, cx),
            cx,
        )),
        titlebar: Some(gpui_kit::TitlebarOptions {
            title: Some(CLIPBOARD_SHELF_TITLE.into()),
            appears_transparent: !cfg!(windows),
            traffic_light_position: Some(gpui_kit::point(px(14.), px(16.))),
        }),
        focus: true,
        show: true,
        kind: WindowKind::Normal,
        is_resizable: true,
        is_minimizable: true,
        window_min_size: Some(size(px(360.), px(360.))),
        window_background: product_window_background(),
        app_id: Some(BUNDLE_ID.to_owned()),
        ..Default::default()
    }
}

pub(crate) fn tool_window_options(title: String, cx: &mut GpuiApp) -> WindowOptions {
    let mut options = tasks_window_options(cx);
    if let Some(titlebar) = options.titlebar.as_mut() {
        titlebar.title = Some(title.into());
    }
    options.window_min_size = Some(size(
        px(crate::ui::LAUNCHER_WIDTH),
        px(crate::ui::TOOL_WINDOW_MIN_HEIGHT),
    ));
    options
}

fn tasks_window_options(cx: &mut GpuiApp) -> WindowOptions {
    let requested = size(
        px(crate::ui::TASKS_WINDOW_WIDTH),
        px(crate::ui::TASKS_WINDOW_HEIGHT),
    );
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(
            constrained_window_size(requested, cx),
            cx,
        )),
        titlebar: Some(gpui_kit::TitlebarOptions {
            title: Some(TASKS_WINDOW_TITLE.into()),
            appears_transparent: !cfg!(windows),
            traffic_light_position: Some(gpui_kit::point(px(14.), px(16.))),
        }),
        focus: true,
        show: true,
        kind: WindowKind::Normal,
        is_resizable: true,
        is_minimizable: true,
        window_min_size: Some(size(px(760.), px(420.))),
        window_background: product_window_background(),
        app_id: Some(BUNDLE_ID.to_owned()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{test::TestWindowExt as _, TestAppContext, VisualTestContext};
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };

    #[gpui_kit::test]
    #[cfg(not(windows))]
    fn main_window_starts_hidden_as_a_floating_spotlight_surface(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        assert!(matches!(
            app_activation_policy(),
            ActivationPolicy::Accessory
        ));
        let options = cx.update(main_window_options);
        assert!(!options.show, "launch must not reveal a foreground window");
        assert!(!options.focus, "launch must not steal keyboard focus");
        assert!(!options.is_resizable);
        assert!(!options.is_minimizable);
        assert!(matches!(options.kind, WindowKind::PopUp));
        assert!(matches!(
            options.window_background,
            WindowBackgroundAppearance::Transparent
        ));
        let titlebar = options.titlebar.expect("launcher titlebar options");
        assert!(titlebar.appears_transparent);
        assert!(titlebar.traffic_light_position.is_none());
    }

    #[gpui_kit::test]
    #[cfg(windows)]
    fn windows_launcher_starts_visible_with_taskbar_restore(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let options = cx.update(main_window_options);
        assert!(options.show && options.focus && options.is_minimizable);
        assert!(matches!(options.kind, WindowKind::Normal));
        assert!(matches!(
            options.window_background,
            WindowBackgroundAppearance::Opaque
        ));
        assert!(options
            .titlebar
            .is_some_and(|titlebar| !titlebar.appears_transparent));
    }

    #[test]
    fn packaged_app_is_an_accessory_menu_bar_application() {
        let info = include_str!("../macos/Info.plist");
        assert!(info.contains("<key>LSUIElement</key>"));
        assert!(info.contains("<true/>"));
    }

    #[test]
    fn runtime_startup_worker_does_not_block_the_caller() {
        let (started_sender, started_receiver) = mpsc::sync_channel(1);
        let (release_sender, release_receiver) = mpsc::sync_channel(1);
        let (returned_sender, returned_receiver) = mpsc::sync_channel(1);
        let runner = std::thread::spawn(move || {
            let receiver = start_runtime_worker(move || {
                started_sender.send(()).unwrap();
                release_receiver.recv().unwrap();
                RuntimeStartup::Failed("synthetic startup failure".to_owned())
            });
            returned_sender.send(receiver).unwrap();
        });

        let started_at = Instant::now();
        let receiver = returned_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("startup submission must return before storage work finishes");
        assert!(started_at.elapsed() < Duration::from_secs(1));
        started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("startup worker must start");
        drop(receiver);
        release_sender.send(()).unwrap();
        runner.join().unwrap();
    }

    #[test]
    fn component_theme_uses_the_product_typography_and_control_tokens() {
        let mut theme = gpui_kit::component::Theme::default();
        configure_component_theme(&mut theme);
        assert_eq!(theme.font_family.as_ref(), fonts::FONT_UI);
        assert_eq!(theme.mono_font_family.as_ref(), fonts::FONT_MONO);
        assert_eq!(theme.font_size, px(metrics::BODY_SIZE));
        assert_eq!(theme.colors.background, theme::transparent());
        assert_eq!(theme.tokens.background, theme::transparent().into());
        assert_eq!(theme.tokens.input.color, theme::control_border());
        assert_eq!(theme.tokens.button_primary.color, theme::accent_fill());
        assert_eq!(theme.tokens.button_secondary.color, theme::transparent());
        assert_eq!(theme.tokens.list_active.color, theme::selected());
        assert_eq!(theme.tokens.primary.color, theme::accent_fill());
        assert_eq!(theme.tokens.switch_thumb.color, theme::on_dark_primary());
    }

    #[gpui_kit::test]
    fn component_theme_customizes_tokens_after_dark_mode_loads(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(install_component_theme);

        cx.update(|cx| {
            let theme = gpui_kit::component::Theme::global(cx);
            assert_eq!(theme.mode, gpui_kit::component::ThemeMode::Dark);
            assert_eq!(theme.font_family.as_ref(), fonts::FONT_UI);
            assert_eq!(theme.colors.background, theme::transparent());
            assert_eq!(theme.tokens.background, theme::transparent().into());
            assert_eq!(theme.tokens.input.color, theme::control_border());
            assert_eq!(theme.tokens.button_primary.color, theme::accent_fill());
            assert_eq!(theme.tokens.button_secondary.color, theme::transparent());
            assert_eq!(theme.tokens.list_active.color, theme::selected());
            assert_eq!(theme.tokens.primary.color, theme::accent_fill());
            assert_eq!(theme.tokens.switch_thumb.color, theme::on_dark_primary());
        });
    }

    #[gpui_kit::test]
    fn component_root_does_not_paint_over_native_material(cx: &mut TestAppContext) {
        use gpui_kit::Styled as _;
        struct ClearContent;
        impl Render for ClearContent {
            fn render(
                &mut self,
                _: &mut Window,
                _: &mut Context<Self>,
            ) -> impl gpui_kit::IntoElement {
                gpui_kit::div().size_full()
            }
        }
        cx.update(gpui_kit::init);
        cx.update(install_component_theme);
        let (handle, _) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    show: false,
                    focus: false,
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                cx,
                |_, cx| cx.new(|_| ClearContent),
            )
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(
                window
                    .painted_quads()
                    .iter()
                    .all(|quad| quad.background.is_transparent()),
                "the component Root must not fill the transparent product canvas"
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn tasks_use_a_separate_resizable_window(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let options = cx.update(tasks_window_options);
        assert!(options.show);
        assert!(options.focus);
        assert!(options.is_resizable);
        assert!(matches!(options.kind, WindowKind::Normal));
        assert_eq!(options.window_min_size, Some(size(px(760.), px(420.))));
    }

    #[gpui_kit::test]
    fn tool_window_uses_transparent_frameless_glass_chrome(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let options = cx.update(|cx| tool_window_options("JSON Workspace".into(), cx));
        assert!(options.show);
        assert!(options.focus);
        assert!(options.is_resizable);
        assert!(matches!(options.kind, WindowKind::Normal));
        assert_eq!(options.window_background, product_window_background());
        assert!(options
            .titlebar
            .as_ref()
            .is_some_and(|titlebar| titlebar.appears_transparent != cfg!(windows)
                && titlebar.traffic_light_position.is_some()
                && titlebar.title.as_deref() == Some("JSON Workspace")));
        assert_eq!(
            options.window_min_size,
            Some(size(
                px(crate::ui::LAUNCHER_WIDTH),
                px(crate::ui::TOOL_WINDOW_MIN_HEIGHT),
            ))
        );
    }

    #[gpui_kit::test]
    fn tasks_restore_launcher_waits_for_the_tasks_window_update_to_finish(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (_main_window, main_launcher) = cx
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
        let (tasks_window, tasks_shell) = cx
            .update(|cx| {
                let main_launcher = main_launcher.downgrade();
                gpui_kit::open_window(
                    WindowOptions {
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    cx,
                    move |window, cx| {
                        let launcher = cx.new(|cx| {
                            LauncherView::tasks_window(window, cx, live, crate::l10n::Locale::En)
                        });
                        cx.new(|cx| ToolWindowView::new(launcher, main_launcher, window, cx))
                    },
                )
            })
            .unwrap();

        cx.update_window(tasks_window, |_, window, cx| {
            tasks_shell.update(cx, |shell, cx| {
                shell.launcher.update(cx, |launcher, cx| {
                    launcher.prepare_tasks_launcher_restore_for_test();
                    launcher.handle_tasks_navigation_key("enter", false, window, cx);
                });
            });
        })
        .unwrap();
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn clipboard_shelf_uses_a_resizable_normal_window_before_floating_configuration(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let options = cx.update(shelf_window_options);
        assert!(options.show);
        assert!(options.focus);
        assert!(options.is_resizable);
        assert!(options.is_minimizable);
        assert!(matches!(options.kind, WindowKind::Normal));
        assert_eq!(options.window_min_size, Some(size(px(360.), px(360.))));
        assert_eq!(
            options
                .titlebar
                .as_ref()
                .and_then(|titlebar| titlebar.traffic_light_position),
            Some(gpui_kit::point(px(14.), px(16.)))
        );
    }

    #[gpui_kit::test]
    fn native_settings_close_keeps_the_window_open_for_an_unsaved_draft(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (window, shell) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        show: false,
                        focus: false,
                        ..Default::default()
                    },
                    cx,
                    |window, cx| {
                        let launcher = cx.new(|cx| LauncherView::new(window, cx));
                        cx.new(|_| ProductShell {
                            launcher,
                            live: None,
                            shelf_window: None,
                            tasks_window: None,
                            tasks_launcher: None,
                            _status_item: None,
                            _launcher_events: None,
                            _window_closed: None,
                            #[cfg(windows)]
                            _window_activation: None,
                        })
                    },
                )
            })
            .unwrap();

        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            shell.update(cx, |shell, cx| {
                shell.launcher.update(cx, |launcher, cx| {
                    launcher.open_settings(window, cx);
                });
            });
            window.render_frame(cx);
            window.click("settings-category-general", cx);
            window.render_frame(cx);
            window.click("open-at-login", cx);
            window.render_frame(cx);
        })
        .unwrap();

        let mut visual = VisualTestContext::from_window(window, cx);
        assert!(
            !visual.simulate_close(),
            "the product window must remain alive as a background app"
        );
        visual.update(|window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("settings-surface").is_some());
            assert!(
                window.try_find("discard-settings").is_some(),
                "native close must use the dirty-settings close route"
            );
        });
    }
}
