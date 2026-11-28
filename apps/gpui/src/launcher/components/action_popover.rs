use super::*;

pub fn action_popover(id: &'static str, title: impl Into<SharedString>) -> Stateful<Div> {
    let title = title.into();
    div()
        .id(id)
        .role(gpui_kit::Role::ListBox)
        .aria_label(title.clone())
        .occlude()
        .w(px(metrics::MENU_WIDTH))
        .max_w_full()
        .max_h(px(metrics::POPOVER_MAX_HEIGHT))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(metrics::FILTER_RADIUS))
        .bg(theme::action_menu_surface())
        .border(px(metrics::CONTROL_BORDER_WIDTH))
        .border_color(theme::control_border())
        .shadow_lg()
        .child(
            div()
                .h(px(metrics::MENU_HEADER_HEIGHT))
                .flex_none()
                .flex()
                .items_center()
                .px(px(metrics::SPACE_MD))
                .caption_text()
                .text_color(theme::secondary())
                .child(div().min_w_0().truncate().child(title)),
        )
}

pub fn action_popover_row(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    icon: IconName,
    selected: bool,
    enabled: bool,
    shortcut: Option<&str>,
) -> gpui_kit::base::ObservedElement<Stateful<Div>> {
    let label = label.into();
    let color = if !enabled {
        theme::muted()
    } else if icon == IconName::Trash {
        theme::danger()
    } else {
        theme::primary()
    };
    div()
        .id(id.into())
        .test_support()
        .role(gpui_kit::Role::ListBoxOption)
        .aria_label(label.clone())
        .aria_selected(selected)
        .w_full()
        .h(px(metrics::MENU_ROW_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(metrics::SPACE_SM))
        .px(px(metrics::SPACE_MD))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_color(color)
        .when(selected && enabled, |row| row.selected_surface())
        .when(enabled, |row| {
            row.cursor_pointer().hover(|style| style.bg(theme::hover()))
        })
        .child(
            Icon::new(icon)
                .size(px(metrics::ICON_SIZE))
                .text_color(color),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .label_text()
                .child(label),
        )
        .when_some(
            shortcut.or((selected && enabled).then_some("↵")),
            |row, key| row.child(keycap(key.to_owned())),
        )
}
