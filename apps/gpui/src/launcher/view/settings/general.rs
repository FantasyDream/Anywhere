use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn general_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let login_error = self.settings.login_item_error.clone();
        let login_controls = div()
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_SM))
            .when(login_error.is_some(), |controls| {
                controls.child(
                    Button::new("retry-login-item")
                        .secondary()
                        .label(l10n::retry(self.locale))
                        .disabled(self.settings.saving || self.settings_dirty(cx))
                        .on_click(cx.listener(|this, _, _, cx| this.retry_login_item(cx))),
                )
            })
            .child(
                Switch::new("open-at-login")
                    .checked(self.settings_draft.launch_at_login)
                    .accessibility_label(l10n::open_at_login(self.locale))
                    .on_change(cx.listener(|this, checked, _, cx| {
                        this.settings_draft.launch_at_login = *checked;
                        cx.notify();
                    })),
            );
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .child(self.language_settings(cx))
            .child(self.settings_row(
                l10n::open_at_login(self.locale),
                login_error.unwrap_or_else(|| l10n::open_at_login_description(self.locale)),
                login_controls,
            ))
    }

    pub(in crate::launcher::view) fn language_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let label = match self.settings_draft.ui_language {
            UiLanguage::System => l10n::language_system(self.locale),
            UiLanguage::En => l10n::language_english(self.locale),
            UiLanguage::Zh => l10n::language_chinese(self.locale),
        };
        let view = cx.weak_entity();
        let locale = self.locale;
        let selected = self.settings_draft.ui_language;
        self.settings_row(
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
                                        this.settings_draft.ui_language = language;
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
