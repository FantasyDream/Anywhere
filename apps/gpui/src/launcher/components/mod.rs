mod action_popover;
mod controls;
mod modal;

pub use action_popover::{action_popover, action_popover_row};
pub use controls::{filter_chip, search_field};
pub use modal::{modal_frame, modal_shell};

mod footer_bar;
mod highlight;
mod list_row;

pub use footer_bar::{footer_action_button, footer_bar, hud_toast};
pub use highlight::highlighted_text;
pub use list_row::{primary_row_action, row_action_button, ListRow, RowDensity};

use crate::ui::styles::AppStyle as _;
use gpui_kit::TestSupportExt as _;
use gpui_kit::{
    assets::IconName,
    component::{
        button::{Button, ButtonVariants as _},
        tooltip::Tooltip,
        Icon, Selectable as _,
    },
    div,
    prelude::*,
    px, AnyElement, AnyView, App, Div, FontWeight, Hsla, IntoElement, SharedString, Stateful,
    Window,
};

pub fn page_actions_button(id: &'static str, label: String, open: bool) -> Button {
    Button::new(id)
        .ghost()
        .icon(IconName::Ellipsis)
        .accessibility_label(label.clone())
        .tooltip(label)
        .open(open)
}

use crate::{
    l10n::{self, Locale},
    ui::{design_tokens::metrics, theme},
};

/// A second level replaces the page results and uses the same selection semantics.
pub fn action_candidates(id: &'static str, title: impl Into<SharedString>) -> Stateful<Div> {
    let title = title.into();
    div()
        .id(id)
        .role(gpui_kit::Role::ListBox)
        .aria_label(title.clone())
        .flex_1()
        .min_h_0()
        .w_full()
        .flex()
        .flex_col()
        .px(px(metrics::SPACE_MD))
        .child(
            div()
                .flex_none()
                .truncate()
                .px(px(metrics::SPACE_SM))
                .py(px(metrics::SPACE_SM))
                .caption_text()
                .text_color(theme::secondary())
                .child(title),
        )
}

pub fn text_tooltip(
    text: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |window, cx| Tooltip::new(text.clone()).build(window, cx)
}

pub fn squircle_icon_plate(icon: IconName, tint: Hsla, icon_size: f32) -> Div {
    div()
        .size(px(metrics::ICON_PLATE_SIZE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(metrics::ICON_PLATE_RADIUS))
        .bg(tint.opacity(0.14))
        .border(px(metrics::DIVIDER_WIDTH))
        .border_color(tint.opacity(0.24))
        .child(Icon::new(icon).size(px(icon_size)).text_color(tint))
}

pub fn loading_state(label: impl Into<SharedString>) -> Div {
    use gpui_kit::component::spinner::Spinner;
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(metrics::SPACE_SM))
        .body_text()
        .text_color(theme::secondary())
        .child(Spinner::new().color(theme::accent()))
        .child(label.into())
}

pub fn keycap(value: impl Into<SharedString>) -> Div {
    let value = value.into();
    let ink = theme::secondary();
    let content = if value.as_ref() == "↵" {
        Icon::new(IconName::CornerDownLeft)
            .size(px(12.))
            .text_color(ink)
            .into_any_element()
    } else {
        div()
            .text_size(px(metrics::KEYCAP_FONT_SIZE))
            .line_height(px(metrics::KEYCAP_FONT_SIZE * metrics::KEYCAP_LINE_HEIGHT))
            .font_weight(FontWeight::MEDIUM)
            .text_color(ink)
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
) -> Div {
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NoticeKind {
    #[default]
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
        .px(px(metrics::SPACE_SM))
        .py(px(metrics::ROW_INSET_Y_COMPACT))
        .flex()
        .items_center()
        .gap(px(metrics::SPACE_XS))
        .rounded(px(metrics::TOOLTIP_RADIUS))
        .bg(color.opacity(0.14))
        .child(
            Icon::new(icon)
                .size(px(metrics::CAPTION_SIZE))
                .text_color(color),
        )
        .child(
            div()
                .text_size(px(metrics::CAPTION_SIZE))
                .line_height(px(metrics::CAPTION_SIZE * metrics::CAPTION_LINE_HEIGHT))
                .font_weight(FontWeight::MEDIUM)
                .text_color(color)
                .child(label.into()),
        )
}

pub fn status_badge_spinning(label: impl Into<SharedString>, color: Hsla) -> Div {
    use gpui_kit::component::{spinner::Spinner, Sizable as _};
    div()
        .flex_none()
        .px(px(metrics::SPACE_SM))
        .py(px(metrics::ROW_INSET_Y_COMPACT))
        .flex()
        .items_center()
        .gap(px(metrics::SPACE_XS))
        .rounded(px(metrics::TOOLTIP_RADIUS))
        .bg(color.opacity(0.14))
        .child(Spinner::new().xsmall().color(color))
        .child(
            div()
                .text_size(px(metrics::CAPTION_SIZE))
                .line_height(px(metrics::CAPTION_SIZE * metrics::CAPTION_LINE_HEIGHT))
                .font_weight(FontWeight::MEDIUM)
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
    type DateCache = (
        std::time::Instant,
        chrono::NaiveDate,
        i32,
        std::collections::HashMap<(i64, bool), String>,
    );
    thread_local! {
        static DATES: std::cell::RefCell<Option<DateCache>> = const { std::cell::RefCell::new(None) };
    }
    DATES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache
            .as_ref()
            .is_none_or(|(at, ..)| at.elapsed() >= std::time::Duration::from_secs(1))
        {
            let now = Local::now();
            let today = now.date_naive();
            let offset = now.offset().local_minus_utc();
            if let Some((at, previous_day, previous_offset, values)) = cache.as_mut() {
                if *previous_day != today || *previous_offset != offset {
                    values.clear();
                }
                *at = std::time::Instant::now();
                *previous_day = today;
                *previous_offset = offset;
            } else {
                *cache = Some((std::time::Instant::now(), today, offset, Default::default()));
            }
        }
        let (_, today, _, values) = cache.as_mut().unwrap();
        if let Some(value) = values.get(&(milliseconds, compact)) {
            return value.clone();
        }
        // Resolve the timestamp's historical offset once, including DST. Using
        // today's offset for old clipboard records would display the wrong time.
        let Some(date) = Local.timestamp_millis_opt(milliseconds).single() else {
            return String::new();
        };
        let value = if compact && date.date_naive() == *today {
            date.format("%H:%M").to_string()
        } else {
            date.format("%x · %H:%M").to_string()
        };
        if values.len() >= 256 {
            values.clear();
        }
        values.insert((milliseconds, compact), value.clone());
        value
    })
}
