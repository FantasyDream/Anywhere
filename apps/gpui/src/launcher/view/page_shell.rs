use super::*;
use gpui_kit::component::ActiveTheme as _;

const CAPTION_BUTTON_WIDTH: f32 = 40.0;
const CAPTION_ICON_SIZE: f32 = 12.0;

#[derive(IntoElement)]
struct WindowTitlebar {
    view: Entity<LauncherView>,
    locale: Locale,
    is_settings: bool,
}

impl RenderOnce for WindowTitlebar {
    fn render(self, window: &mut Window, cx: &mut gpui_kit::App) -> impl IntoElement {
        let resizable = !self.is_settings;
        let drag_band = div()
            .flex_1()
            .h_full()
            .window_control_area(gpui_kit::WindowControlArea::Drag)
            .on_mouse_down(MouseButton::Left, move |event, window, _| {
                if resizable && event.click_count >= 2 {
                    crate::platform::toggle_window_size(window);
                } else {
                    window.start_window_move();
                }
            });
        let mut bar = div()
            .id("tool-window-titlebar")
            .test_support()
            .h(px(metrics::TITLE_BAR_INSET))
            .w_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .child(drag_band);

        if crate::platform::WINDOW_CONTROLS
            != crate::platform::WindowControlsStyle::TrailingCaptionButtons
        {
            return bar;
        }

        let minimize_label = l10n::minimize_window(self.locale);
        let resize_label = l10n::toggle_window_size(self.locale);
        let close_label = l10n::close(self.locale);
        let resize_icon = if crate::platform::is_window_maximized(window) {
            IconName::WindowRestore
        } else {
            IconName::WindowMaximize
        };
        let close_hover_bg = cx.theme().danger;
        let close_active_bg = cx.theme().danger_active;
        let close_hover_fg = cx.theme().danger_foreground;
        let close_view = self.view.clone();
        let is_settings = self.is_settings;

        let minimize_button = div()
            .id("product-window-minimize")
            .test_support()
            .w(px(CAPTION_BUTTON_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .text_color(theme::secondary())
            .hover(|style| style.bg(theme::hover()).text_color(theme::primary()))
            .active(|style| style.bg(theme::selected()).text_color(theme::primary()))
            .tooltip(components::text_tooltip(minimize_label))
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_click(|_, window, cx| {
                cx.stop_propagation();
                window.minimize_window();
            })
            .child(Icon::new(IconName::WindowMinimize).size(px(CAPTION_ICON_SIZE)));

        let close_button = div()
            .id("product-window-close")
            .test_support()
            .w(px(CAPTION_BUTTON_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .text_color(theme::secondary())
            .hover(move |style| style.bg(close_hover_bg).text_color(close_hover_fg))
            .active(move |style| style.bg(close_active_bg).text_color(close_hover_fg))
            .tooltip(components::text_tooltip(close_label))
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                close_view.update(cx, |this, cx| {
                    if is_settings {
                        this.request_dismiss_settings(window, cx);
                    } else {
                        this.go_back(window, cx);
                    }
                });
            })
            .child(Icon::new(IconName::WindowClose).size(px(CAPTION_ICON_SIZE)));

        let mut controls = div()
            .flex_shrink_0()
            .h_full()
            .flex()
            .items_center()
            .child(minimize_button);
        if resizable {
            let resize_button = div()
                .id("product-window-resize")
                .test_support()
                .w(px(CAPTION_BUTTON_WIDTH))
                .h_full()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme::secondary())
                .hover(|style| style.bg(theme::hover()).text_color(theme::primary()))
                .active(|style| style.bg(theme::selected()).text_color(theme::primary()))
                .tooltip(components::text_tooltip(resize_label))
                .on_mouse_down(MouseButton::Left, |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(|_, window, cx| {
                    cx.stop_propagation();
                    crate::platform::toggle_window_size(window);
                })
                .child(Icon::new(resize_icon).size(px(CAPTION_ICON_SIZE)));
            controls = controls.child(resize_button);
        }
        bar = bar.child(controls.child(close_button));
        bar
    }
}

impl LauncherView {
    pub(super) fn empty_page_state(
        &self,
        title: impl Into<SharedString>,
        detail: Option<String>,
    ) -> AnyElement {
        self.empty_state_with_icon(IconName::Search, title, detail)
    }

    pub(super) fn empty_state_with_icon(
        &self,
        icon: IconName,
        title: impl Into<SharedString>,
        detail: Option<String>,
    ) -> AnyElement {
        components::empty_state(icon, title, detail).into_any_element()
    }

    pub(super) fn render_page_shell(
        &self,
        title: SharedString,
        subtitle: SharedString,
        search: Option<(&Entity<InputState>, &'static str, String)>,
        content: AnyElement,
        cx: &mut Context<Self>,
    ) -> Div {
        self.render_page_shell_with_trailing(title, subtitle, search, content, None, cx)
    }

    pub(super) fn render_page_shell_with_trailing(
        &self,
        title: SharedString,
        subtitle: SharedString,
        search: Option<(&Entity<InputState>, &'static str, String)>,
        content: AnyElement,
        trailing: Option<AnyElement>,
        cx: &mut Context<Self>,
    ) -> Div {
        let title_area = if let Some((input, id, placeholder)) =
            search.filter(|_| !(self.screen == Screen::FileSearch && self.page_actions_visible()))
        {
            self.page_search_title(title, input, id, placeholder)
        } else {
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .justify_center()
                .gap(px(metrics::PAGE_TITLE_GAP))
                .child(
                    div()
                        .truncate()
                        .text_size(px(metrics::PAGE_HEADING_SIZE))
                        .line_height(px(metrics::PAGE_HEADING_SIZE * metrics::BODY_LINE_HEIGHT))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::primary())
                        .child(title),
                )
                .when(!subtitle.is_empty(), |element| {
                    element.child(
                        div()
                            .truncate()
                            .caption_text()
                            .text_color(theme::muted())
                            .child(subtitle),
                    )
                })
        };
        let notice = self.notice.clone();
        let notice_kind = self.notice_kind;
        div()
            .size_full()
            .flex()
            .flex_col()
            .when(
                self.standalone_tasks || self.standalone_tool || self.is_settings_surface(),
                |element| element.child(self.render_window_titlebar(cx)),
            )
            .child(self.render_page_header(title_area, trailing, cx))
            .child(if self.standalone_tool {
                Self::full_width_divider()
            } else {
                Self::page_header_divider()
            })
            .when_some(notice, |element, notice| {
                element.child(
                    div()
                        .px(px(metrics::SPACE_XXL))
                        .pt(px(metrics::SPACE_MD))
                        .child(components::notice(notice, notice_kind)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.))
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(content),
            )
    }

    pub(super) fn render_action_overlay(&self, panel: AnyElement, cx: &Context<Self>) -> Div {
        let titlebar = if self.standalone_tasks || self.standalone_tool {
            metrics::TITLE_BAR_INSET
        } else {
            0.
        };
        div()
            .absolute()
            .inset_0()
            .child(
                div()
                    .id("page-action-dismiss")
                    .test_support()
                    .absolute()
                    .inset_0()
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.handle_key("escape", KeyModifiers::default(), window, cx);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.handle_key("escape", KeyModifiers::default(), window, cx);
                        }),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top(px(titlebar
                        + metrics::PAGE_HEADER_HEIGHT
                        + metrics::SPACE_XS))
                    .right(px(metrics::SPACE_XL))
                    .max_w_full()
                    .child(panel),
            )
    }

    pub(super) fn render_window_titlebar(&self, cx: &Context<Self>) -> impl IntoElement {
        WindowTitlebar {
            view: cx.entity(),
            locale: self.locale,
            is_settings: self.is_settings_surface(),
        }
    }

    pub(super) fn page_search_title(
        &self,
        title: SharedString,
        input: &Entity<InputState>,
        id: &'static str,
        placeholder: String,
    ) -> Div {
        let file_search = self.screen == Screen::FileSearch;
        let label = div()
            .truncate()
            .text_size(px(metrics::PAGE_MODE_SIZE))
            .line_height(px(metrics::PAGE_MODE_SIZE * metrics::CAPTION_LINE_HEIGHT))
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme::secondary())
            .child(title);
        div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(metrics::PAGE_TITLE_GAP))
            .child(if file_search {
                label
                    .id("launcher-mode-label")
                    .test_support()
                    .into_any_element()
            } else {
                label.into_any_element()
            })
            .child(
                Input::new(input)
                    .id(id)
                    .aria_label(placeholder)
                    .appearance(false)
                    .bordered(false)
                    .focus_bordered(false)
                    .cleanable(true)
                    .text_size(px(metrics::PAGE_HEADING_SIZE))
                    .line_height(px(metrics::PAGE_HEADING_SIZE * metrics::BODY_LINE_HEIGHT))
                    .p(px(0.))
                    .w_full(),
            )
    }

