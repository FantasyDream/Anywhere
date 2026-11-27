use super::*;
use gpui_kit::Hsla;

fn task_status_style(status: anywhere_protocol::ExecutionSessionStatus) -> (Hsla, IconName) {
    use anywhere_protocol::ExecutionSessionStatus;
    let icon = match status {
        ExecutionSessionStatus::Running => IconName::LoaderCircle,
        ExecutionSessionStatus::Stopping => IconName::Clock3,
        ExecutionSessionStatus::Succeeded => IconName::CircleCheck,
        ExecutionSessionStatus::Failed => IconName::CircleX,
        ExecutionSessionStatus::Stopped => IconName::CirclePause,
    };
    let color = match status {
        ExecutionSessionStatus::Running => theme::accent(),
        ExecutionSessionStatus::Succeeded => theme::success(),
        ExecutionSessionStatus::Failed => theme::danger(),
        ExecutionSessionStatus::Stopping | ExecutionSessionStatus::Stopped => theme::muted(),
    };
    (color, icon)
}

fn directory_display(directory: &str) -> &str {
    directory
        .split(['/', '\\'])
        .rfind(|part| !part.is_empty())
        .unwrap_or(directory)
}

impl LauncherView {
    pub(in crate::launcher::view) fn render_tasks(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.tasks.take_follow_output_request() {
            let scroll = self.scrolls.tasks_output_base();
            let entity_id = cx.entity_id();
            window.on_next_frame(move |window, _cx| {
                window.on_next_frame(move |_, cx| {
                    let maximum = scroll.max_offset().y;
                    let target = -maximum;
                    if scroll.offset().y != target {
                        scroll.set_offset(gpui_kit::point(gpui_kit::px(0.), target));
                        cx.notify(entity_id);
                    }
                });
            });
        }
        let locale = self.locale;
        if self.tasks.tool_options_open
            || (self.tasks.task_actions_open && self.tasks.detail.is_some())
        {
            let panel = self.render_tasks_candidates(locale, cx);
            return self
                .render_page_shell(
                    l10n::running_tasks(locale).into(),
                    SharedString::default(),
                    None,
                    panel,
                    cx,
                )
                .id("tasks-surface")
                .key_context("Tasks")
                .track_focus(&self.surface_focus_handle)
                .into_any_element();
        }
        let indices = self.tasks.visible_indices();
        let visible_count = indices.len();
        let active_count = self
            .tasks
            .rows
            .iter()
            .filter(|row| row.status.is_active())
            .count();
        let subtitle = if active_count == 0 {
            if self.tasks.rows.is_empty() {
                l10n::no_running_tasks(locale)
            } else {
                l10n::all_tasks_finished(locale)
            }
        } else {
            l10n::running_count(locale, active_count as i64)
        };

        let sidebar_rows = div()
            .flex_1()
            .min_h_0()
            .relative()
            .child(
                gpui_kit::uniform_list(
                    "task-list",
                    indices.len(),
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|position| this.render_task_list_row(indices[position], cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .size_full()
                .px(px(metrics::SPACE_SM))
                .track_scroll(&self.scrolls.tool_lists.tasks),
            )
            .vertical_scrollbar(&self.scrolls.tasks_base());

        let sidebar = self.render_tasks_sidebar(locale, visible_count, sidebar_rows);
        let detail = self.tasks.detail.clone();
        let detail_area = if self.tasks.rows.is_empty() {
            components::empty_state(
                IconName::Terminal,
                l10n::no_tasks_yet(locale),
                Some(l10n::choose_aproject_build_test_or_run(locale)),
            )
            .into_any_element()
        } else if visible_count == 0 {
            components::empty_state(IconName::Search, l10n::no_matching_tasks(locale), None)
                .into_any_element()
        } else if let Some(detail) = detail {
            self.render_tasks_detail_area(locale, &detail, cx)
        } else {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(Spinner::new().color(theme::accent()))
                .into_any_element()
        };

        let body = if self.tasks.rows.is_empty() {
            detail_area
        } else {
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(sidebar)
                .child(
                    div()
                        .w(px(metrics::DIVIDER_WIDTH))
                        .h_full()
                        .bg(theme::divider()),
                )
                .child(detail_area)
                .into_any_element()
        };

        let toolbar = self.render_tasks_toolbar(locale, subtitle, cx);
        let find_shortcut = crate::platform::primary_shortcut_label("F");
        let stop_shortcut = crate::platform::primary_shortcut_label(".");
        div()
            .id("tasks-surface")
            .key_context("Tasks")
            .track_focus(&self.surface_focus_handle)
            .on_action(cx.listener(Self::on_action_toggle_object_actions))
            .on_action(cx.listener(Self::on_action_refresh_page))
            .on_action(cx.listener(Self::on_action_focus_search))
            .on_action(cx.listener(Self::on_action_restore_launcher))
            .on_action(cx.listener(Self::on_action_stop_selected_task))
            .on_action(cx.listener(Self::on_action_select_next_task_from_detail))
            .on_action(cx.listener(Self::on_action_select_previous_task_from_detail))
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .when(self.standalone_tasks, |element| {
                element.child(self.render_window_titlebar(cx))
            })
            .child(toolbar)
            .when_some(self.notice.clone(), |element, notice| {
                let notice_kind = self.notice_kind;
                element.child(
                    div()
                        .px(px(metrics::SPACE_XL))
                        .pb(px(metrics::SPACE_MD))
                        .child(components::notice(notice, notice_kind)),
                )
            })
            .child(div().h(px(metrics::DIVIDER_WIDTH)).bg(theme::divider()))
            .child(body)
            .child(div().h(px(metrics::DIVIDER_WIDTH)).bg(theme::divider()))
            .child(
                div()
                    .px(px(metrics::SPACE_XL))
                    .py(px(metrics::SPACE_SM))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::muted())
                            .child(l10n::tasks_keep_running_when_hidden(locale)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(metrics::SPACE_SM))
                            .child(components::keycap(find_shortcut.clone()))
                            .child(components::keycap(stop_shortcut.clone()))
                            .child(
                                Button::new("tasks-keyboard-help-footer")
                                    .ghost()
                                    .label(l10n::keyboard_help(locale))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_notice(
                                            l10n::task_keyboard_help(
                                                this.locale,
                                                &find_shortcut,
                                                &stop_shortcut,
                                            ),
                                            components::NoticeKind::Info,
                                        );
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_tasks_sidebar(
        &self,
        locale: Locale,
        visible_count: usize,
        sidebar_rows: impl IntoElement,
    ) -> Div {
        div()
            .w(px(252.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme::sidebar())
            .child(
                div()
                    .mx(px(metrics::CONTROL_PADDING_X))
                    .mt(px(metrics::SPACE_MD))
                    .mb(px(metrics::SPACE_SM))
                    .h(px(34.))
                    .px(px(metrics::SPACE_SM))
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_XS))
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .bg(theme::field())
                    .border(px(metrics::FIELD_BORDER_WIDTH))
                    .border_color(theme::border())
                    .child(
                        Icon::new(IconName::Search)
                            .size(px(metrics::ICON_SIZE))
                            .text_color(theme::muted()),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.tasks_filter_input)
                                .id("task-search")
                                .aria_label(l10n::find_atask(locale))
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .cleanable(true)
                                .text_size(px(metrics::BODY_SIZE))
                                .w_full(),
                        ),
                    ),
            )
            .child(
                div()
                    .px(px(metrics::SPACE_LG))
                    .pt(px(metrics::SPACE_XS))
                    .pb(px(metrics::CONTROL_PADDING_X))
                    .flex()
                    .items_center()
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::muted())
                    .child(div().flex_1().child(l10n::tasks(locale)))
                    .child(visible_count.to_string()),
            )
            .child(sidebar_rows)
    }

