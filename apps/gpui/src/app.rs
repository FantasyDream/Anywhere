use gpui_kit::{
    px, size, AnyWindowHandle, App as GpuiApp, AppContext as _, Bounds, Context, Entity, Render,
    Subscription, Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions,
};
use std::sync::{Arc, Mutex};

use crate::{
    host::{HotKeyAction, HotKeyController, LiveLauncher},
    launcher::{launcher_window_size, LauncherEvent, LauncherView},
    ui::{LAUNCHER_WIDTH, SETTINGS_HEIGHT},
    BUNDLE_ID, CLIPBOARD_SHELF_TITLE, PRODUCT_NAME,
};

struct ProductShell {
    launcher: Entity<LauncherView>,
    live: Option<Arc<LiveLauncher>>,
    shelf_window: Option<AnyWindowHandle>,
    _status_item: Option<crate::platform::StatusItemHost>,
    _launcher_events: Option<Subscription>,
}

impl ProductShell {
    fn open_clipboard_shelf(&mut self, cx: &mut Context<Self>) {
        if let Some(window) = self.shelf_window {
            let activated = window
                .update(cx, |_, window, _| window.activate_window())
                .is_ok();
            if activated {
                return;
            }
            self.shelf_window = None;
        }
        let Some(live) = self.live.clone() else {
            return;
        };
        let locale = self.launcher.read(cx).locale();
        let opened = gpui_kit::open_window(shelf_window_options(cx), cx, |window, cx| {
            let launcher =
                cx.new(|cx| LauncherView::clipboard_shelf(window, cx, live.clone(), locale));
            cx.new(|cx| ClipboardShelfShell::new(launcher, cx))
        });
        match opened {
            Ok((window, shell)) => {
                if !crate::platform::set_window_floating(CLIPBOARD_SHELF_TITLE) {
                    tracing::warn!("clipboard_shelf_floating_level_failed");
                }
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
}

struct ClipboardShelfShell {
    launcher: Entity<LauncherView>,
    _launcher_observation: Subscription,
}

impl ClipboardShelfShell {
    fn new(launcher: Entity<LauncherView>, cx: &mut Context<Self>) -> Self {
        let launcher_observation = cx.observe(&launcher, |_, _, cx| cx.notify());
        Self {
            launcher,
            _launcher_observation: launcher_observation,
        }
    }
}

impl Render for ClipboardShelfShell {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        let launcher = self.launcher.downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            let _ = launcher.update(cx, |launcher, cx| launcher.prepare_hide(cx));
            true
        });
        self.launcher.clone()
    }
}

impl Render for ProductShell {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        let launcher = self.launcher.downgrade();
        window.on_window_should_close(cx, move |_, cx| {
            let _ = launcher.update(cx, |launcher, cx| launcher.prepare_hide(cx));
            crate::platform::hide_main_window();
            false
        });
        self.launcher.clone()
    }
}

pub fn run(cx: &mut GpuiApp) {
    gpui_kit::init(cx);
    let live = if std::env::var_os("ANYWHERE_PREVIEW").is_some() {
        None
    } else {
        Some(Arc::new(LiveLauncher::open().unwrap_or_else(|error| {
            tracing::error!(%error, "shared_runtime_unavailable");
            panic!("cannot open the Anywhere runtime: {error:#}")
        })))
    };
    let hotkeys = live.as_ref().map(|live| {
        let preferences = live.preferences().unwrap_or_default();
        Arc::new(Mutex::new(
            HotKeyController::new(&preferences)
                .unwrap_or_else(|error| panic!("global hotkey manager unavailable: {error:#}")),
        ))
    });
    let size = launcher_window_size();
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size, cx)),
        titlebar: Some(gpui_kit::TitlebarOptions {
            title: Some(PRODUCT_NAME.into()),
            appears_transparent: true,
            traffic_light_position: Some(gpui_kit::point(px(14.), px(16.))),
        }),
        focus: true,
        show: true,
        kind: WindowKind::Normal,
        is_resizable: false,
        is_minimizable: true,
        window_background: WindowBackgroundAppearance::Transparent,
        app_id: Some(BUNDLE_ID.to_owned()),
        ..Default::default()
    };
    let shell_live = live.clone();
    let (window_handle, shell) = gpui_kit::open_window(options, cx, |window, cx| {
        let launcher = cx.new(|cx| match (live, hotkeys.clone()) {
            (Some(live), Some(hotkeys)) => {
                LauncherView::live_with_hotkeys(window, cx, live, crate::l10n::Locale::En, hotkeys)
            }
            (Some(live), None) => LauncherView::live(window, cx, live, crate::l10n::Locale::En),
            (None, _) => LauncherView::new(window, cx),
        });
        cx.new(|_| ProductShell {
            launcher,
            live: shell_live,
            shelf_window: None,
            _status_item: crate::platform::install_status_item(),
            _launcher_events: None,
        })
    })
    .expect("open the Anywhere window");
    shell.update(cx, |shell, cx| {
        shell._launcher_events = Some(cx.subscribe(
            &shell.launcher,
            |shell: &mut ProductShell, _, event: &LauncherEvent, cx| match event {
                LauncherEvent::OpenClipboardShelf => shell.open_clipboard_shelf(cx),
            },
        ));
    });
    if let Some(hotkeys) = hotkeys {
        let receiver = global_hotkey::GlobalHotKeyEvent::receiver().clone();
        cx.spawn(async move |cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(24))
                .await;
            while let Ok(event) = receiver.try_recv() {
                tracing::debug!(
                    hotkey_event_id = event.id(),
                    hotkey_event_state = ?event.state(),
                    "global_hotkey_event_received"
                );
                let action = hotkeys
                    .lock()
                    .ok()
                    .and_then(|controller| controller.action_for_event(event));
                let Some(action) = action else {
                    continue;
                };
                let mode = match action {
                    HotKeyAction::CurrentApp => anywhere_protocol::InputMode::Environment,
                    HotKeyAction::Universal => anywhere_protocol::InputMode::Search,
                };
                let hotkey_started = std::time::Instant::now();
                crate::platform::show_main_window();
                let _ = window_handle.update(cx, |_, window, cx| {
                    shell.update(cx, |shell, cx| {
                        shell.launcher.update(cx, |launcher, cx| {
                            launcher.activate_from_hotkey(mode, window, cx)
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
}

fn shelf_window_options(cx: &mut GpuiApp) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(480.), px(520.)), cx)),
        titlebar: Some(gpui_kit::TitlebarOptions {
            title: Some(CLIPBOARD_SHELF_TITLE.into()),
            appears_transparent: true,
            traffic_light_position: Some(gpui_kit::point(px(14.), px(16.))),
        }),
        focus: true,
        show: true,
        kind: WindowKind::Normal,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        app_id: Some(BUNDLE_ID.to_owned()),
        ..Default::default()
    }
}

#[allow(dead_code)]
fn _window_bounds_contract() -> Bounds<gpui_kit::Pixels> {
    Bounds {
        origin: Default::default(),
        size: size(px(LAUNCHER_WIDTH), px(SETTINGS_HEIGHT)),
    }
}
