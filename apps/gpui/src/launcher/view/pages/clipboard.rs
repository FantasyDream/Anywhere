use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn render_clipboard(
        &mut self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let body = if self.clipboard.actions_open {
            self.render_clipboard_actions_panel(cx)
        } else {
            self.render_clipboard_results(cx)
        };
        let actions = components::page_actions_button(
            "clipboard-page-actions",
            l10n::actions(self.locale),
            self.clipboard.actions_open && self.clipboard.action_target.is_none(),
        )
        .on_click(cx.listener(|this, _, _, cx| {
            if this.clipboard.actions_open && this.clipboard.action_target.is_none() {
                this.clipboard.actions_open = false;
                cx.notify();
            } else {
                this.open_empty_clipboard_actions(cx);
            }
        }));
        let page = self
            .render_page_shell_with_trailing(
                SharedString::from(l10n::clipboard_history(self.locale)),
                SharedString::from(l10n::clipboard(self.locale)),
                Some((
                    &self.clipboard_input,
                    "clipboard-search",
                    l10n::search_history(self.locale),
                )),
                body,
                Some(actions.into_any_element()),
                cx,
            )
            .into_any_element();
        div()
            .id("clipboard-surface")
            .key_context("Clipboard")
            .track_focus(&self.surface_focus_handle)
            .on_action(cx.listener(Self::on_action_toggle_preview))
            .on_action(cx.listener(Self::on_action_close_preview_to_editor))
            .on_action(cx.listener(Self::on_action_toggle_object_actions))
            .on_action(cx.listener(Self::on_action_refresh_page))
            .on_action(cx.listener(Self::on_action_focus_search))
            .size_full()
            .relative()
            .child(page)
            .into_any_element()
    }

    fn render_clipboard_results(&self, cx: &mut Context<Self>) -> AnyElement {
        let filter_bar = self.render_clipboard_filter_bar(cx);
        let visible_len = self.clipboard.visible_len();
        let body = if visible_len == 0 {
            let title = if self.clipboard.loading {
                l10n::loading(self.locale)
            } else if self.clipboard.query.trim().is_empty()
                && self.clipboard.kind_filter == ClipboardKindFilter::All
            {
                l10n::no_saved_entries(self.locale)
            } else {
                l10n::no_matching_entries(self.locale)
            };
            let detail = (!self.clipboard.loading
                && self.clipboard.query.trim().is_empty()
                && self.clipboard.kind_filter == ClipboardKindFilter::All)
                .then(|| {
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
                .gap(px(metrics::SPACE_MD))
                .p(px(metrics::SPACE_XXL))
                .child(
                    Icon::new(IconName::Clock3)
                        .size(px(metrics::ICON_PLATE_SIZE))
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
            let list = div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .relative()
                .child(
                    gpui_kit::uniform_list(
                        "clipboard-history-list",
                        visible_len,
                        cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                            range
                                .map(|index| this.render_clipboard_list_row(index, cx))
                                .collect::<Vec<_>>()
                        }),
                    )
                    .size_full()
                    .px(px(metrics::SPACE_SM))
                    .track_scroll(&self.scrolls.tool_lists.clipboard),
                )
                .vertical_scrollbar(&self.scrolls.clipboard_base());
            let inspector = self
                .clipboard
                .selected_item()
                .cloned()
                .map(|item| self.render_clipboard_inspector(item, cx));
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(list)
                .when_some(inspector, |container, inspector| {
                    container
                        .child(
                            div()
                                .w(px(metrics::DIVIDER_WIDTH))
                                .h_full()
                                .bg(theme::divider()),
                        )
                        .child(inspector)
                })
                .into_any_element()
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(filter_bar)
            .child(body)
            .child(self.render_clipboard_footer(cx))
            .into_any_element()
    }

    fn render_clipboard_filter_bar(&self, cx: &mut Context<Self>) -> gpui_kit::Stateful<Div> {
        let filters = [
            (
                ClipboardKindFilter::All,
                "clipboard-filter-all",
                l10n::all_types(self.locale),
            ),
            (
                ClipboardKindFilter::Text,
                "clipboard-filter-text",
                l10n::text(self.locale),
            ),
            (
                ClipboardKindFilter::Image,
                "clipboard-filter-image",
                l10n::images(self.locale),
            ),
            (
                ClipboardKindFilter::File,
                "clipboard-filter-file",
                l10n::files(self.locale),
            ),
        ];
        let mut bar = div()
            .id("clipboard-kind-filters")
            .h(px(metrics::PAGE_TOOLBAR_HEIGHT))
            .flex_shrink_0()
            .px(px(metrics::SPACE_LG))
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_XS));
        for (filter, id, label) in filters {
            let active = self.clipboard.kind_filter == filter;
            bar = bar.child(
                div()
                    .id(id)
                    .role(gpui_kit::Role::Button)
                    .aria_selected(active)
                    .h(px(metrics::CONTROL_HEIGHT))
                    .px(px(metrics::CONTROL_PADDING_X))
                    .flex()
                    .items_center()
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .text_size(px(metrics::LABEL_SIZE))
                    .font_weight(if active {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(if active {
                        theme::primary()
                    } else {
                        theme::secondary()
                    })
                    .when(active, |chip| {
                        chip.bg(theme::selected())
                            .border(px(metrics::DIVIDER_WIDTH))
                            .border_color(theme::control_border())
                    })
                    .when(!active, |chip| chip.hover(|style| style.bg(theme::hover())))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.clipboard.set_kind_filter(filter);
                        cx.notify();
                    }))
                    .child(label),
            );
        }
        bar
    }

    fn render_clipboard_inspector(
        &self,
        item: crate::host::ClipboardItem,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Stateful<Div> {
        let swatch = detect_color_swatch(&item.summary);
        let kind_label = components::clipboard_kind_label(self.locale, &item.kind);
        let relative = format_relative_time(item.copied_at_ms, current_unix_ms(), self.locale);
        let size = format_byte_count(item.byte_count);
        let item_id = item.id;
        div()
            .id("clipboard-preview-inspector")
            .w_2_5()
            .min_w_0()
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .p(px(metrics::SPACE_MD))
            .gap(px(metrics::SPACE_SM))
            .bg(theme::sidebar())
            .when_some(swatch, |panel, color| {
                panel.child(
                    div()
                        .h(px(metrics::ROW_HEIGHT_COMFORTABLE))
                        .w_full()
                        .rounded(px(metrics::CONTROL_RADIUS))
                        .bg(color)
                        .border(px(metrics::DIVIDER_WIDTH))
                        .border_color(theme::control_border()),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .p(px(metrics::SPACE_SM))
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .bg(theme::field())
                    .border(px(metrics::DIVIDER_WIDTH))
                    .border_color(theme::border())
                    .text_size(px(metrics::LABEL_SIZE))
                    .line_height(px(metrics::LABEL_SIZE * metrics::BODY_LINE_HEIGHT))
                    .text_color(theme::primary())
                    .child(item.summary),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(metrics::SPACE_SM))
                    .caption_text()
                    .text_color(theme::muted())
                    .child(kind_label)
                    .when(!size.is_empty(), |meta| meta.child(format!("· {size}")))
                    .when(!relative.is_empty(), |meta| {
                        meta.child(format!("· {relative}"))
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_XS))
                    .child(
                        Button::new(format!("clipboard-inspector-preview-{item_id}"))
                            .secondary()
                            .label(l10n::preview(self.locale))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this.select_clipboard_item(item_id) {
                                    this.preview_selected_clipboard(cx);
                                }
                            })),
                    )
                    .child(
                        Button::new(format!("clipboard-inspector-actions-{item_id}"))
                            .ghost()
                            .label(l10n::actions(self.locale))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_clipboard_actions(item_id, cx);
                            })),
                    ),
            )
    }

    fn render_clipboard_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let has_selection = self.clipboard.selected_item().is_some();
        let copy_label = l10n::copy(self.locale);
        let preview_label = l10n::preview(self.locale);
        let actions_label = l10n::actions(self.locale);
        let mut trailing = Vec::new();
        if has_selection {
            trailing.push(
                components::footer_action_button("clipboard-footer-copy", copy_label, "↵", true)
                    .on_click(cx.listener(|this, _, _, cx| this.copy_selected_clipboard(cx)))
                    .into_any_element(),
            );
            trailing.push(
                components::footer_action_button(
                    "clipboard-footer-preview",
                    preview_label,
                    "␣",
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| this.preview_selected_clipboard(cx)))
                .into_any_element(),
            );
        }
        trailing.push(
            components::footer_action_button(
                "clipboard-footer-actions",
                actions_label,
                crate::platform::primary_shortcut_label("K"),
                false,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                if let Some(id) = this.clipboard.selected_id() {
                    this.open_clipboard_actions(id, cx);
                } else {
                    this.open_empty_clipboard_actions(cx);
                }
            }))
            .into_any_element(),
        );
        components::footer_bar(
            "clipboard-footer",
            div()
                .caption_text()
                .text_color(theme::muted())
                .child(format!("{}", self.clipboard.visible_len())),
            trailing,
        )
        .into_any_element()
    }

    fn render_clipboard_actions_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let target = self
            .clipboard
            .action_target
            .and_then(|id| self.clipboard.items().iter().find(|item| item.id == id));
        let title = target
            .map(|item| item.summary.clone())
            .unwrap_or_else(|| l10n::actions(self.locale));
        components::action_candidates("clipboard-actions", title)
            .debug_selector(|| "clipboard-actions".into())
            .children(
                [
                    (IconName::Copy, l10n::copy(self.locale)),
                    (IconName::Eye, l10n::preview(self.locale)),
                    (IconName::Trash, l10n::delete_entry(self.locale)),
                    (IconName::RefreshCw, l10n::refresh(self.locale)),
                ]
                .into_iter()
                .enumerate()
                .filter(|(index, _)| {
                    target.is_some() || *index == Self::clipboard_refresh_action(self.screen)
                })
                .map(|(index, (icon, label))| {
                    components::action_candidate(
                        format!("clipboard-action-row-{index}"),
                        label,
                        icon,
                        self.clipboard.action_selected == index,
                        true,
                    )
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.execute_clipboard_action(index, cx)),
                    )
                }),
            )
            .into_any_element()
    }

    pub(super) fn clipboard_copy_button(
        &self,
        id: String,
        item_id: i64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let label = l10n::copy(self.locale);
        components::primary_row_action(id, label.clone())
            .test_support()
            .tooltip(components::text_tooltip(l10n::shortcut_hint(
                self.locale,
                &label,
                "↵",
            )))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                if this.select_clipboard_item(item_id) {
                    this.copy_selected_clipboard(cx);
                }
            }))
            .child(label)
            .child(
                Icon::new(IconName::CornerDownLeft)
                    .size(px(metrics::CONTROL_FONT_SIZE))
                    .text_color(theme::secondary()),
            )
    }

    pub(super) fn clipboard_leading_visual(
        item: &crate::host::ClipboardItem,
        icon_size: f32,
    ) -> AnyElement {
        if let Some(swatch) = detect_color_swatch(&item.summary) {
            return div()
                .size(px(metrics::ICON_PLATE_SIZE))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(metrics::ICON_PLATE_RADIUS))
                .bg(theme::field())
                .border(px(metrics::DIVIDER_WIDTH))
                .border_color(theme::control_border())
                .child(
                    div()
                        .size(px(metrics::COLOR_SWATCH_SIZE))
                        .rounded(px(metrics::SPACE_XS))
                        .bg(swatch)
                        .border(px(metrics::DIVIDER_WIDTH))
                        .border_color(theme::control_border()),
                )
                .into_any_element();
        }
        components::squircle_icon_plate(
            components::clipboard_kind_icon(&item.kind),
            components::clipboard_kind_tint(&item.kind),
            icon_size,
        )
        .into_any_element()
    }

    pub(super) fn clipboard_item_metadata(
        locale: Locale,
        item: &crate::host::ClipboardItem,
        copied: bool,
        compact: bool,
    ) -> String {
        if copied {
            return l10n::copied(locale);
        }
        let kind = components::clipboard_kind_label(locale, &item.kind);
        let time = {
            let relative = format_relative_time(item.copied_at_ms, current_unix_ms(), locale);
            if relative.is_empty() {
                components::clipboard_copy_date(item.copied_at_ms, compact)
            } else {
                relative
            }
        };
        let mut parts = vec![kind];
        if !time.is_empty() {
            parts.push(time);
        }
        if item.byte_count > 0 {
            parts.push(format_byte_count(item.byte_count));
        }
        parts.join(" · ")
    }

    fn render_clipboard_list_row(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(item) = self.clipboard.visible_item(index) else {
            return div().into_any_element();
        };
        let selected = self.clipboard.browsing && self.clipboard.selected == index;
        let item_id = item.id;
        let summary = SharedString::from(item.summary.clone());
        let copied = selected && self.clipboard.copied_item == Some(item_id);
        let metadata = Self::clipboard_item_metadata(self.locale, item, copied, false);
        let leading = Self::clipboard_leading_visual(item, metrics::ROW_ICON_SIZE);
        let mut row = components::ListRow::new(format!("clipboard-item-{item_id}"))
            .density(components::RowDensity::Comfortable)
            .selected(selected)
            .aria_label(summary.clone())
            .leading(leading)
            .title(summary.clone())
            .title_id(format!("clipboard-title-{item_id}"))
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
                    .child(self.clipboard_copy_button(
                        format!("clipboard-copy-{item_id}"),
                        item_id,
                        cx,
                    )),
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