    pub(super) fn render_page_header(
        &self,
        title_area: Div,
        trailing: Option<AnyElement>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let standalone = self.standalone_tasks || self.standalone_shelf || self.standalone_tool;
        let frameless_tool = self.standalone_tool;
        let file_search = self.screen == Screen::FileSearch;
        let back_label = if standalone {
            l10n::close(self.locale)
        } else {
            l10n::back(self.locale)
        };
        let leading: AnyElement = if frameless_tool {
            components::squircle_icon_plate(IconName::Puzzle, theme::accent(), metrics::ICON_SIZE)
                .into_any_element()
        } else {
            Button::new(if file_search {
                "file-search-back"
            } else {
                "page-back"
            })
            .ghost()
            .icon(if standalone {
                IconName::X
            } else {
                IconName::ArrowLeft
            })
            .accessibility_label(back_label.clone())
            .tooltip(back_label)
            .on_click(cx.listener(|this, _, window, cx| {
                if this.page_actions_visible() {
                    this.handle_key("escape", KeyModifiers::default(), window, cx);
                    this.focus_active_editor(window, cx);
                } else {
                    this.go_back(window, cx);
                }
            }))
            .into_any_element()
        };
        let header_title: AnyElement = if frameless_tool {
            div()
                .id("tool-window-drag-header")
                .flex_1()
                .min_w(px(0.))
                .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                .child(title_area)
                .into_any_element()
        } else {
            title_area.into_any_element()
        };
        div()
            .id(if file_search {
                "file-search-header"
            } else {
                "page-header"
            })
            .test_support()
            .h(px(metrics::PAGE_HEADER_HEIGHT))
            .flex_shrink_0()
            .px(px(metrics::SPACE_XL))
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_MD))
            .child(leading)
            .child(header_title)
            .children(trailing)
            .when(self.can_open_tool_window(), |header| {
                let label = l10n::open_tool_window_copy(self.locale);
                header.child(
                    Button::new("page-open-window")
                        .ghost()
                        .icon(IconName::ExternalLink)
                        .accessibility_label(label.clone())
                        .tooltip(label)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_tool_window(window, cx)),
                        ),
                )
            })
            .child(
                div()
                    .id("page-escape")
                    .test_support()
                    .flex_none()
                    .cursor_pointer()
                    .rounded(px(metrics::KEYCAP_RADIUS))
                    .tooltip(components::text_tooltip(if standalone {
                        l10n::close_esc(self.locale)
                    } else {
                        l10n::back_esc(self.locale)
                    }))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.handle_key("escape", KeyModifiers::default(), window, cx)
                    }))
                    .child(self.keycap("esc")),
            )
    }

    fn full_width_divider() -> Div {
        div()
            .h(px(metrics::DIVIDER_WIDTH))
            .flex_shrink_0()
            .bg(theme::divider())
    }

    pub(super) fn page_header_divider() -> Div {
        Self::full_width_divider().mx(px(metrics::SPACE_XL))
    }

    pub(super) fn scope_button(
        &self,
        scope: Option<String>,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected_scope = scope
            .as_deref()
            .and_then(|id| self.model.scopes.iter().find(|item| item.id == id));
        let label = selected_scope
            .map(|item| item.label.clone())
            .unwrap_or_else(|| l10n::all_types(self.locale));
        let tooltip = if selected_scope.is_some_and(|scope| scope.is_current_app) {
            l10n::shortcut_hint(
                self.locale,
                &l10n::all_actions_available_in_the_current_app(self.locale),
                "Tab / Shift+Tab",
            )
        } else {
            l10n::shortcut_hint(
                self.locale,
                &l10n::filter_actions(self.locale, &label),
                "Tab / Shift+Tab",
            )
        };
        let id = SharedString::from(format!("scope-{}", scope.as_deref().unwrap_or("all")));
        components::filter_chip(id, label, selected)
            .mx(px(metrics::ROW_INSET_Y_COMPACT))
            .tooltip(components::text_tooltip(tooltip))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.mark_scope_user_controlled();
                this.model.select_scope(scope.clone());
                this.reveal_selected_scope();
                this.request_live_update(cx);
                this.clear_notice();
                this.resize_current_window(cx);
                cx.notify();
            }))
    }

    pub(super) fn file_filter_button(
        &self,
        filter: FileSearchFilter,
        label: String,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        components::filter_chip(
            SharedString::from(format!("file-filter-{}", file_filter_id(filter))),
            label,
            selected,
        )
        .mx(px(metrics::ROW_INSET_Y_COMPACT))
        .tooltip(components::text_tooltip(l10n::search_this_file_type(
            self.locale,
        )))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.select_file_filter(filter, cx);
        }))
    }
}
