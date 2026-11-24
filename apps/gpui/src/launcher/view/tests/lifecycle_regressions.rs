use super::*;

fn open_presented_view(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<LauncherView>) {
    let (window, view) = open_view(cx);
    cx.update(|cx| view.update(cx, |view, _| view.presence.presented = true));
    (window, view)
}

#[gpui_kit::test]
fn editing_keeps_window_height_until_the_newest_query_returns(cx: &mut TestAppContext) {
    let (window, view) = open_presented_view(cx);
    let settled = |revision, input: &str| anywhere_protocol::LauncherState {
        session_id: 7,
        revision,
        input: input.into(),
        ..Default::default()
    };
    cx.update(|cx| view.update(cx, |view, cx| view.apply_state(settled(1, ""), cx)));
    sync_test_viewport_to_desired(cx, window, &view);
    let (old_sender, old_receiver) = tokio::sync::mpsc::channel(1);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update_window(window, |_, window, cx| {
        let original_size = window.bounds().size;
        view.update(cx, |view, cx| {
            view.live_state.session_id = Some(7);
            let mut gate = SnapshotGate::new(7);
            gate.begin_request(3);
            view.live_state.snapshot_gate = Some(gate);
            view.model.set_query("latest".into());
            view.poll(old_receiver, view.live_state.generation, cx);
            view.begin_stream(receiver, cx);
            view.live_state.awaiting_query_layout = view.model.query_differs_from_snapshot();
            assert_ne!(view.desired_window_size(), original_size);
            view.resize_current_window(cx);
            assert!(!view.presence.resize_pending);
        });
    })
    .unwrap();
    old_sender
        .try_send(LiveEvent::SharedState(Arc::new(settled(2, "old"))))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(view.read(cx).live_state.awaiting_query_layout));

    sender
        .try_send(LiveEvent::SharedState(Arc::new(settled(3, "latest"))))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        let view = view.read(cx);
        assert!(!view.live_state.awaiting_query_layout);
        assert_eq!(view.model.revision, 3);
        assert_eq!(window.bounds().size, view.desired_window_size());
    })
    .unwrap();
}

#[gpui_kit::test]
fn object_action_reply_cannot_roll_back_a_newer_snapshot(cx: &mut TestAppContext) {
    let (_window, view) = open_presented_view(cx);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.model.rows = vec![file_row("file", "test.txt")].into();
            view.model.revision = 4;
            view.model.selected = 0;
            view.model.show_object_actions();
            view.poll_auxiliary_state(receiver, view.live_state.generation, cx);
            let current = anywhere_protocol::LauncherState {
                session_id: 7,
                revision: 5,
                input: "current".into(),
                ..Default::default()
            };
            let mut gate = SnapshotGate::new(7);
            assert!(gate.accept_update(&current));
            view.live_state.snapshot_gate = Some(gate);
            view.apply_state(current, cx);
        });
    });
    sender
        .try_send(LiveEvent::SharedState(Arc::new(
            anywhere_protocol::LauncherState {
                session_id: 7,
                revision: 4,
                snapshot_seq: 9,
                input: "old".into(),
                ..Default::default()
            },
        )))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(view.read(cx).model.revision, 5);
        assert_eq!(view.read(cx).model.query, "current");
    });
}

#[gpui_kit::test]
fn launcher_errors_cannot_finish_an_inflight_settings_save(cx: &mut TestAppContext) {
    let (_window, view) = open_presented_view(cx);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.model.saving = true;
            view.poll(receiver, view.live_state.generation, cx);
        });
    });
    sender
        .try_send(LiveEvent::Error("old launcher query failed".into()))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(view.read(cx).settings_editor.model.saving);
        assert!(view.read(cx).settings_editor.model.error.is_none());
    });
}

#[gpui_kit::test]
fn a_snapshot_follow_error_cannot_finish_an_inflight_settings_save(cx: &mut TestAppContext) {
    let (_window, view) = open_presented_view(cx);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.model.saving = true;
            view.poll_follow(receiver, view.live_state.generation, None, cx);
        });
    });
    sender
        .try_send(LiveEvent::Error("old follow failed".into()))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(view.read(cx).settings_editor.model.saving);
        assert!(view.read(cx).settings_editor.model.error.is_none());
    });
}

