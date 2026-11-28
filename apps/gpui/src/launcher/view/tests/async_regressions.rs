use super::*;

fn clipboard_snapshot(ids: &[i64]) -> crate::host::ClipboardSnapshot {
    crate::host::ClipboardSnapshot {
        recording: false,
        recording_enabled: false,
        available: true,
        paused_until_ms: None,
        retention_days: 30,
        storage_usage: Some((ids.len() as u32, 0)),
        notice: None,
        items: ids
            .iter()
            .map(|id| crate::host::ClipboardItem {
                id: *id,
                summary: format!("entry {id}"),
                kind: "text".to_owned(),
                copied_at_ms: 0,
                byte_count: 1,
            })
            .collect(),
    }
}

#[gpui_kit::test]
fn initial_clipboard_reads_cannot_replace_a_newer_search(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    for screen in [Screen::Clipboard, Screen::ClipboardShelf] {
        let (initial_sender, initial_receiver) = tokio::sync::mpsc::channel(1);
        let (search_sender, search_receiver) = tokio::sync::mpsc::channel(1);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = screen;
                view.clipboard.query.clear();
                let before = view.clipboard.query_generation;
                view.begin_clipboard_read(initial_receiver, cx);
                assert_eq!(view.clipboard.query_generation, before + 1);
                assert!(view.clipboard.loading);

                view.clipboard.query = "needle".to_owned();
                view.begin_clipboard_read(search_receiver, cx);
                assert_eq!(view.clipboard.query_generation, before + 2);
            });
        });
        search_sender
            .try_send(LiveEvent::Clipboard(clipboard_snapshot(&[2])))
            .unwrap();
        cx.run_until_parked();
        initial_sender
            .try_send(LiveEvent::Clipboard(clipboard_snapshot(&[1])))
            .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.clipboard.query, "needle");
            assert_eq!(view.clipboard.items()[0].id, 2);
            assert!(!view.clipboard.loading);
        });
    }
}

#[gpui_kit::test]
fn clipboard_results_cannot_change_a_page_after_leaving_or_hiding(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    for hide in [false, true] {
        let (query_sender, query_receiver) = tokio::sync::mpsc::channel(1);
        let (operation_sender, operation_receiver) = tokio::sync::mpsc::channel(1);
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Clipboard;
                view.clipboard.query_generation = 1;
                view.clipboard.operation_generation = 1;
                view.poll_clipboard_search(query_receiver, 1, cx);
                view.poll_clipboard_operation(
                    operation_receiver,
                    1,
                    None,
                    false,
                    ClipboardOperationFailure::Copy,
                    cx,
                );
                if hide {
                    view.prepare_hide(cx);
                } else {
                    view.go_back(window, cx);
                }
                view.notice = Some("current page".to_owned());
            });
        })
        .unwrap();
        query_sender
            .try_send(LiveEvent::Error("late search failure".to_owned()))
            .unwrap();
        operation_sender
            .try_send(LiveEvent::Error("late copy failure".to_owned()))
            .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(view.read(cx).notice.as_deref(), Some("current page"));
        });
    }
}

#[gpui_kit::test]
fn preview_responses_are_rejected_after_close_selection_revision_or_page_changes(
    cx: &mut TestAppContext,
) {
    let (_window, view) = open_view(cx);
    for change in 0..6 {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::FileSearch;
                view.model.rows = vec![file_row("a", "a.txt"), file_row("b", "b.txt")].into();
                view.model.selected = 0;
                view.model.revision = 1;
                view.file_search.browsing = true;
                view.poll_preview(receiver, cx);
                match change {
                    0 => {
                        view.close_preview(cx);
                    }
                    1 => view.model.selected = 1,
                    2 => view.model.revision = 2,
                    3 => view.screen = Screen::Settings,
                    4 => view.live_state.generation += 1,
                    _ => {
                        // A newer preview request supersedes one for the same file.
                        let (_sender, receiver) = tokio::sync::mpsc::channel(1);
                        view.poll_preview(receiver, cx);
                    }
                }
                view.notice = Some("current state".to_owned());
            });
        });
        let event = if change % 2 == 0 {
            LiveEvent::Preview(crate::host::PreviewFile {
                id: "stale resource".to_owned(),
                paths: Vec::new(),
                title: "old preview".to_owned(),
            })
        } else {
            LiveEvent::Error("old preview failure".to_owned())
        };
        sender.try_send(event).unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.notice.as_deref(), Some("current state"));
            assert!(!view.preview.open);
            assert!(!view.preview.busy);
        });
    }
}

