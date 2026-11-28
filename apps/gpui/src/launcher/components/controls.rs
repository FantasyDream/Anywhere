use super::*;

pub fn filter_chip(
    id: impl Into<gpui_kit::ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
) -> gpui_kit::base::ObservedElement<Stateful<Div>> {
    let label = label.into();
    div()
        .id(id)
        .test_support()
        .role(gpui_kit::Role::Button)
        .aria_label(label.clone())
        .aria_selected(selected)
        .h(px(metrics::FILTER_HEIGHT))
        .px(px(metrics::SPACE_MD))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(metrics::SPACE_XS))
        .rounded(px(metrics::FILTER_RADIUS))
        .text_size(px(metrics::FILTER_SIZE))
        .line_height(px(metrics::FILTER_SIZE * metrics::BODY_LINE_HEIGHT))
        .font_weight(if selected {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .text_color(if selected {
            theme::primary()
        } else {
            theme::secondary()
        })
        .when(selected, |chip| chip.selected_surface())
        .when(!selected, |chip| {
            chip.hover(|style| style.bg(theme::hover()))
        })
        .cursor_pointer()
        .child(label)
}

pub fn search_field(
    id: impl Into<gpui_kit::ElementId>,
    state: &gpui_kit::Entity<gpui_kit::component::input::InputState>,
    label: impl Into<SharedString>,
) -> gpui_kit::component::input::Input {
    use gpui_kit::component::input::Input;
    Input::new(state)
        .id(id)
        .aria_label(label.into())
        .h(px(metrics::SEARCH_FIELD_HEIGHT))
        .prefix(
            Icon::new(IconName::Search)
                .size(px(metrics::ICON_SIZE))
                .text_color(theme::muted()),
        )
        .bordered(true)
        .focus_bordered(true)
        .cleanable(true)
        .text_size(px(metrics::BODY_SIZE))
        .w_full()
}
