use super::*;

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
                .track_scroll(&self.web_packages_scroll)
                .overflow_y_scroll()
                .vertical_scrollbar(&self.web_packages_scroll);
            for (index, package) in self.web.packages.clone().into_iter().enumerate() {
                let selected = self.web.selected == index;
                let package_id = package.id.clone();
                let package_details = if package.available {
                    format!(
                        "{} {} · {} {}",
                        package.version,
                        package.entry,
                        package.permission_count,
                        l10n::permissions(self.locale)
                    )
                } else {
                    l10n::web_package_damaged(self.locale)
                };
                list = list.child(
                    div()
                        .id(SharedString::from(format!("web-package-{index}")))
                        .aria_label(package.name.clone())
                        .aria_selected(selected)
                        .min_h(px(64.))
                        .px(px(metrics::SPACE_MD))
                        .py(px(metrics::SPACE_SM))
                        .flex()
                        .items_center()
                        .gap(px(metrics::SPACE_MD))
                        .rounded(px(metrics::RESULT_RADIUS))
                        .when(selected, |element| {
                            element
                                .bg(theme::selected())
                                .border(px(metrics::FOCUS_BORDER_WIDTH))
                                .border_color(theme::border())
                        })
                        .when(!selected, |element| {
                            element.hover(|style| style.bg(theme::hover()))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.web.selected = index;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .size(px(34.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(9.))
                                .bg(theme::field())
                                .text_color(theme::accent())
                                .child("◇"),
                        )
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
                                        .body_text()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(package.name),
                                )
                                .child(
                                    div()
                                        .truncate()
                                        .caption_text()
                                        .text_color(if package.available {
                                            theme::secondary()
                                        } else {
                                            theme::warning()
                                        })
                                        .child(package_details),
                                ),
                        )
                        .child(
                            Button::new(SharedString::from(format!("web-toggle-{package_id}")))
                                .secondary()
                                .label(if package.enabled {
                                    l10n::disable_action(self.locale)
                                } else {
                                    l10n::enable_action(self.locale)
                                })
                                .disabled(!package.available && !package.enabled)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.change_web_package_operation(
                                        package_id.clone(),
                                        if package.enabled { "disable" } else { "enable" },
                                        cx,
                                    )
                                })),
                        ),
                );
            }
            list.into_any_element()
        };
        let selected = self.web.packages.get(self.web.selected).cloned();
        let review = self.web_surface.review.clone().map(|review| {
            let approve_token = review.token.clone();
            let cancel_token = review.token.clone();
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
                .child(
                    div()
                        .text_size(px(metrics::CAPTION_SIZE))
                        .text_color(theme::muted())
                        .child(l10n::approval_applies_to_this_inspected_definition_restart(
                            self.locale,
                        )),
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
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.web_surface.review = None;
                                    if let Some(live) = this.live.clone() {
                                        let receiver = live.cancel_web_review(cancel_token.clone());
                                        this.poll(receiver, this.live_state.generation, cx);
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("approve-web-package")
                                .primary()
                                .label(l10n::approve_and_install(self.locale))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(live) = this.live.clone() {
                                        let receiver =
                                            live.approve_web_package(approve_token.clone());
                                        this.poll(receiver, this.live_state.generation, cx);
                                    }
                                })),
                        ),
                )
        });
        let footer = div()
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
                                if let Some(live) = this.live.clone() {
                                    let receiver =
                                        live.inspect_web_package(file.path().display().to_string());
                                    this.poll(receiver, this.live_state.generation, cx);
                                }
                            })
                            .ok();
                        })
                        .detach();
                    })),
            )
            .child(div().flex_1())
            .children(selected.map(|package| {
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
                                    "rollback",
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
                                    "uninstall",
                                    cx,
                                )
                            })),
                    )
            }));
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
                .child(footer)
        };
        self.render_page_shell(
            SharedString::from(l10n::web_tools(self.locale)),
            SharedString::from(l10n::installed_packages_and_permissions(self.locale)),
            None,
            page.into_any_element(),
            cx,
        )
        .into_any_element()
    }
}