#[gpui_kit::test]
fn native_preview_close_revokes_an_in_flight_update(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Clipboard;
            view.clipboard.apply(clipboard_snapshot(&[1]));
            view.clipboard.browsing = true;
            view.preview.open = true;
            view.preview.generation = 42;
            view.poll_preview(receiver, cx);
            view.apply_preview_poll(
                42,
                &PreviewPoll::Closed {
                    resource_id: "old panel".to_owned(),
                    application_active: true,
                },
                None,
                Some("old panel".to_owned()),
                cx,
            );
            view.notice = Some("panel closed".to_owned());
        });
    });
    sender
        .try_send(LiveEvent::Preview(crate::host::PreviewFile {
            id: "late update".to_owned(),
            paths: Vec::new(),
            title: "late update".to_owned(),
        }))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.notice.as_deref(), Some("panel closed"));
        assert!(!view.preview.open);
        assert!(view.preview.host.is_none());
    });
}

#[gpui_kit::test]
fn closing_a_prepared_preview_cancels_the_queued_native_show(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Clipboard;
            view.clipboard.apply(clipboard_snapshot(&[1]));
            view.clipboard.browsing = true;
            let request = PreviewRequest {
                generation: view.preview.request_generation,
                session_generation: view.live_state.generation,
                screen: view.screen,
                target: PreviewTarget::Clipboard(1),
            };
            view.show_preview(
                crate::host::PreviewFile {
                    id: "prepared preview".to_owned(),
                    paths: Vec::new(),
                    title: "prepared preview".to_owned(),
                },
                request,
                cx,
            );
            view.close_preview(cx);
            view.notice = Some("closed before native show".to_owned());
        });
    });
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.notice.as_deref(), Some("closed before native show"));
        assert!(!view.preview.open);
        assert!(!view.preview.busy);
        assert!(view.preview.host.is_none());
    });
}

#[gpui_kit::test]
fn clipboard_preview_requests_follow_the_selected_item(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ClipboardShelf;
            view.clipboard.apply(clipboard_snapshot(&[1, 2]));
            view.clipboard.browsing = true;
            view.clipboard.selected = 0;
            view.poll_preview(receiver, cx);
            view.select_clipboard(1, cx);
            view.notice = Some("selected item 2".to_owned());
        });
    });
    sender
        .try_send(LiveEvent::Error("preview of item 1 failed".to_owned()))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(view.read(cx).notice.as_deref(), Some("selected item 2"));
    });
}

#[gpui_kit::test]
fn a_failed_preview_update_keeps_the_current_panel_monitor_valid(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Clipboard;
            view.clipboard.apply(clipboard_snapshot(&[1]));
            view.clipboard.browsing = true;
            view.preview.open = true;
            view.preview.generation = 42;
            view.poll_preview(receiver, cx);
        });
    });
    sender
        .try_send(LiveEvent::Error("current preview failure".to_owned()))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.notice.as_deref(), Some("current preview failure"));
        assert!(view.preview.open);
        assert_eq!(view.preview.generation, 42);
    });
}

#[gpui_kit::test]
fn shelf_copy_button_submits_once_at_minimum_width(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let plugins = directory.path().join("plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    // The displayed ID is synthetic. The empty, isolated store cannot write a payload.
    let live = LiveLauncher::open_at(
        anywhere_core::application::frontend_runtime::RuntimePaths::new(
            directory.path().join("data"),
            plugins,
        ),
    )
    .unwrap();
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.screen = Screen::ClipboardShelf;
            view.standalone_shelf = true;
            view.live = Some(Arc::new(live));
            view.clipboard.apply(clipboard_snapshot(&[7, 42]));
            assert!(view.select_clipboard_item(42));
        });
    });
    sync_test_viewport(cx, window, gpui_kit::size(px(360.), px(360.)));
    cx.update_window(window, |_, window, cx| {
        let button = window.find("shelf-copy-42").bounds();
        assert!(button.left() >= window.find("shelf-more-42").bounds().right());
        assert!(button.right() <= window.viewport_size().width);
        assert!(window.try_find("shelf-copy-7").is_none());
        let previous = view.read(cx).clipboard.operation_generation;
        window.click("shelf-copy-42", cx);
        assert_eq!(view.read(cx).clipboard.operation_generation, previous + 1);
        assert_eq!(view.read(cx).clipboard.selected_id(), Some(42));
    })
    .unwrap();
}

