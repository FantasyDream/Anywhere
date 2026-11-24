//! Open a separate normal window for Web plugins while the launcher retains its own session.
use super::*;
use crate::launcher::ToolWindowView;

pub(super) struct OpenToolWindow {
    key: String,
    pub(super) handle: AnyWindowHandle,
    pub(super) view: gpui_kit::WeakEntity<LauncherView>,
}

impl LauncherView {
    pub(crate) fn tool_window_handles(
        &self,
    ) -> impl Iterator<Item = (AnyWindowHandle, gpui_kit::WeakEntity<Self>)> + '_ {
        self.tool_windows
            .iter()
            .map(|tool| (tool.handle, tool.view.clone()))
    }

    fn watch_tool_access(
        &self,
        live: &LiveLauncher,
        id: anywhere_protocol::SessionId,
        cx: &mut Context<Self>,
    ) {
        let runtime = live.web_runtime();
        let application = Arc::downgrade(runtime.application());
        let mut updates = runtime.application().state_updates();
        cx.spawn(async move |this, cx| {
            while application
                .upgrade()
                .is_some_and(|app| app.is_session_active(id))
            {
                if updates.changed().await.is_err() {
                    return;
                }
            }
            let _ = this.update(cx, |view, cx| {
                if view.presence.presented && view.live_state.session_id == Some(id) {
                    view.prepare_hide(cx);
                    view.screen = Screen::UnsupportedTool;
                    view.presence.presented = true;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(super) fn can_open_tool_window(&self) -> bool {
        !self.standalone_tool
            && self.live.is_some()
            && self.screen == Screen::WebTool
            && self.web_surface.document.is_some()
            && !self.web_surface.opening
            && !self.web_surface.needs_resume
    }

    pub(crate) fn defer_tool_escape(&self, cx: &mut Context<Self>) {
        let handle = self.window_handle;
        let view = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = view.update(cx, |this, cx| {
                    this.handle_web_view_escape_from_platform(window, cx)
                });
            });
        });
    }

    pub(super) fn open_tool_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_open_tool_window() {
            return;
        }
        let Some(document) = self.web_surface.document.as_ref() else {
            return;
        };
        let Some(live) = self.live.as_ref() else {
            return;
        };
        let title = document.title.clone();
        let key = document.handle.clone();
        let locale = self.locale;
        self.tool_windows.retain(|tool| {
            tool.view.upgrade().is_some_and(|view| {
                let view = view.read(cx);
                view.live_state
                    .session_id
                    .zip(view.live.as_ref())
                    .is_some_and(|(id, live)| {
                        live.web_runtime().application().is_session_active(id)
                    })
            })
        });
        if let Some(tool) = self.tool_windows.iter().find(|tool| tool.key == key) {
            if tool
                .handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return;
            }
        }
        let (live, state, invocation) = match live.tool_window(&key) {
            Ok(tool) => tool,
            Err(error) => {
                self.set_notice(error.to_string(), components::NoticeKind::Error);
                cx.notify();
                return;
            }
        };
        let parent = cx.weak_entity();
        let options = crate::app::tool_window_options(title, cx);
        let opened = gpui_kit::open_window(options, cx, |window, cx| {
            crate::platform::configure_tool_window(window);
            let launcher = cx.new(|cx| {
                let mut view = Self::build(
                    window,
                    cx,
                    LauncherModel::empty(locale),
                    locale,
                    Some(live.clone()),
                    None,
                    ViewKind::ToolWindow,
                );
                view.screen = Screen::WebTool;
                view.presence.presented = true;
                view.live_state.session_id = Some(state.session_id);
                view.live_state.snapshot_gate = Some(SnapshotGate::new(state.session_id));
                view.live_state.request_revision = state.revision;
                view.watch_tool_access(&live, state.session_id, cx);
                view.model.apply(state);
                view.web_surface.invocation_id = Some(invocation.clone());
                view.poll_web_document(live.open_web_document(invocation, None), cx);
                view.refocus_active_editor(window, cx);
                view
            });
            cx.new(|cx| ToolWindowView::new(launcher, parent, window, cx))
        });
        match opened {
            Ok((handle, shell)) => {
                self.tool_windows.push(OpenToolWindow {
                    key,
                    handle,
                    view: shell.read(cx).launcher.downgrade(),
                });
                // The main launcher remains the global search entry point while the
                // copied tool owns its document and session.
                self.go_back(window, cx);
            }
            Err(error) => self.set_notice(error.to_string(), components::NoticeKind::Error),
        }
        cx.notify();
    }
}
