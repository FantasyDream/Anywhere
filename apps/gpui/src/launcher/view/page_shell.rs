use super::*;

impl LauncherView {
    pub(super) fn page_popup(
        window: &Window,
        top: gpui_kit::Pixels,
        right: gpui_kit::Pixels,
        panel: AnyElement,
    ) -> AnyElement {
        gpui_kit::deferred(
            gpui_kit::anchored()
                .anchor(gpui_kit::Anchor::TopRight)
                .position(gpui_kit::point(window.viewport_size().width - right, top))
                .snap_to_window_with_margin(px(metrics::SPACE_SM))
                .child(panel),
        )
        .with_priority(gpui_kit::base::POPUP_PRIORITY)
        .into_any_element()
    }

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
        components::empty_state(icon, title, detail)
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
        let title_area = if let Some((input, id, placeholder)) = search {
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
                        .text_size(px(metrics::PAGE_MODE_SIZE))
                        .line_height(px(metrics::PAGE_MODE_SIZE * metrics::CAPTION_LINE_HEIGHT))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::secondary())
                        .child(title),
                )
                .child(
                    Input::new(input)
                        .id(SharedString::from(id))
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
        let standalone = self.standalone_tasks || self.standalone_shelf;
        let back_label = if standalone {
            l10n::close(self.locale)
        } else {
            l10n::back(self.locale)
        };
        let notice = self.notice.clone();
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
            .child(
                div()
                    .debug_selector(|| "page-header".into())
                    .h(px(metrics::PAGE_HEADER_HEIGHT))
                    .flex_shrink_0()
                    .px(px(metrics::SPACE_XL))
                    .flex()
                    .items_center()
                    .child(
                        Button::new("page-back")
                            .ghost()
                            .icon(if standalone {
                                IconName::X
                            } else {
                                IconName::ArrowLeft
                            })
                            .accessibility_label(back_label.clone())
                            .tooltip(back_label)
                            .on_click(cx.listener(|this, _, window, cx| this.go_back(window, cx))),
                    )
                    .child(div().w(px(metrics::SPACE_MD)))
                    .child(title_area)
                    .child(div().w(px(14.)))
                    .children(trailing)
                    .child(
                        div()
                            .id("page-escape")
                            .test_support()
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
                    ),
            )
            .child(
                div()
                    .h(px(metrics::DIVIDER_WIDTH))
                    .flex_shrink_0()
                    .mx(px(metrics::SPACE_XL))
                    .bg(theme::divider()),
            )
            .when_some(notice, |element, notice| {
                let kind = if self.settings.error.as_deref() == Some(notice.as_str()) {
                    components::NoticeKind::Error
                } else if notice == l10n::copied(self.locale) {
                    components::NoticeKind::Success
                } else {
                    components::NoticeKind::Warning
                };
                element.child(
                    div()
                        .px(px(metrics::SPACE_XXL))
                        .pt(px(metrics::SPACE_MD))
                        .child(components::notice(notice, kind)),
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
                this.notice = None;
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
                this.file_search.filter = filter;
                this.reveal_selected_file_filter();
                this.reset_file_search_browsing(cx);
                this.request_live_update(cx);
                this.notice = None;
                cx.notify();
            }))
            .child(label)
    }
}
