use super::*;
use anywhere_core::application::web_plugins::{
    HostInteraction, HostInteractionKind, WebHostAction, WebHostActionKind,
};
use anywhere_protocol::web_bridge::WebErrorCode;

const SHOW_PATH_BYTES: usize = 180;

impl LauncherView {
    pub(in crate::launcher::view) fn listen_web_host(
        live: &LiveLauncher,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Ok(mut events) = live.web_runtime().application().web_host_interactions() else {
            return;
        };
        let window_handle = window.window_handle();
        cx.spawn(async move |view, cx| {
            while let Some(interaction) = events.recv().await {
                let mut interaction = Some(interaction);
                let delivered = cx.update_window(window_handle, |_, window, cx| {
                    view.update(cx, |this, cx| {
                        if let Some(interaction) = interaction.take() {
                            this.present_web_host_interaction(interaction, window, cx);
                        }
                    })
                    .is_ok()
                });
                if let Some(interaction) = interaction {
                    interaction.respond(Err(WebErrorCode::Inactive));
                }
                if !matches!(delivered, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    pub(in crate::launcher::view) fn sync_web_view_visibility(&self, cx: &Context<Self>) {
        if let Some(web_view) = self.web_surface.view.as_ref() {
            let visible = self.presence.presented
                && self.screen == Screen::WebTool
                && !self.web_surface.actions_open
                && self.web_surface.interaction.is_none();
            let _ = web_view.read(cx).set_visible(visible);
        }
    }

    fn present_web_host_interaction(
        &mut self,
        interaction: HostInteraction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.presence.presented
            || !interaction.is_current()
            || interaction
                .document_handle
                .as_deref()
                .is_some_and(|handle| {
                    self.screen != Screen::WebTool || !event_handle_matches(self, handle)
                })
        {
            interaction.respond(Err(WebErrorCode::Inactive));
            return;
        }
        if let HostInteractionKind::Preview(resource) = &interaction.kind {
            let file = crate::host::PreviewFile {
                id: resource.id.clone(),
                paths: resource.paths.clone(),
                title: resource.title.clone(),
            };
            self.show_web_preview(file, interaction, cx);
            return;
        }
        if self.web_surface.interaction.is_some() {
            interaction.respond(Err(WebErrorCode::Busy));
            return;
        }
        if self.preview.open {
            self.close_preview(cx);
        }
        self.web_surface.interaction_generation =
            self.web_surface.interaction_generation.wrapping_add(1);
        let generation = self.web_surface.interaction_generation;
        let expiry = interaction.expiry();
        let window_handle = self.window_handle;
        if let Some(web_view) = self.web_surface.view.as_ref() {
            let web_view = web_view.read(cx);
            let _ = web_view.raw().evaluate_script(
                "window.dispatchEvent(new CustomEvent('anywhere:panel',{detail:true}));",
            );
            if web_view.set_visible(false).is_err() {
                self.sync_web_view_visibility(cx);
                let _ = web_view.raw().evaluate_script(
                    "window.dispatchEvent(new CustomEvent('anywhere:panel',{detail:false}));",
                );
                interaction.respond(Err(WebErrorCode::HostFailure));
                return;
            }
        }
        let modal_focus = self
            .web_surface
            .interaction_focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        self.web_surface
            .interaction_cancel_focus
            .get_or_insert_with(|| cx.focus_handle());
        let confirm_focus = self
            .web_surface
            .interaction_confirm_focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        self.web_surface.interaction = Some(interaction);
        self.web_surface.interaction_expanded = false;
        let _ = modal_focus;
        confirm_focus.focus(window, cx);
        if self.screen == Screen::Launcher {
            self.resize_current_window(cx);
        }
        cx.notify();
        cx.spawn(async move |view, cx| {
            expiry.await;
            let _ = cx.update_window(window_handle, |_, window, cx| {
                let _ = view.update(cx, |this, cx| {
                    if this.web_surface.interaction_generation == generation {
                        this.respond_web_host_interaction(Err(WebErrorCode::Expired), window, cx);
                    }
                });
            });
        })
        .detach();
    }

    pub(in crate::launcher::view) fn drop_web_host_interaction(&mut self, cx: &mut Context<Self>) {
        if let Some(interaction) = self.web_surface.interaction.take() {
            interaction.respond(Err(WebErrorCode::UserCancelled));
            self.restore_after_web_interaction(cx);
            if self.screen == Screen::Launcher {
                self.resize_current_window(cx);
            }
        }
    }

    fn restore_after_web_interaction(&self, cx: &Context<Self>) {
        self.sync_web_view_visibility(cx);
        if self.presence.presented && self.screen == Screen::WebTool {
            if let Some(web_view) = &self.web_surface.view {
                let web_view = web_view.read(cx);
                let _ = web_view.raw().evaluate_script(
                    "window.dispatchEvent(new CustomEvent('anywhere:panel',{detail:false}));",
                );
                if !self.web_surface.actions_open && self.web_surface.interaction.is_none() {
                    let _ = web_view.raw().focus();
                }
            }
        }
    }

    pub(in crate::launcher::view) fn respond_web_host_interaction(
        &mut self,
        response: Result<(), WebErrorCode>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(interaction) = self.web_surface.interaction.take() else {
            return;
        };
        interaction.respond(response);
        if self.presence.presented && self.screen == Screen::WebTool {
            self.restore_after_web_interaction(cx);
        } else if self.presence.presented {
            self.sync_web_view_visibility(cx);
            self.refocus_active_editor(window, cx);
        }
        if self.screen == Screen::Launcher {
            self.resize_current_window(cx);
        }
        cx.notify();
    }

    pub(in crate::launcher::view) fn intercept_web_interaction_keystroke(
        &mut self,
        event: &gpui_kit::KeystrokeEvent,
        modifiers: KeyModifiers,
        repeated: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        if is_scope_tab_shortcut(key, modifiers) {
            let handles: Vec<FocusHandle> = [
                self.web_surface.interaction_cancel_focus.clone(),
                self.web_surface.interaction_confirm_focus.clone(),
            ]
            .into_iter()
            .flatten()
            .collect();
            if !handles.is_empty() {
                let current = handles
                    .iter()
                    .position(|handle| handle.is_focused(window))
                    .unwrap_or(0);
                let next = if event.keystroke.modifiers.shift {
                    (current + handles.len() - 1) % handles.len()
                } else {
                    (current + 1) % handles.len()
                };
                handles[next].focus(window, cx);
            }
            window.prevent_default();
            cx.stop_propagation();
            return;
        }
        if !repeated && !modifiers.modified {
            match key {
                "escape" => {
                    self.respond_web_host_interaction(Err(WebErrorCode::UserCancelled), window, cx);
                }
                "enter" | "return" => {
                    let cancel_focused = self
                        .web_surface
                        .interaction_cancel_focus
                        .as_ref()
                        .is_some_and(|handle| handle.is_focused(window));
                    let outcome = if cancel_focused {
                        Err(WebErrorCode::UserCancelled)
                    } else {
                        Ok(())
                    };
                    self.respond_web_host_interaction(outcome, window, cx);
                }
                "space" => {
                    let cancel_focused = self
                        .web_surface
                        .interaction_cancel_focus
                        .as_ref()
                        .is_some_and(|handle| handle.is_focused(window));
                    let confirm_focused = self
                        .web_surface
                        .interaction_confirm_focus
                        .as_ref()
                        .is_some_and(|handle| handle.is_focused(window));
                    if cancel_focused {
                        self.respond_web_host_interaction(
                            Err(WebErrorCode::UserCancelled),
                            window,
                            cx,
                        );
                    } else if confirm_focused {
                        self.respond_web_host_interaction(Ok(()), window, cx);
                    }
                }
                _ => {}
            }
        }
        window.prevent_default();
        cx.stop_propagation();
    }

    pub(super) fn schedule_web_host_action(
        &self,
        action: WebHostAction,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        let view = cx.weak_entity();
        let window_handle = self.window_handle;
        cx.defer(move |cx| {
            let _ = window_handle.update(cx, |_, window, cx| {
                let _ = view.update(cx, |this, cx| {
                    if !this.presence.presented
                        || this.screen != Screen::WebTool
                        || this.web_surface.needs_resume
                        || this.web_surface.opening
                        || this.live_state.generation != generation
                        || !event_handle_matches(this, &action.handle)
                        || this.live.as_ref().is_none_or(|live| {
                            live.web_runtime()
                                .validate_web_host_action(&action)
                                .is_err()
                        })
                    {
                        return;
                    }
                    if action.kind == WebHostActionKind::Back || this.standalone_tool {
                        this.go_back(window, cx);
                    } else {
                        this.close_preview(cx);
                        this.close_web_surface(cx);
                        this.screen = Screen::Launcher;
                        this.restore_parent_workspace(window, cx);
                        this.notice = None;
                        this.prepare_hide(cx);
                        crate::platform::hide_main_window();
                        cx.notify();
                    }
                });
            });
        });
    }

    pub(in crate::launcher::view) fn render_web_host_interaction(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let HostInteractionKind::OpenUrl {
            origin,
            destination,
            query_bytes,
        } = &self.web_surface.interaction.as_ref()?.kind
        else {
            return None;
        };
        let path = destination.strip_prefix(origin).unwrap_or(destination);
        let has_more = path.len() > SHOW_PATH_BYTES;
        let expanded = self.web_surface.interaction_expanded;
        let truncated = has_more && !expanded;
        let path = if truncated {
            format!("{}…", &path[..path.floor_char_boundary(SHOW_PATH_BYTES)])
        } else {
            path.to_owned()
        };
        let body = div()
            .p(px(metrics::SPACE_LG))
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_SM))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(l10n::plugin_open_url_title(self.locale)),
            )
            .child(
                div()
                    .id("web-open-url-origin")
                    .w_full()
                    .min_w(px(0.))
                    .overflow_x_scroll()
                    .font_family(fonts::FONT_MONO)
                    .text_color(theme::accent())
                    .child(origin.clone()),
            )
            .child(
                div()
                    .id("web-open-url-destination")
                    .w_full()
                    .min_w(px(0.))
                    .max_h(px(metrics::SPACE_XL * 4.))
                    .overflow_x_scroll()
                    .overflow_y_scroll()
                    .whitespace_normal()
                    .font_family(fonts::FONT_MONO)
                    .text_color(theme::secondary())
                    .child(path),
            )
            .when(has_more, |container| {
                container.child(
                    Button::new("show-url-details")
                        .ghost()
                        .label(if expanded {
                            l10n::hide_link_details(self.locale)
                        } else if *query_bytes > 0 {
                            l10n::show_link_query_details(self.locale, &query_bytes.to_string())
                        } else {
                            l10n::show_link_details(self.locale)
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.web_surface.interaction_expanded =
                                !this.web_surface.interaction_expanded;
                            cx.notify();
                        })),
                )
            });
        let cancel_button = Button::new("cancel-web-url")
            .ghost()
            .label(l10n::cancel(self.locale))
            .on_click(cx.listener(|this, _, window, cx| {
                this.respond_web_host_interaction(Err(WebErrorCode::UserCancelled), window, cx)
            }));
        let confirm_button = Button::new("confirm-web-url")
            .primary()
            .label(l10n::plugin_open_url(self.locale))
            .on_click(cx.listener(|this, _, window, cx| {
                this.respond_web_host_interaction(Ok(()), window, cx)
            }));
        let actions = div()
            .flex()
            .justify_end()
            .gap(px(metrics::SPACE_SM))
            .child(
                div()
                    .when_some(
                        self.web_surface.interaction_cancel_focus.clone(),
                        |el, handle| el.track_focus(&handle),
                    )
                    .child(cancel_button),
            )
            .child(
                div()
                    .when_some(
                        self.web_surface.interaction_confirm_focus.clone(),
                        |el, handle| el.track_focus(&handle),
                    )
                    .child(confirm_button),
            )
            .p(px(metrics::SPACE_LG));
        let mut frame = components::modal_frame(
            "web-open-url-confirmation",
            metrics::MODAL_WIDTH_PICKER,
            body,
            actions,
        )
        .role(gpui_kit::Role::Dialog)
        .aria_label(l10n::plugin_open_url_title(self.locale));
        if let Some(focus) = &self.web_surface.interaction_focus {
            frame = frame.track_focus(focus);
        }
        Some(frame.into_any_element())
    }
}
