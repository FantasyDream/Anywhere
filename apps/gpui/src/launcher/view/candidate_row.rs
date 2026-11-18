use super::*;

impl LauncherView {
    pub(super) fn render_row(
        &mut self,
        index: usize,
        row: LauncherRow,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let enabled = row.enabled;
        let is_menu = row.is_menu;
        let is_file = row.kind == CandidateKind::File;
        let icon = self.row_icon(&row);
        let source_label = row.source_label.clone().filter(|_| {
            !(is_file && (self.screen == Screen::FileSearch || self.model.object_actions_active))
        });
        let subtitle = match (row.subtitle.clone(), source_label) {
            (Some(subtitle), Some(source)) => Some(format!("{source} · {subtitle}")),
            (Some(subtitle), None) => Some(subtitle),
            (None, Some(source)) => Some(source),
            (None, None) => None,
        };
        let title = row.title.clone();
        let accessibility = row.title.clone();
        let shortcut = row.shortcut.clone();
        let shortcut_tooltip = l10n::anywhere_action_shortcut(self.locale);
        let preview_tooltip = l10n::shortcut_hint(self.locale, &l10n::preview(self.locale), "␣");
        let actions_tooltip = l10n::shortcut_hint(self.locale, &l10n::actions(self.locale), "⌘K");
        let primary_label = if is_menu {
            l10n::open(self.locale)
        } else if row.command_id.as_deref() == Some("core.files.search") {
            l10n::search(self.locale)
        } else {
            match row.kind {
                CandidateKind::File | CandidateKind::Application => l10n::open(self.locale),
                _ => l10n::run(self.locale),
            }
        };
        let show_preview = selected && enabled && is_file && self.can_preview_selected_file();
        let show_actions = selected
            && enabled
            && !is_menu
            && !self.model.object_actions_active
            && self.live.is_some()
            && self
                .model
                .can_show_object_actions(&row, self.screen == Screen::FileSearch);
        let has_row_actions = show_preview || show_actions;
        let preview_space = self.file_search.browsing || self.model.query.trim().is_empty();
        let mut row_actions = div().flex_none().flex().items_center();
        if show_preview {
            row_actions = row_actions.child(
                div()
                    .id(SharedString::from(format!("candidate-{index}-preview")))
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label(l10n::preview(self.locale))
                    .tooltip(components::text_tooltip(preview_tooltip.clone()))
                    .h(px(metrics::CONTROL_HEIGHT))
                    .px(px(metrics::CONTROL_PADDING_X))
                    .flex()
                    .items_center()
                    .gap(px(3.))
                    .rounded(px(6.))
                    .text_size(px(metrics::CONTROL_FONT_SIZE))
                    .line_height(px(metrics::CONTROL_FONT_SIZE * metrics::BODY_LINE_HEIGHT))
                    .text_color(theme::secondary())
                    .hover(|style| style.bg(theme::hover()))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.request_preview(cx);
                    }))
                    .child(Icon::new(IconName::Eye).size(px(14.)))
                    .when(preview_space, |element| element.child("␣")),
            );
        }
        if show_actions {
            row_actions = row_actions.child(
                div()
                    .id(SharedString::from(format!("candidate-{index}-actions")))
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label(l10n::actions(self.locale))
                    .tooltip(components::text_tooltip(actions_tooltip.clone()))
                    .h(px(metrics::CONTROL_HEIGHT))
                    .px(px(metrics::CONTROL_PADDING_X))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .rounded(px(6.))
                    .text_size(px(metrics::CONTROL_FONT_SIZE))
                    .line_height(px(metrics::CONTROL_FONT_SIZE * metrics::BODY_LINE_HEIGHT))
                    .text_color(theme::secondary())
                    .hover(|style| style.bg(theme::hover()))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.show_object_actions(cx);
                    }))
                    .child(Icon::new(IconName::Ellipsis).size(px(14.)))
                    .child("⌘K"),
            );
        }
        let surface = div()
            .h_full()
            .px(px(10.))
            .flex()
            .items_center()
            .result_surface(selected)
            .child(
                div()
                    .id(SharedString::from(format!("candidate-{index}-object")))
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .items_center()
                    .on_double_click(cx.listener(move |this, _, _, cx| {
                        if enabled {
                            this.model.select_row(index);
                            this.activate_selection(cx);
                        }
                    }))
                    .child(div().flex_none().size(px(32.)).child(icon))
                    .child(div().w(px(12.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .justify_center()
                            .gap(px(2.))
                            .child(
                                div()
                                    .id(SharedString::from(format!("candidate-{index}-title")))
                                    .test_support()
                                    .truncate()
                                    .text_size(px(metrics::HEADING_SIZE))
                                    .line_height(px(
                                        metrics::HEADING_SIZE * metrics::BODY_LINE_HEIGHT
                                    ))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if enabled {
                                        theme::primary()
                                    } else {
                                        theme::muted()
                                    })
                                    .tooltip(components::text_tooltip(title.clone()))
                                    .child(title),
                            )
                            .when_some(subtitle, |element, subtitle| {
                                let tooltip = subtitle.clone();
                                element.child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "candidate-{index}-subtitle"
                                        )))
                                        .test_support()
                                        .truncate()
                                        .text_size(px(11.5))
                                        .line_height(px(11.5 * metrics::CAPTION_LINE_HEIGHT))
                                        .text_color(if enabled {
                                            theme::secondary()
                                        } else {
                                            theme::warning()
                                        })
                                        .tooltip(components::text_tooltip(tooltip))
                                        .child(subtitle),
                                )
                            }),
                    ),
            )
            .when_some(shortcut, |element, shortcut| {
                element.child(div().w(px(8.))).child(
                    div()
                        .id(SharedString::from(format!("candidate-{index}-shortcut")))
                        .flex_none()
                        .tooltip(components::text_tooltip(shortcut_tooltip))
                        .child(self.keycap(shortcut)),
                )
            })
            .when(has_row_actions, |element| {
                element.child(div().w(px(6.))).child(row_actions)
            })
            .when(selected && enabled, |element| {
                let primary_tooltip = l10n::shortcut_hint(self.locale, primary_label.as_str(), "↵");
                element.child(
                    div()
                        .id(SharedString::from(format!("candidate-{index}-primary")))
                        .flex_none()
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label(primary_label.clone())
                        .tooltip(components::text_tooltip(primary_tooltip))
                        .h(px(metrics::CONTROL_HEIGHT))
                        .px(px(metrics::CONTROL_PADDING_X))
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .rounded(px(6.))
                        .text_size(px(metrics::CONTROL_FONT_SIZE))
                        .line_height(px(metrics::CONTROL_FONT_SIZE * metrics::BODY_LINE_HEIGHT))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::primary())
                        .hover(|style| style.bg(theme::hover()))
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.activate_selection(cx);
                        }))
                        .child(primary_label)
                        .when(!show_actions, |element| {
                            element.child(
                                Icon::new(IconName::CornerDownLeft)
                                    .size(px(12.))
                                    .text_color(theme::secondary()),
                            )
                        }),
                )
            });
        div()
            .id(SharedString::from(format!("candidate-{index}")))
            .role(gpui_kit::Role::ListBoxOption)
            .aria_label(accessibility)
            .aria_selected(selected)
            .test_support()
            .h(px(LAUNCHER_ROW_HEIGHT))
            .flex_shrink_0()
            .py(px(2.))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if !enabled {
                        return;
                    }
                    this.mark_scope_user_controlled();
                    if this.screen == Screen::FileSearch {
                        this.file_search.browsing = true;
                        this.file_search.advance_from = None;
                    }
                    this.model.select_row(index);
                    this.notice = None;
                    cx.notify();
                }),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if is_menu {
                    this.model.open_selected_menu();
                    this.resize_current_window(cx);
                }
                this.notice = None;
                cx.notify();
            }))
            .child(surface)
    }

    fn row_icon(&mut self, row: &LauncherRow) -> AnyElement {
        if row.kind == CandidateKind::Application {
            let cache_key = row.icon_identity.as_deref().unwrap_or(&row.id);
            if let Some(image) = self.application_icons.get(cache_key) {
                return img(image.clone())
                    .size(px(32.))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element();
            }
        }
        let appearance = if row.kind == CandidateKind::File {
            file_icon_appearance(&row.title, row.file_kind)
        } else {
            action_icon_appearance(&row.icon, row.kind)
        };
        let tint = if row.enabled {
            (appearance.tint)()
        } else {
            theme::muted()
        };
        let marked = appearance.mark.is_some() || appearance.stamp.is_some();
        div()
            .size(px(32.))
            .relative()
            .flex()
            .child(
                div()
                    .absolute()
                    .left(px(if marked { 0. } else { 4. }))
                    .top(px(if marked { 1. } else { 4. }))
                    .child(
                        Icon::new(appearance.glyph)
                            .size(px(if marked { 27. } else { 24. }))
                            .text_color(tint),
                    ),
            )
            .when_some(appearance.mark, |element, mark| {
                element.child(
                    div()
                        .absolute()
                        .right(px(0.))
                        .bottom(px(1.))
                        .size(px(13.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.))
                        .bg(tint)
                        .child(
                            Icon::new(mark)
                                .size(px(10.))
                                .text_color(theme::icon_badge_ink()),
                        ),
                )
            })
            .when_some(appearance.stamp, |element, stamp| {
                element.child(
                    div()
                        .absolute()
                        .right(px(0.))
                        .bottom(px(1.))
                        .min_w(px(13.))
                        .h(px(10.))
                        .px(px(2.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.))
                        .bg(tint)
                        .text_size(px(7.5))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::icon_badge_ink())
                        .child(stamp),
                )
            })
            .into_any_element()
    }
}
