use super::*;

fn task_row(id: &str) -> crate::host::ExecutionRow {
    crate::host::ExecutionRow {
        id: id.to_owned(),
        title: format!("Build task-{id}"),
        directory: "/synthetic/project".to_owned(),
        command: "synthetic build".to_owned(),
        status: anywhere_protocol::ExecutionSessionStatus::Running,
        output_revision: 0,
        exit_code: None,
        started_at_ms: 0,
        finished_at_ms: None,
        output_truncated: false,
        error: None,
    }
}

#[gpui_kit::test]
fn failed_icons_are_cached_instead_of_becoming_immediately_requestable(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    for outcome in 0..3 {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                view.application_icons.clear();
                view.application_icons_requested.insert(
                    "application".to_owned(),
                    ApplicationIconRequest {
                        ticket: 1,
                        cache_key: "identity".to_owned(),
                    },
                );
                view.poll_application_icon(
                    receiver,
                    "application".to_owned(),
                    "identity".to_owned(),
                    1,
                    cx,
                );
            });
        });
        match outcome {
            0 => sender
                .try_send(LiveEvent::ApplicationIcon {
                    candidate_id: "application".to_owned(),
                    bytes: None,
                })
                .unwrap(),
            1 => sender
                .try_send(LiveEvent::Error("no icon".to_owned()))
                .unwrap(),
            _ => {}
        }
        drop(sender);
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(view.application_icons.contains("identity"));
            assert!(view.application_icons_requested.is_empty());
            assert!(view.notice.is_none());
        });
    }
}

#[test]
fn an_empty_task_filter_result_has_no_executable_selection() {
    let mut tasks = TasksPageModel::default();
    tasks.apply_rows(vec![task_row("a"), task_row("b")]);
    let request = tasks.begin_detail_request().unwrap();
    tasks.set_filter("no matching task".to_owned());
    assert!(tasks.visible_indices().is_empty());
    assert!(tasks.selected_row().is_none());
    assert!(!tasks.finish_detail_request(request));
    assert!(tasks.detail.is_none());
    tasks.set_filter("build task-b".to_owned());
    assert_eq!(tasks.selected_row().map(|row| row.id.as_str()), Some("b"));
}

#[test]
fn task_detail_clones_share_output_without_changing_whitespace_or_unicode() {
    let stdout = "  中文\n\tvalue\n".repeat(3_000);
    let stderr = "  error\n\t原因\n".repeat(2_000);
    let detail = crate::host::ExecutionDetail {
        row: task_row("shared-output"),
        stdout: stdout.clone().into(),
        stderr: stderr.clone().into(),
    };
    let clone = detail.clone();
    assert_eq!(clone.stdout.as_str(), stdout);
    assert_eq!(clone.stderr.as_str(), stderr);
    assert_eq!(clone.stdout.as_ptr(), detail.stdout.as_ptr());
    assert_eq!(clone.stderr.as_ptr(), detail.stderr.as_ptr());
}

#[gpui_kit::test]
fn pending_task_selection_waits_until_the_task_appears(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.pending_task_selection = Some("new".to_owned());
            view.apply_tasks_rows(vec![task_row("old")], cx);
            assert_eq!(view.pending_task_selection.as_deref(), Some("new"));
            view.apply_tasks_rows(vec![task_row("old"), task_row("new")], cx);
            assert_eq!(
                view.tasks.selected_row().map(|row| row.id.as_str()),
                Some("new")
            );
            assert!(view.pending_task_selection.is_none());
        });
    });
}

#[gpui_kit::test]
fn selecting_an_existing_task_reveals_it_through_a_previous_filter(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            set_input_value(&view.tasks_filter_input, "task-old", window, cx);
            view.tasks.set_filter("task-old".to_owned());
            view.apply_tasks_rows(vec![task_row("old"), task_row("new")], cx);
            assert_eq!(view.tasks.visible_indices().as_ref(), &[0]);
            view.select_execution("new".to_owned(), cx);
            assert_eq!(
                view.tasks.selected_row().map(|row| row.id.as_str()),
                Some("new")
            );
            assert!(view.pending_task_selection.is_none());
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(view.read(cx).tasks_filter_input.read(cx).value().is_empty());
        assert_eq!(
            view.read(cx)
                .tasks
                .selected_row()
                .map(|row| row.id.as_str()),
            Some("new")
        );
    });
}

