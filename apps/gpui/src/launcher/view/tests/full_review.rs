use super::*;

#[test]
fn web_acknowledgements_require_the_current_request_and_success_shape() {
    let request = serde_json::json!({"method":"lifecycle.ready","requestId":"current"});
    let accepted = serde_json::json!({"requestId":"current","result":{"value":null}});
    assert_eq!(
        accepted_web_acknowledgement_values(&request, &accepted).as_deref(),
        Some("lifecycle.ready")
    );
    for reply in [
        serde_json::json!({"requestId":"old","result":{"value":null}}),
        serde_json::json!({"requestId":"current","result":{"value":true}}),
        serde_json::json!({"requestId":"current","result":{"value":null,"extra":true}}),
        serde_json::json!({"requestId":"current","result":{"value":null},"error":{}}),
    ] {
        assert!(accepted_web_acknowledgement_values(&request, &reply).is_none());
    }
    assert!(accepted_web_acknowledgement_values(
        &serde_json::json!({"method":"storage.get","requestId":"current"}),
        &accepted,
    )
    .is_none());
}

#[gpui_kit::test]
fn action_picker_contains_tab_focus_and_matches_localized_titles(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.open_settings(window, cx);
            view.settings.selected = SettingsCategory::Shortcuts;
            view.locale = Locale::Zh;
            view.shortcut_catalog = vec![ShortcutCatalogAction {
                action_id: "developer.terminal.open".into(),
                title: "Open terminal".into(),
                localized_titles: [("zh".into(), "打开终端".into())].into(),
                context_types: vec!["core.file".into()],
            }];
            view.open_action_shortcut_picker(None, None, false, window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            set_input_value(&view.action_shortcut_input, "终端", window, cx);
        });
        window.render_frame(cx);
        assert!(window
            .try_find("settings-action-choice-developer.terminal.open")
            .is_some());
    })
    .unwrap();
    for key in ["tab", "shift-tab"].into_iter().cycle().take(12) {
        cx.update_window(window, |_, window, cx| {
            window.press(key, cx);
            window.render_frame(cx);
            view.update(cx, |view, cx| {
                assert!(view
                    .settings_action_picker_focus
                    .contains_focused(window, cx));
                assert!(view.settings_modal_active());
            });
        })
        .unwrap();
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn clipboard_virtualization_reaches_the_last_of_a_thousand_rows(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Clipboard;
            view.clipboard.status = Some(crate::host::ClipboardSnapshot {
                recording: true,
                recording_enabled: true,
                available: true,
                paused_until_ms: None,
                retention_days: 7,
                storage_usage: Some((1000, 1000)),
                notice: None,
                items: (0..1000)
                    .map(|id| crate::host::ClipboardItem {
                        id,
                        summary: format!("Synthetic row {id}"),
                        kind: "text".into(),
                        copied_at_ms: 0,
                        byte_count: 1,
                    })
                    .collect(),
            });
            cx.notify();
        })
    });
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, cx| {
        assert!(window.try_find("clipboard-item-0").is_some());
        assert!(window.try_find("clipboard-item-999").is_none());
        view.update(cx, |view, cx| {
            view.clipboard.selected = 999;
            view.clipboard.browsing = true;
            view.tool_list_scrolls
                .clipboard
                .scroll_to_item_strict(999, gpui_kit::ScrollStrategy::Bottom);
            cx.notify();
        });
    })
    .unwrap();
    sync_test_viewport_to_desired(cx, window, &view);
    cx.update_window(window, |_, window, _| {
        assert!(window.try_find("clipboard-item-999").is_some());
        assert!(window.try_find("clipboard-item-0").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn hidden_workspace_expires_without_another_activation(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let mut task_output = std::sync::Weak::new();
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.model.set_query("retained".into());
            set_input_value(&view.input, "retained", window, cx);
            set_textarea_value(&view.content_input, "captured text", window, cx);
            set_input_value(&view.clipboard_input, "clipboard query", window, cx);
            set_input_value(&view.history_input, "history query", window, cx);
            set_input_value(&view.tasks_filter_input, "task query", window, cx);
            view.prepare_hide(cx);
            assert!(view.retained_workspace);
            // Model an output cache built while a hidden view was still retained.
            task_output = Arc::downgrade(&view.tasks.output_lines("task\noutput".into()));
        });
    })
    .unwrap();
    assert!(task_output.upgrade().is_some());
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(Duration::from_secs(15));
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(!view.retained_workspace && view.hidden_at.is_none());
        assert!(view.model.query.is_empty());
        assert!(view.input.read(cx).value().is_empty());
        assert!(view.content_input.read(cx).value().is_empty());
        assert!(view.clipboard_input.read(cx).value().is_empty());
        assert!(view.history_input.read(cx).value().is_empty());
        assert!(view.tasks_filter_input.read(cx).value().is_empty());
        assert!(task_output.upgrade().is_none());
    });
}

#[gpui_kit::test]
fn hiding_an_empty_home_releases_its_snapshot_without_waiting(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            let state = Arc::new(anywhere_protocol::LauncherState {
                session_id: 7,
                ..Default::default()
            });
            let snapshot = Arc::downgrade(&state);
            view.apply_shared_state(state, cx);
            view.live_state.session_id = Some(7);
            view.live_state.snapshot_gate = Some(SnapshotGate::new(7));
            view.presented = true;
            assert!(snapshot.upgrade().is_some());

            view.prepare_hide(cx);

            assert!(snapshot.upgrade().is_none());
            assert!(view.live_state.session_id.is_none());
            assert!(view.live_state.snapshot_gate.is_none());
            assert!(view.retention_task.is_none());
            assert!(view.model.rows.is_empty());
            assert!(!view.model.loading);
        });
    });
}

#[gpui_kit::test]
fn old_hidden_workspace_timer_cannot_clear_a_resumed_view(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.model.query = "retained".into();
            view.prepare_hide(cx);
        })
    });
    cx.run_until_parked();
    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.hidden_at = None;
            view.model.query = "resumed".into();
        })
    });
    cx.background_executor
        .advance_clock(Duration::from_secs(15));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).model.query, "resumed"));
}
