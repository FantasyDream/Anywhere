use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn render_clipboard(
        &mut self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.sync_clipboard_inspector(cx);
        let body = self.render_clipboard_results(cx);
        let panel = self
            .clipboard
            .actions_open
            .then(|| self.render_clipboard_actions_panel(cx));
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
            .when_some(panel, |page, panel| {
                page.child(self.render_action_overlay(panel, cx))
            })
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
            if self.clipboard.loading {
                components::loading_state(title).into_any_element()
            } else {
                components::empty_state(IconName::Clock3, title, detail).into_any_element()
            }
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
                    .px(px(metrics::LIST_PADDING_X))
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
                components::filter_chip(id, label, active).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.clipboard.set_kind_filter(filter);
                        cx.notify();
                    },
                )),
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
        let scroll = &self.scrolls.clipboard_inspector;
        let inspector = self
            .clipboard_inspector
            .as_ref()
            .filter(|inspector| inspector.item_id == item_id);
        let body = inspector
            .and_then(state::ClipboardInspector::body)
            .unwrap_or(item.summary.as_str());
        let code = body.lines().nth(1).is_some()
            && (body.trim_start().starts_with(['{', '[', '$', '>'])
                || body
                    .lines()
                    .take(40)
                    .any(|line| line.starts_with("    ") || line.starts_with('\t')));
        let color_values = swatch.map(|color| {
            let ink = gpui_kit::Rgba::from(color);
            let [red, green, blue] =
                [ink.r, ink.g, ink.b].map(|value| (value * 255.).round() as u8);
            let alpha = (ink.a * 255.).round() as u8;
            let hex = if alpha == 255 {
                format!("#{red:02X}{green:02X}{blue:02X}")
            } else {
                format!("#{red:02X}{green:02X}{blue:02X}{alpha:02X}")
            };
            let rgb = if alpha == 255 {
                format!("rgb({red}, {green}, {blue})")
            } else {
                format!("rgba({red}, {green}, {blue}, {:.3})", ink.a)
            };
            [hex, rgb]
        });
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
            .when_some(color_values, |panel, values| {
                panel.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(metrics::SPACE_XS))
                        .children(values.into_iter().enumerate().map(|(index, value)| {
                            Button::new(SharedString::from(format!("clipboard-copy-color-{index}")))
                                .secondary()
                                .icon(IconName::Copy)
                                .label(value.clone())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.copy_text_with_feedback(value.clone(), cx);
                                }))
                        })),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .bg(theme::field())
                    .border(px(metrics::DIVIDER_WIDTH))
                    .border_color(theme::border())
                    .child(
                        div()
                            .id("clipboard-inspector-scroll")
                            .test_support()
                            .size_full()
                            .track_scroll(scroll)
                            .overflow_y_scroll()
                            .p(px(metrics::SPACE_SM))
                            .whitespace_normal()
                            .text_size(px(metrics::LABEL_SIZE))
                            .line_height(px(metrics::LABEL_SIZE * metrics::BODY_LINE_HEIGHT))
                            .when(code, |text| text.font_family(fonts::FONT_MONO))
                            .text_color(theme::primary())
                            .children(body.split('\n').map(|line| {
                                // Keep explicit line boundaries while wrapping each line to the inspector.
                                div().child(SharedString::from(if line.is_empty() {
                                    " "
                                } else {
                                    line
                                }))
                            })),
                    )
                    .vertical_scrollbar(scroll),
            )
            .when_some(
                inspector.filter(|state| state.pages.len() > 1),
                |panel, state| {
                    panel.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(metrics::SPACE_XS))
                            .children(
                                [(-1, IconName::ChevronLeft), (1, IconName::ChevronRight)]
                                    .into_iter()
                                    .enumerate()
                                    .map(|(index, (step, icon))| {
                                        let page = state.selected_page.saturating_add_signed(step);
                                        let label = l10n::clipboard_preview_page(
                                            self.locale,
                                            (page + 1) as i64,
                                        );
                                        Button::new(SharedString::from(format!(
                                            "clipboard-inspector-page-{index}"
                                        )))
                                        .ghost()
                                        .icon(icon)
                                        .accessibility_label(label.clone())
                                        .tooltip(label)
                                        .disabled(
                                            page == state.selected_page
                                                || page >= state.pages.len(),
                                        )
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                if let Some(state) =
                                                    this.clipboard_inspector.as_mut()
                                                {
                                                    if state.item_id == item_id
                                                        && page < state.pages.len()
                                                    {
                                                        state.selected_page = page;
                                                        this.scrolls
                                                            .clipboard_inspector
                                                            .set_offset(gpui_kit::point(
                                                                px(0.),
                                                                px(0.),
                                                            ));
                                                        cx.notify();
                                                    }
                                                }
                                            }),
                                        )
                                    }),
                            ),
                    )
                },
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
        let preview_key = crate::platform::primary_shortcut_label("Y");
        let refresh_key = crate::platform::primary_shortcut_label("R");
        components::action_popover("clipboard-actions", title)
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
                    components::action_popover_row(
                        format!("clipboard-action-row-{index}"),
                        label,
                        icon,
                        self.clipboard.action_selected == index,
                        true,
                        match index {
                            1 => Some(preview_key.as_str()),
                            3 => Some(refresh_key.as_str()),
                            _ => None,
                        },
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
