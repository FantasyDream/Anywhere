use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn render_app_history(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected_source = self
            .app_history
            .selected_source
            .as_deref()
            .and_then(|selected| {
                self.app_history
                    .sources
                    .iter()
                    .find(|source| source.id == selected)
            });
        let page_title = selected_source
            .map(|source| source.title.clone())
            .unwrap_or_else(|| l10n::app_history(self.locale));
        let content = if self.app_history.items.is_empty() {
            self.empty_state_with_icon(
                IconName::Clock,
                if self.app_history.loading {
                    l10n::loading(self.locale)
                } else {
                    l10n::no_matching_entries(self.locale)
                },
                self.app_history.notice_text(self.locale),
            )
        } else {
            let mut list = div()
                .id("application-history-list")
                .flex_1()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .px(px(10.))
                .py(px(3.))
                .track_scroll(&self.history_scroll)
                .overflow_y_scroll()
                .vertical_scrollbar(&self.history_scroll);
            for (index, item) in self.app_history.items.clone().into_iter().enumerate() {
                let selected = self.app_history.selected == index;
                let can_open = item.can_open;
                let item_id = item.id.clone();
                list = list.child(
                    div()
                        .id(SharedString::from(format!("history-item-{item_id}")))
                        .role(gpui_kit::Role::ListBoxOption)
                        .aria_label(item.title.clone())
                        .aria_selected(selected)
                        .h(px(64.))
                        .flex_shrink_0()
                        .py(px(3.))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.app_history.selected = index;
                            this.app_history.close_object_actions();
                            this.notice = None;
                            cx.notify();
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, _, _, cx| {
                                this.app_history.selected = index;
                                this.notice = None;
                                if can_open {
                                    this.request_app_history_object_actions(cx);
                                } else {
                                    this.toggle_fallback_app_history_actions(cx);
                                }
                            }),
                        )
                        .on_double_click(cx.listener(move |this, _, _, cx| {
                            this.app_history.selected = index;
                            if can_open {
                                this.open_selected_history_item(cx);
                            } else {
                                this.copy_selected_history_item(cx);
                            }
                        }))
                        .child(
                            div()
                                .flex()
                                .h_full()
                                .px(px(12.))
                                .items_center()
                                .result_surface(selected)
                                .child(
                                    Icon::new(match item.kind {
                                        "directory" => IconName::Folder,
                                        "workspace" => IconName::Code,
                                        "remote" => IconName::Globe,
                                        _ => IconName::File,
                                    })
                                    .size(px(23.))
                                    .text_color(
                                        if item.kind == "directory" {
                                            theme::icon_blue()
                                        } else {
                                            theme::icon_purple()
                                        },
                                    ),
                                )
                                .child(div().w(px(12.)))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .flex()
                                        .flex_col()
                                        .justify_center()
                                        .gap(px(3.))
                                        .child(
                                            div()
                                                .truncate()
                                                .body_text()
                                                .text_color(theme::primary())
                                                .child(item.title),
                                        )
                                        .child(
                                            div()
                                                .truncate()
                                                .caption_text()
                                                .text_color(theme::secondary())
                                                .child(item.value),
                                        ),
                                )
                                .when(selected, |element| {
                                    element
                                        .child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "history-open-{item_id}"
                                                )))
                                                .role(gpui_kit::Role::Button)
                                                .aria_label(if can_open {
                                                    l10n::open(self.locale)
                                                } else {
                                                    l10n::copy(self.locale)
                                                })
                                                .h(px(metrics::CONTROL_HEIGHT))
                                                .flex_none()
                                                .px(px(metrics::CONTROL_PADDING_X))
                                                .flex()
                                                .items_center()
                                                .gap(px(5.))
                                                .rounded(px(6.))
                                                .hover(|style| style.bg(theme::hover()))
                                                .cursor_pointer()
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.app_history.selected = index;
                                                    if can_open {
                                                        this.open_selected_history_item(cx);
                                                    } else {
                                                        this.copy_selected_history_item(cx);
                                                    }
                                                }))
                                                .child(if can_open {
                                                    l10n::open(self.locale)
                                                } else {
                                                    l10n::copy(self.locale)
                                                })
                                                .when(!can_open, |element| {
                                                    element.child(
                                                        Icon::new(IconName::CornerDownLeft)
                                                            .size(px(12.))
                                                            .text_color(theme::secondary()),
                                                    )
                                                }),
                                        )
                                        .child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "history-actions-{item_id}"
                                                )))
                                                .test_support()
                                                .role(gpui_kit::Role::Button)
                                                .aria_label(l10n::actions(self.locale))
                                                .h(px(metrics::CONTROL_HEIGHT))
                                                .flex_none()
                                                .px(px(metrics::CONTROL_PADDING_X))
                                                .flex()
                                                .items_center()
                                                .gap(px(4.))
                                                .rounded(px(6.))
                                                .hover(|style| style.bg(theme::hover()))
                                                .cursor_pointer()
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.app_history.selected = index;
                                                    if can_open {
                                                        this.request_app_history_object_actions(cx);
                                                    } else {
                                                        this.toggle_fallback_app_history_actions(
                                                            cx,
                                                        );
                                                    }
                                                }))
                                                .child(
                                                    Icon::new(IconName::Ellipsis)
                                                        .size(px(14.))
                                                        .text_color(theme::secondary()),
                                                ),
                                        )
                                }),
                        ),
                );
            }
            list.into_any_element()
        };
        let object_actions_panel = self.app_history.object_actions_active.then(|| {
            let language = match self.locale {
                Locale::En => "en",
                Locale::Zh => "zh",
            };
            let selected_title = self
                .app_history
                .selected_item()
                .map(|item| item.title.clone())
                .unwrap_or_default();
            let actions = self.app_history.object_actions.clone();
            let selected_action = self.app_history.object_action_selected;
            let mut rows = div().flex().flex_col().gap(px(2.)).p(px(4.));
            for (index, action) in actions.into_iter().enumerate() {
                let title = action
                    .localized_titles
                    .get(language)
                    .cloned()
                    .unwrap_or(action.title);
                rows = rows.child(
                    Button::new(SharedString::from(format!("history-object-action-{index}")))
                        .when(index == selected_action, |button| button.secondary())
                        .when(index != selected_action, |button| button.ghost())
                        .label(title)
                        .disabled(!action.enabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.app_history.object_action_selected = index;
                            if this.app_history.selected_object_action().is_none() {
                                return;
                            }
                            this.execute_selected_app_history_action(cx);
                            cx.notify();
                        })),
                );
            }
            let fallback_copy = self.app_history.object_actions_fallback_copy();
            if fallback_copy {
                rows = rows.child(
                    Button::new("history-object-action-copy")
                        .when(selected_action == 0, |button| button.secondary())
                        .when(selected_action != 0, |button| button.ghost())
                        .label(l10n::app_history_copy_target(self.locale))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.app_history.object_action_selected = 0;
                            this.execute_selected_app_history_action(cx);
                            cx.notify();
                        })),
                );
            }
            if !self.app_history.object_actions_loading {
                let refresh_index =
                    self.app_history.object_actions.len() + usize::from(fallback_copy);
                rows = rows.child(
                    Button::new("history-object-action-refresh")
                        .when(selected_action == refresh_index, |button| {
                            button.secondary()
                        })
                        .when(selected_action != refresh_index, |button| button.ghost())
                        .label(l10n::refresh(self.locale))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.app_history.object_action_selected =
                                this.app_history.object_actions.len();
                            this.execute_selected_app_history_action(cx);
                            cx.notify();
                        })),
                );
            }
            div()
                .debug_selector(|| "history-object-actions".into())
                .w(px(300.))
                .max_h(px(340.))
                .overflow_y_scrollbar()
                .rounded(px(metrics::CARD_RADIUS))
                .bg(theme::menu_surface())
                .border(px(metrics::DIVIDER_WIDTH))
                .border_color(theme::selection_outline())
                .child(
                    div()
                        .px(px(metrics::SPACE_MD))
                        .py(px(metrics::SPACE_SM))
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_SM))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .truncate()
                                        .body_text()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(selected_title),
                                )
                                .child(
                                    div()
                                        .caption_text()
                                        .text_color(theme::muted())
                                        .child(l10n::actions(self.locale)),
                                ),
                        )
                        .child(self.keycap("esc")),
                )
                .child(div().h(px(0.5)).bg(theme::divider()))
                .when(self.app_history.object_actions_loading, |panel| {
                    panel.child(
                        div()
                            .p(px(metrics::SPACE_MD))
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::muted())
                            .child(l10n::loading(self.locale)),
                    )
                })
                .child(rows)
                .into_any_element()
        });
        let empty_actions_panel = self.app_history.empty_actions_open.then(|| {
            div()
                .debug_selector(|| "app-history-empty-actions-panel".into())
                .w(px(260.))
                .overflow_hidden()
                .rounded(px(metrics::CARD_RADIUS))
                .bg(theme::menu_surface())
                .border(px(metrics::DIVIDER_WIDTH))
                .border_color(theme::selection_outline())
                .child(
                    div()
                        .px(px(metrics::SPACE_MD))
                        .py(px(metrics::SPACE_SM))
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_SM))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .text_size(px(metrics::BODY_SIZE))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(l10n::actions(self.locale)),
                        )
                        .child(self.keycap("esc")),
                )
                .child(div().h(px(metrics::DIVIDER_WIDTH)).bg(theme::divider()))
                .child(
                    div().p(px(4.)).child(
                        Button::new("app-history-empty-refresh")
                            .secondary()
                            .label(l10n::refresh(self.locale))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh_empty_app_history(cx);
                            })),
                    ),
                )
                .into_any_element()
        });
        let fallback_actions_panel = self.app_history.fallback_actions_open.then(|| {
            let labels = [
                l10n::app_history_copy_target(self.locale),
                l10n::refresh(self.locale),
            ];
            div()
                .id("app-history-fallback-actions-panel")
                .debug_selector(|| "app-history-fallback-actions-panel".into())
                .test_support()
                .w(px(260.))
                .overflow_hidden()
                .rounded(px(metrics::CARD_RADIUS))
                .bg(theme::menu_surface())
                .border(px(metrics::DIVIDER_WIDTH))
                .border_color(theme::selection_outline())
                .child(
                    div()
                        .px(px(metrics::SPACE_MD))
                        .py(px(metrics::SPACE_SM))
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_SM))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .text_size(px(metrics::BODY_SIZE))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(l10n::actions(self.locale)),
                        )
                        .child(self.keycap("esc")),
                )
                .child(div().h(px(metrics::DIVIDER_WIDTH)).bg(theme::divider()))
                .child(div().p(px(4.)).flex().flex_col().children(
                    labels.into_iter().enumerate().map(|(index, label)| {
                        Button::new(SharedString::from(format!(
                            "app-history-fallback-action-{index}"
                        )))
                        .when(
                            index == self.app_history.fallback_action_selected,
                            |button| button.secondary(),
                        )
                        .when(
                            index != self.app_history.fallback_action_selected,
                            |button| button.ghost(),
                        )
                        .label(label)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.app_history.fallback_action_selected = index;
                            this.execute_fallback_app_history_action(index, cx);
                        }))
                    }),
                ))
                .into_any_element()
        });
        let empty_actions = self.app_history.items.is_empty().then(|| {
            div()
                .id("app-history-empty-actions")
                .test_support()
                .role(gpui_kit::Role::Button)
                .aria_label(l10n::actions(self.locale))
                .size(px(26.))
                .mr(px(8.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.))
                .hover(|style| style.bg(theme::hover()))
                .cursor_pointer()
                .tooltip(components::text_tooltip(l10n::actions(self.locale)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_empty_app_history_actions(cx);
                }))
                .child(
                    Icon::new(IconName::Ellipsis)
                        .size(px(16.))
                        .text_color(theme::secondary()),
                )
                .into_any_element()
        });
        let page = self
            .render_page_shell_with_trailing(
                SharedString::from(page_title),
                SharedString::default(),
                Some((
                    &self.history_input,
                    "application-history-search",
                    l10n::app_history_search(self.locale),
                )),
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .child(content)
                    .into_any_element(),
                empty_actions,
                cx,
            )
            .into_any_element();
        div()
            .size_full()
            .relative()
            .child(page)
            .children(object_actions_panel.map(|panel| {
                Self::page_popup(
                    window,
                    px(metrics::PAGE_HEADER_HEIGHT + metrics::DIVIDER_WIDTH + 52.),
                    px(12.),
                    panel,
                )
            }))
            .children(empty_actions_panel.map(|panel| {
                Self::page_popup(
                    window,
                    px(metrics::PAGE_HEADER_HEIGHT + metrics::DIVIDER_WIDTH + 52.),
                    px(12.),
                    panel,
                )
            }))
            .children(fallback_actions_panel.map(|panel| {
                Self::page_popup(
                    window,
                    px(metrics::PAGE_HEADER_HEIGHT + metrics::DIVIDER_WIDTH + 52.),
                    px(12.),
                    panel,
                )
            }))
            .into_any_element()
    }
}