#[gpui_kit::test]
fn manual_task_navigation_cancels_an_older_pending_selection(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    for method in 0..3 {
        cx.update_window(window, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.screen = Screen::Tasks;
                view.tasks
                    .apply_rows(vec![task_row("old"), task_row("other")]);
                view.pending_task_selection = Some("new".to_owned());
                match method {
                    0 => view.select_task_row(0, window, cx),
                    1 => view.handle_key("down", KeyModifiers::default(), window, cx),
                    _ => view.tasks_filter_input.update(cx, |input, cx| {
                        input.set_value("old", window, cx);
                        cx.emit(InputEvent::Change);
                    }),
                }
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                assert!(
                    view.pending_task_selection.is_none(),
                    "manual route {method}"
                );
                view.apply_tasks_rows(
                    vec![task_row("old"), task_row("other"), task_row("new")],
                    cx,
                );
                assert_ne!(
                    view.tasks.selected_row().map(|row| row.id.as_str()),
                    Some("new")
                );
            });
        });
    }
}

#[gpui_kit::test]
fn stale_icon_failure_does_not_settle_a_newer_request(cx: &mut TestAppContext) {
    let (_window, view) = open_view(cx);
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.poll_application_icon(
                receiver,
                "app".to_owned(),
                "old identity".to_owned(),
                1,
                cx,
            );
            view.application_icons_requested.insert(
                "app".to_owned(),
                ApplicationIconRequest {
                    ticket: 2,
                    cache_key: "new identity".to_owned(),
                },
            );
        });
    });
    sender
        .try_send(LiveEvent::Error("old failure".to_owned()))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.application_icons_requested["app"].ticket, 2);
        assert!(!view.application_icons.contains("old identity"));
        assert!(!view.application_icons.contains("new identity"));
    });
}

#[gpui_kit::test]
fn icon_requests_pause_when_the_launcher_is_hidden_or_showing_a_tool(cx: &mut TestAppContext) {
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
    let (_window, view) = open_view(cx);
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.live = Some(live);
            let mut row = file_row("synthetic-app", "Synthetic App");
            row.kind = CandidateKind::Application;
            view.model.rows = vec![row];
            for (screen, presented) in [
                (Screen::Launcher, false),
                (Screen::Settings, true),
                (Screen::ClipboardShelf, true),
            ] {
                view.screen = screen;
                view.presented = presented;
                view.request_application_icons(cx);
                assert!(view.application_icons_requested.is_empty());
            }
            view.screen = Screen::Launcher;
            view.presented = true;
            view.request_application_icons(cx);
            assert_eq!(view.application_icons_requested.len(), 1);
        });
    });
}

#[gpui_kit::test]
fn task_output_updates_do_not_scroll_the_sidebar_away_from_the_user(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    let rows = (0..30)
        .map(|index| task_row(&index.to_string()))
        .collect::<Vec<_>>();
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::Tasks;
            view.standalone_tasks = true;
            view.apply_tasks_rows(rows.clone(), cx);
        });
    });
    sync_test_viewport(cx, window, gpui_kit::size(px(920.), px(600.)));
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, _| {
            view.tasks_scroll
                .set_offset(gpui_kit::point(px(0.), px(-500.)));
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            let offset = view.tasks_scroll.offset();
            assert!(offset.y < px(0.));
            let mut next = rows;
            next[0].output_revision += 1;
            view.apply_tasks_rows(next, cx);
            assert_eq!(
                view.tasks.selected_row().map(|row| row.id.as_str()),
                Some("0")
            );
        });
    });
    cx.run_until_parked();
    cx.update_window(window, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(view.read(cx).tasks_scroll.offset().y, px(-500.));
    })
    .unwrap();
}