    fn render_tasks_detail_area(
        &mut self,
        locale: Locale,
        detail: &crate::host::ExecutionDetail,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let metadata = self.render_tasks_metadata(locale, &detail.row, cx);
        let output_panel = self.render_tasks_output_panel(locale, detail, cx);
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(metadata)
            .child(output_panel)
            .into_any_element()
    }

    fn render_tasks_metadata(
        &self,
        locale: Locale,
        row: &crate::host::ExecutionRow,
        cx: &mut Context<Self>,
    ) -> Div {
        let active = row.status.is_active();
        let readable_directory =
            crate::launcher::presentation::readable_file_path(&row.directory).to_owned();
        let (status_color, status_icon) = task_status_style(row.status);
        let status_label = match row.status {
            anywhere_protocol::ExecutionSessionStatus::Running => l10n::running(locale),
            anywhere_protocol::ExecutionSessionStatus::Stopping => l10n::stopping_progress(locale),
            anywhere_protocol::ExecutionSessionStatus::Succeeded => l10n::completed(locale),
            anywhere_protocol::ExecutionSessionStatus::Failed => l10n::failed(locale),
            anywhere_protocol::ExecutionSessionStatus::Stopped => l10n::stopped(locale),
        };
        div()
            .px(px(metrics::SPACE_XL))
            .pt(px(metrics::SPACE_XL))
            .pb(px(metrics::SPACE_MD))
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_SM))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(metrics::SPACE_MD))
                    .child(
                        div()
                            .id("task-detail-title")
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .line_clamp(2)
                            .text_size(px(metrics::ICON_SIZE))
                            .line_height(px(metrics::ICON_SIZE * metrics::CAPTION_LINE_HEIGHT))
                            .font_weight(FontWeight::SEMIBOLD)
                            .tooltip(components::text_tooltip(row.title.clone()))
                            .child(row.title.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(metrics::SPACE_XS))
                            .when(active, |element| {
                                element.child(
                                    Button::new("stop-execution")
                                        .danger()
                                        .label(if row.status
                                            == anywhere_protocol::ExecutionSessionStatus::Stopping
                                        {
                                            l10n::stopping_progress(locale)
                                        } else {
                                            l10n::stop(locale)
                                        })
                                        .disabled(
                                            row.status
                                                == anywhere_protocol::ExecutionSessionStatus::Stopping,
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.stop_selected_execution(cx)
                                        })),
                                )
                            })
                            .child(
                                Button::new("task-more-actions")
                                    .ghost()
                                    .icon(IconName::Ellipsis)
                                    .accessibility_label(l10n::more_actions(locale))
                                    .tooltip(l10n::more_actions(locale))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if this.tasks.task_actions_open {
                                            this.close_task_actions(cx);
                                        } else {
                                            this.open_task_actions(window, cx);
                                        }
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(metrics::SPACE_MD))
                    .child(if active {
                        components::status_badge_spinning(status_label, status_color)
                    } else {
                        components::status_badge(status_label, status_color, status_icon)
                    })
                    .when_some(
                        {
                            let elapsed = format_duration_ms(
                                row.started_at_ms,
                                row.finished_at_ms,
                                current_unix_ms(),
                            );
                            (!elapsed.is_empty()).then_some(elapsed)
                        },
                        |element, elapsed| {
                            element.child(
                                div()
                                    .text_size(px(metrics::CAPTION_SIZE))
                                    .text_color(theme::muted())
                                    .child(elapsed),
                            )
                        },
                    )
                    .when_some(row.exit_code, |element, code| {
                        element.child(
                            div()
                                .text_size(px(metrics::CAPTION_SIZE))
                                .text_color(theme::muted())
                                .child(l10n::exit_code(locale, &code.to_string())),
                        )
                    }),
            )
            .child(
                div()
                    .id("task-detail-directory")
                    .aria_label(readable_directory.clone())
                    .test_support()
                    .truncate()
                    .text_size(px(metrics::LABEL_SIZE))
                    .text_color(theme::secondary())
                    .tooltip(components::text_tooltip(readable_directory.clone()))
                    .child(readable_directory.clone()),
            )
            .child(
                div()
                    .id("task-command-details-toggle")
                    .role(gpui_kit::Role::Button)
                    .h(px(30.))
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_XS))
                    .cursor_pointer()
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::muted())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tasks.command_details_open = !this.tasks.command_details_open;
                        cx.notify();
                    }))
                    .child(
                        Icon::new(if self.tasks.command_details_open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .size(px(metrics::SPACE_MD)),
                    )
                    .child(l10n::command_details(locale)),
            )
            .when(self.tasks.command_details_open, |element| {
                element.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(metrics::SPACE_XS))
                        .font_family(fonts::FONT_MONO)
                        .text_size(px(metrics::PREVIEW_CODE_SIZE))
                        .line_height(px(
                            metrics::PREVIEW_CODE_SIZE * metrics::PREVIEW_CODE_LINE_HEIGHT
                        ))
                        .text_color(theme::secondary())
                        .child(row.command.clone())
                        .child(
                            div()
                                .id("task-command-directory")
                                .aria_label(readable_directory.clone())
                                .test_support()
                                .child(readable_directory.clone()),
                        ),
                )
            })
            .when_some(row.error.clone(), |element, error| {
                element.child(components::notice(error, components::NoticeKind::Error))
            })
            .when(row.output_truncated, |element| {
                element.child(components::notice(
                    l10n::showing_the_latest_output128_ki_bper(locale),
                    components::NoticeKind::Info,
                ))
            })
    }

    fn render_tasks_output_panel(
        &mut self,
        locale: Locale,
        detail: &crate::host::ExecutionDetail,
        cx: &mut Context<Self>,
    ) -> Div {
        let active = detail.row.status.is_active();
        let output = if self.tasks.diagnostics {
            detail.stderr.clone()
        } else {
            detail.stdout.clone()
        };
        let copy_output = if output.is_empty() {
            None
        } else {
            Some(output.clone())
        };
        let empty_output = if self.tasks.diagnostics {
            l10n::no_diagnostic_output(locale)
        } else if active {
            l10n::waiting_for_output(locale)
        } else {
            l10n::no_standard_output(locale)
        };
        let rendered_output = if output.is_empty() {
            SharedString::from(empty_output)
        } else {
            output
        };
        let (lines, longest_line) = self.tasks.output_lines_and_longest(rendered_output.clone());
        let diagnostic_output = self.tasks.diagnostics;
        let header =
            self.render_tasks_output_header(locale, !detail.stderr.is_empty(), copy_output, cx);
        div()
            .flex_1()
            .min_h_0()
            .mx(px(metrics::SPACE_XL))
            .mb(px(metrics::SPACE_LG))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(metrics::CONTROL_RADIUS))
            .bg(theme::field())
            .border(px(metrics::FIELD_BORDER_WIDTH))
            .border_color(theme::border())
            .child(header)
            .child(div().h(px(metrics::DIVIDER_WIDTH)).bg(theme::divider()))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                        gpui_kit::uniform_list(
                            "task-output-scroll",
                            lines.len(),
                            move |range, _, _| {
                                range
                                    .map(|index| {
                                        div()
                                            .h(px(metrics::PREVIEW_CODE_SIZE
                                                * metrics::PREVIEW_CODE_LINE_HEIGHT))
                                            .font_family(fonts::FONT_MONO)
                                            .text_size(px(metrics::PREVIEW_CODE_SIZE))
                                            .line_height(px(metrics::PREVIEW_CODE_SIZE
                                                * metrics::PREVIEW_CODE_LINE_HEIGHT))
                                            .text_color(if diagnostic_output {
                                                theme::warning()
                                            } else {
                                                theme::primary()
                                            })
                                            .child(rendered_output[lines[index].clone()].to_owned())
                                    })
                                    .collect::<Vec<_>>()
                            },
                        )
                        .size_full()
                        .p(px(metrics::SPACE_LG))
                        .with_width_from_item(longest_line)
                        .with_horizontal_sizing_behavior(
                            gpui_kit::ListHorizontalSizingBehavior::Unconstrained,
                        )
                        .track_scroll(&self.scrolls.tool_lists.tasks_output)
                        .on_scroll(cx.listener(|this, _, _, cx| {
                            let previous = this.tasks.follow_output;
                            this.tasks.update_follow_output(
                                this.scrolls.tasks_output_base().offset().y,
                                this.scrolls.tasks_output_base().max_offset().y,
                            );
                            if previous != this.tasks.follow_output {
                                cx.notify();
                            }
                        })),
                    )
                    .vertical_scrollbar(&self.scrolls.tasks_output_base())
                    .horizontal_scrollbar(&self.scrolls.tasks_output_base()),
            )
    }

    fn render_tasks_candidates(&self, locale: Locale, cx: &mut Context<Self>) -> AnyElement {
        let options = self.tasks.tool_options_open;
        let selected = if options {
            self.tasks.tool_option_selected
        } else {
            self.tasks.task_action_selected
        };
        let entries = if options {
            [
                ("tasks-refresh", IconName::RefreshCw, l10n::refresh(locale)),
                (
                    "tasks-open-launcher",
                    IconName::ArrowLeft,
                    l10n::launcher(locale),
                ),
            ]
        } else {
            [
                (
                    "task-action-copy-command",
                    IconName::Copy,
                    l10n::copy_command(locale),
                ),
                (
                    "task-action-remove-result",
                    IconName::Trash,
                    l10n::remove_result(locale),
                ),
            ]
        };
        components::action_candidates(
            "task-action-candidates",
            if options {
                l10n::tool_options(locale)
            } else {
                l10n::more_actions(locale)
            },
        )
        .debug_selector(move || {
            if options {
                "tasks-tool-options-panel".into()
            } else {
                "tasks-actions".into()
            }
        })
        .children(
            entries
                .into_iter()
                .enumerate()
                .filter(|(index, _)| {
                    options
                        || *index == 0
                        || self
                            .tasks
                            .detail
                            .as_ref()
                            .is_some_and(|detail| !detail.row.status.is_active())
                })
                .map(|(index, (id, icon, label))| {
                    components::action_candidate(id, label, icon, selected == index, true).on_click(
                        cx.listener(move |this, _, _, cx| {
                            if options {
                                this.execute_tool_option(index, cx)
                            } else {
                                this.execute_task_action(index, cx)
                            }
                        }),
                    )
                }),
        )
        .into_any_element()
    }

    pub(in crate::launcher::view) fn select_task_row(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(row_id) = self.tasks.rows.get(index).map(|row| row.id.clone()) else {
            return;
        };
        self.tasks.pending_selection = None;
        self.tasks.selected = index;
        self.tasks.clear_detail();
        self.tasks.request_follow_output();
        self.tasks.diagnostics = false;
        self.tasks.command_details_open = false;
        self.tasks.tool_options_open = false;
        self.tasks.tool_option_selected = 0;
        self.tasks.task_actions_open = false;
        self.tasks.task_action_selected = 0;
        self.request_task_detail(row_id, cx);
        self.surface_focus_handle.focus(window, cx);
        cx.notify();
    }
    fn render_task_list_row(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let locale = self.locale;
        let row = self.tasks.rows[index].clone();
        let selected = self.tasks.selected == index;
        let (status_color, status_icon) = task_status_style(row.status);
        let status_label = match row.status {
            anywhere_protocol::ExecutionSessionStatus::Running => l10n::running(locale),
            anywhere_protocol::ExecutionSessionStatus::Stopping => l10n::stopping(locale),
            anywhere_protocol::ExecutionSessionStatus::Succeeded => l10n::completed(locale),
            anywhere_protocol::ExecutionSessionStatus::Failed => l10n::failed(locale),
            anywhere_protocol::ExecutionSessionStatus::Stopped => l10n::stopped(locale),
        };
        let active = row.status.is_active();
        let elapsed = format_duration_ms(row.started_at_ms, row.finished_at_ms, current_unix_ms());
        let element = div()
            .id(SharedString::from(format!("execution-row-{index}")))
            .role(gpui_kit::Role::ListBoxOption)
            .aria_label(row.title.clone())
            .aria_selected(selected)
            .h(px(86.))
            .flex_shrink_0()
            .pb(px(4.))
            .cursor_pointer()
            .on_click(
                cx.listener(move |this, _, window, cx| this.select_task_row(index, window, cx)),
            )
            .child(
                div()
                    .h_full()
                    .px(px(metrics::ROW_PADDING_X))
                    .py(px(metrics::SPACE_SM))
                    .flex()
                    .items_center()
                    .result_surface(selected)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(metrics::SPACE_XS))
                            .child(
                                div()
                                    .id(SharedString::from(format!("execution-title-{index}")))
                                    .truncate()
                                    .body_text()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::primary())
                                    .tooltip(components::text_tooltip(row.title.clone()))
                                    .child(row.title),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("execution-directory-{index}")))
                                    .truncate()
                                    .text_size(px(metrics::LABEL_SIZE))
                                    .line_height(
                                        px(metrics::LABEL_SIZE * metrics::BODY_LINE_HEIGHT),
                                    )
                                    .text_color(if selected {
                                        theme::primary()
                                    } else {
                                        theme::secondary()
                                    })
                                    .tooltip(components::text_tooltip(row.directory.clone()))
                                    .child(directory_display(&row.directory).to_owned()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(metrics::SPACE_SM))
                                    .child(if active {
                                        components::status_badge_spinning(
                                            status_label,
                                            status_color,
                                        )
                                    } else {
                                        components::status_badge(
                                            status_label,
                                            status_color,
                                            status_icon,
                                        )
                                    })
                                    .when(!elapsed.is_empty(), |element| {
                                        element.child(
                                            div()
                                                .text_size(px(metrics::CAPTION_SIZE))
                                                .text_color(theme::muted())
                                                .child(elapsed),
                                        )
                                    })
                                    .when_some(row.exit_code, |element, code| {
                                        element.child(
                                            div()
                                                .text_size(px(metrics::CAPTION_SIZE))
                                                .text_color(theme::muted())
                                                .child(l10n::exit_code(locale, &code.to_string())),
                                        )
                                    }),
                            ),
                    ),
            );
        element.into_any_element()
    }

    fn render_tasks_toolbar(
        &self,
        locale: Locale,
        subtitle: String,
        cx: &mut Context<Self>,
    ) -> Div {
        let tool_options = div()
            .id("tasks-tool-options")
            .role(gpui_kit::Role::Button)
            .aria_label(l10n::tool_options(locale))
            .h(px(metrics::ICON_BUTTON_SIZE))
            .px(px(metrics::SPACE_SM))
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_XS))
            .rounded(px(metrics::CONTROL_RADIUS))
            .hover(|style| style.bg(theme::hover()))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, window, cx| {
                if this.tasks.tool_options_open {
                    this.close_tool_options(cx);
                } else {
                    this.open_tool_options(window, cx);
                }
            }))
            .child(
                Icon::new(IconName::Ellipsis)
                    .size(px(metrics::ICON_SIZE))
                    .text_color(theme::secondary()),
            )
            .child(
                div()
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::muted())
                    .child(l10n::tool_options(locale)),
            );
        div()
            .debug_selector(|| "tasks-toolbar".into())
            .relative()
            .px(px(metrics::SPACE_XL))
            .py(px(metrics::CONTROL_PADDING_X))
            .flex()
            .items_center()
            .gap(px(metrics::CONTROL_PADDING_X))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .body_text()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::primary())
                            .child(l10n::running_tasks(locale)),
                    )
                    .child(
                        div()
                            .truncate()
                            .caption_text()
                            .text_color(theme::muted())
                            .child(subtitle),
                    ),
            )
            .child(tool_options)
    }

    fn render_tasks_output_header(
        &self,
        locale: Locale,
        has_stderr: bool,
        copy_output: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .h(px(40.))
            .px(px(metrics::SPACE_SM))
            .py(px(metrics::ROW_INSET_Y_COMFORTABLE))
            .flex()
            .items_center()
            .child(
                div()
                    .id("task-output-tab")
                    .role(gpui_kit::Role::Button)
                    .aria_selected(!self.tasks.diagnostics)
                    .h(px(32.))
                    .px(px(metrics::SPACE_SM))
                    .flex()
                    .items_center()
                    .rounded(px(metrics::TOOLTIP_RADIUS))
                    .text_size(px(11.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if self.tasks.diagnostics {
                        theme::muted()
                    } else {
                        theme::primary()
                    })
                    .when(!self.tasks.diagnostics, |element| {
                        element.bg(theme::sidebar_selection())
                    })
                    .hover(|style| style.bg(theme::hover()))
                    .active(|style| style.bg(theme::selected()))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tasks.diagnostics = false;
                        this.tasks.request_follow_output();
                        cx.notify();
                    }))
                    .child(l10n::output(locale)),
            )
            .child(
                div()
                    .id("task-diagnostics-tab")
                    .role(gpui_kit::Role::Button)
                    .aria_selected(self.tasks.diagnostics)
                    .h(px(32.))
                    .px(px(metrics::SPACE_SM))
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_XS))
                    .rounded(px(metrics::TOOLTIP_RADIUS))
                    .text_size(px(11.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if self.tasks.diagnostics {
                        theme::primary()
                    } else {
                        theme::muted()
                    })
                    .when(self.tasks.diagnostics, |element| {
                        element.bg(theme::sidebar_selection())
                    })
                    .hover(|style| style.bg(theme::hover()))
                    .active(|style| style.bg(theme::selected()))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tasks.diagnostics = true;
                        this.tasks.request_follow_output();
                        cx.notify();
                    }))
                    .child(l10n::diagnostics(locale))
                    .when(has_stderr && !self.tasks.diagnostics, |element| {
                        element.child(
                            div()
                                .size(px(metrics::SPACE_XS))
                                .rounded(px(metrics::SPACE_XS))
                                .bg(theme::warning()),
                        )
                    }),
            )
            .child(div().flex_1())
            .child(
                Button::new("follow-task-output")
                    .ghost()
                    .label(if self.tasks.follow_output {
                        l10n::following(locale)
                    } else {
                        l10n::latest(locale)
                    })
                    .tooltip(if self.tasks.follow_output {
                        l10n::follow_new_output(locale)
                    } else {
                        l10n::jump_to_latest_output(locale)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.tasks.request_follow_output();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("copy-task-output")
                    .ghost()
                    .icon(IconName::Copy)
                    .accessibility_label(l10n::copy_output(locale))
                    .tooltip(l10n::copy_output(locale))
                    .disabled(copy_output.is_none())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let Some(output) = copy_output.clone() else {
                            return;
                        };
                        cx.write_to_clipboard(ClipboardItem::new_string(output.to_string()));
                        this.show_hud_toast(
                            l10n::copied(locale),
                            IconName::Check,
                            components::NoticeKind::Success,
                            cx,
                        );
                        cx.notify();
                    })),
            )
    }
}
