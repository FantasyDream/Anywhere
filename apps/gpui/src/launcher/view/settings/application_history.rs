use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn application_history_settings(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Div {
        let filter = self
            .settings_editor
            .history_filter_input
            .read(cx)
            .value()
            .trim()
            .to_ascii_lowercase();
        let enabled_only = self.settings_editor.history_enabled_only;
        let sources = self
            .app_history
            .sources
            .clone()
            .into_iter()
            .filter(|source| !enabled_only || source.action_enabled)
            .filter(|source| {
                filter.is_empty()
                    || source.id.to_ascii_lowercase().contains(&filter)
                    || source.title.to_ascii_lowercase().contains(&filter)
                    || source.file_hint.to_ascii_lowercase().contains(&filter)
                    || source.path.as_ref().is_some_and(|path| {
                        path.to_string_lossy()
                            .to_ascii_lowercase()
                            .contains(&filter)
                    })
            })
            .collect::<Vec<_>>();
        let has_source_filter = !filter.is_empty() || enabled_only;
        let sources_empty = sources.is_empty();
        let filter_input = self.settings_editor.history_filter_input.clone();
        let window_handle = self.window_handle;
        let adapters_directory = self
            .live
            .as_ref()
            .map(|live| live.app_history_adapters_directory().display().to_string())
            .unwrap_or_else(|| {
                "~/Library/Application Support/Anywhere/application-history-sources".to_owned()
            });
        let mut rows = div().flex().flex_col().gap(px(metrics::SPACE_SM));
        for source in sources {
            rows = rows.child(self.render_app_history_source_card(source, cx));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_MD))
            .child(
                div()
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::secondary())
                    .child(l10n::app_history_source_description(self.locale)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(metrics::SPACE_SM))
                    .child(components::search_field(
                        "history-settings-filter",
                        &self.settings_editor.history_filter_input,
                        l10n::app_history_filter_sources(self.locale),
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(metrics::SPACE_SM))
                            .child(
                                Button::new("history-filter-all")
                                    .when(!enabled_only, |button| button.primary())
                                    .when(enabled_only, |button| button.secondary())
                                    .label(l10n::app_history_all_sources(self.locale))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings_editor.history_enabled_only = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("history-filter-enabled")
                                    .when(enabled_only, |button| button.primary())
                                    .when(!enabled_only, |button| button.secondary())
                                    .label(l10n::enabled(self.locale))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings_editor.history_enabled_only = true;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                self.settings_field_row(
                    l10n::app_history_custom_adapters(self.locale),
                    l10n::app_history_custom_adapters_help(self.locale),
                    "app-history-custom-adapters-row",
                    div()
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_SM))
                        .flex_wrap()
                        .child(
                            div()
                                .min_w(px(120.))
                                .flex_1()
                                .max_w(px(360.))
                                .truncate()
                                .text_size(px(metrics::CAPTION_SIZE))
                                .text_color(theme::muted())
                                .child(adapters_directory),
                        )
                        .child(
                            Button::new("open-app-history-adapters")
                                .secondary()
                                .label(l10n::app_history_open_adapters(self.locale))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let Some(live) = this.live.clone() else {
                                        return;
                                    };
                                    let receiver = live.open_app_history_adapters_directory();
                                    this.poll(receiver, this.live_state.generation, cx);
                                    cx.notify();
                                })),
                        ),
                ),
            )
            .child(rows)
            .when(sources_empty, |element| {
                element.child(
                    div()
                        .p(px(metrics::SPACE_MD))
                        .rounded(px(metrics::CARD_RADIUS))
                        .bg(theme::field())
                        .text_size(px(metrics::CAPTION_SIZE))
                        .text_color(theme::secondary())
                        .child(if has_source_filter {
                            l10n::no_matching_entries(self.locale)
                        } else {
                            l10n::app_history_not_detected(self.locale)
                        }),
                )
            })
            .when(has_source_filter && sources_empty, |element| {
                element.child(
                    Button::new("history-clear-source-filter")
                        .secondary()
                        .label(l10n::app_history_show_all_sources(self.locale))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.settings_editor.history_enabled_only = false;
                            let _ = cx.update_window(window_handle, |_, window, cx| {
                                set_input_value(&filter_input, String::new(), window, cx);
                            });
                            cx.notify();
                        })),
                )
            })
    }

    fn render_app_history_source_card(
        &self,
        source: crate::host::AppHistorySource,
        cx: &mut Context<Self>,
    ) -> Div {
        use anywhere_core::features::application_history::model::SourceMode;
        let editing =
            self.settings_editor.history_path_editing.as_deref() == Some(source.id.as_str());
        let id = source.id.clone();
        let configure_id = id.clone();
        let enabled_id = id.clone();
        let automatic_id = id.clone();
        let custom_id = id.clone();
        let disabled_id = id;
        let action_accessibility = format!(
            "{} {}",
            source.title,
            l10n::app_history_action_enabled(self.locale)
        );
        let mode_label = match source.mode {
            anywhere_core::features::application_history::model::SourceMode::Automatic => {
                l10n::app_history_automatic(self.locale)
            }
            anywhere_core::features::application_history::model::SourceMode::Custom => {
                l10n::app_history_custom(self.locale)
            }
            anywhere_core::features::application_history::model::SourceMode::Disabled => {
                l10n::app_history_source_off(self.locale)
            }
        };
        let path = source
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| source.file_hint.clone());
        div()
            .p(px(metrics::SPACE_MD))
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_SM))
            .rounded(px(metrics::CARD_RADIUS))
            .bg(theme::field())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_MD))
                    .child(
                        div()
                            .min_w(px(0.))
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .truncate()
                                    .body_text()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(source.title),
                            )
                            .child(
                                div()
                                    .min_w(px(0.))
                                    .truncate()
                                    .caption_text()
                                    .text_color(theme::secondary())
                                    .child(format!("{mode_label} · {path}")),
                            ),
                    )
                    .child(
                        Switch::new(SharedString::from(format!("history-action-{enabled_id}")))
                            .checked(source.action_enabled)
                            .accessibility_label(action_accessibility)
                            .on_change(cx.listener(move |this, checked, _, cx| {
                                if let Some(live) = this.live.clone() {
                                    let receiver = live.set_app_history_action_enabled(
                                        enabled_id.clone(),
                                        *checked,
                                    );
                                    this.poll(receiver, this.live_state.generation, cx);
                                }
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(metrics::SPACE_XS))
                    .child(
                        components::filter_chip(
                            SharedString::from(format!("history-auto-{automatic_id}")),
                            l10n::source_mode_automatic(self.locale),
                            !editing && source.mode == SourceMode::Automatic,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.settings_editor.history_path_editing = None;
                            if let Some(live) = this.live.as_ref() {
                                let receiver = live.configure_app_history(
                                    automatic_id.clone(),
                                    String::new(),
                                    SourceMode::Automatic,
                                );
                                this.poll(receiver, this.live_state.generation, cx);
                            }
                            cx.notify();
                        })),
                    )
                    .child(
                        components::filter_chip(
                            SharedString::from(format!("history-select-{configure_id}")),
                            l10n::source_mode_custom(self.locale),
                            editing || source.mode == SourceMode::Custom,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                if this.settings_editor.history_path_editing.as_deref()
                                    == Some(configure_id.as_str())
                                {
                                    this.settings_editor.history_path_editing = None;
                                } else {
                                    this.settings_editor.history_path_editing =
                                        Some(configure_id.clone());
                                    let value = this
                                        .app_history
                                        .sources
                                        .iter()
                                        .find(|source| source.id == configure_id)
                                        .and_then(|source| source.path.as_ref())
                                        .map(|path| path.display().to_string())
                                        .unwrap_or_default();
                                    set_input_value(
                                        &this.settings_editor.history_path_input,
                                        value,
                                        window,
                                        cx,
                                    );
                                    this.settings_editor
                                        .history_path_input
                                        .update(cx, |input, cx| input.focus(window, cx));
                                }
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        components::filter_chip(
                            SharedString::from(format!("history-off-{disabled_id}")),
                            l10n::source_mode_disabled(self.locale),
                            !editing && source.mode == SourceMode::Disabled,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.settings_editor.history_path_editing = None;
                            if let Some(live) = this.live.as_ref() {
                                let receiver = live.configure_app_history(
                                    disabled_id.clone(),
                                    String::new(),
                                    SourceMode::Disabled,
                                );
                                this.poll(receiver, this.live_state.generation, cx);
                            }
                            cx.notify();
                        })),
                    ),
            )
            .when(editing, |card| {
                card.child(self.settings_input_row(
                    l10n::app_history_source_path(self.locale),
                    l10n::app_history_unsaved_path(self.locale),
                    &self.settings_editor.history_path_input,
                    "history-custom-path",
                    "~/Library/Application Support/...",
                ))
                .child(
                    Button::new(SharedString::from(format!("history-custom-{custom_id}")))
                        .primary()
                        .label(l10n::app_history_save_custom(self.locale))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.settings_editor.history_path_editing.as_deref()
                                != Some(custom_id.as_str())
                            {
                                return;
                            }
                            let path = this
                                .settings_editor
                                .history_path_input
                                .read(cx)
                                .value()
                                .trim()
                                .to_owned();
                            if let Some(live) = this.live.as_ref() {
                                let receiver = live.configure_app_history(
                                    custom_id.clone(),
                                    path,
                                    SourceMode::Custom,
                                );
                                this.poll(receiver, this.live_state.generation, cx);
                            }
                        })),
                )
            })
    }
}
