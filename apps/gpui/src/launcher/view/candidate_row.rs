use super::*;

impl LauncherView {
    pub(super) fn render_row(
        &mut self,
        index: usize,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let row = self.model.rows.get(index).expect("rendered row index");
        let icon = row_icon(&mut self.icons.cache, &row);
        let enabled = row.enabled;
        let is_menu = row.is_menu;
        let is_file = row.kind == CandidateKind::File;
        let source_label = row.source_label.clone().filter(|label| {
            !label.trim().is_empty()
                && !(is_file
                    && (self.screen == Screen::FileSearch || self.model.object_actions_active))
        });
        let row_subtitle = row.subtitle.clone().filter(|subtitle| {
            !subtitle.trim().is_empty() && subtitle.as_ref() != row.title.as_ref()
        });
        let readable_extended_subtitle = row_subtitle
            .as_deref()
            .and_then(crate::launcher::presentation::readable_extended_path);
        let raw_subtitle_tooltip = if is_file {
            row_subtitle.as_deref().map(|path| {
                SharedString::from(crate::launcher::presentation::readable_file_path(path))
            })
        } else if let Some(path) = readable_extended_subtitle.as_deref() {
            Some(SharedString::from(path))
        } else {
            row_subtitle.clone()
        };
        let display_subtitle = if is_file {
            row_subtitle
                .as_deref()
                .map(|path| SharedString::from(format_path_middle_truncated(path)))
        } else if let Some(path) = readable_extended_subtitle.as_deref() {
            Some(SharedString::from(format_path_middle_truncated(path)))
        } else {
            row_subtitle
        };
        let source_label = source_label.filter(|source| Some(source) != display_subtitle.as_ref());
        let shortcut = row.shortcut.clone();
        let index_shortcut = (!selected && enabled && self.screen == Screen::Launcher && index < 9)
            .then(|| index + 1)
            .filter(|digit| {
                let modifier = if cfg!(target_os = "macos") {
                    "meta"
                } else {
                    "control"
                };
                let chord = format!("{modifier}+Digit{digit}");
                !self
                    .preferences
                    .action_shortcuts
                    .iter()
                    .any(|binding| binding.shortcut == chord)
            })
            .map(|digit| crate::platform::primary_shortcut_label(&digit.to_string()));
        let title = row.title.clone();
        let accessibility = row.title.clone();
        let show_preview = selected && enabled && is_file && self.can_preview_selected_file();
        let can_show_actions = enabled
            && !is_menu
            && !self.model.object_actions_active
            && self.live.is_some()
            && self
                .model
                .can_show_object_actions(&row, self.screen == Screen::FileSearch);
        let show_actions = selected && can_show_actions;
        let primary_label = || primary_row_label(&row, self.locale);
        let mut row = components::ListRow::new(format!("candidate-{index}"))
            .density(components::RowDensity::Regular)
            .selected(selected)
            .enabled(enabled)
            .aria_label(accessibility)
            .leading(icon)
            .title(title.clone())
            .title_highlight_query(self.model.query.clone())
            .title_id(format!("candidate-{index}-title"))
            .title_tooltip(title);
        if let Some(source) = source_label {
            row = row.source_label(source);
        }
        if let Some(subtitle) = display_subtitle {
            let tooltip = raw_subtitle_tooltip.unwrap_or_else(|| subtitle.clone());
            row = row
                .subtitle(subtitle)
                .subtitle_id(format!("candidate-{index}-subtitle"))
                .subtitle_tooltip(tooltip);
        }
        if let Some(shortcut) = shortcut {
            row = row.trailing(
                div()
                    .id(SharedString::from(format!("candidate-{index}-shortcut")))
                    .pl(px(metrics::SPACE_SM))
                    .flex_none()
                    .tooltip(components::text_tooltip(l10n::anywhere_action_shortcut(
                        self.locale,
                    )))
                    .child(self.keycap(shortcut)),
            );
        } else if let Some(shortcut) = index_shortcut {
            row = row.trailing(
                div()
                    .opacity(theme::muted().a)
                    .child(components::keycap(shortcut)),
            );
        }
        if show_preview || show_actions {
            row = row.trailing(self.candidate_row_actions(index, show_preview, show_actions, cx));
        }
        if selected && enabled {
            let primary_label = primary_label();
            let primary_tooltip = l10n::shortcut_hint(self.locale, primary_label.as_str(), "↵");
            row = row.trailing(
                components::primary_row_action(
                    format!("candidate-{index}-primary"),
                    primary_label.clone(),
                )
                .test_support()
                .tooltip(components::text_tooltip(primary_tooltip))
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
            );
        }
        let surface = row.build();
        if !enabled {
            return surface.test_support();
        }
        surface
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
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
            .when(is_menu || can_show_actions, |surface| {
                surface.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, _, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.mark_scope_user_controlled();
                        this.model.select_row(index);
                        this.reveal_result_row(index);
                        this.notice = None;
                        if is_menu {
                            this.browse_selected_menu(cx);
                        } else {
                            this.show_object_actions(cx);
                        }
                        cx.notify();
                    }),
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                if is_menu {
                    this.model.open_selected_menu();
                    this.resize_current_window(cx);
                }
                this.notice = None;
                cx.notify();
            }))
            .on_double_click(cx.listener(move |this, _, _, cx| {
                this.model.select_row(index);
                this.activate_selection(cx);
            }))
            .test_support()
    }

    fn candidate_row_actions(
        &self,
        index: usize,
        show_preview: bool,
        show_actions: bool,
        cx: &Context<Self>,
    ) -> Div {
        let preview_space = self.file_search.browsing || self.model.query.trim().is_empty();
        let mut row_actions = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(metrics::SPACE_XS));
        if show_preview {
            row_actions = row_actions.child(
                components::row_action_button(
                    format!("candidate-{index}-preview"),
                    l10n::preview(self.locale),
                )
                .test_support()
                .tooltip(components::text_tooltip(if preview_space {
                    l10n::shortcut_hint(self.locale, &l10n::preview(self.locale), "␣")
                } else {
                    l10n::preview(self.locale)
                }))
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.request_preview(cx);
                }))
                .child(Icon::new(IconName::Eye).size(px(metrics::ICON_SIZE)))
                .when(preview_space, |element| {
                    element.child(components::keycap("␣"))
                }),
            );
        }
        if show_actions {
            let shortcut = crate::platform::primary_shortcut_label("K");
            row_actions = row_actions.child(
                components::row_action_button(
                    format!("candidate-{index}-actions"),
                    l10n::actions(self.locale),
                )
                .test_support()
                .tooltip(components::text_tooltip(l10n::shortcut_hint(
                    self.locale,
                    &l10n::actions(self.locale),
                    &shortcut,
                )))
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.show_object_actions(cx);
                }))
                .child(Icon::new(IconName::Ellipsis).size(px(metrics::ICON_SIZE)))
                .child(components::keycap(shortcut)),
            );
        }
        row_actions.pl(px(metrics::SPACE_XS))
    }
}

