use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn render_web_tool(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.ensure_web_view(window, cx);
        let (title, subtitle) = if let Some(document) = &self.web_surface.document {
            (document.title.clone(), document.origin.clone())
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
        let trailing = div()
            .id("web-tool-actions")
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(l10n::actions(self.locale))
            .size(px(26.))
            .mr(px(8.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.))
            .hover(|style| style.bg(theme::hover()))
            .cursor_pointer()
            .tooltip(components::text_tooltip(l10n::actions(self.locale)))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_web_actions(cx)))
            .child(
                Icon::new(IconName::Ellipsis)
                    .size(px(16.))
                    .text_color(theme::secondary()),
            )
            .into_any_element();
        let actions_panel = self.web_surface.actions_open.then(|| {
            let labels = [
                (IconName::RefreshCw, l10n::reload_tool(self.locale)),
                (IconName::ArrowLeft, l10n::back_to_search_esc(self.locale)),
            ];
            components::menu_popover()
                .id("web-tool-actions-panel")
                .debug_selector(|| "web-tool-actions-panel".into())
                .test_support()
                .w(px(230.))
                .overflow_hidden()
                .p(px(6.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(
                    labels
                        .into_iter()
                        .enumerate()
                        .map(|(index, (icon, label))| {
                            components::dark_surface_button(
                                SharedString::from(format!("web-tool-action-{index}")),
                                index == self.web_surface.action_selected,
                                cx,
                            )
                            .icon(icon)
                            .label(label)
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.web_surface.action_selected = index;
                                    this.execute_web_action(index, window, cx);
                                },
                            ))
                        }),
                )
                .into_any_element()
        });
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
            .size_full()
            .relative()
            .child(page)
            .children(actions_panel.map(|panel| {
                Self::page_popup(
                    window,
                    px(metrics::PAGE_HEADER_HEIGHT + metrics::DIVIDER_WIDTH),
                    px(12.),
                    panel,
                )
            }))
            .into_any_element()
    }
}
