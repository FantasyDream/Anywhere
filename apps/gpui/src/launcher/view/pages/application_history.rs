use super::*;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};

impl LauncherView {
    pub(in crate::launcher::view) fn render_app_history(
        &mut self,
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
        let content = if self.app_history.object_actions_active {
            self.render_history_object_actions_panel(cx)
        } else if self.app_history.page_actions_open {
            components::action_candidates(
                "app-history-page-actions-panel",
                l10n::actions(self.locale),
            )
            .debug_selector(|| "app-history-page-actions-panel".into())
            .child(
                components::action_candidate(
                    "app-history-page-refresh",
                    l10n::refresh(self.locale),
                    IconName::RefreshCw,
                    true,
                    true,
                )
                .on_click(
                    cx.listener(|this, _, _, cx| this.refresh_app_history_from_page_menu(cx)),
                ),
            )
            .into_any_element()
        } else if self.app_history.fallback_actions_open {
            let title = self
                .app_history
                .selected_item()
                .map(|item| item.title.clone())
                .unwrap_or_else(|| l10n::actions(self.locale));
            components::action_candidates("app-history-fallback-actions-panel", title)
                .test_support()
                .debug_selector(|| "app-history-fallback-actions-panel".into())
                .children(
                    [
                        (IconName::Copy, l10n::app_history_copy_target(self.locale)),
                        (IconName::RefreshCw, l10n::refresh(self.locale)),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (icon, label))| {
                        components::action_candidate(
                            format!("app-history-fallback-action-{index}"),
                            label,
                            icon,
                            self.app_history.fallback_action_selected == index,
                            true,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.execute_fallback_app_history_action(index, cx)
                        }))
                    }),
                )
                .into_any_element()
        } else {
            self.render_history_results(cx)
        };
        let actions = components::page_actions_button(
            "app-history-page-actions",
            l10n::actions(self.locale),
            self.app_history.page_actions_open,
        )
        .on_click(cx.listener(|this, _, _, cx| this.toggle_app_history_page_actions(cx)));
        let trailing = self.render_history_header_trailing(&page_title, actions, cx);
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
                    .min_h_0()
                    .child(content)
                    .child(self.render_history_footer(cx))
                    .into_any_element(),
                Some(trailing),
                cx,
            )
            .into_any_element();
        div()
            .id("app-history-surface")
            .key_context("ApplicationHistory")
            .track_focus(&self.surface_focus_handle)
            .on_action(cx.listener(Self::on_action_toggle_object_actions))
            .on_action(cx.listener(Self::on_action_refresh_page))
            .on_action(cx.listener(Self::on_action_focus_search))
            .size_full()
            .relative()
            .child(page)
            .into_any_element()
    }

    fn render_history_header_trailing(
        &self,
        current_title: &str,
        actions: impl IntoElement,
        cx: &Context<Self>,
    ) -> AnyElement {
        let enabled_sources = self
            .app_history
            .sources
            .iter()
            .filter(|source| source.action_enabled)
            .map(|source| (source.id.clone(), source.title.clone()))
            .collect::<Vec<_>>();
        if enabled_sources.len() <= 1 {
            return actions.into_any_element();
        }
        let selected_id = self.app_history.selected_source.clone();
        let view = cx.weak_entity();
        div()
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_XS))
            .child(
                Button::new("app-history-source-switcher")
                    .ghost()
                    .label(current_title.to_owned())
                    .dropdown_caret(true)
                    .dropdown_menu(move |mut menu, _, _| {
                        for (source_id, source_title) in &enabled_sources {
                            let view = view.clone();
                            let target_id = source_id.clone();
                            let checked = selected_id.as_deref() == Some(source_id.as_str());
                            menu = menu.item(
                                PopupMenuItem::new(source_title.clone())
                                    .checked(checked)
                                    .on_click(move |_, _, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            if this.app_history.selected_source.as_deref()
                                                != Some(target_id.as_str())
                                            {
                                                this.app_history.selected_source =
                                                    Some(target_id.clone());
                                                this.app_history.reset_selection();
                                                this.app_history.close_object_actions();
                                                this.request_app_history_search(cx);
                                                cx.notify();
                                            }
                                        });
                                    }),
                            );
                        }
                        menu
                    }),
            )
            .child(actions)
            .into_any_element()
    }

    fn render_history_footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.app_history.selected_item();
        let can_open = selected.is_some_and(|item| item.can_open);
        let has_selection = selected.is_some();
        let primary_label = if can_open {
            l10n::open(self.locale)
        } else {
            l10n::copy(self.locale)
        };
        let mut trailing = Vec::new();
        if has_selection {
            trailing.push(
                components::footer_action_button(
                    "app-history-footer-primary",
                    primary_label,
                    "↵",
                    true,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if can_open {
                        this.open_selected_history_item(cx);
                    } else {
                        this.copy_selected_history_item(cx);
                    }
                }))
                .into_any_element(),
            );
        }
        trailing.push(
            components::footer_action_button(
                "app-history-footer-refresh",
                l10n::refresh(self.locale),
                "⌘R",
                false,
            )
            .on_click(cx.listener(|this, _, _, cx| this.refresh_app_history_from_page_menu(cx)))
            .into_any_element(),
        );
        trailing.push(
            components::footer_action_button(
                "app-history-footer-actions",
                l10n::actions(self.locale),
                "⌘K",
                false,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                if let Some(item) = this.app_history.selected_item() {
                    if item.can_open {
                        this.request_app_history_object_actions(cx);
                    } else {
                        this.toggle_fallback_app_history_actions(cx);
                    }
                } else {
                    this.toggle_app_history_page_actions(cx);
                }
            }))
            .into_any_element(),
        );
        components::footer_bar(
            "app-history-footer",
            div()
                .caption_text()
                .text_color(theme::muted())
                .child(format!("{}", self.app_history.items.len())),
            trailing,
        )
        .into_any_element()
    }

    fn render_history_results(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.app_history.items.is_empty() {
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
            div()
                .flex_1()
                .min_h(px(0.))
                .relative()
                .child(
                    gpui_kit::uniform_list(
                        "application-history-list",
                        self.app_history.items.len(),
                        cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                            range
                                .map(|index| this.render_history_list_row(index, cx))
                                .collect::<Vec<_>>()
                        }),
                    )
                    .size_full()
                    .px(px(10.))
                    .track_scroll(&self.scrolls.tool_lists.history),
                )
                .vertical_scrollbar(&self.scrolls.history_base())
                .into_any_element()
        }
    }

    fn render_history_object_actions_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let language = match self.locale {
            Locale::En => "en",
            Locale::Zh => "zh",
        };
        let history = &self.app_history;
        let title = history
            .selected_item()
            .map(|item| item.title.clone())
            .unwrap_or_default();
        let selected = history.object_action_selected;
        let scroll = &self.scrolls.history_actions;
        let mut rows = div()
            .id("history-object-action-list")
            .test_support()
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .track_scroll(scroll);
        for (index, action) in history.object_actions.iter().enumerate() {
            let title = action
                .localized_titles
                .get(language)
                .unwrap_or(&action.title)
                .clone();
            rows = rows.child(
                components::action_candidate(
                    format!("history-object-action-{index}"),
                    title,
                    IconName::ChevronRight,
                    selected == index,
                    action.enabled,
                )
                .when(action.enabled, |row| {
                    row.on_click(cx.listener(move |this, _, _, cx| {
                        this.app_history.object_action_selected = index;
                        this.execute_selected_app_history_action(cx);
                    }))
                }),
            );
        }
        let fallback = history.object_actions_fallback_copy();
        for (index, id, icon, label, visible) in [
            (
                0,
                "history-object-action-copy",
                IconName::Copy,
                l10n::app_history_copy_target(self.locale),
                fallback,
            ),
            (
                history.object_actions.len() + usize::from(fallback),
                "history-object-action-refresh",
                IconName::RefreshCw,
                l10n::refresh(self.locale),
                !history.object_actions_loading,
            ),
        ] {
            if visible {
                rows = rows.child(
                    components::action_candidate(id, label, icon, selected == index, true)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.app_history.object_action_selected = index;
                            this.execute_selected_app_history_action(cx);
                        })),
                );
            }
        }
        components::action_candidates("history-object-actions", title)
            .debug_selector(|| "history-object-actions".into())
            .when(history.object_actions_loading, |panel| {
                panel.child(div().caption_text().child(l10n::loading(self.locale)))
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(rows)
                    .vertical_scrollbar(scroll),
            )
            .into_any_element()
    }

    fn render_history_list_row(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let item = &self.app_history.items[index];
        let selected = self.app_history.selected == index;
        let can_open = item.can_open;
        let item_id = &item.id;
        let title = SharedString::from(item.title.clone());
        let path = SharedString::from(item.value.clone());
        let open_label = if can_open {
            l10n::open(self.locale)
        } else {
            l10n::copy(self.locale)
        };
        let display_path = if item.kind == "remote" {
            path.clone()
        } else {
            SharedString::from(format_path_middle_truncated(&item.value))
        };
        let (icon_name, icon_tint) = match item.kind {
            "directory" => (IconName::Folder, theme::icon_blue()),
            "workspace" => (IconName::Code, theme::icon_purple()),
            "remote" => (IconName::Globe, theme::accent()),
            _ => (IconName::File, theme::icon_purple()),
        };
        let mut row = components::ListRow::new(format!("history-item-{item_id}"))
            .density(components::RowDensity::Comfortable)
            .selected(selected)
            .aria_label(title.clone())
            .leading(components::squircle_icon_plate(
                icon_name,
                icon_tint,
                metrics::ROW_ICON_SIZE,
            ))
            .title(title.clone())
            .title_tooltip(title)
            .title_highlight_query(self.app_history.query.clone())
            .subtitle(display_path)
            .subtitle_tooltip(path);
        if selected {
            row = row.trailing(
                div()
                    .pl(px(metrics::SPACE_XS))
                    .flex()
                    .items_center()
                    .child(
                        components::primary_row_action(
                            format!("history-open-{item_id}"),
                            open_label.clone(),
                        )
                        .test_support()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.app_history.selected = index;
                            if can_open {
                                this.open_selected_history_item(cx);
                            } else {
                                this.copy_selected_history_item(cx);
                            }
                        }))
                        .child(open_label)
                        .when(!can_open, |element| {
                            element.child(
                                Icon::new(IconName::CornerDownLeft)
                                    .size(px(12.))
                                    .text_color(theme::secondary()),
                            )
                        }),
                    )
                    .child(
                        components::row_action_button(
                            format!("history-actions-{item_id}"),
                            l10n::actions(self.locale),
                        )
                        .test_support()
                        .gap(px(metrics::SPACE_XS))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.app_history.selected = index;
                            if can_open {
                                this.request_app_history_object_actions(cx);
                            } else {
                                this.toggle_fallback_app_history_actions(cx);
                            }
                        }))
                        .child(
                            Icon::new(IconName::Ellipsis)
                                .size(px(14.))
                                .text_color(theme::secondary()),
                        ),
                    ),
            );
        }
        row.build()
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
                        this.open_fallback_app_history_actions(cx);
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
            .test_support()
            .into_any_element()
    }
}
