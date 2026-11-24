use super::*;

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
            search.filter(|_| !self.page_actions_visible())
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
            // Tasks keeps native traffic lights. Give the first header row its
            // own titlebar band so the close icon and labels never sit below
            // the red/yellow/green controls.
            .when(self.standalone_tasks, |element| {
                element.pt(px(metrics::TITLE_BAR_INSET))
            })
            // Standalone Web tool windows use a frameless transparent titlebar
            // with native traffic lights; reserve a draggable band above the header.
            .when(self.standalone_tool, |element| {
                element.child(
                    div()
                        .id("tool-window-titlebar")
                        .test_support()
                        .h(px(metrics::TITLE_BAR_INSET))
                        .w_full()
                        .flex_shrink_0()
                        .on_mouse_down(MouseButton::Left, |_, window, _| {
                            window.start_window_move()
                        }),
                )
            })
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
                    .cleanable(!file_search)
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
        let standalone = (self.standalone_tasks || self.standalone_shelf || self.standalone_tool)
            && !self.page_actions_visible();
        let frameless_tool = self.standalone_tool && !self.page_actions_visible();
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
        div()
            .id(id)
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(label.clone())
            .aria_selected(selected)
            .h(px(metrics::FILTER_HEIGHT))
            .px(px(metrics::SPACE_MD))
            .mx(px(3.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
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
            .when(selected, |element| element.selected_surface())
            .when(!selected, |element| {
                element.hover(|style| style.bg(theme::hover()))
            })
            .cursor_pointer()
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
            .child(label)
    }

    pub(super) fn file_filter_button(
        &self,
        filter: FileSearchFilter,
        label: String,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(format!(
                "file-filter-{}",
                file_filter_id(filter)
            )))
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(label.clone())
            .aria_selected(selected)
            .h(px(metrics::FILTER_HEIGHT))
            .px(px(metrics::SPACE_MD))
            .mx(px(3.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
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
            .when(selected, |element| element.selected_surface())
            .when(!selected, |element| {
                element.hover(|style| style.bg(theme::hover()))
            })
            .cursor_pointer()
            .tooltip(components::text_tooltip(l10n::search_this_file_type(
                self.locale,
            )))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_file_filter(filter, cx);
            }))
            .child(label)
    }
}
