use gpui_kit::{
    assets::IconName, component::Icon, div, prelude::*, px, AnyElement, Div, FontWeight, Hsla,
    IntoElement, SharedString, Stateful, TestSupportExt as _,
};

use super::keycap;
use crate::ui::{design_tokens::metrics, styles::AppStyle as _, theme};

/// Contextual bottom bar keeping keyboard affordances visible without crowding the selected row.
pub fn footer_bar(
    id: &'static str,
    leading: impl IntoElement,
    trailing: Vec<AnyElement>,
) -> gpui_kit::base::ObservedElement<Stateful<Div>> {
    let mut right = div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(metrics::SPACE_XS));
    for item in trailing {
        right = right.child(item);
    }
    div()
        .id(id)
        .h(px(metrics::FOOTER_HEIGHT))
        .flex_shrink_0()
        .px(px(metrics::SPACE_LG))
        .border_t(px(metrics::DIVIDER_WIDTH))
        .border_color(theme::divider())
        .flex()
        .items_center()
        .justify_between()
        .gap(px(metrics::SPACE_SM))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(metrics::SPACE_SM))
                .child(leading),
        )
        .child(right)
        .test_support()
}

/// Interactive pill button rendered inside the bottom footer bar.
pub fn footer_action_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    shortcut: impl Into<SharedString>,
    emphasized: bool,
) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(id.into())
        .role(gpui_kit::Role::Button)
        .aria_label(label.clone())
        .h(px(metrics::CONTROL_HEIGHT))
        .px(px(metrics::SPACE_SM))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(metrics::SPACE_XS))
        .rounded(px(metrics::TOOLTIP_RADIUS))
        .cursor_pointer()
        .hover(|style| style.bg(theme::hover()))
        .active(|style| style.bg(theme::selected()))
        .child(
            div()
                .caption_text()
                .font_weight(if emphasized {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if emphasized {
                    theme::primary()
                } else {
                    theme::secondary()
                })
                .child(label),
        )
        .child(keycap(shortcut))
}

/// Compact floating HUD pill for transient non-closing confirmations (e.g. "Copied").
pub fn hud_toast(message: impl Into<SharedString>, icon: IconName, tint: Hsla) -> AnyElement {
    div()
        .id("hud-toast")
        .test_support()
        .px(px(metrics::SPACE_MD))
        .py(px(metrics::SPACE_XS))
        .flex()
        .items_center()
        .gap(px(metrics::SPACE_SM))
        .rounded(px(metrics::TOAST_RADIUS))
        .bg(theme::action_menu_surface())
        .border(px(metrics::DIVIDER_WIDTH))
        .border_color(tint.opacity(0.45))
        .child(
            Icon::new(icon)
                .size(px(metrics::ICON_SIZE))
                .text_color(tint),
        )
        .child(
            div()
                .caption_text()
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::primary())
                .child(message.into()),
        )
        .into_any_element()
}
