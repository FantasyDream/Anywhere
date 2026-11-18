use super::*;

impl LauncherView {
    fn choose_search_directories(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.saving {
            return;
        }
        let existing = lines(self.file_roots_input.read(cx).value().as_ref());
        let mut dialog = rfd::AsyncFileDialog::new()
            .set_title(l10n::authorize_search_directories(self.locale))
            .set_parent(window);
        if let Some(first) = existing.first() {
            dialog = dialog.set_directory(first);
        }
        let operation = self.begin_settings_operation();
        let window_handle = self.window_handle;
        let live = self.live.clone();
        let locale = self.locale;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match dialog.pick_folders().await {
                Some(selected) => {
                    let selected = selected
                        .into_iter()
                        .map(|folder| folder.path().display().to_string())
                        .collect::<Vec<_>>();
                    let roots =
                        unique_trimmed(existing.iter().chain(&selected).map(String::as_str));
                    if let Some(live) = live {
                        match live
                            .authorize_search_directories(selected, roots)
                            .recv()
                            .await
                        {
                            Some(LiveEvent::DirectoriesAuthorized { roots }) => Ok(Some(roots)),
                            Some(LiveEvent::Error(error)) => Err(error),
                            _ => Err(l10n::search_directory_authorization_failed(locale)),
                        }
                    } else {
                        Ok(Some(roots))
                    }
                }
                None => Ok(None),
            };
            let _ = cx.update_window(window_handle, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if operation != this.settings_save_generation {
                        return;
                    }
                    this.settings.saving = false;
                    if this.screen == Screen::Settings {
                        this.finish_search_directory_selection(existing, result, window, cx);
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    pub(in crate::launcher::view) fn finish_search_directory_selection(
        &mut self,
        previous_roots: Vec<String>,
        result: Result<Option<Vec<String>>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(Some(roots))
                if lines(self.file_roots_input.read(cx).value().as_ref()) == previous_roots =>
            {
                self.settings_draft.file_search_roots = roots.clone();
                set_input_value(&self.file_roots_input, roots.join("\n"), window, cx);
                self.set_notice(
                    l10n::search_directories_authorized(self.locale),
                    components::NoticeKind::Success,
                );
            }
            Ok(Some(_)) => self.set_notice(
                l10n::new_edits_were_made_while_saving_save(self.locale),
                components::NoticeKind::Warning,
            ),
            Ok(None) if self.settings_close_pending && !self.settings_dirty(cx) => {
                self.finish_settings_exit_after_save(cx);
            }
            Ok(None) => {}
            Err(error) => {
                self.settings.error = Some(error.clone());
                self.set_notice(error, components::NoticeKind::Error);
            }
        }
    }

    pub(in crate::launcher::view) fn search_settings(&mut self, cx: &mut Context<Self>) -> Div {
        let engine = match self.settings_draft.file_search_engine {
            FileSearchEngine::Ripgrep => l10n::search_engine_ripgrep(self.locale),
            FileSearchEngine::Combined => l10n::search_engine_combined(self.locale),
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
                            .dropdown_caret(true)
                            .dropdown_menu({
                                let view = cx.weak_entity();
                                let locale = self.locale;
                                let selected = self.settings_draft.file_search_engine;
                                move |mut menu, _, _| {
                                    for (engine, label) in [
                                        (
                                            FileSearchEngine::SystemIndex,
                                            l10n::search_engine_system_index(locale),
                                        ),
                                        (
                                            FileSearchEngine::Ripgrep,
                                            l10n::search_engine_ripgrep(locale),
                                        ),
                                        (
                                            FileSearchEngine::Combined,
                                            l10n::search_engine_combined(locale),
                                        ),
                                    ] {
                                        let view = view.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(label)
                                                .checked(engine == selected)
                                                .on_click(move |_, _, cx| {
                                                    let _ = view.update(cx, |this, cx| {
                                                        this.settings_draft.file_search_engine =
                                                            engine;
                                                        cx.notify();
                                                    });
                                                }),
                                        );
                                    }
                                    menu
                                }
                            }),
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
                                    .disabled(self.settings.saving)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.choose_search_directories(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("open-full-disk-access")
                                    .ghost()
                                    .label(l10n::full_disk_access(self.locale))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if !open_privacy_settings(
                                            PrivacySettingsPane::FullDiskAccess,
                                        ) {
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
}
