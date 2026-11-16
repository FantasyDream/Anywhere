use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn render_unsupported_tool(
        &mut self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (id, input) = self
            .web_surface
            .unsupported_tool
            .clone()
            .unwrap_or_else(|| ("unknown".to_owned(), String::new()));
        let title = l10n::unsupported_tool(self.locale);
        let description = l10n::unsupported_tool_description(self.locale);
        let mut content = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_MD))
            .p(px(metrics::SPACE_LG))
            .child(
                div()
                    .text_size(px(metrics::BODY_SIZE))
                    .text_color(theme::secondary())
                    .child(description),
            )
            .child(
                div()
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .bg(theme::field())
                    .p(px(metrics::SPACE_MD))
                    .flex()
                    .flex_col()
                    .gap(px(metrics::SPACE_SM))
                    .child(
                        div()
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::muted())
                            .child(l10n::tool_id(self.locale)),
                    )
                    .child(
                        div()
                            .font_family(fonts::FONT_MONO)
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::primary())
                            .child(id),
                    ),
            );
        if !input.is_empty() {
            content = content.child(
                div()
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .bg(theme::field())
                    .p(px(metrics::SPACE_MD))
                    .flex()
                    .flex_col()
                    .gap(px(metrics::SPACE_SM))
                    .child(
                        div()
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::muted())
                            .child(l10n::input_label(self.locale)),
                    )
                    .child(
                        div()
                            .font_family(fonts::FONT_MONO)
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::primary())
                            .child(input),
                    ),
            );
        }
        self.render_page_shell(
            SharedString::from(title),
            SharedString::from(l10n::native_destination_unavailable(self.locale)),
            None,
            content.into_any_element(),
            cx,
        )
        .into_any_element()
    }
}
