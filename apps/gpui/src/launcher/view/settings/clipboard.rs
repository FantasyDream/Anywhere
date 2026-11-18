use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn clipboard_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let status = self.clipboard.status.as_ref();
        let retention_label = match status.map(|status| status.retention_days) {
            Some(0) => l10n::until_storage_limit(self.locale),
            Some(days) => l10n::retention_days(self.locale, i64::from(days)),
            None => l10n::history_storage(self.locale),
        };
        let pause_label = if status.is_some_and(|status| status.paused_until_ms.is_some()) {
            l10n::paused(self.locale)
        } else {
            l10n::pause_for(self.locale)
        };
        let storage_usage = status.and_then(|status| status.storage_usage);
        let controls_disabled = storage_usage.is_none() || self.clipboard.loading;
        let choice_menu = self.clipboard_settings_choice_menu(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .child(self.settings_rows_card(vec![
                self.settings_row_bare(
                    l10n::record_clipboard_history(self.locale),
                    l10n::save_future_copies_while_anywhere_is_running(self.locale),
                    Switch::new("clipboard-recording")
                        .checked(self.settings.clipboard_recording)
                        .disabled(self.settings.saving)
                        .accessibility_label(l10n::clipboard(self.locale))
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.set_clipboard_recording(*checked, cx);
                        })),
                ),
                self.settings_row_bare(
                    l10n::history_storage(self.locale),
                    storage_usage
                        .map(|(entries, bytes)| {
                            l10n::history_storage_usage(
                                self.locale,
                                i64::from(entries),
                                &format!("{:.1}", f64::from(bytes) / 1_048_576.0),
                            )
                        })
                        .unwrap_or_else(|| l10n::clipboard_history(self.locale)),
                    div()
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_SM))
                        .child(
                            Button::new("clipboard-retention")
                                .secondary()
                                .disabled(controls_disabled)
                                .label(retention_label)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.clipboard_settings_choice = Some(
                                        ClipboardSettingsChoice::Retention,
                                    );
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("clipboard-pause")
                                .secondary()
                                .disabled(controls_disabled)
                                .label(pause_label)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.clipboard_settings_choice =
                                        Some(ClipboardSettingsChoice::Pause);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("clipboard-clear-settings")
                                .secondary()
                                .disabled(controls_disabled)
                                .label(l10n::clear_history(self.locale))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.present_clipboard_confirmation(
                                        ClipboardConfirmation::Clear,
                                        window,
                                        cx,
                                    );
                                })),
                        ),
                ),
            ]))
            .when_some(choice_menu, |element, menu| element.child(menu))
            .child(
                div()
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::muted())
                    .child(l10n::text_images_rich_text_and_file_references(self.locale)),
            )
            .child(components::section_heading(
                l10n::when_opening_launcher(self.locale),
                IconName::Search,
            ))
            .child(self.settings_rows_card(vec![self.context_capture_row(
                ContextSource::Clipboard,
                l10n::clipboard_text(self.locale),
                l10n::auto_fill_supported_content_copied_within_the(self.locale),
                self.settings_draft.capture_clipboard,
                Switch::new("capture-clipboard")
                    .checked(self.settings_draft.capture_clipboard)
                    .on_change(cx.listener(|this, checked, _, cx| {
                        this.settings_draft.capture_clipboard = *checked;
                        cx.notify();
                    })),
                cx,
            )]))
    }

    pub(in crate::launcher::view) fn clipboard_settings_choice_menu(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let choice = self.clipboard_settings_choice?;
        let status = self.clipboard.status.as_ref();
        let (title, options): (String, Vec<(String, usize, bool)>) = match choice {
            ClipboardSettingsChoice::Retention => (
                l10n::keep_history_for(self.locale),
                [0_u32, 1, 7, 30, 90]
                    .into_iter()
                    .map(|days| {
                        let label = if days == 0 {
                            l10n::until_storage_limit(self.locale)
                        } else {
                            l10n::retention_days(self.locale, i64::from(days))
                        };
                        (
                            label,
                            days as usize,
                            status.is_some_and(|status| status.retention_days == days),
                        )
                    })
                    .collect(),
            ),
            ClipboardSettingsChoice::Pause => {
                let paused = status.is_some_and(|status| status.paused_until_ms.is_some());
                let mut options = vec![(l10n::resume_recording(self.locale), 0, !paused)];
                options.extend([5_u32, 15, 60].into_iter().map(|minutes| {
                    (
                        l10n::pause_minutes(self.locale, i64::from(minutes)),
                        minutes as usize,
                        false,
                    )
                }));
                (l10n::pause_history_recording(self.locale), options)
            }
        };
        let choice_id = match choice {
            ClipboardSettingsChoice::Retention => "retention",
            ClipboardSettingsChoice::Pause => "pause",
        };
        let options = options.into_iter().map(|(label, value, selected)| {
            let id = match choice {
                ClipboardSettingsChoice::Retention => {
                    format!("clipboard-retention-{value}")
                }
                ClipboardSettingsChoice::Pause if value == 0 => "clipboard-resume".to_owned(),
                ClipboardSettingsChoice::Pause => format!("clipboard-pause-{value}"),
            };
            Button::new(SharedString::from(id))
                .when(selected, |button| button.secondary())
                .when(!selected, |button| button.ghost())
                .label(label)
                .on_click(cx.listener(move |this, _, window, cx| match choice {
                    ClipboardSettingsChoice::Retention => {
                        this.select_clipboard_retention(value as u32, window, cx)
                    }
                    ClipboardSettingsChoice::Pause => this.select_clipboard_pause(value as u32, cx),
                }))
        });
        Some(
            div()
                .id(SharedString::from(format!("clipboard-{choice_id}-choices")))
                .test_support()
                .p(px(metrics::SPACE_MD))
                .flex()
                .flex_col()
                .gap(px(metrics::SPACE_SM))
                .rounded(px(metrics::CARD_RADIUS))
                .bg(theme::field())
                .child(div().caption_text().text_color(theme::muted()).child(title))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(metrics::SPACE_SM))
                        .children(options),
                )
                .into_any_element(),
        )
    }

    pub(in crate::launcher::view) fn select_clipboard_retention(
        &mut self,
        days: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self
            .clipboard
            .status
            .as_ref()
            .map(|status| status.retention_days)
        else {
            return;
        };
        self.clipboard_settings_choice = None;
        if clipboard_retention_removes_entries(current, days) {
            self.present_clipboard_confirmation(
                ClipboardConfirmation::SetRetention(days),
                window,
                cx,
            );
            return;
        }
        self.set_clipboard_retention_now(days, cx);
    }

    pub(in crate::launcher::view) fn set_clipboard_retention_now(
        &mut self,
        days: u32,
        cx: &mut Context<Self>,
    ) {
        let Some(live) = self.live.clone() else {
            return;
        };
        let receiver = live.set_clipboard_retention(days);
        self.run_clipboard_command(receiver, cx);
    }

    pub(in crate::launcher::view) fn select_clipboard_pause(
        &mut self,
        minutes: u32,
        cx: &mut Context<Self>,
    ) {
        self.clipboard_settings_choice = None;
        let Some(live) = self.live.clone() else {
            return;
        };
        let receiver = live.set_clipboard_pause(minutes);
        self.run_clipboard_command(receiver, cx);
        cx.notify();
    }

    pub(in crate::launcher::view) fn present_clipboard_confirmation(
        &mut self,
        confirmation: ClipboardConfirmation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.clipboard.loading {
            return;
        }
        self.settings_close_pending = false;
        self.settings_exit_intent = None;
        self.clipboard_settings_choice = None;
        if self.settings_modal_return_focus.is_none() {
            self.settings_modal_return_focus = window.focused(cx);
        }
        self.clipboard_confirmation = Some(confirmation);
        let focus = self.settings_confirm_cancel_focus.clone();
        cx.defer_in(window, move |this, window, cx| {
            if this.clipboard_confirmation.is_some() {
                focus.focus(window, cx);
            }
        });
        cx.notify();
    }

    pub(in crate::launcher::view) fn cancel_clipboard_confirmation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clipboard_confirmation = None;
        self.restore_settings_modal_focus(window, cx);
        cx.notify();
    }

    pub(in crate::launcher::view) fn confirm_clipboard_confirmation(
        &mut self,
        confirmation: ClipboardConfirmation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A queued click or repeated activation may carry a stale closure after
        // the first confirmation dismisses the prompt. Only the confirmation
        // that is still presented may submit a destructive operation.
        if !clipboard_confirmation_is_submittable(
            self.clipboard.loading,
            self.clipboard_confirmation,
            confirmation,
        ) {
            return;
        }
        let Some(live) = self.live.clone() else {
            return;
        };
        self.clipboard_confirmation = None;
        self.restore_settings_modal_focus(window, cx);
        let receiver = match confirmation {
            ClipboardConfirmation::Clear => live.clear_clipboard(),
            ClipboardConfirmation::SetRetention(days) => live.set_clipboard_retention(days),
        };
        self.run_clipboard_command(receiver, cx);
        cx.notify();
    }
}
