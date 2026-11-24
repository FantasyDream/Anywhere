use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn general_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let login_error = self.settings_editor.model.login_item_error.clone();
        let login_controls = div()
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_SM))
            .when(login_error.is_some(), |controls| {
                controls.child(
                    Button::new("retry-login-item")
                        .secondary()
                        .label(l10n::retry(self.locale))
                        .disabled(self.settings_editor.model.saving || self.settings_dirty(cx))
                        .on_click(cx.listener(|this, _, _, cx| this.retry_login_item(cx))),
                )
            })
            .child(
                Switch::new("open-at-login")
                    .checked(self.settings_editor.draft.launch_at_login)
                    .accessibility_label(l10n::open_at_login(self.locale))
                    .on_change(cx.listener(|this, checked, _, cx| {
                        this.settings_editor.draft.launch_at_login = *checked;
                        cx.notify();
                    })),
            );
        let theme = self.theme_settings(cx);
        let language = self.language_settings(cx);
        self.settings_rows_card([
            theme,
            language,
            self.settings_row_bare(
                l10n::open_at_login(self.locale),
                login_error.unwrap_or_else(|| l10n::open_at_login_description(self.locale)),
                login_controls,
            ),
        ])
    }

    fn theme_settings(&self, cx: &Context<Self>) -> Div {
        let locale = self.locale;
        let selected = self.settings_editor.draft.ui_theme;
        let cards = div()
            .id("settings-theme")
            .w_full()
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_SM))
            .children(UiTheme::ALL.into_iter().map(|mode| {
                let active = mode == selected;
                let id = match mode {
                    UiTheme::System => "settings-theme-system",
                    UiTheme::Light => "settings-theme-light",
                    UiTheme::Dark => "settings-theme-dark",
                };
                self.render_theme_card(id, mode, active, locale, cx)
            }));
        self.settings_stack_row(l10n::theme(locale), l10n::theme_description(locale), cards)
    }

    fn theme_card_preview(mode: UiTheme) -> Div {
        let light_bg: gpui_kit::Hsla = gpui_kit::rgba(crate::ui::argb(
            crate::ui::design_tokens::colors::FIXED.light_fallback_surface,
        ))
        .into();
        let dark_bg: gpui_kit::Hsla = gpui_kit::rgba(crate::ui::argb(
            crate::ui::design_tokens::colors::OPAQUE_DARK.window,
        ))
        .into();
        let light_accent: gpui_kit::Hsla = gpui_kit::rgba(crate::ui::argb(
            crate::ui::design_tokens::colors::GLASS_LIGHT.accent,
        ))
        .into();
        let dark_accent: gpui_kit::Hsla = gpui_kit::rgba(crate::ui::argb(
            crate::ui::design_tokens::colors::GLASS_DARK.accent,
        ))
        .into();
        let frame = div()
            .w_full()
            .h(px(metrics::ROW_HEIGHT_COMFORTABLE))
            .rounded(px(metrics::TOOLTIP_RADIUS))
            .overflow_hidden()
            .border(px(metrics::DIVIDER_WIDTH))
            .border_color(theme::border());
        match mode {
            UiTheme::System => frame
                .flex()
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .bg(light_bg)
                        .p(px(metrics::SPACE_XS))
                        .child(
                            div()
                                .w_full()
                                .h(px(metrics::SPACE_SM))
                                .rounded(px(metrics::ROW_INSET_Y_COMPACT))
                                .bg(light_accent),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .bg(dark_bg)
                        .p(px(metrics::SPACE_XS))
                        .child(
                            div()
                                .w_full()
                                .h(px(metrics::SPACE_SM))
                                .rounded(px(metrics::ROW_INSET_Y_COMPACT))
                                .bg(dark_accent),
                        ),
                ),
            UiTheme::Light => frame
                .bg(light_bg)
                .p(px(metrics::SPACE_SM))
                .flex()
                .flex_col()
                .gap(px(metrics::SPACE_XS))
                .child(
                    div()
                        .w_1_2()
                        .h(px(metrics::SPACE_SM))
                        .rounded(px(metrics::ROW_INSET_Y_COMPACT))
                        .bg(light_accent),
                ),
            UiTheme::Dark => frame
                .bg(dark_bg)
                .p(px(metrics::SPACE_SM))
                .flex()
                .flex_col()
                .gap(px(metrics::SPACE_XS))
                .child(
                    div()
                        .w_1_2()
                        .h(px(metrics::SPACE_SM))
                        .rounded(px(metrics::ROW_INSET_Y_COMPACT))
                        .bg(dark_accent),
                ),
        }
    }

    fn render_theme_card(
        &self,
        id: &'static str,
        mode: UiTheme,
        active: bool,
        locale: Locale,
        cx: &Context<Self>,
    ) -> gpui_kit::Stateful<Div> {
        div()
            .id(id)
            .role(gpui_kit::Role::Button)
            .aria_selected(active)
            .flex_1()
            .min_w_0()
            .p(px(metrics::SPACE_SM))
            .rounded(px(metrics::CARD_RADIUS))
            .border(px(if active {
                metrics::FIELD_BORDER_WIDTH
            } else {
                metrics::DIVIDER_WIDTH
            }))
            .border_color(if active {
                theme::accent()
            } else {
                theme::border()
            })
            .bg(if active {
                theme::selected()
            } else {
                theme::field()
            })
            .hover(|style| style.bg(theme::hover()))
            .cursor_pointer()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_XS))
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.settings_editor.draft.ui_theme != mode {
                    this.settings_editor.draft.ui_theme = mode;
                    this.save_settings(cx);
                }
            }))
            .child(Self::theme_card_preview(mode))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(metrics::SPACE_XS))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(metrics::LABEL_SIZE))
                            .font_weight(if active {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::MEDIUM
                            })
                            .text_color(theme::primary())
                            .child(mode.label(locale)),
                    )
                    .when(active, |row| {
                        row.child(
                            Icon::new(IconName::Check)
                                .size(px(metrics::CAPTION_SIZE))
                                .text_color(theme::accent()),
                        )
                    }),
            )
    }

    pub(in crate::launcher::view) fn language_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let label = match self.settings_editor.draft.ui_language {
            UiLanguage::System => l10n::language_system(self.locale),
            UiLanguage::En => l10n::language_english(self.locale),
            UiLanguage::Zh => l10n::language_chinese(self.locale),
        };
        let view = cx.weak_entity();
        let locale = self.locale;
        let selected = self.settings_editor.draft.ui_language;
        self.settings_row_bare(
            l10n::language(locale),
            l10n::language_description(locale),
            Button::new("settings-language")
                .secondary()
                .label(label)
                .dropdown_caret(true)
                .dropdown_menu(move |mut menu, _, _| {
                    for (language, label) in [
                        (UiLanguage::System, l10n::language_system(locale)),
                        (UiLanguage::En, l10n::language_english(locale)),
                        (UiLanguage::Zh, l10n::language_chinese(locale)),
                    ] {
                        let view = view.clone();
                        menu = menu.item(
                            PopupMenuItem::new(label)
                                .checked(language == selected)
                                .on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.settings_editor.draft.ui_language = language;
                                        cx.notify();
                                    });
                                }),
                        );
                    }
                    menu
                }),
        )
    }
}
