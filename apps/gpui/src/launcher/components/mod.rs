use gpui_kit::{
    component::{tooltip::Tooltip, Icon},
    div,
    prelude::*,
    px, AnyElement, AnyView, App, Div, FontWeight, Hsla, IntoElement, SharedString, Window,
};

use crate::{
    l10n::{self, Locale},
    ui::{design_tokens::metrics, theme},
};

pub fn text_tooltip(
    text: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |window, cx| Tooltip::new(text.clone()).build(window, cx)
}

pub fn keycap(value: impl Into<SharedString>) -> Div {
    let value = value.into();
    let content = if value.as_ref() == "↵" {
        Icon::new(gpui_kit::assets::IconName::CornerDownLeft)
            .size(px(12.))
            .text_color(theme::secondary())
            .into_any_element()
    } else {
        div()
            .text_size(px(metrics::KEYCAP_FONT_SIZE))
            .line_height(px(metrics::KEYCAP_FONT_SIZE * metrics::KEYCAP_LINE_HEIGHT))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme::secondary())
            .child(value)
            .into_any_element()
    };
    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .whitespace_nowrap()
        .min_w(px(metrics::KEYCAP_MIN_WIDTH))
        .px(px(metrics::KEYCAP_PADDING_X))
        .py(px(metrics::KEYCAP_PADDING_Y))
        .rounded(px(metrics::KEYCAP_RADIUS))
        .border(px(0.5))
        .border_color(theme::keycap_border())
        .bg(theme::keycap_surface())
        .child(content)
}

pub fn empty_state(
    icon: gpui_kit::assets::IconName,
    title: impl Into<SharedString>,
    detail: Option<String>,
) -> AnyElement {
    div()
        .flex_1()
        .min_h(px(180.))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(metrics::SPACE_SM))
        .p(px(metrics::SPACE_XXL))
        .child(Icon::new(icon).size(px(32.)).text_color(theme::muted()))
        .child(
            div()
                .text_size(px(metrics::HEADING_SIZE))
                .line_height(px(metrics::HEADING_SIZE * metrics::BODY_LINE_HEIGHT))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::primary())
                .child(title.into()),
        )
        .when_some(detail, |element, detail| {
            element.child(
                div()
                    .max_w(px(460.))
                    .text_center()
                    .text_size(px(metrics::LABEL_SIZE))
                    .line_height(px(metrics::LABEL_SIZE * metrics::CAPTION_LINE_HEIGHT))
                    .text_color(theme::muted())
                    .child(detail),
            )
        })
        .into_any_element()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoticeKind {
    Info,
    Warning,
    Error,
    Success,
}

pub fn notice(message: impl Into<SharedString>, kind: NoticeKind) -> AnyElement {
    let (color, icon) = match kind {
        NoticeKind::Info => (theme::accent(), gpui_kit::assets::IconName::Info),
        NoticeKind::Warning => (theme::warning(), gpui_kit::assets::IconName::TriangleAlert),
        NoticeKind::Error => (theme::danger(), gpui_kit::assets::IconName::CircleX),
        NoticeKind::Success => (theme::success(), gpui_kit::assets::IconName::CircleCheck),
    };
    div()
        .w_full()
        .px(px(10.))
        .py(px(8.))
        .flex()
        .items_start()
        .gap(px(9.))
        .rounded(px(metrics::CONTROL_RADIUS))
        .bg(theme::card())
        .border(px(metrics::CONTROL_BORDER_WIDTH))
        .border_color(color.opacity(0.24))
        .child(Icon::new(icon).size(px(16.)).text_color(color))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .text_size(px(metrics::LABEL_SIZE))
                .line_height(px(metrics::LABEL_SIZE * 1.2))
                .text_color(theme::primary())
                .child(message.into()),
        )
        .into_any_element()
}

pub fn section_heading(title: impl Into<SharedString>, icon: gpui_kit::assets::IconName) -> Div {
    div()
        .pb(px(10.))
        .flex()
        .items_center()
        .gap(px(metrics::SPACE_SM))
        .child(Icon::new(icon).size(px(14.)).text_color(theme::secondary()))
        .child(
            div()
                .text_size(px(12.))
                .line_height(px(12. * metrics::BODY_LINE_HEIGHT))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::secondary())
                .child(title.into()),
        )
}

pub fn status_badge(
    label: impl Into<SharedString>,
    color: Hsla,
    icon: gpui_kit::assets::IconName,
) -> Div {
    div()
        .flex_none()
        .px(px(7.))
        .py(px(3.))
        .flex()
        .items_center()
        .gap(px(5.))
        .rounded(px(6.))
        .child(Icon::new(icon).size(px(12.)).text_color(color))
        .child(
            div()
                .text_size(px(11.))
                .line_height(px(11. * 1.2))
                .text_color(color)
                .child(label.into()),
        )
}

pub fn clipboard_kind_icon(kind: &str) -> gpui_kit::assets::IconName {
    use gpui_kit::assets::IconName;
    match kind {
        "image" => IconName::Image,
        "files" => IconName::Files,
        "rich_text" => IconName::FileText,
        "binary" => IconName::File,
        _ => IconName::Type,
    }
}

pub fn clipboard_kind_tint(kind: &str) -> Hsla {
    match kind {
        "image" => theme::icon_blue(),
        "files" => theme::icon_amber(),
        "rich_text" => theme::icon_purple(),
        _ => theme::icon_neutral(),
    }
}

pub fn clipboard_kind_label(locale: Locale, kind: &str) -> String {
    match kind {
        "image" => l10n::image(locale),
        "files" => l10n::files(locale),
        "rich_text" => l10n::rich_text(locale),
        "binary" => l10n::data(locale),
        _ => l10n::text(locale),
    }
}

pub fn clipboard_copy_date(milliseconds: i64, compact: bool) -> String {
    use chrono::{Local, TimeZone as _};
    let Some(date) = Local.timestamp_millis_opt(milliseconds).single() else {
        return String::new();
    };
    if compact && date.date_naive() == Local::now().date_naive() {
        date.format("%H:%M").to_string()
    } else {
        date.format("%x · %H:%M").to_string()
    }
}
