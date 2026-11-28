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
        let categories = self.settings_editor.model.selected.label(self.locale);
        let dirty = self.settings_dirty(cx);
        let detail = match self.settings_editor.model.selected {
            SettingsCategory::General => self.general_settings(cx),
            SettingsCategory::Shortcuts => self.shortcut_settings(cx),
            SettingsCategory::Context => self.context_settings(cx),
            SettingsCategory::Search => self.search_settings(cx),
            SettingsCategory::Clipboard => self.clipboard_settings(cx),
            SettingsCategory::ApplicationHistory => self.application_history_settings(cx),
            SettingsCategory::Plugins => self.feature_settings(cx),
        };
        let action_picker = self.action_shortcut_picker(cx);
        let clipboard_confirmation = self
            .clipboard_confirmation
            .map(|confirmation| self.render_settings_clipboard_confirmation(confirmation, cx));
        let sidebar = self.render_settings_sidebar(cx);
        div()
            .id("settings-surface")
            .key_context("Settings")
            .on_action(cx.listener(Self::on_action_save_settings))
            .role(gpui_kit::Role::Group)
            .tab_group()
            .track_focus(&self.settings_editor.focus_handle)
            .size_full()
            .relative()
            .flex()
            .test_support()
            .child(sidebar)
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
                    .pt(px(metrics::TITLE_BAR_INSET))
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
                                    .when(self.settings_editor.model.saving, |element| {
                                        element.child(
                                            div()
                                                .id("settings-saving")
                                                .flex_none()
                                                .aria_label(l10n::saving(self.locale))
                                                .child(
                                                    Spinner::new().xsmall().color(theme::accent()),
                                                ),
                                        )
                                    })
                                    .when(dirty, |element| {
                                        element.child(
                                            Button::new("discard-settings-draft")
                                                .ghost()
                                                .label(l10n::discard_changes(self.locale))
                                                .disabled(self.settings_editor.model.saving)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.revert_settings_draft(window, cx);
                                                })),
                                        )
                                    })
                                    .when(dirty || self.hotkey_error.is_some(), |element| {
                                        element.child(
                                            Button::new("save-settings")
                                                .when(dirty, |button| button.primary())
                                                .when(!dirty, |button| button.secondary())
                                                .when(dirty, |button| {
                                                    button.child(components::keycap(
                                                        crate::platform::primary_shortcut_label(
                                                            "S",
                                                        ),
                                                    ))
                                                })
                                                .label(if dirty {
                                                    l10n::save_changes(self.locale)
                                                } else {
                                                    l10n::retry_shortcuts(self.locale)
                                                })
                                                .disabled(
                                                    self.settings_editor.draft_save_queued
                                                        || self.settings_editor.model.saving
                                                            && !dirty,
                                                )
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
                    .when_some(self.render_settings_banners(), |container, banners| {
                        container.child(banners)
                    })
                    .child(
                        div()
                            .id(SharedString::from(format!(
                                "settings-content-scroll-{}",
                                self.settings_editor.model.selected.id()
                            )))
                            .test_support()
                            .flex_1()
                            .min_w(px(0.))
                            .min_h(px(0.))
                            .track_scroll(
                                &self.settings_editor.scrolls
                                    [self.settings_editor.model.selected as usize],
                            )
                            .overflow_y_scroll()
                            .overflow_x_hidden()
                            .px(px(metrics::SPACE_XL))
                            .pt(px(18.))
                            .pb(px(metrics::SPACE_XL))
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .child(detail)
                            .vertical_scrollbar(
                                &self.settings_editor.scrolls
                                    [self.settings_editor.model.selected as usize],
                            ),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .child(self.render_window_titlebar(cx)),
            )
            .when_some(action_picker, |element, picker| element.child(picker))
            .when_some(clipboard_confirmation, |element, prompt| {
                element.child(prompt)
            })
            .into_any_element()
    }

    fn settings_save_banner(&self) -> Option<(String, components::NoticeKind)> {
        if let Some(error) = self.settings_editor.model.error.clone() {
            return Some((error, components::NoticeKind::Error));
        }
        if !self.settings_editor.close_pending
            && !matches!(
                self.shortcuts.recording,
                Some(ShortcutSlot::CurrentApp | ShortcutSlot::Universal)
            )
            && matches!(
                self.notice_kind,
                components::NoticeKind::Error | components::NoticeKind::Warning
            )
        {
            return self
                .notice
                .clone()
                .map(|message| (message, self.notice_kind));
        }
        None
    }

    fn settings_close_error_message(&self) -> Option<String> {
        self.settings_editor.model.error.clone().or_else(|| {
            (self.notice_kind == components::NoticeKind::Error)
                .then(|| self.notice.clone())
                .flatten()
        })
    }

    fn render_settings_banners(&self) -> Option<Div> {
        let hotkey_error = self.hotkey_error_message();
        let save_banner = self.settings_save_banner();
        if hotkey_error.is_none() && save_banner.is_none() {
            return None;
        }
        Some(
            div()
                .flex_shrink_0()
                .px(px(metrics::SPACE_XL))
                .pt(px(metrics::SPACE_SM))
                .flex()
                .flex_col()
                .gap(px(metrics::SPACE_SM))
                .when_some(hotkey_error, |container, error| {
                    container.child(
                        div()
                            .id("settings-hotkey-error")
                            .test_support()
                            .child(components::notice(error, components::NoticeKind::Error)),
                    )
                })
                .when_some(save_banner, |container, (message, kind)| {
                    container.child(
                        div()
                            .id("settings-save-error")
                            .test_support()
                            .child(components::notice(message, kind)),
                    )
                }),
        )
    }

    fn render_settings_clipboard_confirmation(
        &self,
        confirmation: ClipboardConfirmation,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_clear = matches!(confirmation, ClipboardConfirmation::Clear);
        let (title, description, icon) = match confirmation {
            ClipboardConfirmation::Clear => (
                l10n::clear_clipboard_history(self.locale),
                l10n::all_saved_entries_will_be_permanently_removed(self.locale),
                IconName::Trash,
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
                IconName::TriangleAlert,
            ),
        };
        let accept = Button::new("clipboard-confirm-accept")
            .when(is_clear, |button| button.danger())
            .when(!is_clear, |button| button.primary())
            .track_focus(&self.settings_editor.confirm_accept_focus)
            .disabled(self.clipboard.loading)
            .label(if is_clear {
                l10n::clear_history(self.locale)
            } else {
                l10n::save_changes(self.locale)
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.confirm_clipboard_confirmation(confirmation, window, cx)
            }));
        let cancel = Button::new("clipboard-confirm-cancel")
            .secondary()
            .bg(theme::field())
            .border(px(metrics::DIVIDER_WIDTH))
            .border_color(theme::control_border())
            .track_focus(&self.settings_editor.confirm_cancel_focus)
            .label(l10n::cancel(self.locale))
            .on_click(
                cx.listener(|this, _, window, cx| this.cancel_clipboard_confirmation(window, cx)),
            );
        let actions = div()
            .p(px(metrics::SPACE_LG))
            .flex()
            .justify_end()
            .gap(px(metrics::SPACE_SM))
            .child(cancel)
            .child(accept);
        components::modal_shell(
            "clipboard-confirm-prompt",
            icon,
            theme::danger(),
            title,
            description,
            None,
            actions,
        )
    }

    pub(super) fn render_settings_close_prompt(&self, cx: &mut Context<Self>) -> AnyElement {
        let error = self.settings_close_error_message();
        let dirty = self.settings_dirty(cx);
        let (title, description) = if dirty {
            (
                l10n::save_before_close(self.locale),
                l10n::settings_drafts(self.locale),
            )
        } else {
            (
                l10n::changes_not_saved(self.locale),
                l10n::settings_could_not_be_saved_you_can(self.locale),
            )
        };
        let badge_tint = if error.is_some() || !dirty {
            theme::danger()
        } else {
            theme::warning()
        };
        let keep_editing = Button::new("keep-editing")
            .secondary()
            .bg(theme::field())
            .border(px(metrics::DIVIDER_WIDTH))
            .border_color(theme::control_border())
            .track_focus(&self.settings_editor.keep_editing_focus)
            .label(l10n::keep_editing(self.locale))
            .on_click(cx.listener(|this, _, window, cx| {
                this.cancel_settings_close_prompt(window, cx);
            }));
        let discard = Button::new("discard-settings")
            .danger()
            .track_focus(&self.settings_editor.discard_focus)
            .label(l10n::close_without_saving(self.locale))
            .disabled(self.settings_editor.model.saving)
            .on_click(cx.listener(|this, _, window, cx| this.discard_settings(window, cx)));
        let actions = div()
            .p(px(metrics::SPACE_LG))
            .flex()
            .flex_wrap()
            .justify_end()
            .gap(px(metrics::SPACE_SM))
            .child(discard)
            .child(div().flex_1())
            .child(keep_editing)
            .when(dirty, |footer| {
                footer.child(
                    Button::new("save-settings-confirm")
                        .primary()
                        .track_focus(&self.settings_editor.save_focus)
                        .label(l10n::save_changes(self.locale))
                        .disabled(self.settings_editor.model.saving)
                        .on_click(cx.listener(|this, _, _, cx| this.save_settings_and_close(cx))),
                )
            });
        components::modal_shell(
            "settings-close-prompt",
            IconName::TriangleAlert,
            badge_tint,
            title,
            description,
            error.map(|message| {
                div()
                    .id("settings-close-prompt-error")
                    .test_support()
                    .w_full()
                    .child(components::notice(message, components::NoticeKind::Error))
                    .into_any_element()
            }),
            actions,
        )
    }

    fn render_settings_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let draft = self.preferences_from_editors(cx);
        div()
            .id("settings-sidebar")
            .track_focus(&self.settings_editor.sidebar_focus)
            .tab_stop(true)
            .border(px(metrics::DIVIDER_WIDTH))
            .border_color(theme::transparent())
            .focus_visible(|style| style.border_color(theme::selection_outline()))
            .on_key_down(
                cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                    if !this.settings_editor.sidebar_focus.is_focused(window)
                        || this.shortcuts.recording.is_some()
                    {
                        return;
                    }
                    let index = this.settings_editor.model.selected as usize;
                    let categories = SettingsCategory::ALL;
                    let next = match event.keystroke.key.as_str() {
                        "up" => (index + categories.len() - 1) % categories.len(),
                        "down" => (index + 1) % categories.len(),
                        _ => return,
                    };
                    cx.stop_propagation();
                    this.select_settings_category(categories[next], window, cx);
                }),
            )
            .w(px(metrics::SETTINGS_SIDEBAR_WIDTH))
            .h_full()
            .px(px(metrics::LIST_PADDING_X))
            .pt(px(metrics::PAGE_HEADER_HEIGHT - metrics::SPACE_XL))
            .pb(px(metrics::SPACE_MD))
            .flex()
            .flex_col()
            .bg(theme::sidebar())
            .child(
                div()
                    .px(px(metrics::SPACE_SM))
                    .pb(px(metrics::LIST_PADDING_X))
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
                let selected = category == self.settings_editor.model.selected;
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
                    SettingsCategory::General => theme::icon_neutral(),
                    SettingsCategory::Shortcuts => theme::icon_purple(),
                    SettingsCategory::Context => theme::icon_blue(),
                    SettingsCategory::Search => theme::icon_green(),
                    SettingsCategory::Clipboard => theme::icon_amber(),
                    SettingsCategory::ApplicationHistory => theme::icon_coral(),
                    SettingsCategory::Plugins => theme::icon_pink(),
                };
                Button::new(SharedString::from(format!(
                    "settings-category-{}",
                    category.id()
                )))
                .ghost()
                .accessibility_label(category.label(self.locale))
                .selected(selected)
                .w_full()
                .h(px(metrics::ROW_ICON_SLOT))
                .mb(px(metrics::ROW_INSET_Y_COMPACT))
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
                        .relative()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_SM))
                        .child(
                            div()
                                .flex_none()
                                .w(px(metrics::SELECTION_INDICATOR_WIDTH))
                                .h(px(metrics::ICON_SIZE))
                                .rounded(px(metrics::SELECTION_INDICATOR_WIDTH))
                                .when(selected, |indicator| indicator.bg(theme::accent())),
                        )
                        .when(self.settings_category_dirty(category, &draft), |row| {
                            row.child(
                                div()
                                    .absolute()
                                    .right_0()
                                    .size(px(metrics::DRAFT_INDICATOR_SIZE))
                                    .rounded(px(metrics::DRAFT_INDICATOR_SIZE))
                                    .bg(theme::warning()),
                            )
                        })
                        .child(
                            Icon::new(icon)
                                .size(px(metrics::SPACE_LG))
                                .text_color(icon_color),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .line_height(px(metrics::BODY_SIZE * metrics::BODY_LINE_HEIGHT))
                                .child(category.label(self.locale)),
                        ),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.select_settings_category(category, window, cx);
                    this.settings_editor.sidebar_focus.focus(window, cx);
                }))
            }))
    }

    fn select_settings_category(
        &mut self,
        category: SettingsCategory,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings_editor.model.selected != category {
            self.shortcuts.recording = None;
            self.settings_editor.action_picker_open = false;
            self.shortcuts.pending_action = None;
            self.shortcuts.pending_value = None;
            self.settings_editor.model.selected = category;
            if category != SettingsCategory::Clipboard {
                self.clipboard_confirmation = None;
            }
            self.settings_editor.scrolls[category as usize]
                .set_offset(gpui_kit::point(px(0.), px(0.)));
            if category == SettingsCategory::ApplicationHistory {
                self.request_app_history_settings_sources(cx);
            } else if category == SettingsCategory::Clipboard {
                self.request_clipboard_settings_status(cx);
            }
            let settings_scroll = self.settings_editor.scrolls[category as usize].clone();
            cx.defer_in(window, move |this, _, cx| {
                if this.settings_editor.model.selected == category {
                    settings_scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
                    cx.notify();
                }
            });
            cx.notify();
        }
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