#[gpui_kit::test]
fn shelf_command_arrows_reveal_the_selected_row(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, _| {
            view.screen = Screen::ClipboardShelf;
            view.standalone_shelf = true;
            view.clipboard
                .apply(clipboard_snapshot(&(1..=30).collect::<Vec<_>>()));
            view.clipboard.browsing = true;
            view.clipboard.copied_item = Some(1);
        });
    });
    sync_test_viewport(cx, window, gpui_kit::size(px(480.), px(520.)));
    let command = KeyModifiers {
        command: cfg!(target_os = "macos"),
        control: !cfg!(target_os = "macos"),
        modified: true,
        ..Default::default()
    };
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| view.handle_key("down", command, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let view_state = view.read(cx);
        assert_eq!(view_state.clipboard.selected_id(), Some(30));
        assert!(view_state.clipboard.copied_item.is_none());
        assert!(view_state.scrolls.clipboard_shelf_base().offset().y < px(0.));
        view.update(cx, |view, cx| view.handle_key("up", command, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        let view = view.read(cx);
        assert_eq!(view.clipboard.selected_id(), Some(1));
        assert_eq!(view.scrolls.clipboard_shelf_base().offset().y, px(0.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn disconnected_application_history_requests_finish_loading(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    for search in [false, true] {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        drop(sender);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ApplicationHistory;
                view.app_history.loading = true;
                view.app_history.session_generation = 1;
                view.app_history.query_generation = 1;
                view.app_history.selected_source = Some("code".to_owned());
                view.app_history.query.clear();
                if search {
                    view.poll_app_history_search(receiver, 1, "code".to_owned(), String::new(), cx);
                } else {
                    view.poll_app_history_session(receiver, 1, cx);
                }
            });
        });
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(!view.app_history.loading);
            assert_eq!(view.notice_kind, components::NoticeKind::Error);
            assert_eq!(
                view.notice.as_deref(),
                Some(l10n::app_history_operation_failed(Locale::En).as_str())
            );
        });
    }
}

#[gpui_kit::test]
fn stale_clipboard_operations_preserve_the_newer_loading_state(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    for event in [
        LiveEvent::Ignored,
        LiveEvent::Error("old failure".to_owned()),
    ] {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::ClipboardShelf;
                view.clipboard.operation_generation = 1;
                view.poll_clipboard_operation(
                    receiver,
                    1,
                    None,
                    false,
                    ClipboardOperationFailure::Delete,
                    cx,
                );
                // A second operation starts before the first one finishes.
                view.clipboard.operation_generation = 2;
                view.clipboard.loading = true;
                view.notice = Some("new operation".to_owned());
            });
        });
        sender.try_send(event).unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(view.clipboard.loading);
            assert_eq!(view.notice.as_deref(), Some("new operation"));
        });
    }
}

#[gpui_kit::test]
fn application_history_without_sources_finishes_loading(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ApplicationHistory;
            view.app_history.begin_open();
            view.app_history.session_generation = 1;
            view.poll_app_history_session(receiver, 1, cx);
        });
    });
    sender
        .try_send(LiveEvent::AppHistorySources {
            request: 1,
            session_id: Some(42),
            sources: Vec::new(),
        })
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert!(!view.app_history.loading);
        assert_eq!(
            view.app_history.notice,
            Some(anywhere_core::features::application_history::model::Notice::Unconfigured)
        );
    });
}

#[gpui_kit::test]
fn application_history_fallback_actions_toggle_closed(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ApplicationHistory;
            view.toggle_fallback_app_history_actions(cx);
            assert!(view.app_history.fallback_actions_open);
            view.toggle_fallback_app_history_actions(cx);
            assert!(!view.app_history.fallback_actions_open);
        });
    });
}