#[gpui_kit::test]
fn a_closed_settings_channel_keeps_the_close_prompt_open_for_retry(cx: &mut TestAppContext) {
    let (_window, view) = open_presented_view(cx);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.model.saving = true;
            view.settings_editor.close_pending = true;
            view.settings_editor.exit_intent = Some(SettingsExitIntent::Close);
            view.poll_settings_save(receiver, view.settings_editor.save_generation, cx);
        });
    });
    drop(sender);
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!view.read(cx).settings_editor.model.saving);
        assert!(view.read(cx).settings_editor.model.error.is_some());
        assert!(view.read(cx).settings_editor.close_pending);
        assert_eq!(view.read(cx).screen, Screen::Settings);
    });
}

#[gpui_kit::test]
fn cancelling_a_queued_settings_exit_keeps_settings_open(cx: &mut TestAppContext) {
    let (window, view) = open_presented_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.close_pending = true;
            view.settings_editor.exit_intent = Some(SettingsExitIntent::Close);
            view.save_settings_and_close(cx);
            view.cancel_settings_close_prompt(window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).screen, Screen::Settings));
}

#[gpui_kit::test]
fn edits_after_a_successful_save_prevent_a_queued_settings_exit(cx: &mut TestAppContext) {
    let (_window, view) = open_presented_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.close_pending = true;
            view.settings_editor.exit_intent = Some(SettingsExitIntent::Close);
            view.save_settings_and_close(cx);
            view.settings_editor.draft.launch_at_login = !view.preferences.launch_at_login;
        });
    });
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).screen, Screen::Settings));
}

fn web_document(handle: &str) -> WebDocument {
    WebDocument {
        handle: handle.into(),
        origin: "anywhere-plugin://example".into(),
        entry: "index.html".into(),
        title: "Example".into(),
        builtin: false,
    }
}

#[gpui_kit::test]
fn reload_keeps_the_current_web_document_until_a_replacement_arrives(cx: &mut TestAppContext) {
    let (_window, view) = open_presented_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::WebTool;
            view.web_surface.invocation_id = Some("invocation".into());
            view.web_surface.document = Some(web_document("old"));
            view.reload_web_tool(cx);
            assert_eq!(view.web_surface.document.as_ref().unwrap().handle, "old");
        });
    });
}

#[gpui_kit::test]
fn opening_settings_closes_preview_and_web_surfaces(cx: &mut TestAppContext) {
    let (window, view) = open_presented_view(cx);
    for screen in [Screen::Launcher, Screen::WebTool] {
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = screen;
                view.preview.open = screen == Screen::Launcher;
                view.web_surface.invocation_id = Some("invocation".into());
                view.web_surface.document = Some(web_document("document"));
                view.open_settings(window, cx);
                assert_eq!(view.screen, Screen::Settings);
                assert!(!view.preview.open);
                assert!(view.web_surface.document.is_none());
                assert!(view.web_surface.invocation_id.is_none());
            });
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn object_action_replies_obey_the_cursor_at_the_same_revision(cx: &mut TestAppContext) {
    let (_window, view) = open_presented_view(cx);
    for (sequence, expected) in [(1, "current"), (3, "newest")] {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                let current = anywhere_protocol::LauncherState {
                    session_id: 7,
                    revision: 4,
                    snapshot_seq: 2,
                    input: "current".into(),
                    ..Default::default()
                };
                let mut gate = SnapshotGate::new(7);
                assert!(gate.accept_update(&current));
                view.live_state.snapshot_gate = Some(gate);
                view.apply_state(current, cx);
                view.model.hide_object_actions();
                view.model.rows = vec![file_row("file", "test.txt")].into();
                view.model.selected = 0;
                view.model.show_object_actions();
                view.poll_auxiliary_state(receiver, view.live_state.generation, cx);
            });
        });
        sender
            .try_send(LiveEvent::SharedState(Arc::new(
                anywhere_protocol::LauncherState {
                    session_id: 7,
                    revision: 4,
                    snapshot_seq: sequence,
                    input: "newest".into(),
                    ..Default::default()
                },
            )))
            .unwrap();
        cx.run_until_parked();
        cx.update(|cx| assert_eq!(view.read(cx).model.query, expected));
    }
}

