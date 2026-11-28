use gpui_kit::{
    div, prelude::*, px, AnyElement, Div, FontWeight, Hsla, IntoElement, SharedString, Stateful,
    TestSupportExt as _,
};

use super::text_tooltip;
use crate::ui::{design_tokens::metrics, styles::AppStyle as _, theme};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowDensity {
    Regular,
    Comfortable,
    Compact,
}

impl RowDensity {
    pub fn height(self) -> f32 {
        match self {
            Self::Regular => metrics::ROW_HEIGHT,
            Self::Comfortable => metrics::ROW_HEIGHT_COMFORTABLE,
            Self::Compact => metrics::ROW_HEIGHT_COMPACT,
        }
    }

    pub fn inset_y(self) -> f32 {
        match self {
            Self::Regular => metrics::ROW_INSET_Y_REGULAR,
            Self::Comfortable => metrics::ROW_INSET_Y_COMFORTABLE,
            Self::Compact => metrics::ROW_INSET_Y_COMPACT,
        }
    }

    fn default_leading_gap(self) -> f32 {
        match self {
            Self::Regular | Self::Comfortable => metrics::ROW_ICON_GAP,
            Self::Compact => metrics::SPACE_MD,
        }
    }

    fn default_text_gap(self) -> f32 {
        match self {
            Self::Comfortable => 3.0,
            Self::Regular | Self::Compact => 2.0,
        }
    }

    fn title_size(self) -> f32 {
        match self {
            Self::Regular | Self::Comfortable => metrics::ROW_TITLE_REGULAR,
            Self::Compact => metrics::ROW_TITLE_COMPACT,
        }
    }

    fn title_line_height(self) -> f32 {
        self.title_size() * metrics::BODY_LINE_HEIGHT
    }

    fn title_weight(self) -> FontWeight {
        match self {
            Self::Regular => FontWeight::SEMIBOLD,
            Self::Comfortable | Self::Compact => FontWeight::MEDIUM,
        }
    }

    fn subtitle_size(self) -> f32 {
        match self {
            Self::Regular | Self::Comfortable => metrics::ROW_SUBTITLE_REGULAR,
            Self::Compact => metrics::ROW_SUBTITLE_COMPACT,
        }
    }

    fn subtitle_line_height(self) -> f32 {
        self.subtitle_size() * metrics::CAPTION_LINE_HEIGHT
    }
}

pub struct ListRow {
    id: SharedString,
    density: RowDensity,
    selected: bool,
    enabled: bool,
    aria_label: Option<SharedString>,
    leading: Option<AnyElement>,
    title: Option<SharedString>,
    title_highlight_query: Option<String>,
    title_id: Option<SharedString>,
    title_weight: Option<FontWeight>,
    title_tooltip: Option<SharedString>,
    subtitle: Option<SharedString>,
    source_label: Option<SharedString>,
    subtitle_id: Option<SharedString>,
    subtitle_color: Option<Hsla>,
    subtitle_tooltip: Option<SharedString>,
    trailing: Vec<AnyElement>,
}