fn primary_row_label(row: &LauncherRow, locale: Locale) -> String {
    if row.is_menu {
        l10n::open(locale)
    } else if row.command_id.as_deref() == Some("core.files.search") {
        l10n::search(locale)
    } else if matches!(row.kind, CandidateKind::File | CandidateKind::Application) {
        l10n::open(locale)
    } else {
        l10n::run(locale)
    }
}

fn row_icon(cache: &mut ApplicationIconCache, row: &LauncherRow) -> AnyElement {
    if row.kind == CandidateKind::Application {
        let cache_key = row.icon_identity.as_deref().unwrap_or(&row.id);
        if let Some(image) = cache.get(cache_key) {
            return img(image)
                .size(px(metrics::ROW_ICON_SLOT))
                .rounded(px(metrics::TOOLTIP_RADIUS))
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
    let plate = components::squircle_icon_plate(
        appearance.glyph,
        tint,
        if marked {
            metrics::ICON_SIZE
        } else {
            metrics::SPACE_LG
        },
    );
    div()
        .size(px(metrics::ROW_ICON_SLOT))
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .child(plate)
        .when_some(appearance.mark, |element, mark| {
            element.child(
                div()
                    .absolute()
                    .right_0()
                    .bottom_0()
                    .size(px(metrics::SPACE_MD))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(metrics::PAGE_TITLE_GAP))
                    .bg(tint)
                    .child(
                        Icon::new(mark)
                            .size(px(metrics::PAGE_MODE_SIZE))
                            .text_color(theme::icon_badge_ink()),
                    ),
            )
        })
        .when_some(appearance.stamp, |element, stamp| {
            element.child(
                div()
                    .absolute()
                    .right_0()
                    .bottom_0()
                    .min_w(px(metrics::SPACE_MD))
                    .px(px(metrics::ROW_INSET_Y_REGULAR))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(metrics::PAGE_TITLE_GAP))
                    .bg(tint)
                    .text_size(px(metrics::PAGE_MODE_SIZE))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::icon_badge_ink())
                    .child(stamp),
            )
        })
        .into_any_element()
}
