use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn render_clipboard(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let content = if self.clipboard.items().is_empty() {
            let title = if self.clipboard.loading {
                l10n::loading(self.locale)
            } else if self.clipboard.query.trim().is_empty() {
                l10n::no_saved_entries(self.locale)
            } else {
                l10n::no_matching_entries(self.locale)
            };
            let detail =
                (!self.clipboard.loading && self.clipboard.query.trim().is_empty()).then(|| {
                    if self
                        .clipboard
                        .status
                        .as_ref()
                        .is_some_and(|status| status.recording)
                    {
                        l10n::newly_copied_items_will_appear_here_refresh(self.locale)
                    } else {
                        l10n::manage_recording_in_settings_clipboard(self.locale)
                    }
                });
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .p(px(metrics::SPACE_XXL))
                .child(
                    Icon::new(IconName::Clock3)
                        .size(px(30.))
                        .text_color(theme::icon_purple()),
                )
                .child(div().body_text().text_color(theme::primary()).child(title))
                .when_some(detail, |element, detail| {
                    element.child(
                        div()
                            .max_w(px(440.))
                            .text_center()
                            .caption_text()
                            .text_color(theme::muted())
                            .child(detail),
                    )
                })
                .into_any_element()
        } else {
            let mut list = div()
                .id("clipboard-history-list")
                .flex_1()
                .min_h(px(0.))
                .px(px(10.))
                .pb(px(4.))
                .flex()
                .flex_col()
                .track_scroll(&self.clipboard_scroll)
                .overflow_y_scroll()
                .vertical_scrollbar(&self.clipboard_scroll);
            for (index, item) in self.clipboard.items().to_vec().into_iter().enumerate() {
                let selected = self.clipboard.browsing && self.clipboard.selected == index;
                let item_id = item.id;
                let summary = item.summary.clone();
                let copied = selected && self.clipboard.copied_item == Some(item_id);
                let metadata = if copied {
                    l10n::copied(self.locale)
                } else {
                    let kind = components::clipboard_kind_label(self.locale, &item.kind);
                    let date = components::clipboard_copy_date(item.copied_at_ms, false);
                    if date.is_empty() {
                        kind
                    } else {
                        format!("{kind} · {date}")
                    }
                };
                let mut selected_commands = div().flex_none().flex().items_center();
                if selected {
                    let copy_id = item_id;
                    let preview_space =
                        self.clipboard.browsing || self.clipboard.query.trim().is_empty();
                    selected_commands = selected_commands
                        .child(
                            div()
                                .id(SharedString::from(format!("clipboard-preview-{item_id}")))
                                .role(gpui_kit::Role::Button)
                                .aria_label(l10n::preview(self.locale))
                                .tooltip(components::text_tooltip(l10n::shortcut_hint(
                                    self.locale,
                                    &l10n::preview(self.locale),
                                    "␣",
                                )))
                                .h(px(metrics::CONTROL_HEIGHT))
                                .flex_none()
                                .px(px(metrics::CONTROL_PADDING_X))
                                .flex()
                                .items_center()
                                .gap(px(3.))
                                .rounded(px(6.))
                                .hover(|style| style.bg(theme::hover()))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.clipboard.selected = index;
                                    this.clipboard.browsing = true;
                                    this.preview_selected_clipboard(cx);
                                }))
                                .child(
                                    Icon::new(IconName::Eye)
                                        .size(px(14.))
                                        .text_color(theme::secondary()),
                                )
                                .when(preview_space, |element| element.child("␣")),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("clipboard-actions-{item_id}")))
                                .role(gpui_kit::Role::Button)
                                .aria_label(l10n::actions(self.locale))
                                .tooltip(components::text_tooltip(l10n::shortcut_hint(
                                    self.locale,
                                    &l10n::actions(self.locale),
                                    "⌘K",
                                )))
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
                                    this.open_clipboard_actions(item_id, cx);
                                }))
                                .child(
                                    Icon::new(IconName::Ellipsis)
                                        .size(px(14.))
                                        .text_color(theme::secondary()),
                                )
                                .child("⌘K"),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("clipboard-copy-{item_id}")))
                                .role(gpui_kit::Role::Button)
                                .aria_label(l10n::copy(self.locale))
                                .tooltip(components::text_tooltip(l10n::shortcut_hint(
                                    self.locale,
                                    &l10n::copy(self.locale),
                                    "↵",
                                )))
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
                                    if this.select_clipboard_item(copy_id) {
                                        this.copy_selected_clipboard(cx);
                                    }
                                }))
                                .child(l10n::copy(self.locale))
                                .child(
                                    Icon::new(IconName::CornerDownLeft)
                                        .size(px(12.))
                                        .text_color(theme::secondary()),
                                ),
                        );
                }
                list = list.child(
                    div()
                        .id(SharedString::from(format!("clipboard-item-{item_id}")))
                        .role(gpui_kit::Role::ListBoxOption)
                        .aria_label(summary.clone())
                        .aria_selected(selected)
                        .h(px(64.))
                        .flex_shrink_0()
                        .py(px(3.))
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.select_clipboard_row(index, window, cx);
                            }),
                        )
                        .on_double_click(cx.listener(move |this, _, _, cx| {
                            this.clipboard.selected = index;
                            this.clipboard.browsing = true;
                            this.copy_selected_clipboard(cx);
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, _, _, cx| {
                                this.open_clipboard_actions(item_id, cx);
                            }),
                        )
                        .child(
                            div()
                                .h_full()
                                .px(px(12.))
                                .flex()
                                .items_center()
                                .result_surface(selected)
                                .child(
                                    Icon::new(components::clipboard_kind_icon(&item.kind))
                                        .size(px(22.))
                                        .text_color(components::clipboard_kind_tint(&item.kind)),
                                )
                                .child(div().w(px(12.)))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .flex()
                                        .flex_col()
                                        .justify_center()
                                        .gap(px(4.))
                                        .child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "clipboard-title-{item_id}"
                                                )))
                                                .truncate()
                                                .body_text()
                                                .text_color(theme::primary())
                                                .tooltip(components::text_tooltip(summary))
                                                .child(item.summary),
                                        )
                                        .child(
                                            div()
                                                .truncate()
                                                .caption_text()
                                                .text_color(if copied {
                                                    theme::success()
                                                } else {
                                                    theme::muted()
                                                })
                                                .child(metadata),
                                        ),
                                )
                                .when(selected, |element| {
                                    element.child(div().w(px(4.))).child(selected_commands)
                                }),
                        ),
                );
            }
            list.into_any_element()
        };

        let action_target = self
            .clipboard
            .action_target
            .and_then(|id| self.clipboard.items().iter().find(|item| item.id == id))
            .map(|item| item.summary.clone())
            .unwrap_or_default();
        let target_exists = self
            .clipboard
            .action_target
            .is_some_and(|id| self.clipboard.items().iter().any(|item| item.id == id));
        let refresh_action = Self::clipboard_refresh_action(self.screen);
        let actions_panel = self.clipboard.actions_open.then(|| {
            let labels = [
                l10n::copy_with_return_hint(self.locale),
                l10n::shortcut_hint(self.locale, &l10n::preview(self.locale), "␣"),
                l10n::delete_entry(self.locale),
                l10n::refresh(self.locale),
            ];
            div()
                .debug_selector(|| "clipboard-actions".into())
                .w(px(300.))
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
                                .child(if action_target.is_empty() {
                                    l10n::actions(self.locale)
                                } else {
                                    action_target
                                }),
                        )
                        .child(self.keycap("esc")),
                )
                .child(div().h(px(metrics::DIVIDER_WIDTH)).bg(theme::divider()))
                .child(div().p(px(4.)).flex().flex_col().children(
                    labels.into_iter().enumerate().map(|(index, label)| {
                        Button::new(SharedString::from(format!("clipboard-action-row-{index}")))
                            .when(index == self.clipboard.action_selected, |button| {
                                button.secondary()
                            })
                            .when(index != self.clipboard.action_selected, |button| {
                                button.ghost()
                            })
                            .disabled(!target_exists && index != refresh_action)
                            .label(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.clipboard.action_selected = index;
                                this.execute_clipboard_action(index, cx);
                            }))
                    }),
                ))
                .into_any_element()
        });

        let body = div()
            .relative()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .child(content);
        let empty_actions = self.clipboard.items().is_empty().then(|| {
            div()
                .id("history-empty-actions")
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
                    this.open_empty_clipboard_actions(cx);
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
                SharedString::from(l10n::clipboard_history(self.locale)),
                SharedString::from(l10n::clipboard(self.locale)),
                Some((
                    &self.clipboard_input,
                    "clipboard-search",
                    l10n::search_history(self.locale),
                )),
                body.into_any_element(),
                empty_actions,
                cx,
            )
            .into_any_element();
        div()
            .size_full()
            .relative()
            .child(page)
            .children(actions_panel.map(|panel| {
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
