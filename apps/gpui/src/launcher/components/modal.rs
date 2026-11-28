use super::*;

pub fn modal_shell(
    id: &'static str,
    icon: IconName,
    badge_tint: gpui_kit::Hsla,
    title: String,
    description: String,
    extra: Option<AnyElement>,
    actions: Div,
) -> AnyElement {
    let body = div()
        .px(px(metrics::SPACE_XL))
        .pt(px(metrics::SPACE_XL))
        .pb(px(metrics::SPACE_LG))
        .flex()
        .flex_col()
        .items_start()
        .gap(px(metrics::SPACE_SM))
        .child(
            div()
                .size(px(metrics::ROW_HEIGHT_COMPACT))
                .rounded(px(metrics::CARD_RADIUS))
                .bg(badge_tint.opacity(0.14))
                .border(px(metrics::DIVIDER_WIDTH))
                .border_color(badge_tint.opacity(0.28))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Icon::new(icon)
                        .size(px(metrics::ROW_ICON_SIZE))
                        .text_color(badge_tint),
                ),
        )
        .child(
            div()
                .w_full()
                .text_size(px(metrics::MODAL_TITLE_SIZE))
                .line_height(px(metrics::MODAL_TITLE_SIZE * metrics::BODY_LINE_HEIGHT))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::primary())
                .child(title),
        )
        .when(!description.is_empty(), |body| {
            body.child(
                div()
                    .w_full()
                    .text_size(px(metrics::LABEL_SIZE))
                    .line_height(px(metrics::LABEL_SIZE * metrics::CAPTION_LINE_HEIGHT))
                    .whitespace_normal()
                    .text_color(theme::secondary())
                    .child(description),
            )
        })
        .children(extra);
    modal_frame(id, metrics::MODAL_WIDTH_COMPACT, body, actions).into_any_element()
}

pub fn modal_frame(
    id: &'static str,
    width: f32,
    body: impl IntoElement,
    actions: Div,
) -> gpui_kit::base::ObservedElement<Stateful<Div>> {
    div()
        .id(id)
        .test_support()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme::modal_scrim())
        .occlude()
        .child(
            div()
                .w(px(width))
                .max_w_full()
                .max_h_full()
                .rounded(px(metrics::MODAL_RADIUS))
                .bg(theme::action_menu_surface())
                .border(px(metrics::DIVIDER_WIDTH))
                .border_color(theme::control_border())
                .shadow_lg()
                .overflow_hidden()
                .text_color(theme::primary())
                .flex()
                .flex_col()
                .child(body)
                .child(div().h(px(metrics::DIVIDER_WIDTH)).bg(theme::divider()))
                .child(actions),
        )
}
