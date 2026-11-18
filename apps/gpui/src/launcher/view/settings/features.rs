use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn feature_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let feature_rows = self
            .settings
            .features
            .clone()
            .into_iter()
            .filter(|feature| feature.key != "clipboard")
            .map(|feature| {
                let key = feature.key.clone();
                let switch_key = format!("feature-{}", feature.key);
                let accessibility = feature.name.clone();
                self.settings_row_bare(
                    feature.name,
                    l10n::built_in_feature_description(self.locale),
                    Switch::new(switch_key)
                        .checked(feature.enabled)
                        .disabled(self.settings.saving)
                        .accessibility_label(accessibility)
                        .on_change(cx.listener(move |this, checked, _, cx| {
                            this.change_feature_setting(&key, *checked, cx);
                        })),
                )
            })
            .collect::<Vec<_>>();
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .child(
                div()
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::muted())
                    .child(l10n::choose_the_tools_available_in_anywhere(self.locale)),
            )
            .child(components::section_heading(
                l10n::built_in(self.locale),
                IconName::Puzzle,
            ))
            .child(self.settings_rows_card(feature_rows))
            .child(
                self.settings_row(
                    l10n::web_tools(self.locale),
                    l10n::web_tools_settings_description(self.locale),
                    Button::new("manage-web-packages")
                        .secondary()
                        .label(l10n::manage_web_tools(self.locale))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_screen(Screen::WebPackages, window, cx);
                        })),
                ),
            )
    }
}
