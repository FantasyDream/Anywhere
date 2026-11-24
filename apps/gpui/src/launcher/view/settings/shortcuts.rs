use super::*;

impl LauncherView {
    pub(in crate::launcher::view) fn shortcut_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let bindings = self.settings_editor.draft.action_shortcuts.clone();
        let context_editing = self.shortcuts.context_draft.is_some();
        let catalog_empty = self.shortcuts.catalog.is_empty();
        let mut action_rows = div().flex().flex_col();
        for binding in bindings {
            action_rows = action_rows.child(self.render_action_shortcut_binding_row(
                &binding,
                catalog_empty,
                cx,
            ));
        }

        let context_shortcut_builder = self
            .shortcuts
            .context_draft
            .as_ref()
            .map(|draft| self.render_context_shortcut_builder(draft, cx));

        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_LG))
            .child(self.settings_rows_card(vec![
                self.shortcut_setting_row(
                    ShortcutSlot::CurrentApp,
                    l10n::current_app(self.locale),
                    &self.shortcuts.current_input,
                    "current-shortcut",
                    cx,
                ),
                self.shortcut_setting_row(
                    ShortcutSlot::Universal,
                    l10n::universal_search(self.locale),
                    &self.shortcuts.universal_input,
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
                                    .p(px(metrics::SPACE_MD))
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(metrics::SPACE_SM))
                                    .child(
                                        Button::new("settings-add-action-shortcut")
                                            .secondary()
                                            .icon(IconName::Plus)
                                            .label(l10n::add_action_shortcut(self.locale))
                                            .disabled(
                                                self.shortcuts.catalog_loading || catalog_empty,
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
                                                    self.shortcuts.catalog_loading || catalog_empty,
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

    pub(in crate::launcher::view) fn open_action_shortcut_picker(
        &mut self,
        action: Option<ShortcutCatalogAction>,
        shortcut: Option<String>,
        global: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_editor.action_picker_open = true;
        self.shortcuts.recording = None;
        self.shortcuts.pending_action = action;
        self.shortcuts.pending_value = shortcut;
        self.shortcuts.pending_global = global;
        self.notice = None;
        if self.shortcuts.pending_action.is_none() {
            set_input_value(&self.shortcuts.action_input, String::new(), window, cx);
            let input = self.shortcuts.action_input.clone();
            cx.defer_in(window, move |this, window, cx| {
                if this.settings_editor.action_picker_open
                    && this.shortcuts.pending_action.is_none()
                {
                    input.update(cx, |input, cx| input.focus(window, cx));
                }
            });
        }
        cx.notify();
    }

    pub(in crate::launcher::view) fn close_action_shortcut_picker(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.settings_editor.action_picker_open = false;
        self.shortcuts.recording = None;
        self.shortcuts.pending_action = None;
        self.shortcuts.pending_value = None;
        self.shortcuts.pending_global = false;
        self.notice = None;
        cx.notify();
    }

    pub(in crate::launcher::view) fn commit_action_shortcut_picker(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let (Some(action), Some(shortcut)) = (
            self.shortcuts.pending_action.clone(),
            self.shortcuts.pending_value.clone(),
        ) else {
            return;
        };
        let title = shortcut_action_title(&action, self.locale);
        let mut draft = self.settings_editor.draft.clone();
        draft
            .action_shortcuts
            .retain(|binding| binding.action_id != action.action_id);
        draft.action_shortcuts.push(ActionShortcutBinding {
            action_id: action.action_id,
            title,
            shortcut,
            global: self.shortcuts.pending_global,
            routes: Vec::new(),
        });
        match draft.validate() {
            Ok(()) => {
                self.settings_editor.draft = draft;
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

    pub(in crate::launcher::view) fn action_shortcut_picker(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.settings_editor.action_picker_open {
            return None;
        }
        let selected = self.shortcuts.pending_action.clone();
        let body = if let Some(action) = selected {
            self.render_action_shortcut_picker_detail(&action, cx)
        } else {
            self.render_action_shortcut_picker_list(cx)
        };
        Some(
            div()
                .id("settings-action-picker")
                .track_focus(&self.settings_editor.action_picker_focus)
                .tab_group()
                .tab_index(0)
                .tab_stop(false)
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
                        .h(px(350.))
                        .p(px(metrics::SPACE_XL))
                        .flex()
                        .flex_col()
                        .gap(px(metrics::SPACE_MD))
                        .rounded(px(metrics::MODAL_RADIUS))
                        .bg(theme::action_menu_surface())
                        .text_color(theme::primary())
                        .border(px(metrics::DIVIDER_WIDTH))
                        .border_color(theme::selection_outline())
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(metrics::SPACE_MD))
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(metrics::MODAL_TITLE_SIZE))
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

    pub(in crate::launcher::view) fn shortcut_setting_row(
        &self,
        slot: ShortcutSlot,
        title: impl Into<SharedString>,
        input: &Entity<InputState>,
        id: &'static str,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = title.into();
        let recording = self.shortcuts.recording == Some(slot);
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
                    .min_w_0()
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
                        .w(px(132.))
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
                            this.shortcuts.recording = Some(slot);
                            this.notice = None;
                            cx.notify();
                        })),
                ),
            )
    }

    fn render_context_shortcut_builder(
        &self,
        draft: &ContextShortcutDraft,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let recording = self.shortcuts.recording == Some(ShortcutSlot::ContextShortcut);
        let name_is_present = !self
            .shortcuts
            .context_name_input
            .read(cx)
            .value()
            .trim()
            .is_empty();
        let can_save = name_is_present && draft.shortcut.is_some() && !draft.routes.is_empty();
        let query = self
            .shortcuts
            .action_input
            .read(cx)
            .value()
            .to_ascii_lowercase();
        let context_catalog_rows = self.render_context_shortcut_catalog_rows(&query, cx);
        let route_rows = self.render_context_shortcut_route_rows(&draft.routes, cx);
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
                        Input::new(&self.shortcuts.context_name_input)
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
                                    .as_deref()
                                    .and_then(crate::platform::macos_shortcut_label)
                                    .unwrap_or_else(|| l10n::record_shortcut(self.locale))
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.shortcuts.recording = Some(ShortcutSlot::ContextShortcut);
                                this.shortcuts.pending_action = None;
                                this.notice = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Switch::new("settings-context-shortcut-global")
                            .checked(draft.global)
                            .accessibility_label(l10n::global_shortcut(self.locale))
                            .on_change(cx.listener(|this, checked, _, cx| {
                                if let Some(draft) = this.shortcuts.context_draft.as_mut() {
                                    draft.global = *checked;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(metrics::KEYCAP_FONT_SIZE))
                            .text_color(theme::secondary())
                            .child(l10n::global(self.locale)),
                    ),
            )
            .child(route_rows)
            .child(
                div()
                    .text_size(px(metrics::CAPTION_SIZE))
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
                        Input::new(&self.shortcuts.action_input)
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
                    .when(self.shortcuts.catalog_loading, |element| {
                        element.child(
                            div()
                                .p(px(metrics::CONTROL_PADDING_X))
                                .text_size(px(metrics::CAPTION_SIZE))
                                .text_color(theme::muted())
                                .child(l10n::loading(self.locale)),
                        )
                    })
                    .when(
                        !self.shortcuts.catalog_loading && self.shortcuts.catalog.is_empty(),
                        |element| {
                            element.child(
                                div()
                                    .p(px(metrics::CONTROL_PADDING_X))
                                    .text_size(px(metrics::CAPTION_SIZE))
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
                                this.shortcuts.context_draft = None;
                                this.shortcuts.recording = None;
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
    }

    fn render_action_shortcut_binding_row(
        &self,
        binding: &ActionShortcutBinding,
        catalog_empty: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let action = self
            .shortcuts
            .catalog
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
                .shortcuts
                .catalog
                .iter()
                .any(|candidate| candidate.action_id == action.action_id);
        div()
            .min_h(px(58.))
            .px(px(metrics::SPACE_MD))
            .py(px(metrics::SPACE_XS))
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_SM))
            .border_b(px(metrics::DIVIDER_WIDTH))
            .border_color(theme::divider())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(metrics::ROW_INSET_Y_REGULAR))
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
                                .text_size(px(metrics::KEYCAP_FONT_SIZE))
                                .text_color(theme::muted())
                                .child(l10n::context_rules_count(self.locale, route_count)),
                        )
                    })
                    .when(action_missing, |element| {
                        element.child(
                            div()
                                .truncate()
                                .text_size(px(metrics::KEYCAP_FONT_SIZE))
                                .text_color(theme::warning())
                                .child(l10n::action_unavailable_shortcut_paused(self.locale)),
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
                .on_click(cx.listener(move |this, _, window, cx| {
                    if has_context_routes {
                        this.open_context_shortcut_draft(Some(&binding_for_edit), window, cx);
                    } else {
                        this.open_action_shortcut_picker(
                            Some(action_for_edit.clone()),
                            Some(binding_for_edit.shortcut.clone()),
                            binding_for_edit.global,
                            window,
                            cx,
                        );
                    }
                })),
            )
            .child(
                Button::new(SharedString::from(format!("action-remove-{remove_id}")))
                    .ghost()
                    .icon(IconName::Trash)
                    .accessibility_label(l10n::remove_action_shortcut(self.locale, &action_label))
                    .tooltip(l10n::remove_action_shortcut(self.locale, &action_label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings_editor
                            .draft
                            .action_shortcuts
                            .retain(|binding| binding.action_id != remove_id);
                        if this
                            .shortcuts
                            .pending_action
                            .as_ref()
                            .is_some_and(|pending| pending.action_id == remove_id)
                        {
                            this.close_action_shortcut_picker(cx);
                        }
                        cx.notify();
                    })),
            )
    }

    fn render_action_shortcut_picker_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self
            .shortcuts
            .action_input
            .read(cx)
            .value()
            .to_ascii_lowercase();
        let actions = self
            .shortcuts
            .catalog
            .iter()
            .filter(|action| {
                query.is_empty()
                    || shortcut_action_title(action, self.locale)
                        .to_lowercase()
                        .contains(&query)
                    || action.title.to_lowercase().contains(&query)
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
                    .px(px(metrics::CONTROL_PADDING_X))
                    .py(px(metrics::SPACE_XS))
                    .flex()
                    .items_center()
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .hover(|style| style.bg(theme::hover()))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.shortcuts.pending_action = Some(action.clone());
                        this.shortcuts.pending_value = None;
                        this.shortcuts.pending_global = false;
                        this.shortcuts.recording = None;
                        this.notice = None;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(metrics::BODY_SIZE))
                                    .text_color(theme::primary())
                                    .child(title),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(metrics::CAPTION_SIZE))
                                    .text_color(theme::muted())
                                    .child(context_types),
                            ),
                    ),
            );
        }
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(metrics::CONTROL_PADDING_X))
            .child(
                div()
                    .h(px(34.))
                    .px(px(metrics::CONTROL_PADDING_X))
                    .flex()
                    .items_center()
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .bg(theme::field())
                    .border(px(metrics::CONTROL_BORDER_WIDTH))
                    .border_color(theme::control_border())
                    .child(
                        Input::new(&self.shortcuts.action_input)
                            .id("settings-action-shortcut-search")
                            .aria_label(l10n::find_an_action(self.locale))
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .cleanable(true)
                            .text_size(px(metrics::BODY_SIZE))
                            .text_color(theme::primary())
                            .w_full(),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(rows)
                    .when(self.shortcuts.catalog_loading, |element| {
                        element.child(
                            div()
                                .p(px(metrics::SPACE_MD))
                                .text_size(px(metrics::CAPTION_SIZE))
                                .text_color(theme::muted())
                                .child(l10n::loading(self.locale)),
                        )
                    })
                    .when(
                        !self.shortcuts.catalog_loading && self.shortcuts.catalog.is_empty(),
                        |element| {
                            element.child(
                                div()
                                    .p(px(metrics::SPACE_MD))
                                    .text_size(px(metrics::CAPTION_SIZE))
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
    }

    fn render_context_shortcut_catalog_rows(&self, query: &str, cx: &mut Context<Self>) -> Div {
        let mut context_catalog_rows = div().flex().flex_col();
        for action in self
            .shortcuts
            .catalog
            .iter()
            .filter(|action| {
                query.is_empty()
                    || shortcut_action_title(action, self.locale)
                        .to_lowercase()
                        .contains(query)
                    || action.title.to_lowercase().contains(query)
                    || action.action_id.to_ascii_lowercase().contains(query)
            })
            .take(16)
            .cloned()
        {
            let action_id = action.action_id.clone();
            context_catalog_rows = context_catalog_rows.child(
                div()
                    .min_h(px(38.))
                    .px(px(metrics::SPACE_SM))
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_SM))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(metrics::LABEL_SIZE))
                            .child(shortcut_action_title(&action, self.locale)),
                    )
                    .child(
                        Button::new(SharedString::from(format!(
                            "context-action-add-{action_id}"
                        )))
                        .secondary()
                        .label(l10n::add_rule(self.locale))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.add_context_shortcut_action(&action, cx);
                        })),
                    ),
            );
        }
        context_catalog_rows
    }

    fn render_context_shortcut_route_rows(
        &self,
        routes: &[ActionShortcutRoute],
        cx: &mut Context<Self>,
    ) -> Div {
        let mut route_rows = div().flex().flex_col().gap(px(metrics::SPACE_XS));
        for route in routes {
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
                            .min_w_0()
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
                                    .text_size(px(metrics::KEYCAP_FONT_SIZE))
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
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(draft) = this.shortcuts.context_draft.as_mut() {
                                draft.routes.retain(|route| {
                                    route.action_id != remove_action_id
                                        || route.context_type != remove_context_type
                                });
                                cx.notify();
                            }
                        })),
                    ),
            );
        }
        route_rows
    }

    fn render_action_shortcut_picker_detail(
        &self,
        action: &ShortcutCatalogAction,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let recording = self.shortcuts.recording == Some(ShortcutSlot::Action);
        let shortcut_label = self
            .shortcuts
            .pending_value
            .as_deref()
            .and_then(crate::platform::macos_shortcut_label)
            .unwrap_or_else(|| {
                self.shortcuts
                    .pending_value
                    .clone()
                    .unwrap_or_else(|| l10n::record_shortcut(self.locale))
            });
        let context_types = if action.context_types.is_empty() {
            l10n::not_detected_in_the_last_context_the(self.locale)
        } else {
            action.context_types.join(" · ")
        };
        let input = self.shortcuts.action_input.clone();
        let can_save = self.shortcuts.pending_value.is_some() && !recording;
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_MD))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(metrics::PAGE_TITLE_GAP))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(metrics::BODY_SIZE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::primary())
                            .child(shortcut_action_title(action, self.locale)),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(metrics::CAPTION_SIZE))
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
                        this.shortcuts.recording = Some(ShortcutSlot::Action);
                        this.notice = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_size(px(metrics::LABEL_SIZE))
                    .line_height(px(metrics::HEADING_SIZE))
                    .text_color(theme::muted())
                    .child(l10n::use_command_option_or_control_with_a(self.locale)),
            )
            .child(
                div()
                    .h(px(46.))
                    .px(px(metrics::CONTROL_PADDING_X))
                    .flex()
                    .items_center()
                    .gap(px(metrics::CONTROL_PADDING_X))
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .bg(theme::field())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(metrics::BODY_SIZE))
                                    .text_color(theme::primary())
                                    .child(l10n::global_shortcut(self.locale)),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(metrics::CAPTION_SIZE))
                                    .text_color(theme::muted())
                                    .child(l10n::works_from_other_apps_uses_fresh_context(
                                        self.locale,
                                    )),
                            ),
                    )
                    .child(
                        Switch::new("settings-action-picker-global")
                            .checked(self.shortcuts.pending_global)
                            .accessibility_label(l10n::global_shortcut(self.locale))
                            .on_change(cx.listener(|this, checked, _, cx| {
                                this.shortcuts.pending_global = *checked;
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
                    .gap(px(metrics::SPACE_SM))
                    .child(
                        Button::new("settings-action-picker-another")
                            .ghost()
                            .label(l10n::choose_another_action(self.locale))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.shortcuts.pending_action = None;
                                this.shortcuts.pending_value = None;
                                this.shortcuts.pending_global = false;
                                this.shortcuts.recording = None;
                                this.notice = None;
                                let input = input.clone();
                                cx.defer_in(window, move |this, window, cx| {
                                    if this.settings_editor.action_picker_open
                                        && this.shortcuts.pending_action.is_none()
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
    }
}