impl ListRow {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            density: RowDensity::Regular,
            selected: false,
            enabled: true,
            aria_label: None,
            leading: None,
            title: None,
            title_highlight_query: None,
            title_id: None,
            title_weight: None,
            title_tooltip: None,
            subtitle: None,
            source_label: None,
            subtitle_id: None,
            subtitle_color: None,
            subtitle_tooltip: None,
            trailing: Vec::new(),
        }
    }

    pub fn density(mut self, density: RowDensity) -> Self {
        self.density = density;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading = Some(element.into_any_element());
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn title_highlight_query(mut self, query: impl Into<String>) -> Self {
        let query = query.into();
        if !query.trim().is_empty() {
            self.title_highlight_query = Some(query);
        }
        self
    }

    pub fn title_id(mut self, id: impl Into<SharedString>) -> Self {
        self.title_id = Some(id.into());
        self
    }

    pub fn title_weight(mut self, weight: FontWeight) -> Self {
        self.title_weight = Some(weight);
        self
    }

    pub fn title_tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.title_tooltip = Some(tooltip.into());
        self
    }

    pub fn source_label(mut self, label: SharedString) -> Self {
        self.source_label = Some(label);
        self
    }

    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn subtitle_id(mut self, id: impl Into<SharedString>) -> Self {
        self.subtitle_id = Some(id.into());
        self
    }

    pub fn subtitle_color(mut self, color: Hsla) -> Self {
        self.subtitle_color = Some(color);
        self
    }

    pub fn subtitle_tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.subtitle_tooltip = Some(tooltip.into());
        self
    }

    pub fn trailing(mut self, element: impl IntoElement) -> Self {
        self.trailing.push(element.into_any_element());
        self
    }

    pub fn build(self) -> Stateful<Div> {
        let leading_gap = self.density.default_leading_gap();
        let text_gap = self.density.default_text_gap();
        let aria_label = self
            .aria_label
            .or_else(|| self.title.clone())
            .unwrap_or_default();
        let description = match (&self.source_label, &self.subtitle) {
            (Some(source), Some(subtitle)) => Some(format!("{source} · {subtitle}").into()),
            (Some(source), None) => Some(source.clone()),
            (None, subtitle) => subtitle.clone(),
        };
        let title_color = if self.enabled {
            theme::primary()
        } else {
            theme::muted()
        };
        let subtitle_color = self.subtitle_color.unwrap_or_else(|| {
            if self.enabled {
                theme::secondary()
            } else {
                theme::warning()
            }
        });
        let highlight_query = self.title_highlight_query.as_deref();
        let title_element = self.title.map(|title| {
            let content = match highlight_query {
                Some(query) if self.enabled => {
                    super::highlighted_text(title, query, theme::accent())
                }
                _ => title.into_any_element(),
            };
            let node = div()
                .truncate()
                .text_size(px(self.density.title_size()))
                .line_height(px(self.density.title_line_height()))
                .font_weight(
                    self.title_weight
                        .unwrap_or_else(|| self.density.title_weight()),
                )
                .text_color(title_color)
                .child(content);
            let id = self
                .title_id
                .unwrap_or_else(|| format!("{}-title", self.id).into());
            let node = node.id(id).test_support();
            if let Some(tooltip) = self.title_tooltip {
                node.tooltip(text_tooltip(tooltip)).into_any_element()
            } else {
                node.into_any_element()
            }
        });
        let subtitle_element = description.as_ref().map(|_| {
            let has_subtitle = self.subtitle.is_some();
            let has_source = self.source_label.is_some();
            let node = div()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(metrics::SPACE_XS))
                .text_size(px(self.density.subtitle_size()))
                .line_height(px(self.density.subtitle_line_height()))
                .text_color(subtitle_color)
                .when_some(self.source_label, |row, label| {
                    row.child(
                        div()
                            .max_w_1_3()
                            .min_w_0()
                            .truncate()
                            .text_color(theme::muted())
                            .child(label),
                    )
                })
                .when(has_source && has_subtitle, |row| {
                    row.child(div().flex_none().text_color(theme::muted()).child("·"))
                })
                .when_some(self.subtitle, |row, subtitle| {
                    row.child(div().min_w_0().truncate().child(subtitle))
                });
            let id = self
                .subtitle_id
                .unwrap_or_else(|| format!("{}-subtitle", self.id).into());
            let node = node.id(id).test_support();
            if let Some(tooltip) = self.subtitle_tooltip {
                node.tooltip(text_tooltip(tooltip)).into_any_element()
            } else {
                node.into_any_element()
            }
        });
        let center = div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(text_gap))
            .children(title_element)
            .children(subtitle_element)
            .into_any_element();
        let mut surface = div()
            .h_full()
            .relative()
            .px(px(metrics::ROW_PADDING_X - metrics::DIVIDER_WIDTH))
            .flex()
            .items_center()
            .result_surface(self.selected);
        if self.selected && self.enabled {
            surface = surface.child(
                div()
                    .absolute()
                    .left(px(metrics::ROW_INSET_Y_REGULAR))
                    .w(px(metrics::ROW_ACCENT_INDICATOR_WIDTH))
                    .h(px(metrics::ROW_ACCENT_INDICATOR_HEIGHT))
                    .rounded(px(metrics::ROW_ACCENT_INDICATOR_WIDTH))
                    .bg(theme::accent()),
            );
        }
        if let Some(leading) = self.leading {
            surface = surface
                .child(
                    div()
                        .flex_none()
                        .when(self.density != RowDensity::Compact, |slot| {
                            slot.w(px(metrics::ROW_ICON_SLOT))
                                .flex()
                                .items_center()
                                .justify_center()
                        })
                        .child(leading),
                )
                .child(div().w(px(leading_gap)));
        }
        surface = surface.child(center);
        for trailing in self.trailing {
            surface = surface.child(div().flex_none().child(trailing));
        }
        div()
            .id(self.id)
            .role(gpui_kit::Role::ListBoxOption)
            .aria_label(aria_label)
            .aria_selected(self.selected)
            .when_some(description, |row, description| {
                row.aria_description(description)
            })
            .h(px(self.density.height()))
            .flex_shrink_0()
            .py(px(self.density.inset_y()))
            .when(self.enabled, |row| row.cursor_pointer())
            .when(!self.enabled, |row| row.cursor_not_allowed())
            .child(surface)
    }
}

pub fn row_action_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .role(gpui_kit::Role::Button)
        .aria_label(label.into())
        .h(px(metrics::CONTROL_HEIGHT))
        .flex_none()
        .px(px(metrics::CONTROL_PADDING_X))
        .flex()
        .items_center()
        .gap(px(metrics::SPACE_XS))
        .rounded(px(metrics::TOOLTIP_RADIUS))
        .text_size(px(metrics::CONTROL_FONT_SIZE))
        .line_height(px(metrics::CONTROL_FONT_SIZE * metrics::BODY_LINE_HEIGHT))
        .text_color(theme::secondary())
        .hover(|style| style.bg(theme::hover()))
        .active(|style| style.bg(theme::selected()))
        .cursor_pointer()
}

pub fn primary_row_action(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
) -> Stateful<Div> {
    row_action_button(id, label)
        .gap(px(metrics::SPACE_XS))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme::primary())
        .bg(theme::field())
        .border(px(metrics::DIVIDER_WIDTH))
        .border_color(theme::control_border())
}
