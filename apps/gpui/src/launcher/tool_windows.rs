//! Independent tool window render owners; shared controllers keep only their query editors live.
use super::{LauncherEvent, LauncherView};
use gpui_kit::{Context, Entity, Render, Subscription, Window};

pub(crate) struct ClipboardShelfView {
    pub(crate) launcher: Entity<LauncherView>,
    _launcher_observation: Subscription,
}

impl ClipboardShelfView {
    pub(crate) fn new(launcher: Entity<LauncherView>, cx: &mut Context<Self>) -> Self {
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        let launcher = self.launcher.downgrade();
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
        self.launcher.clone()
    }
}

pub(crate) struct TasksWindowView {
    pub(crate) launcher: Entity<LauncherView>,
    _launcher_observation: Subscription,
    _launcher_events: Subscription,
}

impl TasksWindowView {
    pub(crate) fn new(
        launcher: Entity<LauncherView>,
        main_launcher: gpui_kit::WeakEntity<LauncherView>,
        cx: &mut Context<Self>,
    ) -> Self {
        let launcher_observation = cx.observe(&launcher, |_, _, cx| cx.notify());
        let events = cx.subscribe(&launcher, move |_, _, event: &LauncherEvent, cx| {
            if matches!(event, LauncherEvent::RestoreLauncher) {
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

impl Render for TasksWindowView {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        let launcher = self.launcher.downgrade();
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
        self.launcher.clone()
    }
}
