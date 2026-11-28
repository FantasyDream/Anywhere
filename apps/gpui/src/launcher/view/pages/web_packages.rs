use super::*;
use crate::host::WebPackageOperation;

impl LauncherView {
    pub(in crate::launcher::view) fn render_web_packages(
        &mut self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let content = if self.web.packages.is_empty() {
            self.empty_page_state(l10n::no_web_tools_installed(self.locale), None)
        } else {
            let mut list = div()
                .id("web-package-list")
                .flex_1()
                .min_h(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .p(px(6.))
                .track_scroll(&self.scrolls.web_packages)
                .overflow_y_scroll();
            for index in 0..self.web.packages.len() {
                let package = &self.web.packages[index];
                let selected = self.web.selected == index;
                let package_id = package.id.clone();
                let package_name = package.name.clone();
                let package_available = package.available;
                let package_enabled = package.enabled;
                let package_details = if package.available {
                    let summary = format!(
                        "{} {} · {} {}",
                        package.version,
                        package.entry,
                        package.permission_count,
                        l10n::permissions(self.locale)
                    );
                    if package.builtin {
                        format!("{} · {summary}", l10n::built_in(self.locale))
                    } else {
                        summary
                    }
                } else {
                    l10n::web_package_damaged(self.locale)
                };
                list = list.child(
                    components::ListRow::new(format!("web-package-{index}"))
                        .density(components::RowDensity::Comfortable)
                        .selected(selected)
                        .aria_label(package_name.clone())
                        .leading(components::squircle_icon_plate(
                            IconName::Puzzle,
                            theme::accent(),
                            metrics::ICON_SIZE,
                        ))
                        .title(package_name)
                        .title_weight(FontWeight::MEDIUM)
                        .subtitle(package_details)
                        .subtitle_color(if package_available {
                            theme::secondary()
                        } else {
                            theme::warning()
                        })
                        .trailing(
                            div()
                                .id(SharedString::from(format!("web-toggle-slot-{package_id}")))
                                .test_support()
                                .pl(px(metrics::SPACE_SM))
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "web-toggle-{package_id}"
                                    )))
                                    .secondary()
                                    .label(if package_enabled {
                                        l10n::disable_action(self.locale)
                                    } else {
                                        l10n::enable_action(self.locale)
                                    })
                                    .disabled(!package_available && !package_enabled)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.change_web_package_operation(
                                            package_id.clone(),
                                            if package_enabled {
                                                WebPackageOperation::Disable
                                            } else {
                                                WebPackageOperation::Enable
                                            },
                                            cx,
                                        )
                                    })),
                                ),
                        )
                        .build()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.web.selected = index;
                            cx.notify();
                        }))
                        .test_support(),
                );
            }
            list.vertical_scrollbar(&self.scrolls.web_packages)
                .into_any_element()
        };
        let selected = self.web.packages.get(self.web.selected).cloned();
        let diagnostics = selected
            .as_ref()
            .filter(|package| !package.diagnostics.is_empty())
            .map(|package| {
                div()
                    .p(px(metrics::SPACE_SM))
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::secondary())
                    .child(package.diagnostics.join("\n"))
            });
        let review = self
            .web_surface
            .review
            .clone()
            .as_ref()
            .map(|review| self.render_web_package_review(review, cx));
        let footer = self.render_web_packages_footer(selected, cx);
        let page = if let Some(review) = review {
            div()
                .w_full()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .child(
                    div()
                        .w_full()
                        .max_w(px(LAUNCHER_WIDTH))
                        .min_w(px(0.))
                        .flex_1()
                        .min_h(px(0.))
                        .p(px(metrics::SPACE_MD))
                        .overflow_y_scrollbar()
                        .child(review),
                )
        } else {
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .child(content)
                .children(diagnostics)
                .child(footer)
        };
        self.render_page_shell(
            SharedString::from(l10n::web_tools(self.locale)),
            SharedString::from(l10n::installed_packages_and_permissions(self.locale)),
            None,
            page.into_any_element(),
            cx,
        )
        .id("web-packages-surface")
        .key_context("WebPackages")
        .track_focus(&self.surface_focus_handle)
        .into_any_element()
    }

    fn render_web_packages_footer(
        &self,
        selected: Option<crate::host::WebPackage>,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .px(px(metrics::SPACE_MD))
            .py(px(metrics::SPACE_SM))
            .border_t(px(0.5))
            .border_color(theme::divider())
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_SM))
            .child(
                Button::new("import-web-package")
                    .secondary()
                    .label(l10n::import_package(self.locale))
                    .disabled(self.web_surface.review_loading)
                    .on_click(cx.listener(|this, _, window, cx| {
                        let dialog = rfd::AsyncFileDialog::new()
                            .set_title(l10n::import_local_web_tool(this.locale))
                            .add_filter("Anywhere plugin", &["awplugin"])
                            .set_parent(window);
                        cx.spawn(async move |this, cx| {
                            let Some(file) = dialog.pick_file().await else {
                                return;
                            };
                            this.update(cx, |this, cx| {
                                this.inspect_package(file.path().to_owned(), cx);
                            })
                            .ok();
                        })
                        .detach();
                    })),
            )
            .child(div().flex_1())
            .when_some(
                selected
                    .as_ref()
                    .and_then(|package| package.open_action_id.clone()),
                |row, action_id| {
                    row.child(
                        Button::new("open-installed-plugin")
                            .secondary()
                            .label(l10n::open_approved_tool(self.locale))
                            .disabled(self.settings_dirty(cx))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if this.settings_dirty(cx) {
                                    return;
                                }
                                this.screen = Screen::Launcher;
                                this.activate_action_shortcut(
                                    crate::host::ActionShortcutBinding {
                                        action_id: action_id.clone(),
                                        title: String::new(),
                                        shortcut: String::new(),
                                        global: false,
                                        routes: Vec::new(),
                                    },
                                    window,
                                    cx,
                                );
                            })),
                    )
                },
            )
            .children(selected.filter(|package| !package.builtin).map(|package| {
                let id = package.id;
                let rollback_id = id.clone();
                let uninstall_id = id;
                div()
                    .flex()
                    .items_center()
                    .gap(px(metrics::SPACE_SM))
                    .child(
                        Button::new("web-rollback")
                            .secondary()
                            .label(l10n::rollback(self.locale))
                            .disabled(!package.rollback)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_web_package_operation(
                                    rollback_id.clone(),
                                    WebPackageOperation::Rollback,
                                    cx,
                                )
                            })),
                    )
                    .child(
                        Button::new("web-uninstall")
                            .secondary()
                            .label(l10n::remove(self.locale))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_web_package_operation(
                                    uninstall_id.clone(),
                                    WebPackageOperation::Uninstall,
                                    cx,
                                )
                            })),
                    )
            }))
    }

    fn render_web_package_review(
        &self,
        review: &crate::host::WebPackageReview,
        cx: &mut Context<Self>,
    ) -> gpui_kit::Stateful<Div> {
        use anywhere_core::application::web_plugins::SameWebPackageVersion;

        let language = if self.locale == Locale::Zh {
            "zh"
        } else {
            "en"
        };
        let commands = review
            .details
            .commands
            .iter()
            .map(|(title, localized)| localized.get(language).unwrap_or(title).as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let approve_token = review.token.clone();
        div()
            .id("web-package-review")
            .w_full()
            .max_w(px(LAUNCHER_WIDTH - 2.0 * metrics::SPACE_MD))
            .min_w(px(0.))
            .p(px(metrics::SPACE_LG))
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_MD))
            .rounded(px(metrics::CARD_RADIUS))
            .bg(theme::field())
            .border(px(0.7))
            .border_color(theme::warning())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(metrics::SPACE_MD))
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
                                    .text_size(px(metrics::HEADING_SIZE))
                                    .line_height(px(metrics::HEADING_SIZE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(if review.updating {
                                        l10n::update_tool_review(self.locale, &review.name)
                                    } else {
                                        l10n::install_tool_review(self.locale, &review.name)
                                    }),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .caption_text()
                                    .text_color(theme::secondary())
                                    .child(format!("{} · {}", review.id, review.version)),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .px(px(metrics::SPACE_SM))
                            .py(px(2.))
                            .rounded(px(5.0))
                            .bg(theme::field())
                            .font_family(fonts::FONT_MONO)
                            .text_size(px(metrics::CAPTION_SIZE))
                            .text_color(theme::muted())
                            .child(review.digest.chars().take(16).collect::<String>()),
                    ),
            )
            .child(div().text_color(theme::primary()).caption_text().child(
                l10n::plugin_command_count(self.locale, &review.details.command_count.to_string()),
            ))
            .when(!commands.is_empty(), |card| {
                card.child(
                    div()
                        .caption_text()
                        .text_color(theme::secondary())
                        .child(commands),
                )
            })
            .when(
                review.details.resolver_count > 0 || review.details.background_actions > 0,
                |card| {
                    card.child(div().caption_text().text_color(theme::secondary()).child(
                        l10n::plugin_background_activity(
                            self.locale,
                            &review.details.resolver_count.to_string(),
                            &review.details.background_actions.to_string(),
                        ),
                    ))
                },
            )
            .child(self.render_review_permissions(review))
            .child(
                div()
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::muted())
                    .child(l10n::plugin_execution_boundary(self.locale)),
            )
            .when(review.updating, |element| {
                element.child(
                    div()
                        .text_size(px(metrics::CAPTION_SIZE))
                        .text_color(theme::warning())
                        .child(l10n::this_replaces_the_installed_code_closes_its(
                            self.locale,
                        )),
                )
            })
            .when_some(review.same_version, |element, installed| {
                element.child(
                    div()
                        .text_size(px(metrics::CAPTION_SIZE))
                        .text_color(theme::secondary())
                        .child(match installed {
                            SameWebPackageVersion::Enabled => {
                                l10n::plugin_version_installed(self.locale)
                            }
                            SameWebPackageVersion::Disabled => {
                                l10n::plugin_version_disabled(self.locale)
                            }
                        }),
                )
            })
            .child(
                div()
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::muted())
                    .child(l10n::approval_applies_to_this_package_digest(self.locale)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap(px(metrics::SPACE_SM))
                    .child(
                        Button::new("cancel-web-review")
                            .ghost()
                            .label(l10n::cancel(self.locale))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cancel_web_package_review(window, cx);
                            })),
                    )
                    .child(
                        Button::new("approve-web-package")
                            .primary()
                            .label(match review.same_version {
                                Some(SameWebPackageVersion::Disabled) => {
                                    l10n::approve_and_enable(self.locale)
                                }
                                Some(SameWebPackageVersion::Enabled) => l10n::approve(self.locale),
                                None => l10n::approve_and_install(self.locale),
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this
                                    .web_surface
                                    .review
                                    .as_ref()
                                    .is_some_and(|review| review.token == approve_token)
                                {
                                    if let Some(live) = this.live.clone() {
                                        let receiver =
                                            live.approve_web_package(approve_token.clone());
                                        this.web_surface.review = None;
                                        this.poll(receiver, this.live_state.generation, cx);
                                        cx.notify();
                                    }
                                }
                            })),
                    ),
            )
    }

    fn render_review_permissions(&self, review: &crate::host::WebPackageReview) -> Div {
        let grant_labels = |grants: &[String]| {
            grants
                .iter()
                .map(|value| web_permission_label(value, self.locale))
                .collect::<Vec<_>>()
                .join("\n")
        };
        div()
            .flex()
            .flex_col()
            .gap(px(metrics::SPACE_SM))
            .child(
                div()
                    .text_size(px(metrics::CAPTION_SIZE))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme::primary())
                    .child(l10n::requested_access(self.locale)),
            )
            .child(
                div()
                    .text_size(px(metrics::CAPTION_SIZE))
                    .text_color(theme::secondary())
                    .child(if review.permissions.is_empty() {
                        l10n::no_host_permissions(self.locale)
                    } else {
                        review
                            .permissions
                            .iter()
                            .map(|permission| {
                                format!("• {}", web_permission_label(permission, self.locale))
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    }),
            )
            .when(review.updating, |card| {
                card.when(!review.details.added.is_empty(), |card| {
                    card.child(div().caption_text().text_color(theme::warning()).child(
                        l10n::plugin_added_grants(
                            self.locale,
                            &grant_labels(&review.details.added),
                        ),
                    ))
                })
                .when(!review.details.removed.is_empty(), |card| {
                    card.child(div().caption_text().text_color(theme::secondary()).child(
                        l10n::plugin_removed_grants(
                            self.locale,
                            &grant_labels(&review.details.removed),
                        ),
                    ))
                })
                .children(review.details.scope_changes.iter().map(|(old, new)| {
                    div().caption_text().text_color(theme::warning()).child(
                        l10n::plugin_scope_changed(
                            self.locale,
                            &web_permission_label(old, self.locale),
                            &web_permission_label(new, self.locale),
                        ),
                    )
                }))
            })
            .when(review.details.data_to_url, |card| {
                card.child(
                    div()
                        .caption_text()
                        .text_color(theme::warning())
                        .child(l10n::plugin_data_external_risk(self.locale)),
                )
            })
            .when(review.details.any_origin, |card| {
                card.child(
                    div()
                        .caption_text()
                        .text_color(theme::warning())
                        .child(l10n::plugin_any_origin_risk(self.locale)),
                )
            })
            .when(review.details.insecure_http, |card| {
                card.child(
                    div()
                        .caption_text()
                        .text_color(theme::warning())
                        .child(l10n::plugin_http_risk(self.locale)),
                )
            })
    }
}
