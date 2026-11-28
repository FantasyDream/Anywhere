use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn render_clipboard_shelf(
        &mut self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let status = self.clipboard.status.as_ref();
        let header_status = status
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

        let body = self.render_shelf_body(cx);
        let delete = self
            .clipboard
            .delete_pending
            .map(|item_id| self.render_shelf_delete_confirmation(item_id, cx));

        let notice = self
            .notice
            .clone()
            .or_else(|| self.clipboard.notice_text(self.locale));
        div()
            .id("clipboard-shelf-surface")
            .key_context("ClipboardShelf")
            .track_focus(&self.surface_focus_handle)
            .on_action(cx.listener(Self::on_action_toggle_preview))
            .on_action(cx.listener(Self::on_action_close_preview_to_editor))
            .on_action(cx.listener(Self::on_action_toggle_object_actions))
            .on_action(cx.listener(Self::on_action_refresh_page))
            .on_action(cx.listener(Self::on_action_focus_search))
            .on_action(cx.listener(Self::on_action_delete_shelf_item))
            .on_action(cx.listener(Self::on_action_copy_and_return))
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_window_titlebar(cx))
            .child(
                div()
                    .h(px(metrics::ROW_HEIGHT_COMFORTABLE))
                    .flex_shrink_0()
                    .px(px(metrics::SPACE_XL))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .id("shelf-state-icon")
                            .test_support()
                            .w(px(metrics::ICON_BUTTON_SIZE))
                            .h(px(metrics::ICON_BUTTON_SIZE))
                            .flex()
                            .items_center()
                            .justify_center()
                            .tooltip(components::text_tooltip(l10n::shelf_select_hint(
                                self.locale,
                            )))
                            .child(
                                Icon::new(IconName::ClipboardList)
                                    .size(px(metrics::SPACE_XL))
                                    .text_color(theme::icon_amber()),
                            ),
                    )
                    .child(div().w(px(metrics::SPACE_MD)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .justify_center()
                            .gap(px(metrics::ROW_INSET_Y_COMPACT))
                            .child(
                                div()
                                    .id("shelf-drag-region")
                                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                                        window.start_window_move()
                                    })
                                    .truncate()
                                    .text_size(px(metrics::PAGE_MODE_SIZE))
                                    .line_height(px(
                                        metrics::PAGE_MODE_SIZE * metrics::CAPTION_LINE_HEIGHT
                                    ))
                                    .text_color(theme::muted())
                                    .child(header_status),
                            )
                            .child(
                                div().h(px(30.)).min_w_0().child(
                                    Input::new(&self.clipboard_input)
                                        .id("clipboard-shelf-search")
                                        .aria_label(l10n::search_clipboard(self.locale))
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false)
                                        .cleanable(true)
                                        .text_size(px(metrics::HEADING_SIZE))
                                        .p_0()
                                        .w_full(),
                                ),
                            ),
                    )
                    .child(div().w(px(metrics::SPACE_LG)))
                    .when(self.clipboard.items().is_empty(), |element| {
                        element.child(
                            div()
                                .id("shelf-empty-actions")
                                .role(gpui_kit::Role::Button)
                                .aria_label(l10n::actions(self.locale))
                                .size(px(metrics::ICON_BUTTON_SIZE))
                                .mr(px(metrics::SPACE_SM))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(metrics::TOOLTIP_RADIUS))
                                .hover(|style| style.bg(theme::hover()))
                                .cursor_pointer()
                                .tooltip(components::text_tooltip(l10n::actions(self.locale)))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.open_empty_clipboard_actions(cx);
                                }))
                                .child(
                                    Icon::new(IconName::Ellipsis)
                                        .size(px(metrics::ICON_SIZE))
                                        .text_color(theme::secondary()),
                                ),
                        )
                    })
                    .child(
                        div()
                            .id("shelf-escape")
                            .test_support()
                            .cursor_pointer()
                            .tooltip(components::text_tooltip(l10n::close_esc(self.locale)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.handle_key("escape", KeyModifiers::default(), window, cx)
                            }))
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
                        .child(components::notice(notice, self.notice_kind)),
                )
            })
            .child(body)
            .children(delete)
            .into_any_element()
    }

    fn render_shelf_body(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.clipboard.actions_open {
            return self.render_shelf_actions_body(cx);
        }
        if self.clipboard.items().is_empty() {
            return if self.clipboard.loading {
                components::loading_state(l10n::searching(self.locale)).into_any_element()
            } else {
                components::empty_state(
                    IconName::ClipboardList,
                    if self.clipboard.query.trim().is_empty() {
                        l10n::no_saved_entries(self.locale)
                    } else {
                        l10n::no_matching_entries(self.locale)
                    },
                    self.clipboard
                        .query
                        .trim()
                        .is_empty()
                        .then(|| l10n::manage_recording_in_settings_clipboard(self.locale)),
                )
                .into_any_element()
            };
        }
        div()
            .flex_1()
            .min_h_0()
            .relative()
            .child(
                gpui_kit::uniform_list(
                    "clipboard-shelf-list",
                    self.clipboard.items().len(),
                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|index| this.render_clipboard_shelf_list_row(index, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .size_full()
                .px(px(metrics::SPACE_SM))
                .track_scroll(&self.scrolls.tool_lists.clipboard_shelf),
            )
            .vertical_scrollbar(&self.scrolls.clipboard_shelf_base())
            .into_any_element()
    }

    fn render_shelf_delete_confirmation(
        &self,
        delete_id: i64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let summary = self
            .clipboard
            .items()
            .iter()
            .find(|item| item.id == delete_id)
            .map(|item| item.summary.clone())
            .unwrap_or_default();
        let cancel = Button::new("shelf-delete-cancel")
            .secondary()
            .selected(!self.clipboard.delete_confirm_selected)
            .label(l10n::cancel(self.locale))
            .on_click(cx.listener(|this, _, _, cx| {
                this.clipboard.delete_pending = None;
                this.clipboard.delete_confirm_selected = false;
                cx.notify();
            }));
        let confirm = Button::new("shelf-delete-confirm")
            .danger()
            .selected(self.clipboard.delete_confirm_selected)
            .label(l10n::delete_entry(self.locale))
            .on_click(cx.listener(|this, _, _, cx| {
                this.confirm_clipboard_delete(cx);
                this.clipboard.delete_pending = None;
                this.clipboard.delete_confirm_selected = false;
                cx.notify();
            }));
        components::modal_shell(
            "shelf-delete-dialog",
            IconName::Trash,
            theme::danger(),
            l10n::shelf_delete_title(self.locale),
            l10n::shelf_delete_description(self.locale),
            Some(
                div()
                    .truncate()
                    .caption_text()
                    .text_color(theme::muted())
                    .child(summary)
                    .into_any_element(),
            ),
            div()
                .flex()
                .justify_end()
                .p(px(metrics::SPACE_LG))
                .gap(px(metrics::SPACE_SM))
                .child(cancel)
                .child(confirm),
        )
    }

    fn render_shelf_actions_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let labels = [
            l10n::copy(self.locale),
            l10n::shelf_copy_and_return(self.locale),
            l10n::preview(self.locale),
            l10n::delete_entry(self.locale),
            l10n::refresh(self.locale),
        ];
        let shortcuts = [
            "↵".to_owned(),
            crate::platform::primary_shortcut_label("Enter"),
            "␣".to_owned(),
            crate::platform::primary_shortcut_label("Shift+Backspace"),
            crate::platform::primary_shortcut_label("R"),
        ];
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
            .and_then(|id| self.clipboard.items().iter().find(|item| item.id == id));
        let title = target
            .map(|item| item.summary.clone())
            .unwrap_or_else(|| l10n::actions(self.locale));
        components::action_candidates("shelf-action-candidates", title)
            .children(labels.into_iter().enumerate().map(|(index, label)| {
                let enabled =
                    target.is_some() || index == Self::clipboard_refresh_action(self.screen);
                components::ListRow::new(format!("shelf-action-{index}"))
                    .density(components::RowDensity::Compact)
                    .selected(self.clipboard.action_selected == index)
                    .enabled(enabled)
                    .leading(
                        Icon::new(icons[index])
                            .size(px(metrics::ICON_SIZE))
                            .text_color(if index == 3 {
                                theme::danger()
                            } else {
                                theme::secondary()
                            }),
                    )
                    .title(label)
                    .trailing(components::keycap(shortcuts[index].clone()))
                    .build()
                    .test_support()
                    .when(enabled, |row| {
                        row.on_click(cx.listener(move |view, _, _, cx| {
                            view.execute_clipboard_action(index, cx)
                        }))
                    })
            }))
            .into_any_element()
    }

    fn render_clipboard_shelf_list_row(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let item = &self.clipboard.items()[index];
        let selected = self.clipboard.browsing && self.clipboard.selected == index;
        let item_id = item.id;
        let summary = SharedString::from(item.summary.clone());
        let copied = selected && self.clipboard.copied_item == Some(item_id);
        let metadata = Self::clipboard_item_metadata(self.locale, item, copied, true);
        let leading = Self::clipboard_leading_visual(item, metrics::ICON_SIZE);
        let command_hint = if selected {
            None
        } else if index < 9 {
            Some(crate::platform::primary_shortcut_label(
                &(index + 1).to_string(),
            ))
        } else {
            None
        };
        let actions_id = item_id;
        let mut row = components::ListRow::new(format!("clipboard-shelf-item-{item_id}"))
            .density(components::RowDensity::Compact)
            .selected(selected)
            .aria_label(summary.clone())
            .leading(leading)
            .title(summary.clone())
            .title_id(format!("shelf-title-{item_id}"))
            .title_tooltip(summary)
            .title_highlight_query(self.clipboard.query.clone())
            .subtitle(metadata)
            .subtitle_color(if copied {
                theme::success()
            } else {
                theme::muted()
            });
        if selected {
            row = row.trailing(
                div()
                    .pl(px(metrics::SPACE_XS))
                    .flex()
                    .items_center()
                    .child(
                        components::row_action_button(
                            format!("shelf-preview-{item_id}"),
                            l10n::preview(self.locale),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.clipboard.selected = index;
                            this.clipboard.browsing = true;
                            this.preview_selected_clipboard(cx);
                        }))
                        .child(
                            Icon::new(IconName::Eye)
                                .size(px(metrics::ICON_SIZE))
                                .text_color(theme::secondary()),
                        ),
                    )
                    .child(
                        components::row_action_button(
                            format!("shelf-more-{item_id}"),
                            l10n::actions(self.locale),
                        )
                        .test_support()
                        .gap(px(metrics::SPACE_XS))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.open_clipboard_actions(actions_id, cx);
                        }))
                        .child(
                            Icon::new(IconName::Ellipsis)
                                .size(px(metrics::ICON_SIZE))
                                .text_color(theme::secondary()),
                        )
                        .child(components::keycap(
                            crate::platform::primary_shortcut_label("K"),
                        )),
                    )
                    .child(self.clipboard_copy_button(
                        format!("shelf-copy-{item_id}"),
                        item_id,
                        cx,
                    )),
            );
        } else if let Some(hint) = command_hint {
            row = row.trailing(
                div()
                    .pl(px(metrics::SPACE_XS))
                    .child(components::keycap(hint)),
            );
        }
        row.build()
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
            .test_support()
            .into_any_element()
    }
}
