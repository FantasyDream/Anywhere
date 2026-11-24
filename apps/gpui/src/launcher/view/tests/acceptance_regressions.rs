use super::*;
use std::cell::RefCell;

#[gpui_kit::test]
fn startup_hotkey_error_survives_settings_refresh_and_offers_retry(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let plugins = directory.path().join("plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    let live = Arc::new(
        LiveLauncher::open_at(
            anywhere_core::application::frontend_runtime::RuntimePaths::new(
                directory.path().join("data"),
                plugins,
            ),
        )
        .unwrap(),
    );
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.attach_runtime(
                live.clone(),
                None,
                Some("shortcut is occupied".into()),
                window,
                cx,
            );
            assert_eq!(view.runtime_state, RuntimeState::Ready);
            assert!(view
                .notice
                .as_ref()
                .unwrap()
                .contains("shortcut is occupied"));
            assert!(view
                .registered_preferences()
                .current_app_shortcut
                .is_empty());
            view.presence.presented = true;
            view.screen = Screen::Settings;
            view.notice = None;
            // A successful settings refresh clears transient operation errors.
            // The startup registration diagnostic must survive that refresh.
            view.settings_editor
                .model
                .apply_snapshot(live.settings_snapshot().unwrap());
            assert!(view.settings_editor.model.error.is_none());
            assert!(!view.settings_dirty(cx));
        });
    })
    .unwrap();
    sync_test_viewport_to_desired(cx, window, &view);
    for locale in [Locale::En, Locale::Zh] {
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, _| view.locale = locale);
            window.render_frame(cx);
            assert!(window.try_find("settings-hotkey-error").is_some());
            assert!(window.try_find("save-settings").is_some());
            assert_eq!(
                window.find("save-settings").label(),
                Some(l10n::retry_shortcuts(locale).as_str())
            );
            assert!(view
                .read(cx)
                .hotkey_error_message()
                .unwrap()
                .contains("shortcut is occupied"));
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn saving_preferences_cannot_publish_unregistered_shortcut_hints(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    let published = Rc::new(RefCell::new(Vec::new()));
    let observed = published.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&view, move |_, event, _| {
            if let LauncherEvent::PreferencesApplied { preferences, .. } = event {
                observed.borrow_mut().push(preferences.clone());
            }
        })
    });
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.hotkey_error = Some("manager is unavailable".into());
            view.settings_editor.close_pending = true;
            let desired = view.preferences_from_editors(cx);
            view.apply_preferences_result(desired.clone(), None, cx);
            assert_eq!(view.preferences, desired);
            assert!(view.hotkey_error.is_some());
            assert!(view
                .notice
                .as_ref()
                .unwrap()
                .contains("manager is unavailable"));
        })
    });
    cx.run_until_parked();
    let published = published.borrow();
    assert_eq!(published.len(), 1);
    assert!(published[0].current_app_shortcut.is_empty());
    assert!(published[0].universal_shortcut.is_empty());
    cx.update(|cx| assert_eq!(view.read(cx).screen, Screen::Settings));
}

#[gpui_kit::test]
fn settings_reactivation_keeps_confirmation_focus(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            assert!(view.open_settings(window, cx));
            view.settings_editor.draft.launch_at_login = true;
            assert!(!view.request_settings_exit(SettingsExitIntent::Close, window, cx));
        });
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| {
            assert!(view.settings_editor.keep_editing_focus.is_focused(window));
            view.settings_editor.discard_focus.focus(window, cx);
            assert!(view.open_settings(window, cx));
        });
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.focus_settings_navigation(window, cx);
            assert!(view.settings_editor.discard_focus.is_focused(window));

            view.settings_editor.focus_handle.focus(window, cx);
            view.focus_settings_navigation(window, cx);
            assert!(view.settings_editor.keep_editing_focus.is_focused(window));
        });
        window.press("enter", cx);
    })
    .unwrap();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(!view.settings_editor.close_pending);
        assert!(view.settings_editor.draft.launch_at_login);
        assert_eq!(view.screen, Screen::Settings);
    });
}

