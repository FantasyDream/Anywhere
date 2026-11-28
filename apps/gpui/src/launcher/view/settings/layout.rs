use super::*;

impl LauncherView {
    pub(super) fn settings_input_row(
        &self,
        title: impl Into<SharedString>,
        subtitle: impl Into<SharedString>,
        input: &Entity<InputState>,
        id: &'static str,
        placeholder: &'static str,
    ) -> impl IntoElement {
        self.settings_field_row(
            title,
            subtitle,
            id,
            div().w_full().child(
                Input::new(input)
                    .id(id)
                    .appearance(true)
                    .text_size(px(metrics::BODY_SIZE))
                    .aria_label(placeholder),
            ),
        )
    }

    pub(super) fn settings_textarea_row(
        &self,
        title: impl Into<SharedString>,
        subtitle: impl Into<SharedString>,
        input: &Entity<TextareaState>,
        id: &'static str,
        placeholder: &'static str,
    ) -> impl IntoElement {
        self.settings_field_row(
            title,
            subtitle,
            id,
            div().id(id).w_full().test_support().child(
                Textarea::new(input)
                    .appearance(true)
                    .text_size(px(metrics::BODY_SIZE))
                    .aria_label(placeholder)
                    .h(px(metrics::CONTROL_HEIGHT * 3.0))
                    .max_h(px(metrics::CONTROL_HEIGHT * 5.0)),
            ),
        )
    }

    pub(super) fn settings_field_row(
        &self,
        title: impl Into<SharedString>,
        subtitle: impl Into<SharedString>,
        id: &'static str,
        control: impl IntoElement,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(format!("settings-field-{id}")))
            .test_support()
            .px(px(metrics::SPACE_MD))
            .py(px(metrics::SPACE_SM))
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_SM))
            .rounded(px(metrics::CARD_RADIUS))
            .bg(theme::card())
            .border(px(metrics::DIVIDER_WIDTH))
            .border_color(theme::divider())
            .child(self.settings_row_copy(title, subtitle))
            .child(control.into_any_element())
    }

    pub(super) fn settings_row(
        &self,
        title: impl Into<SharedString>,
        subtitle: impl Into<SharedString>,
        control: impl IntoElement,
    ) -> Div {
        self.settings_card(self.settings_row_bare(title, subtitle, control))
    }

    pub(super) fn settings_card(&self, content: impl IntoElement) -> Div {
        div()
            .rounded(px(metrics::CARD_RADIUS))
            .bg(theme::card())
            .border(px(metrics::DIVIDER_WIDTH))
            .border_color(theme::divider())
            .child(content.into_any_element())
    }

    pub(super) fn settings_rows_card(&self, rows: impl IntoIterator<Item = Div>) -> Div {
        let mut content = div().flex().flex_col();
        for (index, row) in rows.into_iter().enumerate() {
            if index > 0 {
                content = content.child(
                    div()
                        .h(px(metrics::DIVIDER_WIDTH))
                        .mx(px(metrics::SPACE_MD))
                        .bg(theme::divider()),
                );
            }
            content = content.child(row);
        }
        self.settings_card(content)
    }

    pub(super) fn settings_row_bare(
        &self,
        title: impl Into<SharedString>,
        subtitle: impl Into<SharedString>,
        control: impl IntoElement,
    ) -> Div {
        div()
            .min_h(px(58.))
            .px(px(metrics::SPACE_MD))
            .py(px(metrics::SPACE_SM))
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_LG))
            .child(self.settings_row_copy(title, subtitle))
            .child(div().flex_shrink_0().child(control.into_any_element()))
    }

    pub(super) fn settings_stack_row(
        &self,
        title: impl Into<SharedString>,
        subtitle: impl Into<SharedString>,
        control: impl IntoElement,
    ) -> Div {
        div()
            .px(px(metrics::SPACE_MD))
            .py(px(metrics::SPACE_MD))
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_SM))
            .child(self.settings_row_copy(title, subtitle))
            .child(control.into_any_element())
    }

    pub(super) fn settings_row_copy(
        &self,
        title: impl Into<SharedString>,
        subtitle: impl Into<SharedString>,
    ) -> Div {
        div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .truncate()
                    .body_text()
                    .font_weight(FontWeight::NORMAL)
                    .text_color(theme::primary())
                    .child(title.into()),
            )
            .child(
                div()
                    .caption_text()
                    .text_color(theme::secondary())
                    .child(subtitle.into()),
            )
    }
}
