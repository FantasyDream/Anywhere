use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn render_file_search(
        &mut self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let progress = self.model.file_search_progress().cloned();
        let status_label = progress.as_ref().map(|progress| {
            let found = l10n::file_found_count(self.locale, progress.confirmed_matches as i64);
            if !progress.finished {
                format!("{found} · {}", l10n::searching(self.locale))
            } else if progress.issue_count > 0 {
                format!(
                    "{found} · {} · {}",
                    l10n::partial_results(self.locale),
                    l10n::search_issues(self.locale, progress.issue_count as i64)
                )
            } else {
                found
            }
        });
        let rows = if self.model.loading {
            self.empty_state_with_icon(
                IconName::Search,
                l10n::searching(self.locale),
                Some(l10n::searching_files(self.locale)),
            )
        } else if self.model.rows.is_empty() {
            let has_issue = progress
                .as_ref()
                .is_some_and(|progress| progress.issue_count > 0);
            let idle = self.model.query.trim().is_empty();
            let title = if idle {
                l10n::find_files_folders(self.locale)
            } else if progress.as_ref().is_some_and(|progress| !progress.finished) {
                l10n::searching(self.locale)
            } else if has_issue {
                l10n::partial_results(self.locale)
            } else {
                l10n::no_matching_files(self.locale)
            };
            let detail = if idle {
                Some(l10n::type_file_name(self.locale))
            } else if has_issue {
                Some(l10n::file_search_incomplete_help(self.locale))
            } else {
                None
            };
            self.empty_state_with_icon(
                if has_issue {
                    IconName::TriangleAlert
                } else {
                    IconName::Search
                },
                title,
                detail,
            )
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
                                        this.model.rows[index].clone(),
                                        this.file_search.browsing && this.model.selected == index,
                                        cx,
                                    )
                                })
                                .collect::<Vec<_>>()
                        }),
                    )
                    .size_full()
                    .px(px(10.))
                    .track_scroll(&self.tool_list_scrolls.file_search)
                    .on_scroll(cx.listener(|this, _, _, cx| {
                        if this.file_search_scroll.offset().y
                            + this.file_search_scroll.max_offset().y
                            <= px(24.)
                        {
                            this.request_more_files(false, cx);
                        }
                    })),
                )
                .vertical_scrollbar(&self.file_search_scroll);
            let mut list = div()
                .flex_1()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .child(scroll_area);
            if progress
                .as_ref()
                .and_then(|progress| progress.next_cursor.as_ref())
                .is_some()
            {
                list = list.child(
                    div()
                        .p(px(metrics::SPACE_SM))
                        .flex()
                        .justify_center()
                        .child(
                            Button::new("load-more-files")
                                .secondary()
                                .loading(self.file_search.page_in_flight)
                                .disabled(self.file_search.page_in_flight)
                                .label(l10n::load_more(self.locale))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.request_more_files(false, cx);
                                })),
                        ),
                );
            }
            list.into_any_element()
        };

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
            .track_scroll(&self.file_filter_scroll);
        for (filter, label) in filters {
            filter_bar = filter_bar.child(self.file_filter_button(
                filter,
                label,
                self.file_search.filter == filter,
                cx,
            ));
        }

        let mut root = div()
            .id("file-search-surface")
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("file-search-header")
                    .test_support()
                    .h(px(metrics::PAGE_HEADER_HEIGHT))
                    .flex_shrink_0()
                    .px(px(metrics::SPACE_XL))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .id("file-search-back")
                            .test_support()
                            .role(gpui_kit::Role::Button)
                            .aria_label(l10n::back_to_results(self.locale))
                            .cursor_pointer()
                            .size(px(36.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(metrics::CONTROL_RADIUS))
                            .hover(|style| style.bg(theme::hover()))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.go_back(window, cx);
                            }))
                            .child(
                                Icon::new(IconName::ArrowLeft)
                                    .size(px(20.))
                                    .text_color(theme::primary()),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .justify_center()
                            .gap(px(metrics::PAGE_TITLE_GAP))
                            .child(
                                div()
                                    .id("launcher-mode-label")
                                    .test_support()
                                    .truncate()
                                    .text_size(px(metrics::PAGE_MODE_SIZE))
                                    .line_height(px(
                                        metrics::PAGE_MODE_SIZE * metrics::CAPTION_LINE_HEIGHT
                                    ))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::secondary())
                                    .child(self.file_range_label()),
                            )
                            .child(
                                Input::new(&self.input)
                                    .id("file-search-input")
                                    .aria_label(l10n::search_files_folders(self.locale))
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .cleanable(false)
                                    .text_size(px(metrics::PAGE_HEADING_SIZE))
                                    .line_height(px(
                                        metrics::PAGE_HEADING_SIZE * metrics::BODY_LINE_HEIGHT
                                    ))
                                    .p(px(0.))
                                    .w_full(),
                            ),
                    )
                    .child(self.keycap("esc")),
            )
            .child(
                div()
                    .h(px(metrics::DIVIDER_WIDTH))
                    .flex_shrink_0()
                    .bg(theme::divider()),
            )
            .child(filter_bar);
        if let Some(label) = status_label {
            root = root.child(
                div()
                    .id("file-search-status")
                    .test_support()
                    .h(px(22.))
                    .flex_shrink_0()
                    .px(px(metrics::SPACE_XL))
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_SM))
                    .text_size(px(10.5))
                    .line_height(px(10.5 * metrics::CAPTION_LINE_HEIGHT))
                    .text_color(theme::muted())
                    .child(div().flex_1().truncate().child(label))
                    .when(
                        progress
                            .as_ref()
                            .is_some_and(|progress| progress.issue_count > 0),
                        |element| {
                            element.child(
                                Button::new("retry-file-search")
                                    .ghost()
                                    .h(px(20.))
                                    .px(px(metrics::SPACE_SM))
                                    .text_size(px(metrics::CAPTION_SIZE))
                                    .label(l10n::retry(self.locale))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_live_update(cx);
                                    })),
                            )
                        },
                    ),
            );
        }
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
}