#[gpui_kit::test]
fn clipboard_command_cannot_replace_new_query_results(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    let (operation_tx, operation_rx) = tokio::sync::mpsc::channel(1);
    let (query_tx, query_rx) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::ClipboardShelf;
            view.clipboard.operation_generation = 1;
            view.poll_clipboard_operation(
                operation_rx,
                1,
                None,
                false,
                ClipboardOperationFailure::Delete,
                cx,
            );
            view.clipboard.query_generation += 1;
            view.clipboard.query = "current".to_owned();
            view.poll_clipboard_search(query_rx, view.clipboard.query_generation, cx);
        });
    });
    query_tx
        .try_send(LiveEvent::Clipboard(clipboard_snapshot(&[2])))
        .unwrap();
    cx.run_until_parked();
    operation_tx
        .try_send(LiveEvent::Clipboard(clipboard_snapshot(&[1])))
        .unwrap();
    cx.run_until_parked();

    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.clipboard.items()[0].id, 2);
        assert_eq!(view.clipboard.query, "current");
    });
}

#[gpui_kit::test]
fn a_read_started_before_clipboard_delete_cannot_resurrect_the_row(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    let (operation_tx, operation_rx) = tokio::sync::mpsc::channel(1);
    let (query_tx, query_rx) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Clipboard;
            view.clipboard.query_generation = 1;
            view.clipboard.operation_generation = 1;
            view.poll_clipboard_search(query_rx, 1, cx);
            view.poll_clipboard_operation(
                operation_rx,
                1,
                None,
                false,
                ClipboardOperationFailure::Delete,
                cx,
            );
        });
    });
    operation_tx
        .try_send(LiveEvent::Clipboard(clipboard_snapshot(&[])))
        .unwrap();
    cx.run_until_parked();
    query_tx
        .try_send(LiveEvent::Clipboard(clipboard_snapshot(&[1])))
        .unwrap();
    cx.run_until_parked();

    cx.update(|cx| assert!(view.read(cx).clipboard.items().is_empty()));
}

#[gpui_kit::test]
fn file_page_and_follow_share_cursor_without_reverting_selection(cx: &mut TestAppContext) {
    fn state(seq: u32, count: usize) -> Arc<anywhere_protocol::LauncherState> {
        Arc::new(anywhere_protocol::LauncherState {
            session_id: 1,
            revision: 2,
            snapshot_seq: seq,
            input: "report".to_owned(),
            file_search: Some(Box::new(anywhere_protocol::FileSearchProgress {
                next_cursor: Some("next".into()),
                ..Default::default()
            })),
            candidates: Arc::new(
                (0..count)
                    .map(|i| {
                        serde_json::from_value(serde_json::json!({
                            "id": format!("file-{i}"),
                            "kind": "file",
                            "title": format!("report-{i}.txt"),
                            "icon": { "key": "file" },
                            "score": 1.0,
                            "group_id": "files",
                            "availability": { "state": "enabled" }
                        }))
                        .unwrap()
                    })
                    .collect(),
            ),
            ..Default::default()
        })
    }

    let (_window, view) = open_view(cx);
    for (keyboard, keyboard_after_start, movement, follow_first, expected) in [
        (false, false, &[-1][..], false, 0),
        (true, false, &[-1], false, 0),
        (true, false, &[-1, 1], false, 1),
        (true, false, &[], false, 2),
        (true, false, &[], true, 2),
        (false, true, &[], true, 2),
    ] {
        let (page_tx, page_rx) = tokio::sync::mpsc::channel(1);
        let (follow_tx, follow_rx) = tokio::sync::mpsc::channel(1);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::FileSearch;
                view.file_search.browsing = true;
                view.live_state.session_id = Some(1);
                let mut gate = SnapshotGate::new(1);
                gate.accept_update(&state(1, 2));
                view.live_state.snapshot_gate = Some(gate);
                view.apply_shared_state(state(1, 2), cx);
                view.model.select_row(1);
                view.poll_file_page(page_rx, keyboard, cx);
                if keyboard_after_start {
                    view.select_file_search(1, cx);
                    assert_eq!(view.model.selected, 1);
                }
                view.poll_follow(follow_rx, view.live_state.generation, None, cx);
                for &direction in movement {
                    view.select_file_search(direction, cx);
                }
            });
        });
        if follow_first {
            follow_tx
                .try_send(LiveEvent::SharedState(state(3, 4)))
                .unwrap();
            cx.run_until_parked();
        }
        page_tx
            .try_send(LiveEvent::SharedState(state(2, 3)))
            .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert_eq!(view.model.rows.len(), if follow_first { 4 } else { 3 });
            assert_eq!(view.model.selected, expected);
            assert!(!view.file_search.page_in_flight);
        });
    }
}