#[gpui_kit::test]
fn an_old_object_action_error_cannot_close_a_newer_panel(cx: &mut TestAppContext) {
    let (_window, view) = open_presented_view(cx);
    let (old_sender, old_receiver) = tokio::sync::mpsc::channel(1);
    let (new_sender, new_receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.model.rows = vec![file_row("file", "test.txt")].into();
            view.model.selected = 0;
            view.model.show_object_actions();
            view.poll_auxiliary_state(old_receiver, view.live_state.generation, cx);
            view.model.hide_object_actions();
            view.model.rows = vec![file_row("file", "test.txt")].into();
            view.model.selected = 0;
            view.model.show_object_actions();
            view.poll_auxiliary_state(new_receiver, view.live_state.generation, cx);
            view.notice = Some("new panel".into());
        });
    });
    old_sender
        .try_send(LiveEvent::Error("old error".into()))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(view.read(cx).model.object_actions_active);
        assert_eq!(view.read(cx).notice.as_deref(), Some("new panel"));
    });
    new_sender
        .try_send(LiveEvent::Error("current error".into()))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!view.read(cx).model.object_actions_active);
        assert_eq!(view.read(cx).notice.as_deref(), Some("current error"));
    });
}

#[gpui_kit::test]
fn settings_operations_finish_after_the_launcher_generation_changes(cx: &mut TestAppContext) {
    let (window, view) = open_presented_view(cx);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.model.saving = true;
            view.settings_editor.save_generation = 3;
            view.poll_settings_save(receiver, 3, cx);
            assert!(!view.request_settings_exit(SettingsExitIntent::Close, window, cx));
            view.live_state.generation += 1;
        });
    })
    .unwrap();
    sender
        .try_send(LiveEvent::Settings(crate::settings::SettingsSnapshot {
            clipboard_recording: true,
            features: vec![],
        }))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(view.read(cx).settings_editor.model.clipboard_recording);
        assert!(!view.read(cx).settings_editor.model.saving);
        assert_eq!(view.read(cx).screen, Screen::Launcher);
    });
}

#[gpui_kit::test]
fn settings_operation_failures_roll_back_the_switch_and_keep_the_draft(cx: &mut TestAppContext) {
    let (_window, view) = open_presented_view(cx);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            view.settings_editor.model.saving = true;
            view.settings_editor.model.clipboard_recording = true;
            view.settings_editor.draft.launch_at_login = !view.preferences.launch_at_login;
            view.settings_editor.close_pending = true;
            view.poll_settings_save(receiver, view.settings_editor.save_generation, cx);
        });
    });
    sender
        .try_send(LiveEvent::SettingsError {
            message: "denied".into(),
            snapshot: Some(crate::settings::SettingsSnapshot::default()),
        })
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(!view.settings_editor.model.clipboard_recording);
        assert!(!view.settings_editor.model.saving);
        assert_eq!(view.settings_editor.model.error.as_deref(), Some("denied"));
        assert_ne!(
            view.settings_editor.draft.launch_at_login,
            view.preferences.launch_at_login
        );
        assert!(view.settings_editor.close_pending);
        assert_eq!(view.screen, Screen::Settings);
    });
}

#[gpui_kit::test]
fn late_folder_authorization_does_not_overwrite_new_roots(cx: &mut TestAppContext) {
    let (window, view) = open_presented_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            set_input_value(
                &view.settings_editor.file_roots_input,
                "/newer-edit".to_owned(),
                window,
                cx,
            );
            view.finish_search_directory_selection(
                &["/original".to_owned()],
                Ok(Some(vec!["/original".to_owned(), "/picked".to_owned()])),
                window,
                cx,
            );
            assert_eq!(
                view.settings_editor
                    .file_roots_input
                    .read(cx)
                    .value()
                    .as_ref(),
                "/newer-edit"
            );
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn login_item_failure_stays_separate_from_subsequent_settings_updates(cx: &mut TestAppContext) {
    let (_window, view) = open_presented_view(cx);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Settings;
            let preferences = view.preferences.clone();
            view.apply_preferences_result(preferences.clone(), Some("needs approval".into()), cx);
            view.settings_editor
                .model
                .apply_snapshot(crate::settings::SettingsSnapshot::default());
            assert_eq!(
                view.settings_editor.model.login_item_error.as_deref(),
                Some("needs approval")
            );
            assert!(!view.settings_dirty(cx));
            view.apply_preferences_result(preferences, None, cx);
            assert_eq!(
                view.settings_editor.model.login_item_error.as_deref(),
                Some("needs approval")
            );
            view.poll_settings_save(receiver, view.settings_editor.save_generation, cx);
        });
    });
    sender
        .try_send(LiveEvent::LoginItemReconciled(None))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(view
            .read(cx)
            .settings_editor
            .model
            .login_item_error
            .is_none())
    });
}

