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
                if cx
                    .update_window(window_handle, |_, window, cx| {
                        let _ = view.update(cx, |this, cx| {
                            this.present_web_host_interaction(interaction, window, cx);
                        });
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
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
                interaction.respond(Err(WebErrorCode::HostFailure));
                return;
            }
        }
        self.web_surface.interaction = Some(interaction);
        self.web_surface.interaction_expanded = false;
        self.surface_focus_handle.focus(window, cx);
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

    pub(in crate::launcher::view) fn drop_web_host_interaction(&mut self) {
        if let Some(interaction) = self.web_surface.interaction.take() {
            interaction.respond(Err(WebErrorCode::UserCancelled));
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
        let current_web = interaction
            .document_handle
            .as_deref()
            .is_some_and(|handle| event_handle_matches(self, handle));
        interaction.respond(response);
        if self.presence.presented && current_web {
            if let Some(web_view) = &self.web_surface.view {
                let web_view = web_view.read(cx);
                let _ = web_view.set_visible(!self.web_surface.actions_open);
                if !self.web_surface.actions_open {
                    let _ = web_view.raw().focus();
                    let _ = web_view.raw().evaluate_script(
                        "window.dispatchEvent(new CustomEvent('anywhere:panel',{detail:false}));",
                    );
                }
            }
        } else if self.presence.presented {
            self.refocus_active_editor(window, cx);
        }
        cx.notify();
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
                        this.go_back(window, cx);
                        this.prepare_hide(cx);
                        crate::platform::hide_main_window();
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
        Some(
            div()
                .id("web-open-url-confirmation")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .track_focus(&self.surface_focus_handle)
                .bg(theme::modal_scrim())
                .occlude()
                .child(
                    div()
                        .w(px(LAUNCHER_WIDTH - 2. * metrics::SPACE_XL))
                        .max_h_full()
                        .p(px(metrics::SPACE_LG))
                        .rounded(px(metrics::MODAL_RADIUS))
                        .bg(theme::action_menu_surface())
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
                                .overflow_x_scroll()
                                .text_color(theme::accent())
                                .child(origin.clone()),
                        )
                        .child(
                            div()
                                .id("web-open-url-destination")
                                .max_h(px(metrics::SPACE_XL * 4.))
                                .overflow_y_scroll()
                                .whitespace_normal()
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
                                        l10n::show_link_query_details(
                                            self.locale,
                                            &query_bytes.to_string(),
                                        )
                                    } else {
                                        l10n::show_link_details(self.locale)
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.web_surface.interaction_expanded =
                                            !this.web_surface.interaction_expanded;
                                        cx.notify();
                                    })),
                            )
                        })
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap(px(metrics::SPACE_SM))
                                .child(
                                    Button::new("cancel-web-url")
                                        .ghost()
                                        .label(l10n::cancel(self.locale))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.respond_web_host_interaction(
                                                Err(WebErrorCode::UserCancelled),
                                                window,
                                                cx,
                                            )
                                        })),
                                )
                                .child(
                                    Button::new("confirm-web-url")
                                        .primary()
                                        .label(l10n::plugin_open_url(self.locale))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.respond_web_host_interaction(Ok(()), window, cx)
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}
