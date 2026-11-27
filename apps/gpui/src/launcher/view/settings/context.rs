use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn context_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let file_selection_label = crate::platform::capabilities::file_selection_label(self.locale);
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .child(components::section_heading(
                l10n::context_sources(self.locale),
                IconName::Layers,
            ))
            .child(
                div()
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::muted())
                    .child(crate::platform::capabilities::context_sources_description(
                        self.locale,
                    )),
            )
            .when(
                crate::platform::capabilities::DESKTOP_CONTEXT_CAPTURE,
                |page| {
                    page.child(self.settings_rows_card(vec![
                self.context_capture_row(
                    ContextSource::FinderSelection,
                    file_selection_label.clone(),
                    l10n::selected_files_description(self.locale),
                    self.settings_editor.draft.capture_finder_selection,
                    Switch::new("capture-finder")
                        .checked(self.settings_editor.draft.capture_finder_selection)
                        .accessibility_label(file_selection_label)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_editor.draft.capture_finder_selection = *checked;
                            cx.notify();
                        })),
                    cx,
                ),
                self.context_capture_row(
                    ContextSource::SelectedText,
                    l10n::selected_text(self.locale),
                    l10n::selected_text_description(self.locale),
                    self.settings_editor.draft.capture_selected_text,
                    Switch::new("capture-selected-text")
                        .checked(self.settings_editor.draft.capture_selected_text)
                        .accessibility_label(l10n::selected_text(self.locale))
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_editor.draft.capture_selected_text = *checked;
                            cx.notify();
                        })),
                    cx,
                ),
                self.context_capture_row(
                    ContextSource::BrowserCurrentPage,
                    l10n::browser_current_page(self.locale),
                    l10n::browser_page_description(self.locale),
                    self.settings_editor.draft.capture_browser_current_page,
                    Switch::new("capture-browser")
                        .checked(self.settings_editor.draft.capture_browser_current_page)
                        .accessibility_label(l10n::browser_current_page(self.locale))
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_editor.draft.capture_browser_current_page = *checked;
                            cx.notify();
                        })),
                    cx,
                ),
            ]))
                },
            )
    }

    pub(in crate::launcher::view) fn context_capture_row(
        &self,
        source: ContextSource,
        title: String,
        subtitle: String,
        enabled: bool,
        control: Switch,
        cx: &mut Context<Self>,
    ) -> Div {
        let capture = self
            .model
            .context_captures()
            .iter()
            .rev()
            .find(|capture| capture.source == source);
        let (status, detail, attention) = if !enabled {
            (
                l10n::capture_off(self.locale),
                l10n::last_context_capture(self.locale),
                false,
            )
        } else if let Some(capture) = capture {
            (
                context_capture_status_label(capture.status, source, self.locale),
                context_capture_status_detail(capture.status, source, self.locale),
                matches!(
                    capture.status,
                    ContextCaptureStatus::PermissionDenied
                        | ContextCaptureStatus::ApplicationChanged
                        | ContextCaptureStatus::TimedOut
                        | ContextCaptureStatus::TooLarge
                        | ContextCaptureStatus::TooManyItems
                        | ContextCaptureStatus::Unavailable
                        | ContextCaptureStatus::Failed
                ),
            )
        } else {
            (
                l10n::capture_not_read(self.locale),
                l10n::context_sources_not_captured(self.locale),
                false,
            )
        };
        let privacy_pane = crate::platform::capabilities::FILE_SEARCH_PRIVACY_SETTINGS
            .then(|| capture.and_then(|capture| privacy_pane_for_capture(source, capture.status)))
            .flatten();
        div()
            .min_h(px(70.))
            .px(px(metrics::SPACE_MD))
            .py(px(metrics::SPACE_SM))
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_LG))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        div()
                            .truncate()
                            .body_text()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::primary())
                            .child(title),
                    )
                    .child(
                        div()
                            .caption_text()
                            .text_color(theme::secondary())
                            .child(subtitle),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("context-status-{source:?}")))
                            .tooltip(components::text_tooltip(detail.clone()))
                            .aria_description(detail)
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .text_size(px(10.5))
                            .line_height(px(10.5 * metrics::CAPTION_LINE_HEIGHT))
                            .text_color(if attention {
                                theme::warning()
                            } else {
                                theme::muted()
                            })
                            .child(
                                Icon::new(if attention {
                                    IconName::TriangleAlert
                                } else {
                                    IconName::CircleCheck
                                })
                                .size(px(11.)),
                            )
                            .child(div().flex_1().min_w(px(0.)).truncate().child(status))
                            .when_some(privacy_pane, |element, pane| {
                                element.child(
                                    Button::new(SharedString::from(format!(
                                        "context-open-settings-{source:?}"
                                    )))
                                    .ghost()
                                    .label(l10n::open_settings(self.locale))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !open_privacy_settings(pane) {
                                            this.set_notice(
                                                l10n::open_system_settings_failed(this.locale),
                                                components::NoticeKind::Error,
                                            );
                                            cx.notify();
                                        }
                                    })),
                                )
                            }),
                    ),
            )
            .child(control)
    }
}