#[gpui_kit::test]
fn feature_switches_cannot_start_a_second_settings_operation(cx: &mut TestAppContext) {
    let (_window, view) = open_presented_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.settings_editor.model.saving = true;
            view.settings_editor.model.clipboard_recording = false;
            view.settings_editor.model.features = vec![crate::settings::FeatureSetting {
                key: "feature".into(),
                name: "Feature".into(),
                enabled: false,
            }];
            view.set_clipboard_recording(true, cx);
            view.change_feature_setting("feature", true, cx);
            assert!(!view.settings_editor.model.clipboard_recording);
            assert!(!view.settings_editor.model.features[0].enabled);
            assert!(view.settings_editor.model.saving);
        });
    });
}

#[gpui_kit::test]
fn web_document_replies_cannot_replace_a_newer_request_or_a_closed_page(cx: &mut TestAppContext) {
    let (_window, view) = open_presented_view(cx);
    let (old_sender, old_receiver) = tokio::sync::mpsc::channel(1);
    let (new_sender, new_receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::WebTool;
            view.web_surface.invocation_id = Some("invocation".into());
            view.poll_web_document(old_receiver, cx);
            view.poll_web_document(new_receiver, cx);
        });
    });
    new_sender
        .try_send(LiveEvent::WebDocument(web_document("new")))
        .unwrap();
    cx.run_until_parked();
    old_sender
        .try_send(LiveEvent::WebDocument(web_document("old")))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).web_surface.document.as_ref().unwrap().handle,
            "new"
        )
    });
    for failure in [false, true] {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::WebTool;
                view.web_surface.invocation_id = Some("invocation".into());
                view.poll_web_document(receiver, cx);
                view.close_web_surface(cx);
                view.screen = Screen::Launcher;
                view.notice = Some("current page".into());
            });
        });
        sender
            .try_send(if failure {
                LiveEvent::Error("late error".into())
            } else {
                LiveEvent::WebDocument(web_document("late"))
            })
            .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(view.read(cx).web_surface.document.is_none());
            assert!(view.read(cx).web_surface.error.is_none());
            assert_eq!(view.read(cx).notice.as_deref(), Some("current page"));
        });
    }
}

#[gpui_kit::test]
fn web_operation_disconnects_and_failures_allow_retry(cx: &mut TestAppContext) {
    let (_window, view) = open_presented_view(cx);
    for resume in [false, true] {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::WebTool;
                view.web_surface.invocation_id = Some("invocation".into());
                view.web_surface.document = Some(web_document("old"));
                if resume {
                    view.poll_web_resume(receiver, "old".into(), cx);
                } else {
                    view.poll_web_document(receiver, cx);
                }
            });
        });
        drop(sender);
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(!view.web_surface.opening);
            assert!(!view.web_surface.resume_busy);
            assert!(view.web_surface.error.is_some());
            assert_eq!(view.web_surface.document.as_ref().unwrap().handle, "old");
        });
    }
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| view.update(cx, |view, cx| view.poll_web_document(receiver, cx)));
    sender
        .try_send(LiveEvent::WebDocument(web_document("retry")))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(view.read(cx).web_surface.error.is_none());
        assert_eq!(
            view.read(cx).web_surface.document.as_ref().unwrap().handle,
            "retry"
        );
    });
}

#[gpui_kit::test]
fn a_hidden_web_document_retains_metadata_and_requires_resume_before_mount(
    cx: &mut TestAppContext,
) {
    let (window, view) = open_presented_view(cx);
    for resume in [false, true] {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::WebTool;
                view.presence.presented = true;
                view.web_surface.invocation_id = Some("invocation".into());
                view.web_surface.document = Some(web_document("old"));
                if resume {
                    view.poll_web_resume(receiver, "old".into(), cx);
                } else {
                    view.poll_web_document(receiver, cx);
                }
                view.prepare_hide(cx);
                assert!(view.presence.retained_workspace);
                assert!(view.web_surface.needs_resume);
                view.ensure_web_view(window, cx);
                assert!(view.web_surface.view.is_none());
            });
        })
        .unwrap();
        sender
            .try_send(LiveEvent::WebDocument(web_document("retained")))
            .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(
                view.web_surface.document.as_ref().unwrap().handle,
                "retained"
            );
            assert!(view.web_surface.needs_resume);
            assert!(!view.web_surface.opening);
            assert!(!view.web_surface.resume_busy);
            assert!(view.web_surface.view.is_none());
        });
    }
}
