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
            view.presented = true;
            view.screen = Screen::Settings;
            view.notice = None;
            // A successful settings refresh clears transient operation errors.
            // The startup registration diagnostic must survive that refresh.
            view.settings
                .apply_snapshot(live.settings_snapshot().unwrap());
            assert!(view.settings.error.is_none());
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
            view.settings_close_pending = true;
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
