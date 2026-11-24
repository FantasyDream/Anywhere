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
    for outcome in 0..2 {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        cx.update(|cx| {
            view.update(cx, |view, cx| {
                let _ = view.icons.cache.clear();
                view.icons.requested.insert(
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
        if outcome == 0 {
            let _ = sender.send(None);
        } else {
            drop(sender);
        }
        cx.run_until_parked();
        cx.update(|cx| {
            let view = view.read(cx);
            assert!(view.icons.cache.contains("identity"));
            assert!(view.icons.requested.is_empty());
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
            view.tasks.pending_selection = Some("new".to_owned());
            view.apply_tasks_rows(vec![task_row("old")], cx);
            assert_eq!(view.tasks.pending_selection.as_deref(), Some("new"));
            view.apply_tasks_rows(vec![task_row("old"), task_row("new")], cx);
            assert_eq!(
                view.tasks.selected_row().map(|row| row.id.as_str()),
                Some("new")
            );
            assert!(view.tasks.pending_selection.is_none());
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
            assert!(view.tasks.pending_selection.is_none());
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
                view.tasks.pending_selection = Some("new".to_owned());
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
                    view.tasks.pending_selection.is_none(),
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
    let (sender, receiver) = tokio::sync::oneshot::channel();
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.poll_application_icon(
                receiver,
                "app".to_owned(),
                "old identity".to_owned(),
                1,
                cx,
            );
            view.icons.requested.insert(
                "app".to_owned(),
                ApplicationIconRequest {
                    ticket: 2,
                    cache_key: "new identity".to_owned(),
                },
            );
        });
    });
    let _ = sender.send(None);
    cx.run_until_parked();
    cx.update(|cx| {
        let view = view.read(cx);
        assert_eq!(view.icons.requested["app"].ticket, 2);
        assert!(!view.icons.cache.contains("old identity"));
        assert!(!view.icons.cache.contains("new identity"));
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
            view.model.rows = vec![row].into();
            for (screen, presented) in [
                (Screen::Launcher, false),
                (Screen::Settings, true),
                (Screen::ClipboardShelf, true),
            ] {
                view.screen = screen;
                view.presence.presented = presented;
                view.request_application_icons(cx);
                assert!(view.icons.requested.is_empty());
            }
            view.screen = Screen::Launcher;
            view.presence.presented = true;
            view.request_application_icons(cx);
            assert_eq!(view.icons.requested.len(), 1);
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
            view.scrolls
                .tasks_base()
                .set_offset(gpui_kit::point(px(0.), px(-500.)));
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            let offset = view.scrolls.tasks_base().offset();
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
        assert_eq!(view.read(cx).scrolls.tasks_base().offset().y, px(-500.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn file_search_empty_filter_recovery_preserves_input_and_exposes_scan_status(
    cx: &mut TestAppContext,
) {
    let (window, view) = open_view(cx);
    let state = |finished, issue_count| anywhere_protocol::LauncherState {
        input: "invoice".into(),
        revision: 4,
        file_search: Some(Box::new(anywhere_protocol::FileSearchProgress {
            finished,
            issue_count,
            ..Default::default()
        })),
        ..Default::default()
    };
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::FileSearch;
            view.file_search.filter = FileSearchFilter::Pdf;
            view.model.apply(state(false, 2));
            view.input
                .update(cx, |input, cx| input.set_value("invoice", window, cx));
            cx.notify();
        });
        window.render_frame(cx);
        let summary = window.find("file-search-summary");
        assert_eq!(summary.role(), Some(gpui_kit::Role::Status));
        assert!(summary.label().unwrap().contains("2 issues"));
        assert!(window.try_find("file-search-all-types").is_none());
        view.update(cx, |view, cx| {
            view.model.apply(state(true, 0));
            cx.notify();
        });
        window.render_frame(cx);
        window.click("file-search-all-types", cx);
        assert_eq!(view.read(cx).file_search.filter, FileSearchFilter::All);
        assert_eq!(view.read(cx).model.query, "invoice");
        view.update(cx, |view, cx| {
            assert!(view.active_editor_focused(window, cx))
        });
        window.render_frame(cx);
        assert!(window.try_find("file-search-all-types").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn clicking_active_file_filter_preserves_the_browsed_row(cx: &mut TestAppContext) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            view.screen = Screen::FileSearch;
            view.model.rows = vec![file_row("one", "One.pdf"), file_row("two", "Two.pdf")].into();
            view.file_search.filter = FileSearchFilter::Pdf;
            view.file_search.browsing = true;
            view.model.select_row(1);
            cx.notify();
        });
        window.render_frame(cx);
        window.click("file-filter-pdf", cx);
        let view = view.read(cx);
        assert!(view.file_search.browsing);
        assert_eq!(view.model.selection_id(), Some("two"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_disabled_menu_row_does_not_open_the_selected_menu_or_dismiss_a_notice(
    cx: &mut TestAppContext,
) {
    let (window, view) = open_view(cx);
    cx.update_window(window, |_, window, cx| {
        view.update(cx, |view, cx| {
            let mut menu = file_row("menu:parent", "Menu");
            menu.is_menu = true;
            menu.menu_path = vec!["Menu".into()];
            let mut disabled = menu.clone();
            disabled.id = "unavailable".into();
            disabled.enabled = false;
            view.model.rows = vec![menu, disabled].into();
            view.set_notice("Unavailable", components::NoticeKind::Warning);
            cx.notify();
        });
        window.render_frame(cx);
        window.click("candidate-1", cx);
        let view = view.read(cx);
        assert_eq!(view.model.selection_id(), Some("menu:parent"));
        assert!(view.model.menu_path().is_none());
        assert_eq!(view.notice.as_deref(), Some("Unavailable"));
    })
    .unwrap();
}
