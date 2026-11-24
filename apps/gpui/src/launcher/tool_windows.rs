//! Independent tool window render owners; shared controllers keep only their query editors live.
use super::{LauncherEvent, LauncherView};
use gpui_kit::{Context, Entity, Render, Subscription, Window};

fn register_tool_window_close<V: 'static>(
    launcher: &Entity<LauncherView>,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    let launcher = launcher.downgrade();
    window.on_window_should_close(cx, move |window, cx| {
        let _ = launcher.update(cx, |launcher, cx| launcher.prepare_hide(cx));
        let handle = window.window_handle();
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(0))
                .await;
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        })
        .detach();
        false
    });
}

pub(crate) struct ClipboardShelfView {
    pub(crate) launcher: Entity<LauncherView>,
    _launcher_observation: Subscription,
}

impl ClipboardShelfView {
    pub(crate) fn new(
        launcher: Entity<LauncherView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        register_tool_window_close(&launcher, window, cx);
        let launcher_observation = cx.observe(&launcher, |_, _, cx| cx.notify());
        Self {
            launcher,
            _launcher_observation: launcher_observation,
        }
    }
}

impl Render for ClipboardShelfView {
    fn render(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        self.launcher.clone()
    }
}

pub(crate) struct ToolWindowView {
    pub(crate) launcher: Entity<LauncherView>,
    _launcher_observation: Subscription,
    _launcher_events: Subscription,
}

impl ToolWindowView {
    pub(crate) fn new(
        launcher: Entity<LauncherView>,
        main_launcher: gpui_kit::WeakEntity<LauncherView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        register_tool_window_close(&launcher, window, cx);
        let launcher_observation = cx.observe(&launcher, |_, _, cx| cx.notify());
        let events = cx.subscribe(&launcher, move |_, tool, event: &LauncherEvent, cx| {
            if matches!(event, LauncherEvent::WebViewEscapeRequested) {
                tool.update(cx, |view, cx| view.defer_tool_escape(cx));
            } else if matches!(
                event,
                LauncherEvent::OpenTasks(_)
                    | LauncherEvent::OpenClipboardShelf
                    | LauncherEvent::QuitRequested
            ) {
                let _ = main_launcher.update(cx, |_, cx| cx.emit(event.clone()));
            } else if matches!(event, LauncherEvent::RestoreLauncher) {
                let _ = main_launcher.update(cx, |view, cx| view.restore_from_tool(cx));
            }
        });
        Self {
            _launcher_events: events,
            launcher,
            _launcher_observation: launcher_observation,
        }
    }
}

impl Render for ToolWindowView {
    fn render(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        self.launcher.clone()
    }
}
