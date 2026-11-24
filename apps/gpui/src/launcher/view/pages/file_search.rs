use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn render_file_search(
        &mut self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let progress = self.model.file_search_progress();
        let status = progress.map(|progress| self.render_file_search_status(progress, cx));
        let rows = if self.model.loading {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(metrics::SPACE_MD))
                .p(px(metrics::SPACE_XXL))
                .child(Spinner::new().color(theme::accent()))
                .child(
                    div()
                        .body_text()
                        .text_color(theme::primary())
                        .child(l10n::searching(self.locale)),
                )
                .child(
                    div()
                        .caption_text()
                        .text_color(theme::muted())
                        .child(l10n::searching_files(self.locale)),
                )
                .into_any_element()
        } else if self.model.rows.is_empty() {
            let has_issue = progress.is_some_and(|progress| progress.issue_count > 0);
            let idle = self.model.query.trim().is_empty();
            let title = if idle {
                l10n::find_files_folders(self.locale)
            } else if progress.is_some_and(|progress| !progress.finished) {
                l10n::searching(self.locale)
            } else if has_issue {
                l10n::partial_results(self.locale)
            } else {
                l10n::no_matching_files(self.locale)
            };
            let detail = if idle {
                Some(l10n::type_file_name(self.locale))
            } else if has_issue {
                progress
                    .filter(|_| cfg!(windows))
                    .and_then(|progress| progress.issue_examples.first().cloned())
                    .or_else(|| Some(l10n::file_search_incomplete_help(self.locale)))
            } else {
                None
            };
            let empty = components::empty_state(
                if has_issue {
                    IconName::TriangleAlert
                } else {
                    IconName::Search
                },
                title,
                detail,
            );
            let offer_all_types = !idle
                && self.file_search.filter != FileSearchFilter::All
                && !self.model.object_actions_active
                && progress.is_some_and(|progress| progress.finished && progress.issue_count == 0)
                && !self.model.query_differs_from_snapshot()
                && self.model.revision >= self.live_state.request_revision;
            empty
                .when(offer_all_types, |state| {
                    state.child(
                        Button::new("file-search-all-types")
                            .secondary()
                            .label(l10n::search_all_file_types(self.locale))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.select_file_filter(FileSearchFilter::All, cx);
                                this.focus_active_editor(window, cx);
                            })),
                    )
                })
                .into_any_element()
        } else {
            let scroll_area = div()
                .flex_1()
                .min_h(px(0.))
                .relative()
                .child(
                    gpui_kit::uniform_list(
                        "file-search-list",
                        self.model.rows.len(),
                        cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                            range
                                .map(|index| {
                                    this.render_row(
                                        index,
                                        this.file_search.browsing && this.model.selected == index,
                                        cx,
                                    )
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .size_full()
                    .px(px(10.))
                    .track_scroll(&self.scrolls.tool_lists.file_search)
                    .on_scroll(cx.listener(|this, _, _, cx| {
                        if this.scrolls.file_search_base().offset().y
                            + this.scrolls.file_search_base().max_offset().y
                            <= px(24.)
                        {
                            this.request_more_files(false, cx);
                        }
                    })),
                )
                .vertical_scrollbar(&self.scrolls.file_search_base());
            let mut list = div()
                .flex_1()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .child(scroll_area);
            if self.file_search.page_in_flight {
                list = list.child(
                    div()
                        .h(px(metrics::FOOTER_HEIGHT))
                        .px(px(metrics::SPACE_MD))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(metrics::SPACE_SM))
                        .child(Spinner::new().xsmall().color(theme::accent()))
                        .child(
                            div()
                                .caption_text()
                                .text_color(theme::muted())
                                .child(l10n::loading(self.locale)),
                        ),
                );
            }
            list.into_any_element()
        };

        let filter_bar = self.render_file_search_filter_bar(cx);

        let title = self.page_search_title(
            SharedString::from(self.file_range_label()),
            &self.input,
            "file-search-input",
            l10n::search_files_folders(self.locale),
        );
        let mut root = div()
            .id("file-search-surface")
            .key_context("FileSearch")
            .track_focus(&self.surface_focus_handle)
            .on_action(cx.listener(Self::on_action_toggle_preview))
            .on_action(cx.listener(Self::on_action_close_preview_to_editor))
            .on_action(cx.listener(Self::on_action_toggle_object_actions))
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_page_header(title, None, cx))
            .child(Self::page_header_divider())
            .child(filter_bar);
        root = root.children(status);
        if let Some(notice) = self.notice.clone() {
            root = root.child(
                div()
                    .mx(px(metrics::SPACE_MD))
                    .mb(px(metrics::SPACE_SM))
                    .child(components::notice(notice, self.notice_kind)),
            );
        }
        root.child(div().flex_1().min_h(px(0.)).flex().flex_col().child(rows))
            .into_any_element()
    }

    fn render_file_search_status(
        &self,
        progress: &anywhere_protocol::FileSearchProgress,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let count = l10n::file_found_count(self.locale, progress.confirmed_matches as i64);
        let issues = progress.issue_count > 0;
        let (icon, tint) = if issues {
            (IconName::TriangleAlert, theme::warning())
        } else if progress.finished {
            (IconName::Check, theme::secondary())
        } else {
            (IconName::Search, theme::accent())
        };
        let mut details = Vec::new();
        if !progress.finished {
            details.push(l10n::searching(self.locale));
        } else if issues {
            details.push(l10n::partial_results(self.locale));
        }
        if issues {
            details.push(l10n::search_issues(
                self.locale,
                progress.issue_count as i64,
            ));
        }
        let detail = details.join(" · ");
        let label = if detail.is_empty() {
            count.clone()
        } else {
            format!("{count} · {detail}")
        };
        let leading_indicator: AnyElement = if !progress.finished && !issues {
            Spinner::new()
                .xsmall()
                .color(theme::accent())
                .into_any_element()
        } else {
            Icon::new(icon)
                .size(px(metrics::ICON_SIZE))
                .text_color(tint)
                .into_any_element()
        };
        div()
            .id("file-search-status")
            .test_support()
            .h(px(metrics::MENU_ROW_HEIGHT))
            .flex_shrink_0()
            .px(px(metrics::SPACE_XL))
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_SM))
            .child(
                div()
                    .id("file-search-summary")
                    .test_support()
                    .when_some(
                        progress.issue_examples.first().filter(|_| cfg!(windows)),
                        |summary, issue| summary.tooltip(components::text_tooltip(issue.clone())),
                    )
                    .role(gpui_kit::Role::Status)
                    .aria_label(label)
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_SM))
                    .child(leading_indicator)
                    .child(
                        div()
                            .flex_none()
                            .caption_text()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::primary())
                            .child(count),
                    )
                    .when(!detail.is_empty(), |row| {
                        row.child(
                            div()
                                .truncate()
                                .caption_text()
                                .text_color(theme::secondary())
                                .child(detail),
                        )
                    }),
            )
            .when(issues, |row| {
                row.child(
                    Button::new("retry-file-search")
                        .ghost()
                        .h(px(metrics::CONTROL_HEIGHT))
                        .label(l10n::retry(self.locale))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.model.hide_object_actions();
                            this.request_live_update(cx);
                        })),
                )
            })
            .into_any_element()
    }

    fn render_file_search_filter_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let filters = [
            (FileSearchFilter::All, l10n::all_types(self.locale)),
            (FileSearchFilter::Files, l10n::files(self.locale)),
            (FileSearchFilter::Folders, l10n::folders(self.locale)),
            (FileSearchFilter::Documents, l10n::documents(self.locale)),
            (FileSearchFilter::Pdf, l10n::pdf(self.locale)),
            (FileSearchFilter::Images, l10n::images(self.locale)),
            (FileSearchFilter::Audio, l10n::audio(self.locale)),
            (FileSearchFilter::Video, l10n::video(self.locale)),
            (FileSearchFilter::Archives, l10n::archives(self.locale)),
            (FileSearchFilter::Code, l10n::code(self.locale)),
        ];
        let mut filter_bar = div()
            .id("file-search-controls")
            .test_support()
            .h(px(metrics::PAGE_TOOLBAR_HEIGHT))
            .flex_shrink_0()
            .px(px(metrics::SPACE_LG))
            .flex()
            .items_center()
            .overflow_x_scroll()
            .track_scroll(&self.scrolls.file_filter);
        for (filter, label) in filters {
            filter_bar = filter_bar.child(self.file_filter_button(
                filter,
                label,
                self.file_search.filter == filter,
                cx,
            ));
        }
        filter_bar.into_any_element()
    }
}
