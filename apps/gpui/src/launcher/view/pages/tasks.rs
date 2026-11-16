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
            let scroll = self.tasks_output_scroll.clone();
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
        let visible_indices = self.tasks.visible_indices();
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

        let mut sidebar_rows = div()
            .id("task-list")
            .flex_1()
            .min_h(px(0.))
            .px(px(8.))
            .pb(px(8.))
            .flex()
            .flex_col()
            .track_scroll(&self.tasks_scroll)
            .overflow_y_scroll()
            .vertical_scrollbar(&self.tasks_scroll);
        for index in visible_indices.iter().copied() {
            let Some(row) = self.tasks.rows.get(index).cloned() else {
                continue;
            };
            let selected = self.tasks.selected == index;
            let (status_color, status_icon) = task_status_style(row.status);
            let status_label = match row.status {
                anywhere_protocol::ExecutionSessionStatus::Running => l10n::running(locale),
                anywhere_protocol::ExecutionSessionStatus::Stopping => l10n::stopping(locale),
                anywhere_protocol::ExecutionSessionStatus::Succeeded => l10n::completed(locale),
                anywhere_protocol::ExecutionSessionStatus::Failed => l10n::failed(locale),
                anywhere_protocol::ExecutionSessionStatus::Stopped => l10n::stopped(locale),
            };
            let color = if selected {
                theme::primary()
            } else {
                status_color
            };
            sidebar_rows = sidebar_rows.child(
                div()
                    .id(SharedString::from(format!("execution-row-{index}")))
                    .role(gpui_kit::Role::ListBoxOption)
                    .aria_label(row.title.clone())
                    .aria_selected(selected)
                    .h(px(86.))
                    .flex_shrink_0()
                    .pb(px(4.))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_task_row(index, window, cx)
                    }))
                    .child(
                        div()
                            .h_full()
                            .px(px(10.))
                            .py(px(8.))
                            .flex()
                            .items_center()
                            .result_surface(selected)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "execution-title-{index}"
                                            )))
                                            .truncate()
                                            .body_text()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme::primary())
                                            .tooltip(components::text_tooltip(row.title.clone()))
                                            .child(row.title),
                                    )
                                    .child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "execution-directory-{index}"
                                            )))
                                            .truncate()
                                            .text_size(px(metrics::LABEL_SIZE))
                                            .line_height(px(
                                                metrics::LABEL_SIZE * metrics::BODY_LINE_HEIGHT
                                            ))
                                            .text_color(if selected {
                                                theme::primary()
                                            } else {
                                                theme::secondary()
                                            })
                                            .tooltip(components::text_tooltip(
                                                row.directory.clone(),
                                            ))
                                            .child(directory_display(&row.directory).to_owned()),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.))
                                            .child(components::status_badge(
                                                status_label,
                                                color,
                                                status_icon,
                                            ))
                                            .when_some(row.exit_code, |element, code| {
                                                element.child(
                                                    div()
                                                        .text_size(px(11.))
                                                        .text_color(if selected {
                                                            theme::primary()
                                                        } else {
                                                            theme::muted()
                                                        })
                                                        .child(l10n::exit_code(
                                                            locale,
                                                            &code.to_string(),
                                                        )),
                                                )
                                            }),
                                    ),
                            ),
                    ),
            );
        }

        let sidebar = div()
            .w(px(252.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme::sidebar())
            .child(
                div()
                    .mx(px(10.))
                    .mt(px(12.))
                    .mb(px(8.))
                    .h(px(34.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .bg(theme::field())
                    .border(px(metrics::FIELD_BORDER_WIDTH))
                    .border_color(theme::border())
                    .child(
                        Icon::new(IconName::Search)
                            .size(px(16.))
                            .text_color(theme::muted()),
                    )
                    .child(
                        div().flex_1().min_w(px(0.)).child(
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
                    .px(px(16.))
                    .pt(px(4.))
                    .pb(px(10.))
                    .flex()
                    .items_center()
                    .text_size(px(11.))
                    .text_color(theme::muted())
                    .child(div().flex_1().child(l10n::tasks(locale)))
                    .child(visible_indices.len().to_string()),
            )
            .child(sidebar_rows);

        let detail = self.tasks.detail.clone();
        let detail_area = if self.tasks.rows.is_empty() {
            components::empty_state(
                IconName::Terminal,
                l10n::no_tasks_yet(locale),
                Some(l10n::choose_aproject_build_test_or_run(locale)),
            )
        } else if visible_indices.is_empty() {
            components::empty_state(IconName::Search, l10n::no_matching_tasks(locale), None)
        } else if let Some(detail) = detail {
            let row = detail.row.clone();
            let active = row.status.is_active();
            let output = if self.tasks.diagnostics {
                detail.stderr.as_str()
            } else {
                detail.stdout.as_str()
            };
            let copy_output = if output.is_empty() {
                None
            } else {
                Some(output.to_owned())
            };
            let (status_color, status_icon) = task_status_style(row.status);
            let status_label = match row.status {
                anywhere_protocol::ExecutionSessionStatus::Running => l10n::running(locale),
                anywhere_protocol::ExecutionSessionStatus::Stopping => {
                    l10n::stopping_progress(locale)
                }
                anywhere_protocol::ExecutionSessionStatus::Succeeded => l10n::completed(locale),
                anywhere_protocol::ExecutionSessionStatus::Failed => l10n::failed(locale),
                anywhere_protocol::ExecutionSessionStatus::Stopped => l10n::stopped(locale),
            };
            let empty_output = if self.tasks.diagnostics {
                l10n::no_diagnostic_output(locale)
            } else if active {
                l10n::waiting_for_output(locale)
            } else {
                l10n::no_standard_output(locale)
            };
            let metadata = div()
                .px(px(20.))
                .pt(px(18.))
                .pb(px(12.))
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(12.))
                        .child(
                            div()
                                .id("task-detail-title")
                                .flex_1()
                                .min_w(px(0.))
                                .overflow_hidden()
                                .line_clamp(2)
                                .text_size(px(16.))
                                .line_height(px(16. * 1.35))
                                .font_weight(FontWeight::SEMIBOLD)
                                .tooltip(components::text_tooltip(row.title.clone()))
                                .child(row.title),
                        )
                        .child(
                            div()
                                .flex_none()
                                .flex()
                                .items_center()
                                .gap(px(6.))
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
                        .gap(px(12.))
                        .child(components::status_badge(
                            status_label,
                            status_color,
                            status_icon,
                        ))
                        .when_some(row.exit_code, |element, code| {
                            element.child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme::muted())
                                    .child(l10n::exit_code(locale, &code.to_string())),
                            )
                        }),
                )
                .child(
                    div()
                        .id("task-detail-directory")
                        .truncate()
                        .text_size(px(metrics::LABEL_SIZE))
                        .text_color(theme::secondary())
                        .tooltip(components::text_tooltip(row.directory.clone()))
                        .child(row.directory.clone()),
                )
                .child(
                    div()
                        .id("task-command-details-toggle")
                        .role(gpui_kit::Role::Button)
                        .h(px(30.))
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .cursor_pointer()
                        .text_size(px(11.))
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
                            .size(px(12.)),
                        )
                        .child(l10n::command_details(locale)),
                )
                .when(self.tasks.command_details_open, |element| {
                    element.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .font_family(fonts::FONT_MONO)
                            .text_size(px(metrics::PREVIEW_CODE_SIZE))
                            .line_height(px(
                                metrics::PREVIEW_CODE_SIZE
                                    * metrics::PREVIEW_CODE_LINE_HEIGHT,
                            ))
                            .text_color(theme::secondary())
                            .child(row.command.clone())
                            .child(row.directory.clone()),
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
                });
            let output_panel = div()
                .flex_1()
                .min_h(px(0.))
                .mx(px(20.))
                .mb(px(16.))
                .flex()
                .flex_col()
                .overflow_hidden()
                .rounded(px(metrics::CONTROL_RADIUS))
                .bg(theme::field())
                .border(px(metrics::FIELD_BORDER_WIDTH))
                .border_color(theme::border())
                .child(
                    div()
                        .h(px(40.))
                        .px(px(8.))
                        .py(px(3.))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .id("task-output-tab")
                                .role(gpui_kit::Role::Button)
                                .aria_selected(!self.tasks.diagnostics)
                                .h(px(32.))
                                .px(px(8.))
                                .flex()
                                .items_center()
                                .rounded(px(6.))
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
                                .px(px(8.))
                                .flex()
                                .items_center()
                                .gap(px(5.))
                                .rounded(px(6.))
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
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.tasks.diagnostics = true;
                                    this.tasks.request_follow_output();
                                    cx.notify();
                                }))
                                .child(l10n::diagnostics(locale))
                                .when(
                                    !detail.stderr.is_empty() && !self.tasks.diagnostics,
                                    |element| {
                                        element.child(
                                            div().size(px(5.)).rounded(px(3.)).bg(theme::warning()),
                                        )
                                    },
                                ),
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
                                    cx.write_to_clipboard(ClipboardItem::new_string(output));
                                    this.set_notice(
                                        l10n::copied(locale),
                                        components::NoticeKind::Success,
                                    );
                                    cx.notify();
                                })),
                        ),
                )
                .child(div().h(px(metrics::DIVIDER_WIDTH)).bg(theme::divider()))
                .child(
                    div()
                        .id("task-output-scroll")
                        .debug_selector(|| "task-output-scroll".into())
                        .flex_1()
                        .min_h(px(0.))
                        .p(px(14.))
                        .track_scroll(&self.tasks_output_scroll)
                        .on_scroll(cx.listener(|this, _, _, cx| {
                            let offset = this.tasks_output_scroll.offset().y;
                            let maximum = this.tasks_output_scroll.max_offset().y;
                            this.tasks.update_follow_output(offset, maximum);
                            cx.notify();
                        }))
                        .overflow_y_scroll()
                        .vertical_scrollbar(&self.tasks_output_scroll)
                        .child(
                            div()
                                .font_family(fonts::FONT_MONO)
                                .text_size(px(metrics::PREVIEW_CODE_SIZE))
                                .line_height(px(
                                    metrics::PREVIEW_CODE_SIZE * metrics::PREVIEW_CODE_LINE_HEIGHT
                                ))
                                .whitespace_normal()
                                .text_color(if self.tasks.diagnostics {
                                    theme::warning()
                                } else {
                                    theme::primary()
                                })
                                .child(if output.is_empty() {
                                    empty_output
                                } else {
                                    output.to_owned()
                                }),
                        ),
                );
            div()
                .flex_1()
                .min_w(px(0.))
                .h_full()
                .flex()
                .flex_col()
                .child(metadata)
                .child(output_panel)
                .into_any_element()
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
                .min_h(px(0.))
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

        let tool_options = div()
            .id("tasks-tool-options")
            .role(gpui_kit::Role::Button)
            .aria_label(l10n::tool_options(locale))
            .h(px(26.))
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(6.))
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
                    .size(px(16.))
                    .text_color(theme::secondary()),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(theme::muted())
                    .child(l10n::tool_options(locale)),
            );
        let toolbar = div()
            .debug_selector(|| "tasks-toolbar".into())
            .relative()
            .px(px(18.))
            .py(px(10.))
            .flex()
            .items_center()
            .gap(px(10.))
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
            .child(tool_options);
        let tool_options_panel = self.tasks.tool_options_open.then(|| {
            div()
                .debug_selector(|| "tasks-tool-options-panel".into())
                .w(px(210.))
                .p(px(6.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .rounded(px(metrics::CARD_RADIUS))
                .bg(theme::menu_surface())
                .border(px(metrics::DIVIDER_WIDTH))
                .border_color(theme::control_border())
                .child(
                    div()
                        .id("tasks-refresh")
                        .role(gpui_kit::Role::Button)
                        .h(px(32.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(6.))
                        .when(self.tasks.tool_option_selected == 0, |element| {
                            element.bg(theme::sidebar_selection())
                        })
                        .hover(|style| style.bg(theme::hover()))
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.tasks.tool_option_selected = 0;
                            this.execute_tool_option(0, cx);
                        }))
                        .child(
                            Icon::new(IconName::RefreshCw)
                                .size(px(15.))
                                .text_color(theme::secondary()),
                        )
                        .child(l10n::shortcut_hint(locale, &l10n::refresh(locale), "⌘R")),
                )
                .child(
                    div()
                        .id("tasks-open-launcher")
                        .debug_selector(|| "tasks-open-launcher".into())
                        .role(gpui_kit::Role::Button)
                        .h(px(32.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(6.))
                        .when(self.tasks.tool_option_selected == 1, |element| {
                            element.bg(theme::sidebar_selection())
                        })
                        .hover(|style| style.bg(theme::hover()))
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.tasks.tool_option_selected = 1;
                            this.execute_tool_option(1, cx);
                        }))
                        .child(
                            Icon::new(IconName::ArrowLeft)
                                .size(px(15.))
                                .text_color(theme::secondary()),
                        )
                        .child(l10n::shortcut_hint(locale, &l10n::launcher(locale), "⌘L")),
                )
                .into_any_element()
        });
        let task_actions_panel = (self.tasks.task_actions_open && self.tasks.detail.is_some())
            .then(|| {
                let active = self
                    .tasks
                    .detail
                    .as_ref()
                    .is_some_and(|detail| detail.row.status.is_active());
                let copy_selected = self.tasks.task_action_selected == 0;
                let remove_selected = self.tasks.task_action_selected == 1;
                div()
                    .debug_selector(|| "tasks-actions".into())
                    .w(px(210.))
                    .overflow_hidden()
                    .rounded(px(metrics::CARD_RADIUS))
                    .bg(theme::menu_surface())
                    .border(px(metrics::DIVIDER_WIDTH))
                    .border_color(theme::control_border())
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
                                    .text_size(px(metrics::BODY_SIZE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(l10n::more_actions(locale)),
                            )
                            .child(self.keycap("esc")),
                    )
                    .child(div().h(px(metrics::DIVIDER_WIDTH)).bg(theme::divider()))
                    .child(
                        div()
                            .p(px(4.))
                            .flex()
                            .flex_col()
                            .child(
                                Button::new("task-action-copy-command")
                                    .when(copy_selected, |button| button.secondary())
                                    .when(!copy_selected, |button| button.ghost())
                                    .label(l10n::copy_command(locale))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.tasks.task_action_selected = 0;
                                        this.execute_task_action(0, cx);
                                    })),
                            )
                            .when(!active, |element| {
                                element.child(
                                    Button::new("task-action-remove-result")
                                        .when(remove_selected, |button| button.secondary())
                                        .when(!remove_selected, |button| button.ghost())
                                        .label(l10n::remove_result(locale))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tasks.task_action_selected = 1;
                                            this.execute_task_action(1, cx);
                                        })),
                                )
                            }),
                    )
                    .into_any_element()
            });

        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .rounded(px(LAUNCHER_CORNER_RADIUS))
            .overflow_hidden()
            .when(self.standalone_tasks, |element| {
                element.pt(px(metrics::TITLE_BAR_INSET))
            })
            .child(toolbar)
            .when_some(self.notice.clone(), |element, notice| {
                let notice_kind = self.notice_kind;
                element.child(
                    div()
                        .px(px(18.))
                        .pb(px(12.))
                        .child(components::notice(notice, notice_kind)),
                )
            })
            .child(div().h(px(metrics::DIVIDER_WIDTH)).bg(theme::divider()))
            .child(body)
            .child(div().h(px(metrics::DIVIDER_WIDTH)).bg(theme::divider()))
            .child(
                div()
                    .px(px(18.))
                    .py(px(9.))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(11.))
                            .text_color(theme::muted())
                            .child(l10n::tasks_keep_running_when_hidden(locale)),
                    )
                    .child(
                        Button::new("tasks-keyboard-help-footer")
                            .ghost()
                            .label(l10n::keyboard_help(locale))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_notice(
                                    l10n::task_keyboard_help(this.locale, "⌘F", "⌘."),
                                    components::NoticeKind::Info,
                                );
                                cx.notify();
                            })),
                    ),
            )
            .children(tool_options_panel.map(|panel| {
                Self::page_popup(window, px(metrics::TITLE_BAR_INSET + 52.), px(18.), panel)
            }))
            .children(task_actions_panel.map(|panel| {
                Self::page_popup(window, px(metrics::TITLE_BAR_INSET + 84.), px(18.), panel)
            }))
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
        self.tasks.selected = index;
        self.tasks.detail = None;
        self.tasks.detail_request = self.tasks.detail_request.wrapping_add(1);
        self.tasks.request_follow_output();
        self.tasks.diagnostics = false;
        self.tasks.command_details_open = false;
        self.tasks.tool_options_open = false;
        self.tasks.tool_option_selected = 0;
        self.tasks.task_actions_open = false;
        self.tasks.task_action_selected = 0;
        self.request_task_detail(row_id, cx);
        window.blur(cx);
        cx.notify();
    }
}
