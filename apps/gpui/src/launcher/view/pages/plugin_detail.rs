use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn render_plugin_detail(
        &mut self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(detail) = self.web_surface.plugin_detail.clone() else {
            return self.render_launcher(cx);
        };
        let has_body = detail
            .body
            .as_deref()
            .is_some_and(|body| !body.trim().is_empty());
        let has_rows = !detail.rows.is_empty();
        let subtitle = l10n::read_only_action_details(self.locale);
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
                    .line_height(px(metrics::BODY_SIZE * 1.6))
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
}
