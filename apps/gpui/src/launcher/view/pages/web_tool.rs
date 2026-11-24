use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn render_web_tool(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if !self.web_surface.actions_open {
            self.ensure_web_view(window, cx);
        }
        let (title, subtitle) = if let Some(document) = &self.web_surface.document {
            let subtitle = if self.standalone_tool {
                if document.builtin {
                    l10n::built_in(self.locale)
                } else {
                    l10n::web_tools(self.locale)
                }
            } else {
                String::new()
            };
            (document.title.clone(), subtitle)
        } else {
            (
                l10n::web_tool_title(self.locale),
                l10n::preparing_plugin_document(self.locale),
            )
        };
        let body = if let Some(error) = &self.web_surface.error {
            self.empty_page_state(
                error.clone(),
                Some(l10n::return_to_launcher_and_try_again(self.locale)),
            )
        } else if let Some(web_view) = &self.web_surface.view {
            div()
                .relative()
                .flex_1()
                .min_h(px(0.))
                .child(web_view.clone())
                .when(!self.web_surface.ready, |element| {
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
                            .child(l10n::starting_web_tool(self.locale)),
                    )
                })
                .into_any_element()
        } else {
            self.empty_page_state(
                l10n::waiting_for_host_document(self.locale),
                Some(l10n::plugin_execution_boundary(self.locale)),
            )
        };
        let trailing = components::page_actions_button(
            "web-tool-actions",
            l10n::tool_options(self.locale),
            self.web_surface.actions_open,
        )
        .on_click(cx.listener(|this, _, window, cx| this.toggle_web_actions(window, cx)))
        .into_any_element();
        let body = if self.web_surface.actions_open {
            components::action_candidates("web-tool-actions-panel", l10n::tool_options(self.locale))
                .debug_selector(|| "web-tool-actions-panel".into())
                .test_support()
                .children(
                    [
                        (IconName::RefreshCw, l10n::reload_tool(self.locale)),
                        (
                            IconName::ArrowLeft,
                            if self.standalone_tool {
                                l10n::close(self.locale)
                            } else {
                                l10n::back_to_search(self.locale)
                            },
                        ),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (icon, label))| {
                        components::action_candidate(
                            format!("web-tool-action-{index}"),
                            label,
                            icon,
                            self.web_surface.action_selected == index,
                            true,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| this.execute_web_action(index, window, cx),
                        ))
                    }),
                )
                .into_any_element()
        } else {
            body
        };
        let page = self
            .render_page_shell_with_trailing(
                SharedString::from(title),
                SharedString::from(subtitle),
                None,
                body,
                Some(trailing),
                cx,
            )
            .into_any_element();
        div()
            .id("web-tool-surface")
            .key_context("WebTool")
            .track_focus(&self.surface_focus_handle)
            .size_full()
            .relative()
            .child(page)
            .into_any_element()
    }
}
