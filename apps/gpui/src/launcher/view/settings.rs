use super::*;

mod layout;

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
                        .border_color(theme::selection_outline())
                        .child(
                            div()
                                .text_size(px(16.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::primary())
                                .child(title),
                        )
                        .child(
                            div()
                                .body_text()
                                .whitespace_normal()
                                .text_color(theme::secondary())
                                .child(description),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap(px(metrics::SPACE_SM))
                                .child(
                                    Button::new("clipboard-confirm-cancel")
                                        .ghost()
                                        .track_focus(&self.settings_confirm_cancel_focus)
                                        .label(l10n::cancel(self.locale))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.cancel_clipboard_confirmation(window, cx)
                                        })),
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
        let close_prompt =
            self.settings_close_pending.then(|| {
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
                            .border_color(theme::selection_outline())
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::primary())
                                    .child(if dirty {
                                        l10n::save_before_close(self.locale)
                                    } else {
                                        l10n::changes_not_saved(self.locale)
                                    }),
                            )
                            .child(div().body_text().text_color(theme::secondary()).child(
                                if dirty {
                                    l10n::settings_drafts(self.locale)
                                } else {
                                    self.settings
                                        .error
                                        .clone()
                                        .or_else(|| self.notice.clone())
                                        .unwrap_or_else(|| {
                                            l10n::settings_could_not_be_saved_you_can(self.locale)
                                        })
                                },
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .justify_end()
                                    .gap(px(metrics::SPACE_SM))
                                    .child(
                                        Button::new("keep-editing")
                                            .ghost()
                                            .track_focus(&self.settings_keep_editing_focus)
                                            .label(l10n::keep_editing(self.locale))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.cancel_settings_close_prompt(window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("discard-settings")
                                            .secondary()
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
                                    .when(dirty, |element| {
                                        element.child(
                                            Button::new("save-settings")
                                                .secondary()
                                                .label(l10n::save_changes(self.locale))
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
                            .vertical_scrollbar(
                                &self.settings_scrolls[self.settings.selected as usize],
                            )
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
                            .child(detail),
                    ),
            )
            .when_some(action_picker, |element, picker| element.child(picker))
            .when_some(clipboard_confirmation, |element, prompt| {
                element.child(prompt)
            })
            .when_some(close_prompt, |element, prompt| element.child(prompt))
            .into_any_element()
    }

    fn general_settings(&mut self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .child(self.language_settings(cx))
            .child(
                self.settings_row(
                    l10n::open_at_login(self.locale),
                    l10n::open_at_login_description(self.locale),
                    Switch::new("open-at-login")
                        .checked(self.settings_draft.launch_at_login)
                        .accessibility_label(l10n::open_at_login(self.locale))
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_draft.launch_at_login = *checked;
                            cx.notify();
                        })),
                ),
            )
    }

    fn shortcut_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let bindings = self.settings_draft.action_shortcuts.clone();
        let context_editing = self.context_shortcut_draft.is_some();
        let catalog_empty = self.shortcut_catalog.is_empty();
        let mut action_rows = div().flex().flex_col();
        for binding in bindings {
            let action = self
                .shortcut_catalog
                .iter()
                .find(|action| action.action_id == binding.action_id)
                .cloned()
                .unwrap_or_else(|| ShortcutCatalogAction {
                    action_id: binding.action_id.clone(),
                    title: binding.title.clone(),
                    localized_titles: std::collections::BTreeMap::new(),
                    context_types: Vec::new(),
                });
            let remove_id = action.action_id.clone();
            let has_context_routes = !binding.routes.is_empty();
            let route_count = binding.routes.len() as i64;
            let binding_for_edit = binding.clone();
            let action_for_edit = action.clone();
            let action_label = shortcut_action_title(&action, self.locale);
            let shortcut_label = crate::platform::macos_shortcut_label(&binding.shortcut)
                .unwrap_or_else(|| binding.shortcut.clone());
            let action_missing = !catalog_empty
                && !self
                    .shortcut_catalog
                    .iter()
                    .any(|candidate| candidate.action_id == action.action_id);
            action_rows = action_rows.child(
                div()
                    .min_h(px(58.))
                    .px(px(12.))
                    .py(px(5.))
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_SM))
                    .border_b(px(metrics::DIVIDER_WIDTH))
                    .border_color(theme::divider())
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(metrics::BODY_SIZE))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(action_label.clone()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(metrics::CAPTION_SIZE))
                                    .text_color(theme::muted())
                                    .child(if binding.global {
                                        l10n::global_works_from_other_apps(self.locale)
                                    } else {
                                        l10n::while_anywhere_is_open(self.locale)
                                    }),
                            )
                            .when(has_context_routes, |element| {
                                element.child(
                                    div()
                                        .truncate()
                                        .text_size(px(10.5))
                                        .text_color(theme::muted())
                                        .child(l10n::context_rules_count(self.locale, route_count)),
                                )
                            })
                            .when(action_missing, |element| {
                                element.child(
                                    div()
                                        .truncate()
                                        .text_size(px(10.5))
                                        .text_color(theme::warning())
                                        .child(l10n::action_unavailable_shortcut_paused(
                                            self.locale,
                                        )),
                                )
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!(
                            "action-edit-{}",
                            action.action_id
                        )))
                        .secondary()
                        .icon(IconName::Pencil)
                        .label(shortcut_label)
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                if has_context_routes {
                                    this.open_context_shortcut_draft(
                                        Some(&binding_for_edit),
                                        window,
                                        cx,
                                    );
                                } else {
                                    this.open_action_shortcut_picker(
                                        Some(action_for_edit.clone()),
                                        Some(binding_for_edit.shortcut.clone()),
                                        binding_for_edit.global,
                                        window,
                                        cx,
                                    );
                                }
                            },
                        )),
                    )
                    .child(
                        Button::new(SharedString::from(format!("action-remove-{remove_id}")))
                            .ghost()
                            .icon(IconName::Trash)
                            .accessibility_label(l10n::remove_action_shortcut(
                                self.locale,
                                &action_label,
                            ))
                            .tooltip(l10n::remove_action_shortcut(self.locale, &action_label))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.settings_draft
                                    .action_shortcuts
                                    .retain(|binding| binding.action_id != remove_id);
                                if this
                                    .pending_action_shortcut
                                    .as_ref()
                                    .is_some_and(|pending| pending.action_id == remove_id)
                                {
                                    this.close_action_shortcut_picker(cx);
                                }
                                cx.notify();
                            })),
                    ),
            );
        }

        let context_shortcut_builder = self.context_shortcut_draft.clone().map(|draft| {
            let recording = self.shortcut_recording == Some(ShortcutSlot::ContextShortcut);
            let name_is_present = !self
                .context_shortcut_name_input
                .read(cx)
                .value()
                .trim()
                .is_empty();
            let can_save = name_is_present && draft.shortcut.is_some() && !draft.routes.is_empty();
            let query = self
                .action_shortcut_input
                .read(cx)
                .value()
                .to_ascii_lowercase();
            let mut context_catalog_rows = div().flex().flex_col();
            for action in self
                .shortcut_catalog
                .iter()
                .filter(|action| {
                    query.is_empty()
                        || action.title.to_ascii_lowercase().contains(&query)
                        || action.action_id.to_ascii_lowercase().contains(&query)
                })
                .take(16)
                .cloned()
            {
                let action_id = action.action_id.clone();
                context_catalog_rows = context_catalog_rows.child(
                    div()
                        .min_h(px(38.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .text_size(px(12.))
                                .child(shortcut_action_title(&action, self.locale)),
                        )
                        .child(
                            Button::new(SharedString::from(format!(
                                "context-action-add-{action_id}"
                            )))
                            .secondary()
                            .label(l10n::add_rule(self.locale))
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.add_context_shortcut_action(&action, cx);
                                },
                            )),
                        ),
                );
            }
            let mut route_rows = div().flex().flex_col().gap(px(metrics::SPACE_XS));
            for route in &draft.routes {
                let remove_action_id = route.action_id.clone();
                let remove_context_type = route.context_type.clone();
                route_rows = route_rows.child(
                    div()
                        .min_h(px(36.))
                        .px(px(metrics::SPACE_SM))
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_SM))
                        .rounded(px(metrics::CONTROL_RADIUS))
                        .bg(theme::field())
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(metrics::BODY_SIZE))
                                        .child(route.title.clone()),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(10.5))
                                        .text_color(theme::muted())
                                        .child(route.context_type.clone().unwrap_or_else(|| {
                                            l10n::any_compatible_context(self.locale)
                                        })),
                                ),
                        )
                        .child(
                            Button::new(SharedString::from(format!(
                                "context-route-remove-{remove_action_id}-{}",
                                remove_context_type.as_deref().unwrap_or("any")
                            )))
                            .ghost()
                            .label(l10n::remove(self.locale))
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    if let Some(draft) = this.context_shortcut_draft.as_mut() {
                                        draft.routes.retain(|route| {
                                            route.action_id != remove_action_id
                                                || route.context_type != remove_context_type
                                        });
                                        cx.notify();
                                    }
                                },
                            )),
                        ),
                );
            }
            div()
                .flex()
                .flex_col()
                .gap(px(metrics::SPACE_SM))
                .p(px(metrics::SPACE_MD))
                .rounded(px(metrics::CARD_RADIUS))
                .bg(theme::card())
                .border(px(metrics::DIVIDER_WIDTH))
                .border_color(theme::accent())
                .child(
                    div()
                        .text_size(px(metrics::HEADING_SIZE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(l10n::context_shortcut(self.locale)),
                )
                .child(
                    div()
                        .h(px(34.))
                        .rounded(px(metrics::CONTROL_RADIUS))
                        .bg(theme::field())
                        .px(px(metrics::SPACE_SM))
                        .flex()
                        .items_center()
                        .child(
                            Input::new(&self.context_shortcut_name_input)
                                .id("settings-context-shortcut-name")
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .cleanable(true)
                                .text_size(px(metrics::BODY_SIZE))
                                .w_full(),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_SM))
                        .child(
                            Button::new("settings-context-shortcut-record")
                                .when(recording, |button| button.primary())
                                .when(!recording, |button| button.secondary())
                                .label(if recording {
                                    l10n::press_ashortcut(self.locale)
                                } else {
                                    draft
                                        .shortcut
                                        .clone()
                                        .unwrap_or_else(|| l10n::record_shortcut(self.locale))
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.shortcut_recording = Some(ShortcutSlot::ContextShortcut);
                                    this.pending_action_shortcut = None;
                                    this.notice = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Switch::new("settings-context-shortcut-global")
                                .checked(draft.global)
                                .accessibility_label(l10n::global_shortcut(self.locale))
                                .on_change(cx.listener(|this, checked, _, cx| {
                                    if let Some(draft) = this.context_shortcut_draft.as_mut() {
                                        draft.global = *checked;
                                        cx.notify();
                                    }
                                })),
                        )
                        .child(
                            div()
                                .text_size(px(10.5))
                                .text_color(theme::secondary())
                                .child(l10n::global(self.locale)),
                        ),
                )
                .child(route_rows)
                .child(
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::secondary())
                        .child(l10n::add_action_shortcut(self.locale)),
                )
                .child(
                    div()
                        .h(px(34.))
                        .rounded(px(metrics::CONTROL_RADIUS))
                        .bg(theme::field())
                        .px(px(metrics::SPACE_SM))
                        .flex()
                        .items_center()
                        .child(
                            Input::new(&self.action_shortcut_input)
                                .id("settings-context-action-shortcut-search")
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .cleanable(true)
                                .text_size(px(metrics::BODY_SIZE))
                                .w_full(),
                        ),
                )
                .child(
                    div()
                        .max_h(px(190.))
                        .overflow_y_scrollbar()
                        .rounded(px(metrics::CONTROL_RADIUS))
                        .bg(theme::field())
                        .when(self.shortcut_catalog_loading, |element| {
                            element.child(
                                div()
                                    .p(px(10.))
                                    .text_size(px(11.))
                                    .text_color(theme::muted())
                                    .child(l10n::loading(self.locale)),
                            )
                        })
                        .when(
                            !self.shortcut_catalog_loading && self.shortcut_catalog.is_empty(),
                            |element| {
                                element.child(
                                    div()
                                        .p(px(10.))
                                        .text_size(px(11.))
                                        .text_color(theme::muted())
                                        .child(l10n::no_actions_available(self.locale)),
                                )
                            },
                        )
                        .child(context_catalog_rows),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(metrics::SPACE_SM))
                        .child(
                            Button::new("settings-context-shortcut-cancel")
                                .ghost()
                                .label(l10n::cancel(self.locale))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.context_shortcut_draft = None;
                                    this.shortcut_recording = None;
                                    this.notice = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("settings-context-shortcut-save")
                                .primary()
                                .disabled(!can_save)
                                .label(l10n::save_context_shortcut(self.locale))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    match this.commit_context_shortcut_draft(cx) {
                                        Ok(()) => this.clear_notice(),
                                        Err(error) => {
                                            this.set_notice(error, components::NoticeKind::Warning)
                                        }
                                    }
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element()
        });

        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .child(self.settings_rows_card(vec![
                self.shortcut_setting_row(
                    ShortcutSlot::CurrentApp,
                    l10n::current_app(self.locale),
                    &self.current_shortcut_input,
                    "current-shortcut",
                    cx,
                ),
                self.shortcut_setting_row(
                    ShortcutSlot::Universal,
                    l10n::universal_search(self.locale),
                    &self.universal_shortcut_input,
                    "universal-shortcut",
                    cx,
                ),
            ]))
            .child(
                div()
                    .caption_text()
                    .text_color(theme::muted())
                    .child(l10n::shortcut_help(self.locale)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(metrics::SPACE_SM))
                    .child(components::section_heading(
                        l10n::action_shortcuts(self.locale),
                        IconName::Zap,
                    ))
                    .child(
                        div()
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::secondary())
                            .child(l10n::bind_akey_to_an_action_or(self.locale)),
                    )
                    .child(
                        div()
                            .rounded(px(metrics::CARD_RADIUS))
                            .bg(theme::card())
                            .border(px(metrics::DIVIDER_WIDTH))
                            .border_color(theme::divider())
                            .child(action_rows)
                            .child(
                                div()
                                    .p(px(12.))
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(8.))
                                    .child(
                                        Button::new("settings-add-action-shortcut")
                                            .secondary()
                                            .icon(IconName::Plus)
                                            .label(l10n::add_action_shortcut(self.locale))
                                            .disabled(
                                                self.shortcut_catalog_loading || catalog_empty,
                                            )
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.open_action_shortcut_picker(
                                                    None, None, false, window, cx,
                                                );
                                            })),
                                    )
                                    .when(!context_editing, |element| {
                                        element.child(
                                            Button::new("settings-add-context-shortcut")
                                                .secondary()
                                                .icon(IconName::Route)
                                                .label(l10n::add_context_shortcut(self.locale))
                                                .disabled(
                                                    self.shortcut_catalog_loading || catalog_empty,
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.open_context_shortcut_draft(
                                                        None, window, cx,
                                                    );
                                                })),
                                        )
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::muted())
                    .child(l10n::tools_such_as_file_search_are_always(self.locale)),
            )
            .when_some(context_shortcut_builder, |element, builder| {
                element.child(builder)
            })
    }

    fn open_action_shortcut_picker(
        &mut self,
        action: Option<ShortcutCatalogAction>,
        shortcut: Option<String>,
        global: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_action_picker_open = true;
        self.shortcut_recording = None;
        self.pending_action_shortcut = action;
        self.pending_action_shortcut_value = shortcut;
        self.pending_action_shortcut_global = global;
        self.notice = None;
        if self.pending_action_shortcut.is_none() {
            set_input_value(&self.action_shortcut_input, String::new(), window, cx);
            let input = self.action_shortcut_input.clone();
            cx.defer_in(window, move |this, window, cx| {
                if this.settings_action_picker_open && this.pending_action_shortcut.is_none() {
                    input.update(cx, |input, cx| input.focus(window, cx));
                }
            });
        }
        cx.notify();
    }

    pub(super) fn close_action_shortcut_picker(&mut self, cx: &mut Context<Self>) {
        self.settings_action_picker_open = false;
        self.shortcut_recording = None;
        self.pending_action_shortcut = None;
        self.pending_action_shortcut_value = None;
        self.pending_action_shortcut_global = false;
        self.notice = None;
        cx.notify();
    }

    fn commit_action_shortcut_picker(&mut self, cx: &mut Context<Self>) {
        let (Some(action), Some(shortcut)) = (
            self.pending_action_shortcut.clone(),
            self.pending_action_shortcut_value.clone(),
        ) else {
            return;
        };
        let title = shortcut_action_title(&action, self.locale);
        let mut draft = self.settings_draft.clone();
        draft
            .action_shortcuts
            .retain(|binding| binding.action_id != action.action_id);
        draft.action_shortcuts.push(ActionShortcutBinding {
            action_id: action.action_id,
            title,
            shortcut,
            global: self.pending_action_shortcut_global,
            routes: Vec::new(),
        });
        match draft.validate() {
            Ok(()) => {
                self.settings_draft = draft;
                self.close_action_shortcut_picker(cx);
            }
            Err(_) => {
                self.set_notice(
                    l10n::shortcut_recording_failed_try_again(self.locale),
                    components::NoticeKind::Warning,
                );
                cx.notify();
            }
        }
    }

    fn action_shortcut_picker(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.settings_action_picker_open {
            return None;
        }
        let selected = self.pending_action_shortcut.clone();
        let body = if let Some(action) = selected {
            let recording = self.shortcut_recording == Some(ShortcutSlot::Action);
            let shortcut_label = self
                .pending_action_shortcut_value
                .as_deref()
                .and_then(crate::platform::macos_shortcut_label)
                .unwrap_or_else(|| {
                    self.pending_action_shortcut_value
                        .clone()
                        .unwrap_or_else(|| l10n::record_shortcut(self.locale))
                });
            let context_types = if action.context_types.is_empty() {
                l10n::not_detected_in_the_last_context_the(self.locale)
            } else {
                action.context_types.join(" · ")
            };
            let input = self.action_shortcut_input.clone();
            let can_save = self.pending_action_shortcut_value.is_some() && !recording;
            div()
                .flex_1()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(
                            div()
                                .truncate()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::primary())
                                .child(shortcut_action_title(&action, self.locale)),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(11.))
                                .text_color(theme::muted())
                                .child(context_types),
                        ),
                )
                .child(
                    Button::new("settings-action-picker-record")
                        .when(recording, |button| button.primary())
                        .when(!recording, |button| button.secondary())
                        .label(if recording {
                            l10n::press_ashortcut(self.locale)
                        } else {
                            shortcut_label
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.shortcut_recording = Some(ShortcutSlot::Action);
                            this.notice = None;
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(15.))
                        .text_color(theme::muted())
                        .child(l10n::use_command_option_or_control_with_a(self.locale)),
                )
                .child(
                    div()
                        .h(px(46.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .rounded(px(metrics::CONTROL_RADIUS))
                        .bg(theme::field())
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .text_color(theme::primary())
                                        .child(l10n::global_shortcut(self.locale)),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(11.))
                                        .text_color(theme::muted())
                                        .child(l10n::works_from_other_apps_uses_fresh_context(
                                            self.locale,
                                        )),
                                ),
                        )
                        .child(
                            Switch::new("settings-action-picker-global")
                                .checked(self.pending_action_shortcut_global)
                                .accessibility_label(l10n::global_shortcut(self.locale))
                                .on_change(cx.listener(|this, checked, _, cx| {
                                    this.pending_action_shortcut_global = *checked;
                                    cx.notify();
                                })),
                        ),
                )
                .when_some(self.notice.clone(), |element, notice| {
                    element.child(components::notice(notice, components::NoticeKind::Warning))
                })
                .child(
                    div()
                        .mt_auto()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_end()
                        .gap(px(8.))
                        .child(
                            Button::new("settings-action-picker-another")
                                .ghost()
                                .label(l10n::choose_another_action(self.locale))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.pending_action_shortcut = None;
                                    this.pending_action_shortcut_value = None;
                                    this.pending_action_shortcut_global = false;
                                    this.shortcut_recording = None;
                                    this.notice = None;
                                    let input = input.clone();
                                    cx.defer_in(window, move |this, window, cx| {
                                        if this.settings_action_picker_open
                                            && this.pending_action_shortcut.is_none()
                                        {
                                            input.update(cx, |input, cx| input.focus(window, cx));
                                        }
                                    });
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("settings-action-picker-cancel")
                                .ghost()
                                .label(l10n::cancel(self.locale))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.close_action_shortcut_picker(cx);
                                })),
                        )
                        .child(
                            Button::new("settings-action-picker-save")
                                .primary()
                                .disabled(!can_save)
                                .label(l10n::save_shortcut(self.locale))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.commit_action_shortcut_picker(cx);
                                })),
                        ),
                )
                .into_any_element()
        } else {
            let query = self
                .action_shortcut_input
                .read(cx)
                .value()
                .to_ascii_lowercase();
            let actions = self
                .shortcut_catalog
                .iter()
                .filter(|action| {
                    query.is_empty()
                        || action.title.to_ascii_lowercase().contains(&query)
                        || action.action_id.to_ascii_lowercase().contains(&query)
                })
                .take(24)
                .cloned()
                .collect::<Vec<_>>();
            let mut rows = div().flex().flex_col();
            for action in actions {
                let action_id = action.action_id.clone();
                let title = shortcut_action_title(&action, self.locale);
                let context_types = if action.context_types.is_empty() {
                    l10n::not_detected_in_the_last_context(self.locale)
                } else {
                    action.context_types.join(" · ")
                };
                rows = rows.child(
                    div()
                        .id(SharedString::from(format!(
                            "settings-action-choice-{action_id}"
                        )))
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label(title.clone())
                        .min_h(px(46.))
                        .px(px(10.))
                        .py(px(6.))
                        .flex()
                        .items_center()
                        .rounded(px(metrics::CONTROL_RADIUS))
                        .hover(|style| style.bg(theme::hover()))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.pending_action_shortcut = Some(action.clone());
                            this.pending_action_shortcut_value = None;
                            this.pending_action_shortcut_global = false;
                            this.shortcut_recording = None;
                            this.notice = None;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(13.))
                                        .text_color(theme::primary())
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(px(11.))
                                        .text_color(theme::muted())
                                        .child(context_types),
                                ),
                        ),
                );
            }
            div()
                .flex_1()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(
                    div()
                        .h(px(34.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .rounded(px(metrics::CONTROL_RADIUS))
                        .bg(theme::field())
                        .border(px(metrics::CONTROL_BORDER_WIDTH))
                        .border_color(theme::control_border())
                        .child(
                            Input::new(&self.action_shortcut_input)
                                .id("settings-action-shortcut-search")
                                .aria_label(l10n::find_an_action(self.locale))
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .cleanable(true)
                                .text_size(px(metrics::BODY_SIZE))
                                .w_full(),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h(px(0.))
                        .overflow_y_scrollbar()
                        .child(rows)
                        .when(self.shortcut_catalog_loading, |element| {
                            element.child(
                                div()
                                    .p(px(12.))
                                    .text_size(px(11.))
                                    .text_color(theme::muted())
                                    .child(l10n::loading(self.locale)),
                            )
                        })
                        .when(
                            !self.shortcut_catalog_loading && self.shortcut_catalog.is_empty(),
                            |element| {
                                element.child(
                                    div()
                                        .p(px(12.))
                                        .text_size(px(11.))
                                        .text_color(theme::muted())
                                        .child(l10n::no_actions_available(self.locale)),
                                )
                            },
                        ),
                )
                .child(
                    div().flex().justify_end().child(
                        Button::new("settings-action-picker-cancel")
                            .ghost()
                            .label(l10n::cancel(self.locale))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_action_shortcut_picker(cx);
                            })),
                    ),
                )
                .into_any_element()
        };
        Some(
            div()
                .id("settings-action-picker")
                .test_support()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(Into::<gpui_kit::Hsla>::into(gpui_kit::rgba(0x00000066)))
                .occlude()
                .child(
                    div()
                        .w(px(420.))
                        .h(px(350.))
                        .p(px(18.))
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .rounded(px(14.))
                        .bg(theme::card())
                        .border(px(metrics::DIVIDER_WIDTH))
                        .border_color(theme::selection_outline())
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(12.))
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(16.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme::primary())
                                        .child(l10n::action_shortcut(self.locale)),
                                )
                                .child(
                                    Button::new("settings-action-picker-close")
                                        .ghost()
                                        .icon(IconName::X)
                                        .accessibility_label(l10n::cancel(self.locale))
                                        .tooltip(l10n::cancel(self.locale))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.close_action_shortcut_picker(cx);
                                        })),
                                ),
                        )
                        .child(body),
                )
                .into_any_element(),
        )
    }

    fn shortcut_setting_row(
        &self,
        slot: ShortcutSlot,
        title: impl Into<SharedString>,
        input: &Entity<InputState>,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = title.into();
        let recording = self.shortcut_recording == Some(slot);
        let shortcut = input.read(cx).value();
        let shortcut_label = crate::platform::macos_shortcut_label(&shortcut)
            .unwrap_or_else(|| shortcut.to_string());
        let shortcut_action_label =
            format!("{}: {}", l10n::record_shortcut(self.locale), shortcut_label);
        div()
            .h(px(42.))
            .px(px(metrics::SPACE_MD))
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_MD))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .body_text()
                    .text_color(theme::primary())
                    .child(title),
            )
            .child(
                div().flex_none().flex().items_center().justify_end().child(
                    Button::new(SharedString::from(format!("{id}-record")))
                        .ghost()
                        .accessibility_label(shortcut_action_label.clone())
                        .tooltip(shortcut_action_label)
                        .w(px(98.))
                        .h(px(26.))
                        .px(px(metrics::SPACE_SM))
                        .rounded(px(8.))
                        .border(px(metrics::DIVIDER_WIDTH))
                        .border_color(if recording {
                            theme::accent()
                        } else {
                            theme::divider()
                        })
                        .bg(if recording {
                            theme::button_selection()
                        } else {
                            theme::transparent()
                        })
                        .text_size(px(12.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if recording {
                            theme::accent()
                        } else {
                            theme::secondary()
                        })
                        .cursor_pointer()
                        .label(if recording {
                            l10n::press_ashortcut(self.locale)
                        } else {
                            shortcut_label
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.shortcut_recording = Some(slot);
                            this.notice = None;
                            cx.notify();
                        })),
                ),
            )
    }

    fn context_settings(&mut self, cx: &mut Context<Self>) -> Div {
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
                    .child(l10n::context_description(self.locale)),
            )
            .child(self.settings_rows_card(vec![
                self.context_capture_row(
                    ContextSource::FinderSelection,
                    l10n::finder_selection(self.locale),
                    l10n::selected_files_description(self.locale),
                    self.settings_draft.capture_finder_selection,
                    Switch::new("capture-finder")
                        .checked(self.settings_draft.capture_finder_selection)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_draft.capture_finder_selection = *checked;
                            cx.notify();
                        })),
                    cx,
                ),
                self.context_capture_row(
                    ContextSource::SelectedText,
                    l10n::selected_text(self.locale),
                    l10n::selected_text_description(self.locale),
                    self.settings_draft.capture_selected_text,
                    Switch::new("capture-selected-text")
                        .checked(self.settings_draft.capture_selected_text)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_draft.capture_selected_text = *checked;
                            cx.notify();
                        })),
                    cx,
                ),
                self.context_capture_row(
                    ContextSource::BrowserCurrentPage,
                    l10n::browser_current_page(self.locale),
                    l10n::browser_page_description(self.locale),
                    self.settings_draft.capture_browser_current_page,
                    Switch::new("capture-browser")
                        .checked(self.settings_draft.capture_browser_current_page)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_draft.capture_browser_current_page = *checked;
                            cx.notify();
                        })),
                    cx,
                ),
            ]))
    }

    fn context_capture_row(
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
        let privacy_pane =
            capture.and_then(|capture| privacy_pane_for_capture(source, capture.status));
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

    fn search_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let engine = match self.settings_draft.file_search_engine.as_str() {
            "ripgrep" => l10n::search_engine_ripgrep(self.locale),
            "combined" => l10n::search_engine_combined(self.locale),
            _ => l10n::search_engine_system_index(self.locale),
        };
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .child(self.settings_rows_card(vec![
                self.settings_row_bare(
                    l10n::file_search(self.locale),
                    l10n::search_file_names_in_the_file_search(self.locale),
                    Switch::new("file-search-enabled")
                        .checked(self.settings_draft.file_search_enabled)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_draft.file_search_enabled = *checked;
                            cx.notify();
                        })),
                ),
                self.settings_row_bare(
                    l10n::search_engine(self.locale),
                    l10n::search_engine_description(self.locale),
                    Button::new("cycle-file-engine")
                        .secondary()
                        .label(engine)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings_draft.file_search_engine =
                                match this.settings_draft.file_search_engine.as_str() {
                                    "system_index" | "systemIndex" => "ripgrep".to_owned(),
                                    "ripgrep" => "combined".to_owned(),
                                    _ => "system_index".to_owned(),
                                };
                            cx.notify();
                        })),
                ),
                self.settings_row_bare(
                    l10n::include_hidden_files(self.locale),
                    l10n::include_dotfiles_and_files_in_hidden_directories(self.locale),
                    Switch::new("file-search-hidden")
                        .checked(self.settings_draft.file_search_include_hidden)
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings_draft.file_search_include_hidden = *checked;
                            cx.notify();
                        })),
                ),
                self.settings_stack_row(
                    l10n::directory_access(self.locale),
                    l10n::file_search_access_message(self.locale),
                    div()
                        .flex()
                        .items_center()
                        .flex_wrap()
                        .gap(px(metrics::SPACE_SM))
                        .child(
                            Button::new("authorize-search-directories")
                                .secondary()
                                .label(l10n::authorize_search_directories(self.locale))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    let existing =
                                        lines(this.file_roots_input.read(cx).value().as_ref());
                                    let mut dialog = rfd::AsyncFileDialog::new()
                                        .set_title(l10n::authorize_search_directories(this.locale))
                                        .set_parent(window);
                                    if let Some(first) = existing.first() {
                                        dialog = dialog.set_directory(first);
                                    }
                                    let window_handle = this.window_handle;
                                    cx.spawn(async move |this, cx| {
                                        let Some(selected) = dialog.pick_folders().await else {
                                            return;
                                        };
                                        let selected = selected
                                            .into_iter()
                                            .map(|path| path.path().display().to_string())
                                            .collect::<Vec<_>>();
                                        let roots = unique_trimmed(
                                            existing
                                                .iter()
                                                .map(String::as_str)
                                                .chain(selected.iter().map(String::as_str)),
                                        );
                                        this.update(cx, |this, cx| {
                                            if let Some(live) = this.live.clone() {
                                                this.set_notice(
                                                    l10n::preparing_directory_access(this.locale),
                                                    components::NoticeKind::Info,
                                                );
                                                let receiver = live
                                                    .authorize_search_directories(selected, roots);
                                                this.poll(receiver, this.live_state.generation, cx);
                                            } else {
                                                this.settings_draft.file_search_roots =
                                                    roots.clone();
                                                let input = this.file_roots_input.clone();
                                                let value = roots.join("\n");
                                                let _ = cx.update_window(
                                                    window_handle,
                                                    |_, window, cx| {
                                                        set_input_value(&input, value, window, cx);
                                                    },
                                                );
                                            }
                                            cx.notify();
                                        })
                                        .ok();
                                    })
                                    .detach();
                                })),
                        )
                        .child(
                            Button::new("open-full-disk-access")
                                .ghost()
                                .label(l10n::full_disk_access(self.locale))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if !open_privacy_settings(PrivacySettingsPane::FullDiskAccess) {
                                        this.set_notice(
                                            l10n::open_system_settings_failed(this.locale),
                                            components::NoticeKind::Error,
                                        );
                                        cx.notify();
                                    }
                                })),
                        ),
                ),
            ]))
            .child(self.settings_input_row(
                l10n::authorized_directories(self.locale),
                l10n::authorized_directories_help(self.locale),
                &self.file_roots_input,
                "file-search-roots",
                "/Users/me/Documents, /Users/me/Projects",
            ))
            .child(self.settings_input_row(
                l10n::excluded_directory_names(self.locale),
                l10n::one_directory_name_per_line(self.locale),
                &self.file_exclusions_input,
                "file-search-exclusions",
                "node_modules, target, .git",
            ))
            .child(self.settings_input_row(
                l10n::excluded_applications(self.locale),
                l10n::excluded_applications_help(self.locale),
                &self.excluded_apps_input,
                "excluded-apps",
                "com.example.Editor, com.example.Terminal",
            ))
    }

    fn clipboard_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let status = self.clipboard.status.clone();
        let retention_label = match status.as_ref().map(|status| status.retention_days) {
            Some(0) => l10n::until_storage_limit(self.locale),
            Some(days) => l10n::retention_days(self.locale, i64::from(days)),
            None => l10n::history_storage(self.locale),
        };
        let pause_label = if status
            .as_ref()
            .is_some_and(|status| status.paused_until_ms.is_some())
        {
            l10n::paused(self.locale)
        } else {
            l10n::pause_for(self.locale)
        };
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
                        .accessibility_label(l10n::clipboard(self.locale))
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.settings.clipboard_recording = *checked;
                            if let Some(live) = this.live.clone() {
                                this.settings.saving = true;
                                this.settings.error = None;
                                let receiver = live.set_clipboard_recording(*checked);
                                this.poll(receiver, this.live_state.generation, cx);
                            }
                            cx.notify();
                        })),
                ),
                self.settings_row_bare(
                    l10n::history_storage(self.locale),
                    status
                        .as_ref()
                        .map(|status| {
                            l10n::history_storage_usage(
                                self.locale,
                                i64::from(status.stored_entries),
                                &format!("{:.1}", f64::from(status.stored_bytes) / 1_048_576.0),
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
                                .disabled(status.is_none() || self.clipboard.loading)
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
                                .disabled(status.is_none() || self.clipboard.loading)
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
                                .disabled(status.is_none() || self.clipboard.loading)
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

    fn clipboard_settings_choice_menu(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
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

    fn select_clipboard_retention(
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

    fn set_clipboard_retention_now(&mut self, days: u32, cx: &mut Context<Self>) {
        let Some(live) = self.live.clone() else {
            return;
        };
        let receiver = live.set_clipboard_retention(days, self.clipboard.query.clone());
        self.run_clipboard_command(receiver, cx);
    }

    fn select_clipboard_pause(&mut self, minutes: u32, cx: &mut Context<Self>) {
        self.clipboard_settings_choice = None;
        let Some(live) = self.live.clone() else {
            return;
        };
        let receiver = live.set_clipboard_pause(minutes, self.clipboard.query.clone());
        self.run_clipboard_command(receiver, cx);
        cx.notify();
    }

    fn present_clipboard_confirmation(
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

    pub(super) fn cancel_clipboard_confirmation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clipboard_confirmation = None;
        self.restore_settings_modal_focus(window, cx);
        cx.notify();
    }

    fn confirm_clipboard_confirmation(
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
        let query = self.clipboard.query.clone();
        let receiver = match confirmation {
            ClipboardConfirmation::Clear => live.clear_clipboard(query),
            ClipboardConfirmation::SetRetention(days) => live.set_clipboard_retention(days, query),
        };
        self.run_clipboard_command(receiver, cx);
        cx.notify();
    }

    fn application_history_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let filter = self
            .history_settings_filter_input
            .read(cx)
            .value()
            .trim()
            .to_ascii_lowercase();
        let enabled_only = self.history_settings_enabled_only;
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
        let filter_input = self.history_settings_filter_input.clone();
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
            rows = rows.child(
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
                                Switch::new(SharedString::from(format!(
                                    "history-action-{enabled_id}"
                                )))
                                .checked(source.action_enabled)
                                .accessibility_label(action_accessibility)
                                .on_change(cx.listener(move |this, checked, _, cx| {
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live
                                            .set_app_history_action_enabled(
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
                            .items_center()
                            .gap(px(metrics::SPACE_XS))
                            .flex_wrap()
                            .child(
                                Button::new(SharedString::from(format!(
                                    "history-auto-{automatic_id}"
                                )))
                                .secondary()
                                .label(l10n::app_history_use_automatic(self.locale))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live.configure_app_history(
                                            automatic_id.clone(),
                                            String::new(),
                                            anywhere_core::features::application_history::model::SourceMode::Automatic,
                                        );
                                        this.poll(receiver, this.live_state.generation, cx);
                                    }
                                })),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "history-custom-{custom_id}"
                                )))
                                .secondary()
                                .label(l10n::app_history_save_custom(self.locale))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let path = this
                                        .history_path_input
                                        .read(cx)
                                        .value()
                                        .trim()
                                        .to_owned();
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live.configure_app_history(
                                            custom_id.clone(),
                                            path,
                                            anywhere_core::features::application_history::model::SourceMode::Custom,
                                        );
                                        this.poll(receiver, this.live_state.generation, cx);
                                    }
                                })),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "history-off-{disabled_id}"
                                )))
                                .secondary()
                                .label(l10n::app_history_source_off(self.locale))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live.configure_app_history(
                                            disabled_id.clone(),
                                            String::new(),
                                            anywhere_core::features::application_history::model::SourceMode::Disabled,
                                        );
                                        this.poll(receiver, this.live_state.generation, cx);
                                    }
                                })),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!(
                            "history-select-{configure_id}"
                        )))
                        .ghost()
                        .label(l10n::app_history_edit_path(self.locale))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.app_history.selected_source = Some(configure_id.clone());
                            let value = this
                                .app_history
                                .sources
                                .iter()
                                .find(|source| source.id == configure_id)
                                .and_then(|source| source.path.clone())
                                .map(|path| path.display().to_string())
                                .unwrap_or_default();
                            set_input_value(&this.history_path_input, value, window, cx);
                        })),
                    ),
            );
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
                    .child(
                        div()
                            .h(px(34.))
                            .rounded(px(metrics::CONTROL_RADIUS))
                            .bg(theme::field())
                            .px(px(metrics::SPACE_SM))
                            .flex()
                            .items_center()
                            .child(
                                Input::new(&self.history_settings_filter_input)
                                    .id("history-settings-filter")
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .cleanable(true)
                                    .text_size(px(metrics::BODY_SIZE))
                                    .w_full(),
                            ),
                    )
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
                                        this.history_settings_enabled_only = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("history-filter-enabled")
                                    .when(enabled_only, |button| button.primary())
                                    .when(!enabled_only, |button| button.secondary())
                                    .label(l10n::enabled(self.locale))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.history_settings_enabled_only = true;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(self.settings_input_row(
                l10n::app_history_source_path(self.locale),
                l10n::app_history_unsaved_path(self.locale),
                &self.history_path_input,
                "history-custom-path",
                "~/Library/Application Support/...",
            ))
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
                            this.history_settings_enabled_only = false;
                            let _ = cx.update_window(window_handle, |_, window, cx| {
                                set_input_value(&filter_input, String::new(), window, cx);
                            });
                            cx.notify();
                        })),
                )
            })
    }

    fn feature_settings(&mut self, cx: &mut Context<Self>) -> Div {
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
                        .accessibility_label(accessibility)
                        .on_change(cx.listener(move |this, checked, _, cx| {
                            this.settings.set_feature_enabled(&key, *checked);
                            if let Some(live) = this.live.clone() {
                                this.settings.saving = true;
                                this.settings.error = None;
                                let receiver = live.change_feature(key.clone(), *checked);
                                this.poll(receiver, this.live_state.generation, cx);
                            }
                            cx.notify();
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

    fn language_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let (label, next) = match self.settings_draft.ui_language {
            UiLanguage::System => (l10n::language_system(self.locale), UiLanguage::Zh),
            UiLanguage::Zh => (l10n::language_chinese(self.locale), UiLanguage::En),
            UiLanguage::En => (l10n::language_english(self.locale), UiLanguage::System),
        };
        self.settings_row(
            l10n::language(self.locale),
            l10n::language_description(self.locale),
            div()
                .id("settings-language")
                .role(gpui_kit::Role::Button)
                .aria_label(label.clone())
                .h(px(26.))
                .px(px(10.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(metrics::CONTROL_RADIUS))
                .border(px(metrics::DIVIDER_WIDTH))
                .border_color(theme::divider())
                .hover(|style| style.bg(theme::hover()))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.settings_draft.ui_language = next;
                    let next_locale = locale_for_language(next);
                    if this.locale != next_locale {
                        this.locale = next_locale;
                        this.model.set_locale(next_locale);
                    }
                    set_input_value(
                        &this.input,
                        this.input.read(cx).value().to_string(),
                        window,
                        cx,
                    );
                    cx.notify();
                }))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme::secondary())
                        .child(label),
                )
                .child(
                    Icon::new(IconName::ChevronDown)
                        .size(px(14.))
                        .text_color(theme::secondary()),
                ),
        )
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