#[gpui_kit::test]
fn settings_save_takes_precedence_over_a_local_action_binding(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let (key, shortcut) = if cfg!(target_os = "macos") {
        ("cmd-s", "meta+KeyS")
    } else {
        ("ctrl-s", "control+KeyS")
    };
    let binding = ActionShortcutBinding {
        action_id: "core.files.search".into(),
        title: "File Search".into(),
        shortcut: shortcut.into(),
        global: false,
        routes: vec![],
    };
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.preferences.action_shortcuts.push(binding.clone());
            view.settings_editor.draft = view.preferences.clone();
            assert!(view.open_settings(window, cx));
            view.settings_editor.draft.launch_at_login = true;
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, key);
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(view.preferences.launch_at_login);
        assert_eq!(view.preferences.action_shortcuts, [binding]);
    });
}

#[gpui_kit::test]
fn web_package_review_isolates_the_package_list_and_cancels_first(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.open_screen(Screen::WebPackages, window, cx);
            view.web.apply(
                ["first", "second"]
                    .map(|id| crate::host::WebPackage {
                        id: id.into(),
                        name: id.into(),
                        version: "1".into(),
                        digest: "digest".into(),
                        entry: "index.html".into(),
                        enabled: true,
                        available: true,
                        rollback: false,
                        builtin: false,
                        permission_count: 0,
                    })
                    .to_vec(),
            );
            view.apply_auxiliary_event(
                LiveEvent::WebReview(crate::host::WebPackageReview {
                    token: "test-token".into(),
                    id: "imported".into(),
                    name: "Imported".into(),
                    version: "1".into(),
                    digest: "digest".into(),
                    permissions: vec![],
                    updating: false,
                }),
                cx,
            );
            cx.notify();
        });
    })
    .unwrap();
    dispatch_shortcut(cx, window, "down");
    cx.update_window(window, |_, window, cx| window.press("enter", cx))
        .unwrap();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.web.selected, 0);
        assert!(view.web_surface.review.is_some());
    });
    dispatch_shortcut(cx, window, "escape");
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.screen, Screen::WebPackages);
        assert!(view.web_surface.review.is_none());
    });
    dispatch_shortcut(cx, window, "down");
    cx.update(|cx| assert_eq!(view.read(cx).web.selected, 1));
    dispatch_shortcut(cx, window, "escape");
    cx.update(|cx| assert_eq!(view.read(cx).screen, Screen::Settings));
}

#[gpui_kit::test]
fn launcher_reveals_a_selected_row_moved_by_a_snapshot(cx: &mut TestAppContext) {
    use anywhere_protocol::{Availability, CandidatePresentation, IconDescriptor, LauncherState};

    let snapshot = |indices: Vec<usize>, snapshot_seq| LauncherState {
        input: "captured".into(),
        revision: 1,
        snapshot_seq,
        candidates: Arc::new(
            indices
                .into_iter()
                .map(|index| CandidatePresentation {
                    id: format!("row-{index}"),
                    kind: CandidateKind::Action,
                    is_menu: false,
                    file_kind: None,
                    title: format!("Row {index}"),
                    localized_titles: Default::default(),
                    subtitle: None,
                    shortcut: None,
                    icon: IconDescriptor {
                        key: "action".into(),
                    },
                    score: 1.,
                    group_id: "test".into(),
                    availability: Availability::enabled(),
                    command_id: None,
                    bundle_id: None,
                    owning_app: None,
                    menu_path: vec![],
                    context_keys: vec![],
                    execution_context_id: None,
                    execution_target_label: None,
                    primary_action: false,
                    section_label: None,
                })
                .collect(),
        ),
        ..Default::default()
    };
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.input_mode = InputMode::Content;
            view.apply_state(snapshot((0..7).collect(), 1), cx);
            view.focus_launcher_input(window, cx);
            cx.notify();
        });
    })
    .unwrap();
    sync_test_viewport(cx, window, gpui_kit::size(px(648.), px(260.)));
    dispatch_shortcut(cx, window, "up");

    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        view.update(cx, |view, cx| {
            assert_eq!(view.model.selected, 6);
            assert!(view.scrolls.launcher_base().offset().y < px(-60.));
            view.apply_state(snapshot((0..7).rev().collect(), 2), cx);
            assert_eq!(view.model.selection_id(), Some("row-6"));
            assert_eq!(view.model.selected, 0);
            cx.notify();
        });
        window.render_frame(cx);
        assert!(view.read(cx).scrolls.launcher_base().offset().y >= px(-4.));
    })
    .unwrap();
}
