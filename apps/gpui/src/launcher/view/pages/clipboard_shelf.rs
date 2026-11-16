use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn render_clipboard_shelf(
        &mut self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let status = self.clipboard.status.clone();
        let header_status = status
            .as_ref()
            .map(|status| {
                if !status.available {
                    l10n::unavailable(self.locale)
                } else if !status.recording_enabled {
                    l10n::disabled(self.locale)
                } else if status.paused_until_ms.is_some() {
                    format!(
                        "{} · {}",
                        l10n::clipboard_always_on_top(self.locale),
                        l10n::paused(self.locale)
                    )
                } else {
                    l10n::clipboard_always_on_top(self.locale)
                }
            })
            .unwrap_or_else(|| l10n::clipboard_always_on_top(self.locale));

        let body = if let Some(delete_id) = self.clipboard.delete_pending {
            let summary = self
                .clipboard
                .items()
                .iter()
                .find(|item| item.id == delete_id)
                .map(|item| item.summary.clone())
                .unwrap_or_default();
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(metrics::SPACE_SM))
                .p(px(16.))
                .child(
                    div()
                        .body_text()
                        .font_weight(FontWeight::MEDIUM)
                        .child(l10n::shelf_delete_title(self.locale)),
                )
                .child(
                    div()
                        .max_w(px(360.))
                        .truncate()
                        .text_center()
                        .text_size(px(metrics::CAPTION_SIZE))
                        .text_color(theme::muted())
                        .child(summary),
                )
                .child(
                    div()
                        .max_w(px(380.))
                        .text_center()
                        .caption_text()
                        .text_color(theme::muted())
                        .child(l10n::shelf_delete_description(self.locale)),
                )
                .child(
                    div()
                        .w_full()
                        .max_w(px(380.))
                        .pt(px(6.))
                        .flex()
                        .gap(px(metrics::SPACE_SM))
                        .child(
                            div()
                                .id("shelf-delete-cancel")
                                .role(gpui_kit::Role::Button)
                                .aria_selected(!self.clipboard.delete_confirm_selected)
                                .flex_1()
                                .p(px(10.))
                                .rounded(px(metrics::RESULT_RADIUS))
                                .when(!self.clipboard.delete_confirm_selected, |element| {
                                    element.selected_surface()
                                })
                                .hover(|style| style.bg(theme::hover()))
                                .cursor_pointer()
                                .text_center()
                                .text_size(px(metrics::LABEL_SIZE))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.clipboard.delete_pending = None;
                                    this.clipboard.delete_confirm_selected = false;
                                    cx.notify();
                                }))
                                .child(l10n::cancel(self.locale)),
                        )
                        .child(
                            div()
                                .id("shelf-delete-confirm")
                                .role(gpui_kit::Role::Button)
                                .aria_selected(self.clipboard.delete_confirm_selected)
                                .flex_1()
                                .p(px(10.))
                                .rounded(px(metrics::RESULT_RADIUS))
                                .when(self.clipboard.delete_confirm_selected, |element| {
                                    element.selected_surface()
                                })
                                .hover(|style| style.bg(theme::hover()))
                                .cursor_pointer()
                                .text_center()
                                .text_size(px(metrics::LABEL_SIZE))
                                .text_color(theme::danger())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.confirm_clipboard_delete(cx);
                                    this.clipboard.delete_pending = None;
                                    this.clipboard.delete_confirm_selected = false;
                                    cx.notify();
                                }))
                                .child(l10n::delete_entry(self.locale)),
                        ),
                )
                .into_any_element()
        } else if self.clipboard.actions_open {
            let labels = [
                l10n::copy(self.locale),
                l10n::shelf_copy_and_return(self.locale),
                l10n::preview(self.locale),
                l10n::delete_entry(self.locale),
                l10n::refresh(self.locale),
            ];
            let shortcuts = ["↵", "⌘↵", "␣", "⌘⇧⌫", "⌘R"];
            let icons = [
                IconName::Copy,
                IconName::CornerUpLeft,
                IconName::Eye,
                IconName::Trash,
                IconName::RefreshCw,
            ];
            let target = self
                .clipboard
                .action_target
                .and_then(|id| self.clipboard.items().iter().find(|item| item.id == id))
                .map(|item| item.summary.clone())
                .unwrap_or_else(|| l10n::actions(self.locale));
            let target_exists = self
                .clipboard
                .action_target
                .is_some_and(|id| self.clipboard.items().iter().any(|item| item.id == id));
            div()
                .flex_1()
                .min_h(px(0.))
                .p(px(8.))
                .pt(px(8.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .px(px(8.))
                        .pb(px(6.))
                        .truncate()
                        .text_size(px(metrics::CAPTION_SIZE))
                        .text_color(theme::muted())
                        .child(target),
                )
                .child(div().flex_1().min_h(px(0.)).flex().flex_col().children(
                    labels.into_iter().enumerate().map(|(index, label)| {
                        let selected = self.clipboard.action_selected == index;
                        let enabled = target_exists || index == 4;
                        div()
                            .id(SharedString::from(format!("shelf-action-{index}")))
                            .role(gpui_kit::Role::Button)
                            .aria_selected(selected)
                            .h(px(40.))
                            .my(px(1.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .result_surface(selected)
                            .when(!enabled, |element| element.opacity(0.45))
                            .when(enabled, |element| {
                                element.cursor_pointer().on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.clipboard.action_selected = index;
                                        this.execute_clipboard_action(index, cx);
                                    },
                                ))
                            })
                            .child(Icon::new(icons[index]).size(px(16.)).text_color(
                                if index == 3 {
                                    theme::danger()
                                } else {
                                    theme::secondary()
                                },
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .truncate()
                                    .text_size(px(metrics::LABEL_SIZE))
                                    .child(label),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(theme::secondary())
                                    .child(shortcuts[index]),
                            )
                    }),
                ))
                .into_any_element()
        } else if self.clipboard.items().is_empty() {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .p(px(16.))
                .child(
                    div()
                        .max_w(px(360.))
                        .text_center()
                        .caption_text()
                        .text_color(theme::muted())
                        .child(if self.clipboard.loading {
                            l10n::searching(self.locale)
                        } else if self.clipboard.query.trim().is_empty() {
                            l10n::no_saved_entries_manage_recording_in_settings(self.locale)
                        } else {
                            l10n::no_matching_entries(self.locale)
                        }),
                )
                .into_any_element()
        } else {
            let mut list = div()
                .id("clipboard-shelf-list")
                .flex_1()
                .min_h(px(0.))
                .px(px(6.))
                .pb(px(4.))
                .flex()
                .flex_col()
                .track_scroll(&self.clipboard_shelf_scroll)
                .overflow_y_scroll()
                .vertical_scrollbar(&self.clipboard_shelf_scroll);
            for (index, item) in self.clipboard.items().to_vec().into_iter().enumerate() {
                let selected = self.clipboard.browsing && self.clipboard.selected == index;
                let item_id = item.id;
                let summary = item.summary.clone();
                let metadata = if selected && self.clipboard.copied_item == Some(item_id) {
                    l10n::copied(self.locale)
                } else {
                    let kind = components::clipboard_kind_label(self.locale, &item.kind);
                    let date = components::clipboard_copy_date(item.copied_at_ms, true);
                    if date.is_empty() {
                        kind
                    } else {
                        format!("{kind} · {date}")
                    }
                };
                let command_hint = if selected {
                    None
                } else if index < 9 {
                    Some(format!("⌘{}", index + 1))
                } else {
                    None
                };
                list = list.child(
                    div()
                        .id(SharedString::from(format!(
                            "clipboard-shelf-item-{item_id}"
                        )))
                        .role(gpui_kit::Role::ListBoxOption)
                        .aria_label(summary.clone())
                        .aria_selected(selected)
                        .h(px(48.))
                        .flex_shrink_0()
                        .py(px(1.))
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
                                .gap(px(12.))
                                .result_surface(selected)
                                .child(
                                    Icon::new(components::clipboard_kind_icon(&item.kind))
                                        .size(px(18.))
                                        .text_color(components::clipboard_kind_tint(&item.kind)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .flex()
                                        .flex_col()
                                        .justify_center()
                                        .gap(px(2.))
                                        .child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "shelf-title-{item_id}"
                                                )))
                                                .truncate()
                                                .text_size(px(12.))
                                                .line_height(px(12. * metrics::BODY_LINE_HEIGHT))
                                                .text_color(theme::primary())
                                                .tooltip(components::text_tooltip(summary))
                                                .child(item.summary),
                                        )
                                        .child(
                                            div()
                                                .truncate()
                                                .text_size(px(10.))
                                                .line_height(px(10. * metrics::CAPTION_LINE_HEIGHT))
                                                .text_color(
                                                    if selected
                                                        && self.clipboard.copied_item
                                                            == Some(item_id)
                                                    {
                                                        theme::success()
                                                    } else {
                                                        theme::muted()
                                                    },
                                                )
                                                .child(metadata),
                                        ),
                                )
                                .when(selected, |element| {
                                    let actions_id = item_id;
                                    element
                                        .child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "shelf-preview-{item_id}"
                                                )))
                                                .role(gpui_kit::Role::Button)
                                                .aria_label(l10n::preview(self.locale))
                                                .h(px(metrics::CONTROL_HEIGHT))
                                                .flex_none()
                                                .px(px(metrics::CONTROL_PADDING_X))
                                                .flex()
                                                .items_center()
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
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "shelf-more-{item_id}"
                                                )))
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
                                                    this.open_clipboard_actions(actions_id, cx);
                                                }))
                                                .child(
                                                    Icon::new(IconName::Ellipsis)
                                                        .size(px(14.))
                                                        .text_color(theme::secondary()),
                                                )
                                                .child("⌘K"),
                                        )
                                        .child(div().child(self.keycap("↵")))
                                })
                                .when(!selected, |element| {
                                    element.when_some(command_hint, |element, hint| {
                                        element.child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(theme::muted())
                                                .child(hint),
                                        )
                                    })
                                }),
                        ),
                );
            }
            list.into_any_element()
        };

        let notice = self.notice.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(64.))
                    .flex_shrink_0()
                    .px(px(metrics::SPACE_XL))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .id("shelf-state-icon")
                            .w(px(26.))
                            .h(px(26.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .tooltip(components::text_tooltip(l10n::shelf_select_hint(
                                self.locale,
                            )))
                            .child(
                                Icon::new(IconName::ClipboardList)
                                    .size(px(20.))
                                    .text_color(theme::icon_amber()),
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
                            .gap(px(1.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(10.))
                                    .line_height(px(10. * metrics::CAPTION_LINE_HEIGHT))
                                    .text_color(theme::muted())
                                    .child(header_status),
                            )
                            .child(
                                div().h(px(30.)).min_w(px(0.)).child(
                                    Input::new(&self.clipboard_input)
                                        .id("clipboard-shelf-search")
                                        .aria_label(l10n::search_clipboard(self.locale))
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false)
                                        .cleanable(true)
                                        .text_size(px(15.))
                                        .p(px(0.))
                                        .w_full(),
                                ),
                            ),
                    )
                    .child(div().w(px(14.)))
                    .when(self.clipboard.items().is_empty(), |element| {
                        element.child(
                            div()
                                .id("shelf-empty-actions")
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
                                ),
                        )
                    })
                    .child(
                        div()
                            .id("shelf-escape")
                            .cursor_pointer()
                            .tooltip(components::text_tooltip(l10n::close_esc(self.locale)))
                            .on_click(cx.listener(|this, _, window, cx| this.go_back(window, cx)))
                            .child(self.keycap("esc")),
                    ),
            )
            .child(
                div()
                    .h(px(metrics::DIVIDER_WIDTH))
                    .mx(px(metrics::SPACE_XL))
                    .bg(theme::divider()),
            )
            .when_some(notice, |element, notice| {
                element.child(
                    div()
                        .px(px(metrics::SPACE_XXL))
                        .pt(px(metrics::SPACE_MD))
                        .child(components::notice(notice, components::NoticeKind::Warning)),
                )
            })
            .child(body)
            .into_any_element()
    }
}
