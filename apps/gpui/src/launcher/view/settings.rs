use super::*;
use anywhere_platform::FileSearchEngine;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};

mod layout;

mod application_history;
mod clipboard;
mod context;
mod features;
mod general;
mod search;
mod shortcuts;

impl LauncherView {
    pub(super) fn render_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let categories = self.settings.selected.label(self.locale).to_owned();
        let dirty = self.settings_dirty(cx);
        let detail = match self.settings.selected {
            SettingsCategory::General => self.general_settings(cx),
            SettingsCategory::Shortcuts => self.shortcut_settings(cx),
            SettingsCategory::Context => self.context_settings(cx),
            SettingsCategory::Search => self.search_settings(cx),
            SettingsCategory::Clipboard => self.clipboard_settings(cx),
            SettingsCategory::ApplicationHistory => self.application_history_settings(cx),
            SettingsCategory::Plugins => self.feature_settings(cx),
        };
        let action_picker = self.action_shortcut_picker(cx);
        let clipboard_confirmation = self.clipboard_confirmation.map(|confirmation| {
            let (title, description) = match confirmation {
                ClipboardConfirmation::Clear => (
                    l10n::clear_clipboard_history(self.locale),
                    l10n::all_saved_entries_will_be_permanently_removed(self.locale),
                ),
                ClipboardConfirmation::SetRetention(days) => (
                    l10n::change_history_retention(self.locale),
                    format!(
                        "{}\n{}",
                        l10n::history_retention_warning(self.locale),
                        if days == 0 {
                            l10n::until_storage_limit(self.locale)
                        } else {
                            l10n::retention_days(self.locale, i64::from(days))
                        }
                    ),
                ),
            };
            div()
                .id("clipboard-confirm-prompt")
                .test_support()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme::modal_scrim())
                .occlude()
                .child(
                    div()
                        .w(px(440.))
                        .p(px(20.))
                        .flex()
                        .flex_col()
                        .gap(px(14.))
                        .rounded(px(14.))
                        .bg(theme::menu_surface())
                        .border(px(metrics::DIVIDER_WIDTH))
                        .border_color(theme::on_dark_selection_outline())
                        .text_color(theme::on_dark_primary())
                        .child(
                            div()
                                .text_size(px(16.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::on_dark_primary())
                                .child(title),
                        )
                        .child(
                            div()
                                .body_text()
                                .whitespace_normal()
                                .text_color(theme::on_dark_secondary())
                                .child(description),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap(px(metrics::SPACE_SM))
                                .child(
                                    components::dark_surface_button(
                                        "clipboard-confirm-cancel",
                                        false,
                                        cx,
                                    )
                                    .track_focus(&self.settings_confirm_cancel_focus)
                                    .label(l10n::cancel(self.locale))
                                    .on_click(cx.listener(
                                        |this, _, window, cx| {
                                            this.cancel_clipboard_confirmation(window, cx)
                                        },
                                    )),
                                )
                                .child(
                                    Button::new("clipboard-confirm-accept")
                                        .primary()
                                        .track_focus(&self.settings_confirm_accept_focus)
                                        .disabled(self.clipboard.loading)
                                        .label(
                                            if matches!(confirmation, ClipboardConfirmation::Clear)
                                            {
                                                l10n::clear_history(self.locale)
                                            } else {
                                                l10n::save_changes(self.locale)
                                            },
                                        )
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.confirm_clipboard_confirmation(
                                                confirmation,
                                                window,
                                                cx,
                                            )
                                        })),
                                ),
                        ),
                )
        });
        let close_prompt = self.settings_close_pending.then(|| {
            div()
                .id("settings-close-prompt")
                .test_support()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme::modal_scrim())
                .occlude()
                .child(
                    div()
                        .w(px(420.))
                        .p(px(20.))
                        .flex()
                        .flex_col()
                        .gap(px(14.))
                        .rounded(px(14.))
                        .bg(theme::menu_surface())
                        .border(px(metrics::DIVIDER_WIDTH))
                        .border_color(theme::on_dark_selection_outline())
                        .text_color(theme::on_dark_primary())
                        .child(
                            div()
                                .text_size(px(16.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::on_dark_primary())
                                .child(if dirty {
                                    l10n::save_before_close(self.locale)
                                } else {
                                    l10n::changes_not_saved(self.locale)
                                }),
                        )
                        .child(
                            div()
                                .body_text()
                                .text_color(theme::on_dark_secondary())
                                .child(if dirty {
                                    l10n::settings_drafts(self.locale)
                                } else {
                                    self.settings
                                        .error
                                        .clone()
                                        .or_else(|| self.notice.clone())
                                        .unwrap_or_else(|| {
                                            l10n::settings_could_not_be_saved_you_can(self.locale)
                                        })
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .justify_end()
                                .gap(px(metrics::SPACE_SM))
                                .child(
                                    components::dark_surface_button("keep-editing", false, cx)
                                        .track_focus(&self.settings_keep_editing_focus)
                                        .label(l10n::keep_editing(self.locale))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.cancel_settings_close_prompt(window, cx);
                                        })),
                                )
                                .child(
                                    components::dark_surface_button("discard-settings", false, cx)
                                        .track_focus(&self.settings_discard_focus)
                                        .label(l10n::close_without_saving(self.locale))
                                        .disabled(self.settings.saving)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.discard_settings(window, cx)
                                        })),
                                )
                                .when(dirty, |element| {
                                    element.child(
                                        Button::new("save-settings-confirm")
                                            .primary()
                                            .track_focus(&self.settings_save_focus)
                                            .label(l10n::save_changes(self.locale))
                                            .disabled(self.settings.saving)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.save_settings_and_close(cx)
                                            })),
                                    )
                                }),
                        ),
                )
        });
        div()
            .id("settings-surface")
            .role(gpui_kit::Role::Group)
            .tab_group()
            .track_focus(&self.settings_focus_handle)
            .size_full()
            .relative()
            .flex()
            .test_support()
            .child(
                div()
                    .w(px(164.))
                    .h_full()
                    .px(px(10.))
                    .pt(px(42.))
                    .pb(px(metrics::SPACE_MD))
                    .flex()
                    .flex_col()
                    .bg(theme::sidebar())
                    .child(
                        div()
                            .px(px(metrics::SPACE_SM))
                            .pb(px(10.))
                            .flex()
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(metrics::CAPTION_SIZE))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::muted())
                                    .child(l10n::settings(self.locale)),
                            ),
                    )
                    .children(SettingsCategory::ALL.into_iter().map(|category| {
                        let selected = category == self.settings.selected;
                        let icon = match category {
                            SettingsCategory::General => IconName::Settings,
                            SettingsCategory::Shortcuts => IconName::Keyboard,
                            SettingsCategory::Context => IconName::Layers,
                            SettingsCategory::Search => IconName::Search,
                            SettingsCategory::Clipboard => IconName::Clipboard,
                            SettingsCategory::ApplicationHistory => IconName::Clock,
                            SettingsCategory::Plugins => IconName::Puzzle,
                        };
                        let icon_color = match category {
                            SettingsCategory::General => theme::secondary(),
                            SettingsCategory::Shortcuts => theme::icon_purple(),
                            SettingsCategory::Context => theme::accent(),
                            SettingsCategory::Search => theme::icon_green(),
                            SettingsCategory::Clipboard
                            | SettingsCategory::ApplicationHistory
                            | SettingsCategory::Plugins => theme::icon_amber(),
                        };
                        Button::new(SharedString::from(format!(
                            "settings-category-{}",
                            category.id()
                        )))
                        .ghost()
                        .accessibility_label(category.label(self.locale))
                        .selected(selected)
                        .w_full()
                        .h(px(32.))
                        .mb(px(3.))
                        .px(px(metrics::SPACE_SM))
                        .justify_start()
                        .rounded(px(metrics::CONTROL_RADIUS))
                        .bg(if selected {
                            theme::sidebar_selection()
                        } else {
                            theme::transparent()
                        })
                        .text_size(px(metrics::BODY_SIZE))
                        .text_color(if selected {
                            theme::primary()
                        } else {
                            theme::secondary()
                        })
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .items_center()
                                .gap(px(metrics::SPACE_SM))
                                .child(Icon::new(icon).size(px(18.)).text_color(icon_color))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .truncate()
                                        .line_height(px(
                                            metrics::BODY_SIZE * metrics::BODY_LINE_HEIGHT
                                        ))
                                        .child(category.label(self.locale)),
                                ),
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                if this.settings.selected != category {
                                    this.shortcut_recording = None;
                                    this.settings_action_picker_open = false;
                                    this.pending_action_shortcut = None;
                                    this.pending_action_shortcut_value = None;
                                    this.settings.selected = category;
                                    this.clipboard_settings_choice = None;
                                    if category != SettingsCategory::Clipboard {
                                        this.clipboard_confirmation = None;
                                    }
                                    this.settings_scrolls[category as usize]
                                        .set_offset(gpui_kit::point(px(0.), px(0.)));
                                    if category == SettingsCategory::ApplicationHistory {
                                        this.request_app_history_settings_sources(cx);
                                    } else if category == SettingsCategory::Clipboard {
                                        this.request_clipboard_settings_status(cx);
                                    }
                                    let settings_scroll =
                                        this.settings_scrolls[category as usize].clone();
                                    cx.defer_in(window, move |this, _, cx| {
                                        if this.settings.selected == category {
                                            settings_scroll
                                                .set_offset(gpui_kit::point(px(0.), px(0.)));
                                            cx.notify();
                                        }
                                    });
                                    cx.notify();
                                }
                                this.settings_focus_handle.focus(window, cx);
                            },
                        ))
                    })),
            )
            .child(
                div()
                    .w(px(metrics::DIVIDER_WIDTH))
                    .h_full()
                    .bg(theme::divider()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .overflow_x_hidden()
                    .pt(px(28.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(36.))
                            .flex_shrink_0()
                            .px(px(18.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(metrics::SPACE_MD))
                            .child(
                                div()
                                    .debug_selector(|| "settings-toolbar-title".into())
                                    .flex_1()
                                    .min_w(px(0.))
                                    .overflow_hidden()
                                    .text_size(px(metrics::BODY_SIZE))
                                    .line_height(px(metrics::BODY_SIZE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::primary())
                                    .truncate()
                                    .child(categories),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "settings-toolbar-actions".into())
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .gap(px(metrics::SPACE_SM))
                                    .when(self.settings.saving || dirty, |element| {
                                        element.child(
                                            div()
                                                .debug_selector(|| "settings-dirty-label".into())
                                                .flex_none()
                                                .whitespace_nowrap()
                                                .caption_text()
                                                .text_color(if dirty {
                                                    theme::warning()
                                                } else {
                                                    theme::muted()
                                                })
                                                .child(if self.settings.saving {
                                                    l10n::saving(self.locale)
                                                } else {
                                                    l10n::changes_not_saved(self.locale)
                                                }),
                                        )
                                    })
                                    .when(dirty || self.hotkey_error.is_some(), |element| {
                                        element.child(
                                            Button::new("save-settings")
                                                .secondary()
                                                .label(if dirty {
                                                    l10n::save_changes(self.locale)
                                                } else {
                                                    l10n::retry_shortcuts(self.locale)
                                                })
                                                .disabled(self.settings.saving)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.save_settings(cx)
                                                })),
                                        )
                                    })
                                    .child(
                                        Button::new("done-settings")
                                            .secondary()
                                            .border(px(metrics::DIVIDER_WIDTH))
                                            .border_color(theme::border())
                                            .label(l10n::done(self.locale))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.request_close_settings(window, cx)
                                            })),
                                    ),
                            ),
                    )
                    .child(div().h(px(0.5)).bg(theme::divider()))
                    .child(
                        div()
                            .id(SharedString::from(format!(
                                "settings-content-{}",
                                self.settings.selected.id()
                            )))
                            .test_support()
                            .flex_1()
                            .min_w(px(0.))
                            .min_h(px(0.))
                            .track_scroll(&self.settings_scrolls[self.settings.selected as usize])
                            .overflow_y_scroll()
                            .overflow_x_hidden()
                            .id(SharedString::from(format!(
                                "settings-content-scroll-{}",
                                self.settings.selected.id()
                            )))
                            .px(px(metrics::SPACE_XL))
                            .pt(px(18.))
                            .pb(px(metrics::SPACE_XL))
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .when_some(self.hotkey_error_message(), |element, error| {
                                element.child(
                                    div()
                                        .id("settings-hotkey-error")
                                        .test_support()
                                        .p(px(metrics::SPACE_MD))
                                        .rounded(px(metrics::CONTROL_RADIUS))
                                        .bg(theme::field())
                                        .text_size(px(metrics::CAPTION_SIZE))
                                        .text_color(theme::danger())
                                        .child(error),
                                )
                            })
                            .when_some(self.settings.error.clone(), |element, error| {
                                element.child(
                                    div()
                                        .p(px(metrics::SPACE_MD))
                                        .rounded(px(metrics::CONTROL_RADIUS))
                                        .bg(theme::field())
                                        .text_size(px(metrics::CAPTION_SIZE))
                                        .text_color(theme::danger())
                                        .child(error),
                                )
                            })
                            .child(detail)
                            .vertical_scrollbar(
                                &self.settings_scrolls[self.settings.selected as usize],
                            ),
                    ),
            )
            .when_some(action_picker, |element, picker| element.child(picker))
            .when_some(clipboard_confirmation, |element, prompt| {
                element.child(prompt)
            })
            .when_some(close_prompt, |element, prompt| element.child(prompt))
            .into_any_element()
    }
}

fn clipboard_retention_removes_entries(current_days: u32, next_days: u32) -> bool {
    next_days != 0 && (current_days == 0 || next_days < current_days)
}

#[cfg(test)]
mod tests {
    use super::clipboard_retention_removes_entries;

    #[test]
    fn only_shorter_finite_clipboard_retention_requires_deletion_confirmation() {
        assert!(clipboard_retention_removes_entries(90, 30));
        assert!(clipboard_retention_removes_entries(0, 1));
        assert!(!clipboard_retention_removes_entries(30, 90));
        assert!(!clipboard_retention_removes_entries(30, 0));
        assert!(!clipboard_retention_removes_entries(30, 30));
    }
}
